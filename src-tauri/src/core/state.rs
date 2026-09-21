//! Shared application state.
//!
//! Plain Rust state with no framework dependency, owned by the Tauri command
//! layer (`commands/`).

use crate::core::indexer::IndexManager;
use crate::core::storage::Store;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

pub struct AppState {
    /// The active index. Replaceable, because the user may move the index to a
    /// custom directory at runtime (`commands::settings::set_index_dir`).
    index: RwLock<Arc<IndexManager>>,
    pub store: Store,
    pub scan_roots: Mutex<Vec<String>>,
}

impl AppState {
    /// Open (or create) the index and the store rooted in the portable-aware
    /// data directory.
    pub fn new() -> Result<Self, String> {
        let store = Store::new();
        let index_dir = crate::core::resolve_index_dir(&store);
        let index = IndexManager::open(&index_dir).map_err(|e| e.to_string())?;
        Ok(Self {
            index: RwLock::new(Arc::new(index)),
            store,
            scan_roots: Mutex::new(Vec::new()),
        })
    }

    /// Snapshot the active index handle. Cheap (an `Arc` clone), so callers can
    /// move it into a blocking task without holding the state lock.
    pub fn index(&self) -> Arc<IndexManager> {
        self.index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Directory backing the currently active index.
    pub fn index_dir(&self) -> PathBuf {
        self.index().dir().to_path_buf()
    }

    /// Swap in another index (used after the index directory has been moved).
    pub fn replace_index(&self, index: IndexManager) {
        let mut guard = self
            .index
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Arc::new(index);
    }
}
