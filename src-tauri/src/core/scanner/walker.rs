//! Directory walker implemented on top of `walkdir`.
//!
//! Handles recursion, permission errors, hidden-file filtering and configurable
//! directory exclusions. Collects cheap metadata (path / name / ext / size / mtime)
//! for downstream extraction and indexing.

use serde::Serialize;
use std::path::Path;
use walkdir::WalkDir;

/// Cap on the number of per-scan warnings (permission denied, unreadable
/// entries, …) carried back to the UI, so a huge tree cannot blow up the
/// report payload.
pub const MAX_SCAN_WARNINGS: usize = 100;

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
            include_hidden: true,
            exclude_dirs: default_exclude_dirs(),
            follow_links: false,
        }
    }
}

/// System directories excluded by default on macOS (absolute prefixes).
#[cfg(target_os = "macos")]
fn default_exclude_dirs() -> Vec<String> {
    [
        "/System",
        "/Library",
        "/Applications",
        "/private",
        "/usr",
        "/bin",
        "/sbin",
        "/opt",
        "/var",
        "/etc",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// System directories excluded by default on Linux.
#[cfg(target_os = "linux")]
fn default_exclude_dirs() -> Vec<String> {
    [
        "/proc", "/sys", "/dev", "/run", "/boot", "/usr", "/bin", "/sbin", "/lib", "/lib32",
        "/lib64", "/etc", "/var", "/snap",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// System locations excluded by default on Windows. Drive-agnostic values come
/// from the environment (SystemRoot / ProgramFiles / ProgramData), which also
/// handles non-`C:` system drives and localized installs.
#[cfg(target_os = "windows")]
fn default_exclude_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    for var in ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramData"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                dirs.push(v);
            }
        }
    }
    dirs
}

/// Fallback for other platforms: exclude nothing.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn default_exclude_dirs() -> Vec<String> {
    Vec::new()
}

/// Walk `root` and return metadata for every regular file under it, plus a
/// bounded list of warnings describing entries that could not be read
/// (permission denied, I/O error, symlink loop). A scan never aborts just
/// because one protected directory cannot be traversed, but the failures are no
/// longer silently discarded.
pub fn collect_files(root: &Path, options: &ScanOptions) -> (Vec<FileEntry>, Vec<String>) {
    let walker = WalkDir::new(root)
        .follow_links(options.follow_links)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), options));

    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for entry in walker {
        match entry {
            Ok(entry) => {
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
            Err(e) => {
                if warnings.len() < MAX_SCAN_WARNINGS {
                    let loc = e
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    let msg = e
                        .io_error()
                        .map(|io| io.to_string())
                        .unwrap_or_else(|| e.to_string());
                    warnings.push(if loc.is_empty() {
                        msg
                    } else {
                        format!("{loc}: {msg}")
                    });
                }
            }
        }
    }
    (out, warnings)
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
    #[cfg(not(target_os = "windows"))]
    {
        let p = path.to_string_lossy();
        options
            .exclude_dirs
            .iter()
            .any(|d| p == d.as_str() || p.starts_with(&format!("{d}/")))
    }
    #[cfg(target_os = "windows")]
    {
        // Windows paths are case-insensitive and use `\` separators; compare
        // normalized forward-slash, lowercased prefixes.
        let norm = |s: &str| s.replace('\\', "/").to_lowercase();
        let p = norm(&path.to_string_lossy());
        let prefix_hit = options.exclude_dirs.iter().any(|d| {
            let d = norm(d);
            p == d || p.starts_with(&format!("{d}/"))
        });
        // Always skip the per-drive recycle bin and shadow-copy store, whose
        // locations are not exposed via environment variables.
        let special_hit = path.components().any(|c| {
            let name = c
                .as_os_str()
                .to_string_lossy()
                .to_lowercase();
            name == "$recycle.bin" || name == "system volume information"
        });
        prefix_hit || special_hit
    }
}
