//! Pluggable state-store backends with revision tokens.
//!
//! Each backend exposes:
//! - [`StateStore::load`]: read the raw state bytes (if any) together with a
//!   [`StateHandle`] that uniquely identifies that revision of the store
//!   (content hash for local files, ETag + hash for S3).
//! - [`StateStore::save`]: persist new bytes guarded by the handle that was
//!   returned at load time. If the on-disk/remote revision has moved since the
//!   handle was issued, the save fails with [`SaveError::Conflict`] and the
//!   caller is given the latest [`LoadedState`] so it can merge or retry.
//!
//! Higher-level wrappers in [`super::io`] keep the legacy `save_state` API
//! working by performing a load → save round-trip against the appropriate
//! store, but mutating commands are expected to thread the handle explicitly
//! to get real compare-and-swap semantics.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use rusoto_core::Region;
use rusoto_s3::{S3Client, S3};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use yansi::Paint;

use crate::config::{RemoteStateConfig, StateConfig};

use super::aws_credentials_provider::AwsCredentialsProvider;

const STATE_SUFFIX: &str = ".lithos-state.yml";
const LEGACY_STATE_SUFFIX: &str = ".mantle-state.yml";

/// Opaque revision token returned by a [`StateStore`].
///
/// Callers must treat this as opaque: hold onto whatever the store handed back
/// at load time and pass it back unchanged to the matching save. Saves are
/// only allowed to succeed if the store's current revision still matches.
#[derive(Debug, Clone)]
pub struct StateHandle {
    inner: StateHandleInner,
}

#[derive(Debug, Clone)]
enum StateHandleInner {
    /// The state did not exist when loaded. Only the first write may use this
    /// handle; once anything has been persisted the store will refuse to
    /// re-treat the slot as empty.
    Empty,
    Existing {
        content_hash: String,
        // Reserved for backend-native conditional writes (e.g. S3 If-Match
        // once we switch off rusoto 0.47). Today only `content_hash` drives
        // the compare-and-swap check.
        #[allow(dead_code)]
        backend_token: Option<String>,
    },
}

impl StateHandle {
    pub fn empty() -> Self {
        Self {
            inner: StateHandleInner::Empty,
        }
    }

    fn existing(content_hash: String, backend_token: Option<String>) -> Self {
        Self {
            inner: StateHandleInner::Existing {
                content_hash,
                backend_token,
            },
        }
    }

    /// Returns `true` if this handle was issued for an empty (not-yet-written)
    /// state slot.
    pub fn is_empty_slot(&self) -> bool {
        matches!(self.inner, StateHandleInner::Empty)
    }

    fn matches(&self, other: &StateHandle) -> bool {
        match (&self.inner, &other.inner) {
            (StateHandleInner::Empty, StateHandleInner::Empty) => true,
            (
                StateHandleInner::Existing {
                    content_hash: a, ..
                },
                StateHandleInner::Existing {
                    content_hash: b, ..
                },
            ) => a == b,
            _ => false,
        }
    }
}

/// Result of [`StateStore::load`]: the raw bytes that were stored (if any) and
/// a [`StateHandle`] identifying that revision.
pub struct LoadedState {
    pub bytes: Option<Vec<u8>>,
    pub handle: StateHandle,
}

/// Errors that can occur when persisting state via [`StateStore::save`].
pub enum SaveError {
    /// The store's current revision did not match the handle the caller
    /// passed in. The latest state is included so the caller can attempt to
    /// merge or retry on top of it.
    Conflict { latest: LoadedState },
    /// Any non-conflict failure (I/O error, serialization error, network
    /// failure, etc.). The message is intended to be surfaced to the user.
    Other(String),
}

impl From<String> for SaveError {
    fn from(value: String) -> Self {
        SaveError::Other(value)
    }
}

/// Backend-agnostic interface for loading and saving Lithos state.
#[async_trait(?Send)]
pub trait StateStore {
    /// Load the raw bytes currently stored in this slot together with a
    /// handle identifying that revision.
    async fn load(&self) -> Result<LoadedState, String>;

