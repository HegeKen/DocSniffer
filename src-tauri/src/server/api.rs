//! JSON API handlers for the headless server mode.
//!
//! Mirrors the Tauri command layer (`commands/`) 1:1 so both front-ends expose
//! identical functionality; only the transport differs (IPC vs. local HTTP).

use crate::core::extractor::extract_text;
use crate::core::scanner::{collect_files, ScanOptions};
use crate::core::searcher as searcher;
use crate::core::sensitive::{self, Hit, Rule};
use crate::core::state::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tiny_http::Method;

/// Everything a request handler needs: shared app state + the scan job slot.
pub struct ServerContext {
    pub app: Arc<AppState>,
    pub scan: Arc<ScanJob>,
}

/// State of the background scan job (there is at most one at a time, matching
/// the desktop behaviour where a second scan waits for the running one).
pub struct ScanJob {
    running: AtomicBool,
    indexed: AtomicUsize,
    total: AtomicUsize,
    current: Mutex<String>,
    outcome: Mutex<Option<ScanOutcome>>,
}

/// Terminal state of a scan job, serialized as `{"ok": n}` / `{"err": "msg"}`.
#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanOutcome {
    Ok(i64),
    Err(String),
}

impl ScanJob {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            indexed: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            current: Mutex::new(String::new()),
            outcome: Mutex::new(None),
        }
    }

    fn begin(&self, path: &str) {
        self.running.store(true, Ordering::SeqCst);
        self.indexed.store(0, Ordering::SeqCst);
        self.total.store(0, Ordering::SeqCst);
        *self.current.lock().unwrap() = path.to_string();
        *self.outcome.lock().unwrap() = None;
    }

    fn set_total(&self, total: usize) {
        self.total.store(total, Ordering::SeqCst);
    }

    fn set_progress(&self, indexed: usize, path: &str) {
        self.indexed.store(indexed, Ordering::SeqCst);
        *self.current.lock().unwrap() = path.to_string();
    }

    fn finish(&self, outcome: ScanOutcome) {
        *self.outcome.lock().unwrap() = Some(outcome);
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Default for ScanJob {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Request payloads (snake_case; the front-end adapter sends these keys) ----

#[derive(Deserialize)]
struct ScanReq {
    path: String,
    #[serde(default = "default_true")]
    include_content: bool,
}

#[derive(Deserialize)]
struct SearchReq {
    query: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct RulesReq {
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
struct SensitiveReq {
    path: String,
    include_content: Option<bool>,
    active_rules: Option<Vec<Rule>>,
}

fn default_true() -> bool {
    true
}

/// Dispatch an `/api/*` request. Returns `(HTTP status, JSON body)`.
pub fn handle(ctx: &ServerContext, method: &Method, path: &str, body: &[u8]) -> (u16, Value) {
    match (method, path) {
        (Method::Get, "/api/index_status") => index_status(ctx),
        (Method::Post, "/api/scan_directory") => start_scan(ctx, body),
        (Method::Get, "/api/scan_status") => scan_status(ctx),
        (Method::Post, "/api/search_files") => search_files(ctx, body),
        (Method::Get, "/api/rules") => (200, json!(sensitive::load_rules(&ctx.app.store))),
        (Method::Post, "/api/rules") => save_rules(ctx, body),
        (Method::Post, "/api/sensitive_scan") => sensitive_scan(ctx, body),
        _ => err(404, format!("未知接口: {path}")),
    }
}

fn ok(status: u16, value: Value) -> (u16, Value) {
    (status, value)
}

fn err(status: u16, message: impl Into<String>) -> (u16, Value) {
    (status, json!({ "error": message.into() }))
}

fn parse<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, (u16, Value)> {
    serde_json::from_slice(body)
        .map_err(|e| err(400, format!("请求体不是合法 JSON: {e}")))
}

fn index_status(ctx: &ServerContext) -> (u16, Value) {
    let reader = match ctx.app.index.reader() {
        Ok(r) => r,
        Err(e) => return err(500, e.to_string()),
    };
    let count = reader.searcher().num_docs();
    ok(
        200,
        json!({
            "documents": count,
            "data_dir": crate::core::resolve_data_dir().to_string_lossy(),
        }),
    )
}

fn start_scan(ctx: &ServerContext, body: &[u8]) -> (u16, Value) {
    let req: ScanReq = match parse(body) {
        Ok(r) => r,
        Err(e) => return e,
    };
    if ctx.scan.running.load(Ordering::SeqCst) {
        return err(409, "已有扫描任务正在进行中，请等待其完成");
    }
    if !Path::new(&req.path).is_dir() {
        return err(400, format!("不是有效目录: {}", req.path));
    }

    ctx.scan.begin(&req.path);
    let index = ctx.app.index.clone();
    let job = ctx.scan.clone();
    let root = req.path;
    let include_content = req.include_content;

    // Same pipeline as the Tauri command: collect → extract → add → commit,
    // with progress published into the shared job slot instead of IPC events.
    std::thread::spawn(move || {
        let files = collect_files(Path::new(&root), &ScanOptions::default());
        job.set_total(files.len());
        let mut indexed = 0usize;
        for fe in &files {
            let content = if include_content {
                extract_text(Path::new(&fe.path))
            } else {
                None
            };
            let _ = index.add_file(fe, content);
            indexed += 1;
            job.set_progress(indexed, &fe.path);
        }
        let outcome = match index.commit() {
            Ok(()) => ScanOutcome::Ok(indexed as i64),
            Err(e) => ScanOutcome::Err(e.to_string()),
        };
        job.finish(outcome);
    });

    ok(202, json!({ "started": true }))
}

fn scan_status(ctx: &ServerContext) -> (u16, Value) {
    let job = &ctx.scan;
    let outcome = job.outcome.lock().unwrap().clone();
    ok(
        200,
        json!({
            "running": job.running.load(Ordering::SeqCst),
            "indexed": job.indexed.load(Ordering::SeqCst),
            "total": job.total.load(Ordering::SeqCst),
            "path": job.current.lock().unwrap().clone(),
            "outcome": outcome,
        }),
    )
}

fn search_files(ctx: &ServerContext, body: &[u8]) -> (u16, Value) {
    let req: SearchReq = match parse(body) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let limit = req.limit.unwrap_or(200);
    match searcher::search(&ctx.app.index, &req.query, limit) {
        Ok(results) => ok(200, serde_json::to_value(results).unwrap_or(json!([]))),
        Err(e) => err(400, e.to_string()),
    }
}

fn save_rules(ctx: &ServerContext, body: &[u8]) -> (u16, Value) {
    let req: RulesReq = match parse(body) {
        Ok(r) => r,
        Err(e) => return e,
    };
    match sensitive::save_rules(&ctx.app.store, req.rules) {
        Ok(()) => ok(200, json!({ "saved": true })),
        Err(e) => err(500, e.to_string()),
    }
}

fn sensitive_scan(ctx: &ServerContext, body: &[u8]) -> (u16, Value) {
    let req: SensitiveReq = match parse(body) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let include_content = req.include_content.unwrap_or(true);
    let rules = match req.active_rules {
        Some(r) => r,
        None => sensitive::load_rules(&ctx.app.store),
    };
    let p = Path::new(&req.path);
    let hits: Vec<Hit> = if p.is_dir() {
        sensitive::scan_dir(p, &rules, include_content)
    } else {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut hits = Vec::new();
        sensitive::scan_file(&req.path, &name, &rules, include_content, &mut hits);
        hits
    };
    ok(200, serde_json::to_value(hits).unwrap_or(json!([])))
}
