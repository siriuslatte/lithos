//! Environment-scoped advisory locks used to serialize mutating commands.
//!
//! Locks live *inside* the state document so the same compare-and-swap
//! machinery that protects every other state write also protects the lock
//! itself. Acquiring a lock therefore performs a regular CAS-guarded save,
//! and any race between two would-be lock holders is resolved at the
//! store layer.
//!
//! A lock carries enough identity (owner id, host, PID, operation, started
//! time, last heartbeat) to be diagnostically useful and to support stale
//! lock recovery: if the heartbeat is older than the configured TTL, an
//! operator can break the lock with the `lithos lock` CLI command (or any
//! mutating command in `--force` mode in a future iteration).

use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::StateConfig;

use super::io::{load_state_from_source, save_state_cas, ResourceStateVLatest, SaveTarget};
use super::store::StateHandle;
use super::v7::ResourceStateV7;

/// Default heartbeat interval used by long-running operations.
pub const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// Default age at which a lock is considered abandoned.
pub const DEFAULT_LOCK_TTL: Duration = Duration::from_secs(15 * 60);

/// Metadata describing the owner of an environment-scoped lock.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentLock {
    /// Unique id generated when the lock was acquired. The owning process
    /// keeps this around so it can prove it still holds the lock when
    /// renewing or releasing.
    pub owner_id: String,
    /// Best-effort host name of the machine that took the lock.
    pub host: String,
    /// Process id of the holder.
    pub pid: u32,
    /// User-visible label for the operation (e.g. "deploy", "undo").
    pub operation: String,
    /// RFC3339 timestamp of when the lock was first acquired.
    pub acquired_at: String,
    /// RFC3339 timestamp of the most recent heartbeat. Refreshed on every
    /// progress write while the lock is held.
    pub heartbeat_at: String,
    /// RFC3339 timestamp at which other writers may consider the lock
    /// abandoned if no fresh heartbeat has been recorded. Currently equal
    /// to `heartbeat_at + DEFAULT_LOCK_TTL`.
    pub expires_at: String,
}

impl EnvironmentLock {
    /// Construct a fresh lock for the supplied owner/operation.
    pub fn new(owner_id: String, operation: String) -> Self {
        let host = hostname();
        let pid = std::process::id();
        let now = Utc::now();
        Self {
            owner_id,
            host,
            pid,
            operation,
            acquired_at: now.to_rfc3339(),
            heartbeat_at: now.to_rfc3339(),
            expires_at: (now + chrono::Duration::from_std(DEFAULT_LOCK_TTL).unwrap()).to_rfc3339(),
        }
    }

    /// Returns true if the lock has not been refreshed within
    /// [`DEFAULT_LOCK_TTL`].
    pub fn is_stale(&self) -> bool {
        match DateTime::parse_from_rfc3339(&self.heartbeat_at) {
            Ok(parsed) => {
                let age = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
                match age.to_std() {
                    Ok(age) => age > DEFAULT_LOCK_TTL,
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }

    /// Update the lock's heartbeat / expiry to "now". Does not change the
    /// owner identity, so a heartbeat is a no-op for stale lock recovery.
    pub fn touch(&mut self) {
        let now = Utc::now();
        self.heartbeat_at = now.to_rfc3339();
        self.expires_at =
            (now + chrono::Duration::from_std(DEFAULT_LOCK_TTL).unwrap()).to_rfc3339();
    }
}

/// Outcome of trying to acquire an environment lock.
#[derive(Debug)]
pub enum AcquireOutcome {
    /// We hold the lock. The returned [`EnvironmentLock`] is also written
    /// into the state document; callers should now CAS-save the state.
    Acquired(EnvironmentLock),
    /// Another holder still owns this environment.
    Held {
        existing: EnvironmentLock,
        is_stale: bool,
    },
}

#[derive(Debug)]
pub enum LockError {
    /// The expected owner id no longer matches the lock recorded in state.
    /// This means our lock was broken or stolen by another writer.
    NotOwner {
        environment: String,
        current: Option<Box<EnvironmentLock>>,
    },
    /// The environment is not currently locked.
    NotLocked { environment: String },
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::NotOwner {
                environment,
                current,
            } => {
                write!(
                    f,
                    "Lock for environment '{}' is no longer owned by this process",
                    environment
                )?;
                if let Some(lock) = current {
                    write!(
                        f,
                        " (current holder: pid {} on {}, operation '{}')",
                        lock.pid, lock.host, lock.operation
                    )?;
                }
                Ok(())
            }
            LockError::NotLocked { environment } => {
                write!(f, "Environment '{}' is not currently locked", environment)
            }
        }
    }
}

impl ResourceStateV7 {
    /// Look up the lock entry for an environment, if any.
    pub fn environment_lock(&self, label: &str) -> Option<&EnvironmentLock> {
        self.locks.get(label)
    }

