//! DocSniffer library entry-point.
//!
//! Two front-ends share the same core:
//! - the Tauri 2 desktop app (`run`, requires the `tauri-app` feature), and
//! - the headless HTTP server (`server`, used for the Windows 7 build).

pub mod core;
pub mod server;

#[cfg(feature = "tauri-app")]
pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(feature = "tauri-app")]
pub fn run() {
    use commands::AppState;

    let state = AppState::new().expect("failed to initialize DocSniffer app state");

    tauri::Builder::default()
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
