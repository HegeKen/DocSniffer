//! Search command: run a full-text query against the index.

use crate::commands::AppState;
use crate::core::searcher::{search, SearchResult};
use tauri::State;

/// Run a search across file name/path/content, returning BM25-ranked results.
/// When `batch_id` is provided the search is restricted to that import batch.
#[tauri::command]
pub async fn search_files(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
    batch_id: Option<String>,
) -> Result<Vec<SearchResult>, String> {
    let index = state.index.clone();
    let limit = limit.unwrap_or(200);
    let handle = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<SearchResult>, String> {
        let results = search(&index, &query, limit, batch_id.as_deref())
            .map_err(|e| e.to_string())?;
        Ok(results)
    });
    handle.await.map_err(|e| e.to_string())?
}
