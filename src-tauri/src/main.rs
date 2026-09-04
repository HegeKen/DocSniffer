//! DocSniffer binary entry-point.
//!
//! The `main.rs` is a thin shell that delegates to the library crate, keeping
//! the heavy lifting in `lib.rs` so it's testable and reusable.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "tauri-app")]
fn main() {
    docsniffer_lib::run();
}

#[cfg(not(feature = "tauri-app"))]
fn main() {
    eprintln!("This binary was built without the `tauri-app` feature.");
    eprintln!("For the headless server build, run `docsniffer-server` instead.");
    std::process::exit(1);
}
