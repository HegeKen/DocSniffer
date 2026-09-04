//! DocSniffer binary entry-point.
//!
//! The `main.rs` is a thin shell that delegates to the library crate, keeping
//! the heavy lifting in `lib.rs` so it's testable and reusable.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    docsniffer_lib::run();
}