    /// Attempt to claim the lock for `label`. If the slot is empty or holds
    /// a stale lock the new owner takes over; otherwise the caller is told
    /// who still holds it. The state document is mutated in memory; the
    /// caller is responsible for persisting via `save_state_cas`.
    pub fn try_acquire_environment_lock(
        &mut self,
        label: &str,
        owner_id: String,
        operation: String,
    ) -> AcquireOutcome {
        if let Some(existing) = self.locks.get(label).cloned() {
            if existing.is_stale() {
                let lock = EnvironmentLock::new(owner_id, operation);
                self.locks.insert(label.to_owned(), lock.clone());
                return AcquireOutcome::Acquired(lock);
            }
            return AcquireOutcome::Held {
                existing,
                is_stale: false,
            };
        }
        let lock = EnvironmentLock::new(owner_id, operation);
        self.locks.insert(label.to_owned(), lock.clone());
        AcquireOutcome::Acquired(lock)
    }

    /// Refresh the heartbeat for a lock we already hold. Returns an error
    /// if the lock was taken over by someone else.
    pub fn heartbeat_environment_lock(
        &mut self,
        label: &str,
        owner_id: &str,
    ) -> Result<(), LockError> {
        match self.locks.get_mut(label) {
            Some(lock) if lock.owner_id == owner_id => {
                lock.touch();
                Ok(())
            }
            Some(_) => Err(LockError::NotOwner {
                environment: label.to_owned(),
                current: self.locks.get(label).cloned().map(Box::new),
            }),
            None => Err(LockError::NotLocked {
                environment: label.to_owned(),
            }),
        }
    }

    /// Release a lock we hold. Returns an error if we no longer own it.
    pub fn release_environment_lock(
        &mut self,
        label: &str,
        owner_id: &str,
    ) -> Result<(), LockError> {
        match self.locks.get(label) {
            Some(lock) if lock.owner_id == owner_id => {
                self.locks.remove(label);
                Ok(())
            }
            Some(_) => Err(LockError::NotOwner {
                environment: label.to_owned(),
                current: self.locks.get(label).cloned().map(Box::new),
            }),
            None => Err(LockError::NotLocked {
                environment: label.to_owned(),
            }),
        }
    }

    /// Forcibly remove a lock regardless of ownership. Intended for
    /// operator-initiated stale-lock recovery.
    pub fn force_break_environment_lock(&mut self, label: &str) -> Option<EnvironmentLock> {
        self.locks.remove(label)
    }

    /// Iterate over all currently held locks.
    pub fn iter_environment_locks(&self) -> impl Iterator<Item = (&String, &EnvironmentLock)> {
        self.locks.iter()
    }
}

/// Generate a new opaque owner id. Used by command-line entry points when
/// they take a lock.
pub fn new_owner_id() -> String {
    format!(
        "{}-{}-{}",
        hostname(),
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    )
}

fn hostname() -> String {
    // Avoid pulling in a hostname crate for this best-effort field.
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_owned())
}

/// RAII-style handle returned by [`acquire_environment_lock`]. Holds the
/// owner id that identifies us to the lock metadata in state.
#[derive(Clone, Debug)]
pub struct EnvironmentLockSession {
    pub environment: String,
    pub owner_id: String,
    pub operation: String,
}

/// Format a human-readable diagnostic for a lock currently held by some
/// other process. Used both when a fresh acquire is blocked and when our
/// own lock has been broken by an admin.
pub fn describe_existing_lock(lock: &EnvironmentLock) -> String {
    format!(
        "held by pid {} on {} (operation '{}', acquired_at {}, heartbeat {})",
        lock.pid, lock.host, lock.operation, lock.acquired_at, lock.heartbeat_at
    )
}