    /// Persist new bytes, refusing to overwrite anything newer than
    /// `expected`. Returns the handle that identifies the freshly written
    /// revision so callers can chain further saves.
    async fn save(&self, bytes: &[u8], expected: &StateHandle) -> Result<StateHandle, SaveError>;

    /// Human-readable description of where this store points. Used in log
    /// output and error messages.
    fn describe(&self) -> String;
}

fn hash_bytes(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    format!("{:x}", digest)
}

/// Construct a [`StateStore`] for the supplied [`StateConfig`].
pub fn build_store(project_path: &Path, config: &StateConfig) -> Box<dyn StateStore> {
    match config {
        StateConfig::Local => Box::new(LocalFileStateStore::new(project_path, None)),
        StateConfig::LocalKey(key) => {
            Box::new(LocalFileStateStore::new(project_path, Some(key.clone())))
        }
        StateConfig::Remote(remote) => Box::new(RemoteS3StateStore::new(remote.clone())),
    }
}

/// Local-file backed state store.
///
/// Saves are guarded with two layers of defence:
/// 1. A short-lived sidecar lock file (`<state>.lock`) created with
///    `O_CREAT|O_EXCL` semantics so two processes on the same machine can not
///    open the write window concurrently.
/// 2. A content-hash compare-and-swap inside the critical section: if another
///    writer slipped in between the caller's load and our save, the hash will
///    no longer match and the save fails with [`SaveError::Conflict`].
pub struct LocalFileStateStore {
    path: PathBuf,
    legacy_path: PathBuf,
    lock_path: PathBuf,
}

impl LocalFileStateStore {
    pub fn new(project_path: &Path, key: Option<String>) -> Self {
        let key = key.unwrap_or_default();
        let path = project_path.join(format!("{}{}", key, STATE_SUFFIX));
        let legacy_path = project_path.join(format!("{}{}", key, LEGACY_STATE_SUFFIX));
        let lock_path = path.with_extension("yml.lock");
        Self {
            path,
            legacy_path,
            lock_path,
        }
    }

    fn resolved_read_path(&self) -> Option<PathBuf> {
        if self.path.exists() {
            Some(self.path.clone())
        } else if self.legacy_path.exists() {
            Some(self.legacy_path.clone())
        } else {
            None
        }
    }

    fn write_atomic(&self, bytes: &[u8]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create parent directory for state file {}: {}",
                        self.path.display(),
                        e
                    )
                })?;
            }
        }
        let tmp_path = self.path.with_extension("yml.tmp");
        // Best-effort cleanup of a previous aborted save.
        let _ = fs::remove_file(&tmp_path);
        {
            let mut file = fs::File::create(&tmp_path).map_err(|e| {
                format!(
                    "Failed to open temporary state file {}: {}",
                    tmp_path.display(),
                    e
                )
            })?;
            file.write_all(bytes).map_err(|e| {
                format!(
                    "Failed to write temporary state file {}: {}",
                    tmp_path.display(),
                    e
                )
            })?;
            file.sync_all().ok();
        }
        // On Windows `fs::rename` refuses to overwrite, so remove first.
        if cfg!(target_os = "windows") && self.path.exists() {
            fs::remove_file(&self.path).map_err(|e| {
                format!(
                    "Failed to replace existing state file {}: {}",
                    self.path.display(),
                    e
                )
            })?;
        }
        fs::rename(&tmp_path, &self.path)
            .map_err(|e| format!("Failed to commit state file {}: {}", self.path.display(), e))?;
        Ok(())
    }
}

#[async_trait(?Send)]
impl StateStore for LocalFileStateStore {
    async fn load(&self) -> Result<LoadedState, String> {
        match self.resolved_read_path() {
            None => Ok(LoadedState {
                bytes: None,
                handle: StateHandle::empty(),
            }),
            Some(path) => {
                let bytes = fs::read(&path)
                    .map_err(|e| format!("Unable to read state file {}: {}", path.display(), e))?;
                let hash = hash_bytes(&bytes);
                Ok(LoadedState {
                    bytes: Some(bytes),
                    handle: StateHandle::existing(hash, None),
                })
            }
        }
    }

