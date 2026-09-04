//! DocSniffer library entry-point.
//!
//! Registers the shared application state and all Tauri commands on the builder.

pub mod commands;
pub mod core;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new().expect("failed to initialize DocSniffer app state");

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::scan::scan_directory,
            commands::scan::index_status,
            commands::search::search_files,
            commands::sensitive::get_rules,
            commands::sensitive::save_rules,
            commands::sensitive::sensitive_scan,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
