//! `docsniffer-server` — headless DocSniffer server.
//!
//! Same core as the desktop app, exposed over a local HTTP API with the web UI
//! embedded. This binary is the Windows 7 compatible distribution target
//! (built with `--no-default-features`, see README "Windows 7 支持") and is a
//! handy remote/headless mode on any platform.

fn main() {
    docsniffer_lib::server::run_cli();
}
