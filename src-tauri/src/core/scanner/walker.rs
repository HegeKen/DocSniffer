//! Directory walker implemented on top of `walkdir`.
//!
//! Handles recursion, permission errors, hidden-file filtering and configurable
//! directory exclusions. Collects cheap metadata (path / name / ext / size / mtime)
//! for downstream extraction and indexing.

use serde::Serialize;
use std::path::Path;
use walkdir::WalkDir;

/// A single file entry collected during a scan.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub mtime: u64,
}

/// Filtering options controlling a scan run.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub include_hidden: bool,
    pub exclude_dirs: Vec<String>,
    pub follow_links: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            exclude_dirs: vec![
                "/System".into(),
                "/Library".into(),
                "/Applications".into(),
                "/private".into(),
                "/usr".into(),
                "/bin".into(),
                "/sbin".into(),
                "/opt".into(),
                "/var".into(),
                "/etc".into(),
            ],
            follow_links: false,
        }
    }
}

/// Walk `root` and return metadata for every regular file under it.
///
/// Permission-denied entries are silently skipped so a scan never aborts just
/// because a protected system directory cannot be read.
pub fn collect_files(root: &Path, options: &ScanOptions) -> Vec<FileEntry> {
    let walker = WalkDir::new(root)
        .follow_links(options.follow_links)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), options));

    let mut out = Vec::new();
    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        if !options.include_hidden && is_hidden(entry.path()) {
            continue;
        }
        if let Some(fe) = to_entry(entry.path()) {
            out.push(fe);
        }
    }
    out
}

fn to_entry(path: &Path) -> Option<FileEntry> {
    let meta = std::fs::metadata(path).ok()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(FileEntry {
        path: path.to_string_lossy().into_owned(),
        name,
        ext,
        size: meta.len(),
        mtime,
    })
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
}

fn is_excluded(path: &Path, options: &ScanOptions) -> bool {
    let p = path.to_string_lossy();
    options.exclude_dirs.iter().any(|d| p == *d || p.starts_with(&(d.clone() + "/")))
}
