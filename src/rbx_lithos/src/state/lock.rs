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

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
}
