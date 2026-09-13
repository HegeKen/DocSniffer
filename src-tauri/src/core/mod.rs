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

use std::path::PathBuf;

/// Application identifier, used for the data-directory naming and storage keys.
pub const APP_ID: &str = "cn.helilab.docsniffer";

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
