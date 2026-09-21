//! Index-location commands plus the OS file-manager / default-application
//! integration they share with the search UI.
//!
//! The index lives in `<data_dir>/index` by default; the user may move it to a
//! custom directory ("索引存储位置"). Moving migrates the existing index data
//! and persists the new location in the `Store`, so it survives a restart.
//! `open_index_dir` reveals the active directory in the OS file manager, and
//! `open_path` hands a single file over to its default application.

use crate::commands::AppState;
use crate::core::indexer::IndexManager;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::State;

/// Outcome of a successful index move.
#[derive(Serialize)]
pub struct IndexMoveReport {
    pub index_dir: String,
    pub documents: u64,
}

/// Reveal the active index directory in the system file manager (Finder /
/// Explorer / xdg-open). Returns the path that was opened.
#[tauri::command]
pub fn open_index_dir(state: State<'_, AppState>) -> Result<String, String> {
    let dir = state.index_dir();
    shell_open(&dir)?;
    Ok(dir.to_string_lossy().into_owned())
}

/// Open `path` with the OS default application: a document in its associated
/// program, a directory in the file manager.
#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    let path = PathBuf::from(path.trim());
    if !path.exists() {
        return Err(format!("文件不存在：{}", path.display()));
    }
    shell_open(&path)
}

/// Move the index to `dir` — migrating the existing index data — and switch to
/// it. The former directory is removed once nothing holds it open any more.
#[tauri::command]
pub async fn set_index_dir(
    state: State<'_, AppState>,
    dir: String,
) -> Result<IndexMoveReport, String> {
    let target = PathBuf::from(dir.trim());
    if target.as_os_str().is_empty() {
        return Err("请选择有效的目录".to_string());
    }

    let old_dir = state.index_dir();
    if same_dir(&old_dir, &target) {
        return Err("新位置与当前索引目录相同".to_string());
    }
    if target.starts_with(&old_dir) || old_dir.starts_with(&target) {
        return Err("新位置不能位于当前索引目录内部，也不能是它的父目录".to_string());
    }
    if target.join("meta.json").exists() {
        return Err(format!("目标目录已包含索引数据：{}", target.display()));
    }

    let index = state.index();
    let src = old_dir.clone();
    let dst = target.clone();
    let (new_index, documents) = tauri::async_runtime::spawn_blocking(
        move || -> Result<(IndexManager, u64), String> {
            // Flush pending writes so every segment is on disk before copying,
            // then release this handle: the source directory must not back an
            // open writer while it is copied and deleted.
            index.commit().map_err(|e| e.to_string())?;
            drop(index);

            copy_dir_recursive(&src, &dst)
                .map_err(|e| format!("复制索引到 {} 失败：{e}", dst.display()))?;

            let moved = IndexManager::open(&dst).map_err(|e| e.to_string())?;
            let documents = moved
                .reader()
                .map_err(|e| e.to_string())?
                .searcher()
                .num_docs();
            Ok((moved, documents))
        },
    )
    .await
    .map_err(|e| e.to_string())??;

    // Swap first: dropping the previous handle releases its writer and the OS
    // file handles, so the old directory can be deleted afterwards.
    state.replace_index(new_index);
    let _ = std::fs::remove_dir_all(&old_dir);

    let index_dir = target.to_string_lossy().into_owned();
    state
        .store
        .save(crate::core::INDEX_DIR_KEY, &index_dir)
        .map_err(|e| format!("索引已迁移到 {index_dir}，但保存设置失败：{e}"))?;

    Ok(IndexMoveReport {
        index_dir,
        documents,
    })
}

/// Compare two directories by canonical path, falling back to a lexical
/// comparison when the target does not exist yet.
fn same_dir(a: &Path, b: &Path) -> bool {
    let ca = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let cb = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    ca == cb
}

/// Copy the whole `src` tree into `dst` (creating `dst`), including the hidden
/// `.tantivy-*` marker files Tantivy keeps next to the segments.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src).min_depth(1) {
        let entry = entry.map_err(|e| std::io::Error::other(e.to_string()))?;
        let rel = entry.path().strip_prefix(src).map_err(std::io::Error::other)?;
        let to = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&to)?;
        } else {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Hand `path` to the OS: macOS `open`, Windows `start`, Linux `xdg-open`.
/// Directories land in the file manager, files in their default application.
fn shell_open(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        // `start` reads the first quoted argument as the window title, so an
        // empty one has to precede the path.
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");

    command
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开 {}：{e}", path.display()))
}
