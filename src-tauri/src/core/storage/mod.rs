//! Storage layer.
//!
//! A minimal, dependency-light key/value JSON store rooted in the resolved data
//! directory (portable-aware). It persists metadata that Tantivy itself does not
//! own: configured scan roots, custom sensitive rules, index/build status, etc.
//!
//! The README mentions LMDB/SQLite; a JSON store is used here to keep the
//! single-file build free of extra native dependencies while honouring the
//! "no registry / no system dir writes" green-portable requirement.

use crate::core::resolve_data_dir;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// A tiny JSON-backed key/value store.
pub struct Store {
    dir: PathBuf,
    /// Serializes multi-step read/modify/write sequences (e.g. the batch
    /// list) so concurrent requests cannot lose each other's updates.
    write_lock: Mutex<()>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

impl Store {
    /// Open a store rooted in the portable-aware data directory.
    pub fn new() -> Self {
        let dir = resolve_data_dir();
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            write_lock: Mutex::new(()),
        }
    }

    /// Hold the store-wide write lock for the duration of a read/modify/write
    /// sequence. The store is only used by one process, so an in-process lock
    /// is sufficient (no on-disk flock needed).
    pub fn lock_write(&self) -> MutexGuard<'_, ()> {
        self.write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Resolve the on-disk path for `key`.
    pub fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// Load and deserialise a value stored under `key`.
    pub fn load<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let p = self.path(key);
        let raw = std::fs::read_to_string(p).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Serialise and persist `value` under `key`.
    ///
    /// The write is atomic: bytes first go to a temporary file in the same
    /// directory, which is then renamed over the target. A crash mid-write can
    /// therefore never leave a half-truncated JSON file behind.
    pub fn save<T: Serialize>(&self, key: &str, value: &T) -> std::io::Result<()> {
        let p = self.path(key);
        let json = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = self
            .dir
            .join(format!(".{key}.json.{}.{nanos}.tmp", std::process::id()));
        match std::fs::write(&tmp, &json).and_then(|()| std::fs::rename(&tmp, &p)) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                Err(e)
            }
        }
    }

    /// Remove the value stored under `key`.
    pub fn remove(&self, key: &str) {
        let _ = std::fs::remove_file(self.path(key));
    }

    /// Expose the underlying directory (useful for API responses / diagnostics).
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}
