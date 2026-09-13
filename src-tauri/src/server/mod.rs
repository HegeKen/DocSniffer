//! Headless server mode: local HTTP API + embedded web UI.
//!
//! This is the Windows 7 compatibility path. It has **no** WebView / GUI
//! dependency — only `std` TCP via `tiny_http` — so the binary can be built
//! for Windows 7 (see README "Windows 7 支持") and driven from any modern
//! browser at `http://127.0.0.1:<port>`.
//!
//! The frontend bundle (`pnpm build` → repo-root `dist/`) is embedded into
//! release binaries and served as static assets; API endpoints mirror the
//! Tauri commands one-to-one (see `server/api.rs`).

pub mod api;
pub mod signal;

use crate::core::state::AppState;
use api::ServerContext;
use std::io::{Cursor, Read};
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Response, Server};

/// Addresses that only accept connections from the local machine.
const LOOPBACK_HOSTS: &[&str] = &["127.0.0.1", "localhost", "::1"];

/// CSP delivered with every embedded-UI response. The UI is same-origin only,
/// so no remote scripts/styles/connections are permitted.
const CSP_HEADER: &str = concat!(
    "default-src 'self'; ",
    "script-src 'self'; ",
    "style-src 'self' 'unsafe-inline'; ",
    "img-src 'self' data: blob:; ",
    "font-src 'self' data:; ",
    "connect-src 'self'; ",
    "object-src 'none'; ",
    "base-uri 'self'; ",
    "form-action 'none'"
);

/// Frontend assets from the repo-root `dist/` folder. In release builds the
/// files are embedded at compile time; in debug builds rust-embed reads them
/// from disk so `pnpm build` output is picked up without recompiling.
#[derive(rust_embed::RustEmbed)]
#[folder = "../dist"]
struct Frontend;

const DEFAULT_PORT: u16 = 8765;
const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

/// CLI entry-point invoked by the `docsniffer-server` binary.
pub fn run_cli() {
    let mut host = String::from("127.0.0.1");
    let mut port = DEFAULT_PORT;
    // Auto-open the UI in the default browser after the server starts. This is
    // the desired behaviour for the desktop-style server flow; pass --no-open
    // when running headless (e.g. CI / remote).
    let mut open = true;
    // The API has no authentication, so non-loopback binds are refused unless
    // the operator explicitly acknowledges the exposure with --allow-remote.
    let mut allow_remote = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => match args.next().and_then(|v| v.parse::<u16>().ok()) {
                Some(p) => port = p,
                None => die("--port expects a number (e.g. --port 9000)"),
            },
            "--host" => match args.next() {
                Some(h) => host = h,
                None => die("--host expects an address (e.g. --host 127.0.0.1)"),
            },
            "--allow-remote" => allow_remote = true,
            "--open" => open = true,
            "--no-open" => open = false,
            "-h" | "--help" => {
                print_usage();
                return;
            }
            other => {
                eprintln!("Unknown argument: {other}");
                print_usage();
                std::process::exit(2);
            }
        }
    }

    if !LOOPBACK_HOSTS.contains(&host.as_str()) && !allow_remote {
        die(&format!(
            "refusing to bind non-loopback address '{host}': the API is unauthenticated.\n\
             Use a loopback address (127.0.0.1 / localhost / ::1), or pass --allow-remote\n\
             if you deliberately want to expose the API to the network (e.g. trusted LAN)."
        ));
    }

    let state = match AppState::new() {
        Ok(s) => Arc::new(s),
        Err(e) => die(&format!("failed to initialize app state: {e}")),
    };
    let ctx = Arc::new(ServerContext {
        app: state,
        scan: Arc::new(api::ScanJob::new()),
    });

    let addr = format!("{host}:{port}");
    let server = match Server::http(&addr) {
        Ok(s) => Arc::new(s),
        Err(e) => die(&format!("failed to bind {addr}: {e}")),
    };

    println!("DocSniffer server mode");
    println!("  Listening on http://{addr}/");
    println!("  Data dir: {}", crate::core::resolve_data_dir().display());
    println!("  Press Ctrl+C to stop.");
    if !LOOPBACK_HOSTS.contains(&host.as_str()) {
        println!("  WARNING: --allow-remote enabled, bound to {host}.");
        println!("  The API is unauthenticated: anyone reachable can scan files and");
        println!("  delete the index. Only do this on a fully trusted network.");
    }

    if open {
        open_browser(&format!("http://{addr}/"));
    }

    let workers = 8;
    for _ in 0..workers {
        let server = server.clone();
        let ctx = ctx.clone();
        thread::spawn(move || loop {
            match server.recv() {
                Ok(request) => handle_request(request, &ctx),
                Err(e) => eprintln!("request error: {e}"),
            }
        });
    }

    // Worker threads are detached; block until SIGINT/SIGTERM (Unix) or a
    // Windows console Ctrl event, then exit cleanly.
    signal::wait_for_shutdown();
    println!("\nShutdown signal received, stopping.");
}

