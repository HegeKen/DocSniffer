//! Scan command: walk a directory, extract text and index all files, streaming
//! progress events to the front-end.

use crate::commands::AppState;
use crate::core::extractor::extract_text;
use crate::core::scanner::{collect_files, ScanOptions};
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter, State};

/// Progress payload emitted on "scan-progress".
#[derive(Clone, Serialize)]
struct ScanProgress {
    indexed: usize,
    total: usize,
    path: String,
}

/// Scan `path` and add every file to the index.
/// Returns the number of files indexed.
#[tauri::command]
pub async fn scan_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    include_content: bool,
) -> Result<i64, String> {
    let index = state.index.clone();
    let handle = tauri::async_runtime::spawn_blocking(move || -> Result<i64, String> {
        let root = Path::new(&path);
        if !root.is_dir() {
            return Err(format!("不是有效目录: {path}"));
        }
        let files = collect_files(root, &ScanOptions::default());
        let total = files.len();
        let mut indexed = 0usize;

        for fe in &files {
            let content = if include_content {
                extract_text(Path::new(&fe.path))
            } else {
                None
            };
            let _ = index.add_file(fe, content);
            indexed += 1;

            if indexed % 200 == 0 || indexed == total {
                let _ = app.emit(
                    "scan-progress",
                    ScanProgress {
                        indexed,
                        total,
                        path: fe.path.clone(),
                    },
                );
            }
        }

        let _ = index.commit();
        Ok(indexed as i64)
    });

    handle.await.map_err(|e| e.to_string())?
}

/// Return a coarse index status (document count + data directory).
#[tauri::command]
pub async fn index_status(state: State<'_, AppState>) -> Result<IndexStatus, String> {
    let index = state.index.clone();
    let status = tauri::async_runtime::spawn_blocking(move || {
        let reader = index.reader().map_err(|e| e.to_string())?;
        let searcher = reader.searcher();
        let count = searcher.num_docs();
        Ok(IndexStatus {
            documents: count,
            data_dir: crate::core::resolve_data_dir().to_string_lossy().into_owned(),
        })
    });
    status.await.map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct IndexStatus {
    pub documents: u64,
    pub data_dir: String,
}
