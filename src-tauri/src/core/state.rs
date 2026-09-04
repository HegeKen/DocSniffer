//! Shared application state.
//!
//! Plain Rust state with no framework dependency, so both the Tauri command
//! layer and the headless HTTP server (`docsniffer-server`) can own it.

use crate::core::indexer::IndexManager;
use crate::core::storage::Store;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub index: Arc<IndexManager>,
    pub store: Store,
    pub scan_roots: Mutex<Vec<String>>,
}

impl AppState {
    /// Open (or create) the index and the store rooted in the portable-aware
    /// data directory.
    pub fn new() -> Result<Self, String> {
        let index_dir = crate::core::resolve_data_dir().join("index");
        let index = IndexManager::open(&index_dir).map_err(|e| e.to_string())?;
        Ok(Self {
            index: Arc::new(index),
            store: Store::new(),
            scan_roots: Mutex::new(Vec::new()),
        })
    }
}
