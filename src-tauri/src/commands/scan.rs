//! Scan command: walk a directory, extract text and index all files, streaming
//! progress events to the front-end. Each scan creates an import batch, so the
//! index can be managed per batch (list / delete) or cleared entirely.

use crate::commands::AppState;
use crate::core::batch::{self, BatchInfo};
use crate::core::extractor::extract_text;
use crate::core::scanner::walker::MAX_SCAN_WARNINGS;
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

/// Terminal result of a scan: files indexed plus non-fatal warnings about
/// entries that could not be traversed or indexed.
#[derive(Clone, Serialize)]
pub struct ScanReport {
    pub indexed: i64,
    pub warnings: Vec<String>,
}

/// Scan `path` and add every file to the index under a new import batch.
/// Returns the number of files indexed and any non-fatal warnings. An optional
/// `batch_name` labels the batch; when omitted a name is derived from the
/// directory and timestamp.
#[tauri::command]
pub async fn scan_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    include_content: bool,
    batch_name: Option<String>,
) -> Result<ScanReport, String> {
    if !Path::new(&path).is_dir() {
        return Err(format!("不是有效目录: {path}"));
    }
    let batch_id = batch::new_id();
    let batch = BatchInfo {
        id: batch_id.clone(),
        name: batch_name.unwrap_or_else(|| batch::default_name(&path)),
        path: path.clone(),
        created_at: batch::now_millis(),
    };
    batch::add(&state.store, batch)?;

    let index = state.index.clone();
    let scan_batch = batch_id.clone();
    let handle = tauri::async_runtime::spawn_blocking(move || -> Result<ScanReport, String> {
        let root = Path::new(&path);
        let (files, mut warnings) = collect_files(root, &ScanOptions::default());
        let total = files.len();
        let mut indexed = 0usize;

        for fe in &files {
            let content = if include_content {
                extract_text(Path::new(&fe.path))
            } else {
                None
            };
            if let Err(e) = index.add_file(fe, content, &scan_batch) {
                if warnings.len() < MAX_SCAN_WARNINGS {
                    warnings.push(format!("{}：索引失败（{e}）", fe.path));
                }
            }
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

        index.commit().map_err(|e| e.to_string())?;
        Ok(ScanReport {
            indexed: indexed as i64,
            warnings,
        })
    });

    match handle.await.map_err(|e| e.to_string())? {
        Ok(report) => Ok(report),
        Err(e) => {
            // Scan failed: drop the (now empty) batch metadata we added above.
            batch::remove(&state.store, &batch_id);
            Err(e)
        }
    }
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

/// A batch plus its live document count (computed from the index).
#[derive(Serialize)]
pub struct BatchItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: u64,
    pub documents: usize,
}

/// List all import batches, newest first, with their current document counts.
#[tauri::command]
pub async fn list_batches(state: State<'_, AppState>) -> Result<Vec<BatchItem>, String> {
    let batches = batch::load_all(&state.store);
    let index = state.index.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::with_capacity(batches.len());
        for b in batches {
            let documents = index.count_by_batch(&b.id).unwrap_or(0);
            out.push(BatchItem {
                id: b.id,
                name: b.name,
                path: b.path,
                created_at: b.created_at,
                documents,
            });
        }
        Ok::<_, String>(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Delete one import batch and all of its documents. Returns the number of
/// documents removed.
#[tauri::command]
pub async fn delete_batch(state: State<'_, AppState>, batch_id: String) -> Result<i64, String> {
    let index = state.index.clone();
    let id = batch_id.clone();
    let deleted = tauri::async_runtime::spawn_blocking(move || -> Result<i64, String> {
        let count = index.count_by_batch(&id).unwrap_or(0) as i64;
        index.delete_by_batch(&id).map_err(|e| e.to_string())?;
        index.commit().map_err(|e| e.to_string())?;
        Ok(count)
    })
    .await
    .map_err(|e| e.to_string())??;

    batch::remove(&state.store, &batch_id);
    Ok(deleted)
}

/// Clear the entire index (all batches) and forget all batch metadata.
#[tauri::command]
pub async fn clear_all_index(state: State<'_, AppState>) -> Result<(), String> {
    let index = state.index.clone();
    let res = tauri::async_runtime::spawn_blocking(move || index.clear())
        .await
        .map_err(|e| e.to_string())?;
    res.map_err(|e| e.to_string())?;
    batch::clear(&state.store);
    Ok(())
}

/// Re-scan an import batch's directory and refresh its documents: files that
/// changed are re-indexed, newly added files are indexed, and files removed
/// from disk are dropped from the batch. Returns the number of files currently
/// indexed in the batch.
#[tauri::command]
pub async fn update_batch(state: State<'_, AppState>, batch_id: String) -> Result<i64, String> {
    let batch = batch::get(&state.store, &batch_id).ok_or_else(|| "批次不存在".to_string())?;
    let path = batch.path.clone();
    if !Path::new(&path).is_dir() {
        return Err(format!("目录不存在: {path}"));
    }
    let index = state.index.clone();
    let res = tauri::async_runtime::spawn_blocking(move || -> Result<i64, String> {
        let (files, _warnings) = collect_files(Path::new(&path), &ScanOptions::default());
        index.delete_by_batch(&batch_id).map_err(|e| e.to_string())?;
        for fe in &files {
            let content = extract_text(Path::new(&fe.path));
            let _ = index.add_file(fe, content, &batch_id);
        }
        let _ = index.commit();
        Ok(files.len() as i64)
    })
    .await
    .map_err(|e| e.to_string())?;
    res
}
