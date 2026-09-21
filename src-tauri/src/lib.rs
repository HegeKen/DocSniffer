//! DocSniffer library entry-point.
//!
//! The Tauri 2 desktop app (`run`) drives the shared core over IPC commands
//! (`commands/`).

pub mod commands;
pub mod core;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use commands::AppState;

    let state = AppState::new().expect("failed to initialize DocSniffer app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::scan::scan_directory,
            commands::scan::index_status,
            commands::scan::list_batches,
            commands::scan::delete_batch,
            commands::scan::clear_all_index,
            commands::scan::update_batch,
            commands::search::search_files,
            commands::sensitive::get_rules,
            commands::sensitive::save_rules,
            commands::sensitive::sensitive_scan,
            commands::settings::open_index_dir,
            commands::settings::open_path,
            commands::settings::set_index_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