fn print_usage() {
    println!("DocSniffer server mode (Windows 7 compatible)");
    println!();
    println!("Usage: docsniffer-server [options]");
    println!("  --port <n>      Port to listen on (default {DEFAULT_PORT})");
    println!("  --host <ip>     Bind address (default 127.0.0.1, local only)");
    println!("  --allow-remote  Permit binding a non-loopback --host (unauthenticated!)");
    println!("  --open          Open the UI in the default browser on start (default)");
    println!("  --no-open       Do not auto-open the browser (headless mode)");
    println!("  -h, --help      Show this help");
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
}

fn handle_request(mut request: tiny_http::Request, ctx: &ServerContext) {
    let method = request.method().clone();
    let path = request
        .url()
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();

    let mut body = Vec::new();
    let _ = request
        .as_reader()
        .take(MAX_BODY_BYTES)
        .read_to_end(&mut body);

    let response = if path.starts_with("/api/") {
        let (status, json) = api::handle(ctx, &method, &path, &body);
        let payload = match json {
            serde_json::Value::Null => b"null".to_vec(),
            other => other.to_string().into_bytes(),
        };
        json_response(status, &payload)
    } else if method == Method::Options {
        // Cross-origin preflight: no CORS headers are ever emitted, so the
        // browser blocks the follow-up request. Return 204 without policy.
        Response::from_data(Vec::new()).with_status_code(204)
    } else {
        serve_static(&path)
    };

    let _ = request.respond(response);
}

fn json_response(status: u16, payload: &[u8]) -> Response<Cursor<Vec<u8>>> {
    let content_type = Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap();
    Response::from_data(payload.to_vec())
        .with_status_code(status)
        .with_header(content_type)
}

/// Attach browser-side hardening headers to UI responses (strict CSP, no MIME
/// sniffing, no framing).
fn with_security_headers(mut resp: Response<Cursor<Vec<u8>>>) -> Response<Cursor<Vec<u8>>> {
    for (name, value) in [
        ("Content-Security-Policy", CSP_HEADER),
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        ("Referrer-Policy", "no-referrer"),
    ] {
        if let Ok(h) = Header::from_bytes(name, value) {
            resp.add_header(h);
        }
    }
    resp
}

/// Serve the embedded frontend. Extension-less unknown paths fall back to
/// `index.html` so the SPA keeps working under deep links.
fn serve_static(path: &str) -> Response<Cursor<Vec<u8>>> {
    let rel = path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };

    let file = Frontend::get(rel).or_else(|| {
        if rel.contains('.') {
            None
        } else {
            Frontend::get("index.html")
        }
    });

    match file {
        Some(file) => {
            let mime = mime_of(rel);
            let header = Header::from_bytes("Content-Type", mime).unwrap();
            with_security_headers(Response::from_data(file.data.into_owned()).with_header(header))
        }
        None => {
            // Fresh checkout without `pnpm build`: guide the user instead of a
            // bare 404.
            let hint = "DocSniffer frontend assets are missing.\n\n\
                        Run `pnpm install && pnpm build` in the repository root,\n\
                        then rebuild this binary (release builds embed dist/).\n";
            let header = Header::from_bytes("Content-Type", "text/plain; charset=utf-8").unwrap();
            with_security_headers(
                Response::from_data(hint.as_bytes().to_vec())
                    .with_status_code(404)
                    .with_header(header),
            )
        }
    }
}

fn mime_of(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let _ = url;
}