    async fn save(&self, bytes: &[u8], expected: &StateHandle) -> Result<StateHandle, SaveError> {
        let guard = LocalLockGuard::acquire(&self.lock_path).map_err(SaveError::Other)?;

        // Re-read inside the critical section so we detect any writer that
        // slipped in between the caller's load and now.
        let current = self.load().await.map_err(SaveError::Other)?;
        if !expected.matches(&current.handle) {
            drop(guard);
            return Err(SaveError::Conflict { latest: current });
        }

        self.write_atomic(bytes).map_err(SaveError::Other)?;
        // Clean up legacy file once we have committed the modern path at
        // least once.
        if self.legacy_path.exists() && self.legacy_path != self.path {
            let _ = fs::remove_file(&self.legacy_path);
        }
        let new_hash = hash_bytes(bytes);
        drop(guard);
        Ok(StateHandle::existing(new_hash, None))
    }

    fn describe(&self) -> String {
        format!("local file {}", self.path.display())
    }
}

/// RAII sidecar-file lock used by [`LocalFileStateStore`].
struct LocalLockGuard {
    path: PathBuf,
}

impl LocalLockGuard {
    fn acquire(path: &Path) -> Result<Self, String> {
        const MAX_ATTEMPTS: u32 = 50;
        const SLEEP_MS: u64 = 100;
        const STALE_AFTER_SECS: u64 = 5 * 60;

        for attempt in 0..MAX_ATTEMPTS {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(mut file) => {
                    let pid = std::process::id();
                    let _ = writeln!(file, "{}", pid);
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // If the lock file is stale (process died mid-write),
                    // reclaim it. Otherwise back off and retry.
                    if let Ok(meta) = fs::metadata(path) {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(age) = modified.elapsed() {
                                if age.as_secs() > STALE_AFTER_SECS {
                                    logger::log(format!(
                                        "{} Reclaiming stale local state lock at {} (age {}s)",
                                        Paint::yellow("warning:"),
                                        path.display(),
                                        age.as_secs()
                                    ));
                                    let _ = fs::remove_file(path);
                                    continue;
                                }
                            }
                        }
                    }
                    if attempt + 1 == MAX_ATTEMPTS {
                        return Err(format!(
                            "Timed out waiting for local state lock {}; another Lithos process \
                             may be writing state. If the previous process crashed, remove the \
                             file and retry.",
                            path.display()
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(SLEEP_MS));
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to acquire local state lock {}: {}",
                        path.display(),
                        e
                    ));
                }
            }
        }
        Err(format!(
            "Unable to acquire local state lock {}",
            path.display()
        ))
    }
}

impl Drop for LocalLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// S3-backed state store.
///
/// Rusoto 0.47 does not expose `If-Match` for `PutObject`, so this backend
/// approximates compare-and-swap with a load-then-write pattern: it re-reads
/// the object inside `save` and compares ETags before writing. There is a
/// small race window between that check and the write, which is why the
/// per-environment lock metadata stored *inside* the document is the real
/// safety net for concurrent mutations. See `docs/site/pages/docs/state.mdx`.
pub struct RemoteS3StateStore {
    config: RemoteStateConfig,
}

impl RemoteS3StateStore {
    pub fn new(config: RemoteStateConfig) -> Self {
        Self { config }
    }

    fn client(&self) -> S3Client {
        create_client(self.config.region.clone())
    }

    fn primary_key(&self) -> String {
        format!("{}.lithos-state.yml", self.config.key)
    }

    fn legacy_key(&self) -> String {
        format!("{}.mantle-state.yml", self.config.key)
    }
}

fn create_client(region: Region) -> S3Client {
    S3Client::new_with(
        rusoto_core::HttpClient::new().expect("failed to construct AWS HTTP client"),
        AwsCredentialsProvider::new(),
        region,
    )
}

