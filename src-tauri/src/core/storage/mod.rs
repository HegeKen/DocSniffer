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

/// A tiny JSON-backed key/value store.
pub struct Store {
    dir: PathBuf,
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
        Self { dir }
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
    pub fn save<T: Serialize>(&self, key: &str, value: &T) -> std::io::Result<()> {
        let p = self.path(key);
        let json = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
        std::fs::write(p, json)
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