/// Acquire a lock on `environment` and persist the change via CAS. Returns
/// an [`EnvironmentLockSession`] the caller threads through subsequent
/// state writes.
///
/// The caller passes a mutable reference to the freshly loaded state plus
/// the [`StateHandle`] it came with; both are updated on success so that
/// the next CAS write can resume from the post-lock revision.
///
/// On contention this function does not block: if a non-stale lock is
/// already in place we return an error describing the holder. Stale locks
/// are reclaimed automatically.
pub async fn acquire_environment_lock(
    project_path: &Path,
    state_config: &StateConfig,
    state: &mut ResourceStateVLatest,
    handle: &mut StateHandle,
    environment: &str,
    operation: &str,
) -> Result<EnvironmentLockSession, String> {
    let owner_id = new_owner_id();
    let baseline = state.environment(environment).cloned();

    match state.try_acquire_environment_lock(environment, owner_id.clone(), operation.to_owned()) {
        AcquireOutcome::Acquired(_) => {}
        AcquireOutcome::Held { existing, is_stale } => {
            return Err(format!(
                "Cannot acquire lock for environment '{}': {}{}. Use `lithos lock break --environment {}` to release a stale lock.",
                environment,
                describe_existing_lock(&existing),
                if is_stale { " (stale)" } else { "" },
                environment,
            ));
        }
    }

    let target = SaveTarget::new(environment, baseline);
    let new_handle = save_state_cas(project_path, state_config, state, handle, &target)
        .await
        .map_err(|e| format!("Failed to persist lock acquisition: {}", e))?;
    *handle = new_handle;

    Ok(EnvironmentLockSession {
        environment: environment.to_owned(),
        owner_id,
        operation: operation.to_owned(),
    })
}

/// Refresh the heartbeat for an already-held lock and persist via CAS.
/// Returns an error if our lock has been stolen or broken; callers should
/// treat that as a hard abort.
pub async fn heartbeat_environment_lock(
    project_path: &Path,
    state_config: &StateConfig,
    state: &mut ResourceStateVLatest,
    handle: &mut StateHandle,
    session: &EnvironmentLockSession,
) -> Result<(), String> {
    let baseline = state.environment(&session.environment).cloned();
    state
        .heartbeat_environment_lock(&session.environment, &session.owner_id)
        .map_err(|e| e.to_string())?;
    let target = SaveTarget::new(&session.environment, baseline);
    let new_handle = save_state_cas(project_path, state_config, state, handle, &target)
        .await
        .map_err(|e| format!("Failed to heartbeat lock: {}", e))?;
    *handle = new_handle;
    Ok(())
}

/// Release an environment lock and persist via CAS. Best-effort: if the
/// caller no longer owns the lock we surface a warning but do not retry.
pub async fn release_environment_lock(
    project_path: &Path,
    state_config: &StateConfig,
    state: &mut ResourceStateVLatest,
    handle: &mut StateHandle,
    session: &EnvironmentLockSession,
) -> Result<(), String> {
    let baseline = state.environment(&session.environment).cloned();
    state
        .release_environment_lock(&session.environment, &session.owner_id)
        .map_err(|e| e.to_string())?;
    let target = SaveTarget::new(&session.environment, baseline);
    let new_handle = save_state_cas(project_path, state_config, state, handle, &target)
        .await
        .map_err(|e| format!("Failed to release lock: {}", e))?;
    *handle = new_handle;
    Ok(())
}