#[async_trait(?Send)]
impl StateStore for RemoteS3StateStore {
    async fn load(&self) -> Result<LoadedState, String> {
        let client = self.client();
        let keys = [(self.primary_key(), false), (self.legacy_key(), true)];

        for (key, is_legacy) in keys.iter() {
            let res = client
                .get_object(rusoto_s3::GetObjectRequest {
                    bucket: self.config.bucket.clone(),
                    key: key.clone(),
                    ..Default::default()
                })
                .await;

            match res {
                Ok(object) => {
                    if *is_legacy {
                        logger::log(format!(
                            "{} Loaded legacy remote state object '{}'. It will be migrated to '{}' on next save.",
                            Paint::yellow("warning:"),
                            key,
                            self.primary_key(),
                        ));
                    }
                    let etag = object.e_tag.clone();
                    if let Some(stream) = object.body {
                        let mut buffer = Vec::new();
                        stream
                            .into_async_read()
                            .read_to_end(&mut buffer)
                            .await
                            .map_err(|e| format!("Failed to read remote state body: {}", e))?;
                        let hash = hash_bytes(&buffer);
                        return Ok(LoadedState {
                            bytes: Some(buffer),
                            handle: StateHandle::existing(hash, etag),
                        });
                    }
                    return Ok(LoadedState {
                        bytes: None,
                        handle: StateHandle::existing(hash_bytes(&[]), etag),
                    });
                }
                Err(rusoto_core::RusotoError::Service(rusoto_s3::GetObjectError::NoSuchKey(_))) => {
                    continue
                }
                Err(e) => {
                    return Err(format!("Failed to get state from remote: {}", e));
                }
            }
        }

        Ok(LoadedState {
            bytes: None,
            handle: StateHandle::empty(),
        })
    }

    async fn save(&self, bytes: &[u8], expected: &StateHandle) -> Result<StateHandle, SaveError> {
        // Re-read to detect any concurrent writer. This is best effort on
        // S3: we cannot atomically PUT-if-match through rusoto 0.47, but the
        // in-document lock metadata still protects mutating workflows.
        let current = self.load().await.map_err(SaveError::Other)?;
        if !expected.matches(&current.handle) {
            return Err(SaveError::Conflict { latest: current });
        }

        let client = self.client();
        client
            .put_object(rusoto_s3::PutObjectRequest {
                bucket: self.config.bucket.clone(),
                key: self.primary_key(),
                body: Some(rusoto_core::ByteStream::from(bytes.to_vec())),
                ..Default::default()
            })
            .await
            .map_err(|e| SaveError::Other(format!("Failed to save state to remote: {}", e)))?;

        let new_hash = hash_bytes(bytes);
        Ok(StateHandle::existing(new_hash, None))
    }

    fn describe(&self) -> String {
        format!(
            "remote object {}/{}",
            self.config.bucket,
            self.primary_key()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store(dir: &TempDir) -> LocalFileStateStore {
        LocalFileStateStore::new(dir.path(), None)
    }

    #[tokio::test]
    async fn load_returns_empty_handle_when_no_state_file_exists() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let loaded = store.load().await.unwrap();
        assert!(loaded.bytes.is_none());
        assert!(loaded.handle.is_empty_slot());
    }

    #[tokio::test]
    async fn first_save_writes_when_expected_is_empty() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let handle = store
            .save(b"hello", &StateHandle::empty())
            .await
            .unwrap_or_else(|_| panic!("first save should succeed"));
        assert!(!handle.is_empty_slot());

        let loaded = store.load().await.unwrap();
        assert_eq!(loaded.bytes.as_deref(), Some(&b"hello"[..]));
    }

    #[tokio::test]
    async fn save_conflicts_when_handle_is_stale() {
        let dir = TempDir::new().unwrap();
        let store = make_store(&dir);
        let stale = store
            .save(b"v1", &StateHandle::empty())
            .await
            .unwrap_or_else(|_| panic!("seed save failed"));
        let _ = store
            .save(b"v2", &stale)
            .await
            .unwrap_or_else(|_| panic!("second save failed"));

        match store.save(b"v3", &stale).await {
            Err(SaveError::Conflict { latest }) => {
                assert_eq!(latest.bytes.as_deref(), Some(&b"v2"[..]));
            }
            _ => panic!("expected conflict when re-using a stale handle"),
        }
    }
}
