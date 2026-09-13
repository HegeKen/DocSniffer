//! Console signal handling for the headless server.
//!
//! The server used to park the main thread forever, so Ctrl+C killed the
//! process with no chance to print a clean shutdown message. Tantivy commits
//! per finished scan and its committed segments are immutable, so an in-flight
//! scan interrupted here costs at most that scan's uncommitted progress — never
//! index corruption.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static STOP: AtomicBool = AtomicBool::new(false);

/// Block the main thread until a termination signal arrives.
pub fn wait_for_shutdown() {
    install_handler();
    while !STOP.load(Ordering::SeqCst) {
        thread_sleep();
    }
}

#[allow(dead_code)] // only unused on platforms whose install_handler parks
fn thread_sleep() {
    std::thread::sleep(Duration::from_millis(200));
}

#[cfg(unix)]
fn install_handler() {
    extern "C" fn handler(_: libc::c_int) {
        STOP.store(true, Ordering::SeqCst);
    }
    unsafe {
        libc::signal(libc::SIGINT, handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, handler as libc::sighandler_t);
        // A client disconnecting while we respond must not SIGPIPE-kill us.
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

#[cfg(windows)]
fn install_handler() {
    use windows_sys::Win32::Foundation::TRUE;
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

    extern "system" fn handler(_: u32) -> i32 {
        STOP.store(true, Ordering::SeqCst);
        TRUE
    }
    // Ctrl+C / Ctrl+Break / console close all route through this handler.
    unsafe {
        let _ = SetConsoleCtrlHandler(Some(handler), TRUE);
    }
}

#[cfg(not(any(unix, windows)))]
fn install_handler() {
    // No signal support on this target: retain the old block-forever behaviour.
    loop {
        std::thread::park();
    }
}
