//! File system scanning engine.
//!
//! Walks a directory tree, collects file metadata and emits progress so the
//! front-end can render a live scan status.

pub mod walker;

pub use walker::{collect_files, FileEntry, ScanOptions};