/// Forcibly break a lock for an environment, regardless of ownership.
/// Intended for the `lithos lock break` CLI subcommand. Returns the lock
/// that was broken, if any.
pub async fn force_break_environment_lock(
    project_path: &Path,
    state_config: &StateConfig,
    environment: &str,
) -> Result<Option<EnvironmentLock>, String> {
    let (mut state, handle) = load_state_from_source(project_path, state_config).await?;
    let baseline = state.environment(environment).cloned();
    let broken = state.force_break_environment_lock(environment);
    if broken.is_some() {
        let target = SaveTarget::new(environment, baseline);
        save_state_cas(project_path, state_config, &mut state, &handle, &target).await?;
    }
    Ok(broken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn empty_state() -> ResourceStateV7 {
        ResourceStateV7 {
            environments: BTreeMap::new(),
            locks: BTreeMap::new(),
        }
    }

    #[test]
    fn acquires_lock_when_empty() {
        let mut state = empty_state();
        match state.try_acquire_environment_lock("prod", "owner-a".to_owned(), "deploy".to_owned())
        {
            AcquireOutcome::Acquired(lock) => {
                assert_eq!(lock.owner_id, "owner-a");
                assert_eq!(lock.operation, "deploy");
            }
            _ => panic!("expected acquire"),
        }
        assert!(state.environment_lock("prod").is_some());
    }

    #[test]
    fn second_acquire_is_blocked_while_lock_is_fresh() {
        let mut state = empty_state();
        let _ =
            state.try_acquire_environment_lock("prod", "owner-a".to_owned(), "deploy".to_owned());
        match state.try_acquire_environment_lock("prod", "owner-b".to_owned(), "deploy".to_owned())
        {
            AcquireOutcome::Held { existing, is_stale } => {
                assert_eq!(existing.owner_id, "owner-a");
                assert!(!is_stale);
            }
            _ => panic!("expected held"),
        }
    }

    #[test]
    fn release_requires_ownership() {
        let mut state = empty_state();
        let _ =
            state.try_acquire_environment_lock("prod", "owner-a".to_owned(), "deploy".to_owned());
        assert!(state.release_environment_lock("prod", "owner-b").is_err());
        state
            .release_environment_lock("prod", "owner-a")
            .expect("rightful owner releases");
        assert!(state.environment_lock("prod").is_none());
    }

    #[test]
    fn force_break_clears_lock() {
        let mut state = empty_state();
        let _ =
            state.try_acquire_environment_lock("prod", "owner-a".to_owned(), "deploy".to_owned());
        let broken = state.force_break_environment_lock("prod").unwrap();
        assert_eq!(broken.owner_id, "owner-a");
        assert!(state.environment_lock("prod").is_none());
    }

    #[tokio::test]
    async fn second_acquire_against_persisted_lock_is_rejected() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = StateConfig::Local;

        // First acquirer.
        let (mut state, mut handle) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let _session =
            acquire_environment_lock(dir.path(), &cfg, &mut state, &mut handle, "prod", "deploy")
                .await
                .expect("first acquire succeeds");

        // Second acquirer loads fresh state and should see the existing lock.
        let (mut state_b, mut handle_b) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let err = acquire_environment_lock(
            dir.path(),
            &cfg,
            &mut state_b,
            &mut handle_b,
            "prod",
            "deploy",
        )
        .await
        .expect_err("second acquire must be rejected");
        assert!(
            err.contains("lithos lock break"),
            "diagnostic should point at recovery command, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn force_break_then_reacquire_succeeds() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = StateConfig::Local;

        let (mut state, mut handle) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let _ =
            acquire_environment_lock(dir.path(), &cfg, &mut state, &mut handle, "prod", "deploy")
                .await
                .unwrap();

        let broken = force_break_environment_lock(dir.path(), &cfg, "prod")
            .await
            .unwrap();
        assert!(broken.is_some(), "should have broken an existing lock");

        // Reacquire from a fresh load.
        let (mut state_b, mut handle_b) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        acquire_environment_lock(
            dir.path(),
            &cfg,
            &mut state_b,
            &mut handle_b,
            "prod",
            "deploy",
        )
        .await
        .expect("reacquire after force_break must succeed");
    }

    #[tokio::test]
    async fn force_break_when_no_lock_returns_none() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = StateConfig::Local;
        let result = force_break_environment_lock(dir.path(), &cfg, "prod")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn release_drops_lock_and_unblocks_next_acquirer() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = StateConfig::Local;

        let (mut state, mut handle) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let session =
            acquire_environment_lock(dir.path(), &cfg, &mut state, &mut handle, "prod", "deploy")
                .await
                .unwrap();
        release_environment_lock(dir.path(), &cfg, &mut state, &mut handle, &session)
            .await
            .unwrap();

        let (mut state_b, mut handle_b) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        acquire_environment_lock(
            dir.path(),
            &cfg,
            &mut state_b,
            &mut handle_b,
            "prod",
            "deploy",
        )
        .await
        .expect("acquire after release should succeed");
    }

    #[tokio::test]
    async fn cross_environment_locks_do_not_conflict() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = StateConfig::Local;

        let (mut state_a, mut handle_a) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        acquire_environment_lock(
            dir.path(),
            &cfg,
            &mut state_a,
            &mut handle_a,
            "dev",
            "deploy",
        )
        .await
        .unwrap();

        // A second client takes prod from its own freshly-loaded state; this
        // exercises the cross-env merge path in save_state_cas.
        let (mut state_b, mut handle_b) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        acquire_environment_lock(
            dir.path(),
            &cfg,
            &mut state_b,
            &mut handle_b,
            "prod",
            "deploy",
        )
        .await
        .expect("different-environment lock should succeed");

        // Final state on disk should carry both locks.
        let (final_state, _) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        assert!(final_state.environment_lock("dev").is_some());
        assert!(final_state.environment_lock("prod").is_some());
    }
}
