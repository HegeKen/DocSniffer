//! Sensitive-information commands: rule management + scanning.

use crate::commands::AppState;
use crate::core::sensitive::{
    compile_rules, load_rules, save_rules as save_rules_store, scan_dir, Hit, Rule,
};
use std::path::Path;
use tauri::State;

/// Return the active rule set (user overrides, else embedded defaults).
#[tauri::command]
pub fn get_rules(state: State<'_, AppState>) -> Vec<Rule> {
    load_rules(&state.store)
}

/// Persist the user's rule set.
#[tauri::command]
pub fn save_rules(state: State<'_, AppState>, rules: Vec<Rule>) -> Result<(), String> {
    save_rules_store(&state.store, rules).map_err(|e| e.to_string())
}

/// Scan `path` (file or directory) against the active rules.
#[tauri::command]
pub async fn sensitive_scan(
    state: State<'_, AppState>,
    path: String,
    include_content: Option<bool>,
    active_rules: Option<Vec<Rule>>,
) -> Result<Vec<Hit>, String> {
    let include_content = include_content.unwrap_or(true);
    // Compile before blocking: an invalid regex fails fast with a message the
    // UI can show instead of silently never matching.
    let rules = compile_rules(match active_rules {
        Some(r) => r,
        None => load_rules(&state.store),
    })?;
    let handle = tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&path);
        let hits = if p.is_dir() {
            scan_dir(p, &rules, include_content)
        } else {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let mut hits = Vec::new();
            crate::core::sensitive::scan_file(
                &path,
                &name,
                &rules,
                include_content,
                &mut hits,
            );
            hits
        };
        Ok::<Vec<Hit>, String>(hits)
    });
    handle.await.map_err(|e| e.to_string())?
}
