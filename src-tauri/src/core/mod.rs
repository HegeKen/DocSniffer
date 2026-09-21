//! Core business modules of DocSniffer's Rust backend.
//!
//! Each submodule maps to one component of the system architecture described in
//! the project README (scanner / extractor / indexer / searcher / watcher /
//! sensitive / storage).

pub mod batch;
pub mod extractor;
pub mod indexer;
pub mod scanner;
pub mod searcher;
pub mod sensitive;
pub mod state;
pub mod storage;
pub mod watcher;

use crate::core::storage::Store;
use std::path::PathBuf;

/// Application identifier, used for the data-directory naming and storage keys.
pub const APP_ID: &str = "cn.helilab.docsniffer";

/// `Store` key holding the user-configured index directory (if any).
pub const INDEX_DIR_KEY: &str = "index_dir";

/// Resolve the application data directory.
///
/// Portable / U-disk mode: if an empty `PORTABLE.flag` file exists next to the
/// executable, *all* runtime data (index, config, meta) is written into the
/// `Data/` folder relative to the executable and the host is left untouched.
/// Otherwise a per-user OS data directory is used (default isolated mode).
pub fn resolve_data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.join("PORTABLE.flag").exists() {
                let data_dir = parent.join("Data");
                let _ = std::fs::create_dir_all(&data_dir);
                return data_dir;
            }
        }
    }

    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    let data_dir = base.join("DocSniffer");
    let _ = std::fs::create_dir_all(&data_dir);
    data_dir
}

/// Resolve where the Tantivy index lives.
///
/// A location chosen by the user on the settings page (`INDEX_DIR_KEY`) wins;
/// otherwise the index lives in `<data_dir>/index`.
pub fn resolve_index_dir(store: &Store) -> PathBuf {
    if let Some(custom) = store.load::<String>(INDEX_DIR_KEY) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    resolve_data_dir().join("index")
}
