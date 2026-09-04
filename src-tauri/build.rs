// Register the `mobile` cfg used by tauri's mobile_entry_point attribute in
// lib.rs (normally registered by tauri-build, which is optional here).
#[cfg(feature = "tauri-app")]
fn main() {
    println!("cargo::rustc-check-cfg=cfg(mobile)");
    // tauri-build only drives the desktop build (tauri-app feature). The headless
    // server build (--no-default-features) must work without tauri.conf.json
    // processing, so skip it entirely in that case.
    tauri_build::build()
}

#[cfg(not(feature = "tauri-app"))]
fn main() {
    println!("cargo::rustc-check-cfg=cfg(mobile)");
}
