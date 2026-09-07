//! Tauri command handlers.
//!
//! Bridges the Rust core to the web UI over IPC. Each submodule maps to a group
//! of commands (scan / search / sensitive).

pub mod scan;
pub mod search;
pub mod sensitive;

pub use crate::core::state::AppState;

/// The command names, re-exported for `generate_handler!` in `lib.rs`.
pub use scan::{
    clear_all_index, delete_batch, index_status, list_batches, scan_directory, update_batch,
};
pub use search::search_files;
pub use sensitive::{get_rules, save_rules, sensitive_scan};
