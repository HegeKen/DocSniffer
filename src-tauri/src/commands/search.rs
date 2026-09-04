//! Search command: run a full-text query against the index.

use crate::commands::AppState;
use crate::core::searcher::{search, SearchResult};
use tauri::State;

/// Run a search across file name/path/content, returning BM25-ranked results.
#[tauri::command]
pub async fn search_files(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResult>, String> {
    let index = state.index.clone();
    let limit = limit.unwrap_or(200);
    let handle = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<SearchResult>, String> {
        let results = search(&index, &query, limit).map_err(|e| e.to_string())?;
        Ok(results)
    });
    handle.await.map_err(|e| e.to_string())?
}
