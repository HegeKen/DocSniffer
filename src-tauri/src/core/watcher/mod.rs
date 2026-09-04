//! File-system watcher (the "文件监控器" in the README).
//!
//! Uses `notify` to watch a root directory recursively, debounces bursts of
//! events, and re-indexes the affected files incrementally (created/modified ->
//! upsert, deleted -> remove). This avoids re-running a full scan for every
//! change.

use crate::core::extractor::extract_text;
use crate::core::indexer::IndexManager;
use crate::core::scanner::FileEntry;
use notify::{RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Debounce window: how long to wait after the last event before flushing.
const DEBOUNCE_MS: u64 = 1000;

/// A running watcher. Dropping it stops watching (the bg thread is detached).
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Start watching `root`; changed files are re-indexed via `index`.
    pub fn start(root: PathBuf, index: Arc<IndexManager>) -> notify::Result<FileWatcher> {
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;
        watcher.watch(&root, RecursiveMode::Recursive)?;

        std::thread::spawn(move || watcher_loop(rx, index));
        Ok(FileWatcher { _watcher: watcher })
    }
}

fn watcher_loop(rx: Receiver<notify::Result<notify::Event>>, index: Arc<IndexManager>) {
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last = Instant::now();

    loop {
        // Wait for an event, or wake up every 200ms to check the debounce timer.
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                for p in event.paths {
                    pending.insert(p);
                }
                last = Instant::now();
            }
            Ok(Err(_)) => {}
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        if !pending.is_empty() && last.elapsed().as_millis() >= DEBOUNCE_MS as u128 {
            flush(&pending, &index);
            pending.clear();
        }
    }
}

/// Re-index every path in the pending set (deleted files are removed).
fn flush(paths: &HashSet<PathBuf>, index: &Arc<IndexManager>) {
    let list: Vec<&PathBuf> = paths.iter().collect();
    list.par_iter().for_each(|p| {
        let p = p.as_path();
        if p.exists() {
            if let Some(fe) = entry_from_path(p) {
                let content = extract_text(p);
                let _ = index.upsert_file(&fe, content);
            }
        } else {
            let _ = index.delete_by_path(&p.to_string_lossy());
        }
    });
    let _ = index.commit();
}

/// Build a `FileEntry` from an existing path (mirrors the walker's metadata logic).
fn entry_from_path(path: &Path) -> Option<FileEntry> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
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
