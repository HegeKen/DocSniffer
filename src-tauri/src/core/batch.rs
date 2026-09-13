//! Import-batch metadata.
//!
//! The Tantivy index itself only knows about documents; "batches" are a
//! user-facing concept (one scan operation creates one batch). We persist a
//! lightweight list of batch metadata in the `Store` (JSON) and compute the
//! live document count from the index on demand, so the displayed count always
//! reflects the actual index state.

use crate::core::storage::Store;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Storage key holding the `Vec<BatchInfo>` list.
pub const STORE_KEY: &str = "batches";

/// A single import batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Creation time in Unix milliseconds.
    pub created_at: u64,
}

/// Load all batches (newest first). Returns an empty list when none exist yet.
pub fn load_all(store: &Store) -> Vec<BatchInfo> {
    store.load::<Vec<BatchInfo>>(STORE_KEY).unwrap_or_default()
}

/// Look up a single batch by id.
pub fn get(store: &Store, id: &str) -> Option<BatchInfo> {
    load_all(store).into_iter().find(|b| b.id == id)
}

/// Persist the full batch list.
pub fn save_all(store: &Store, batches: &[BatchInfo]) -> Result<(), String> {
    store
        .save(STORE_KEY, &batches.to_vec())
        .map_err(|e| e.to_string())
}

/// Append a batch and keep the list sorted newest first.
pub fn add(store: &Store, batch: BatchInfo) -> Result<(), String> {
    // Read/modify/write must be atomic: concurrent scans otherwise lose each
    // other's freshly appended batch metadata.
    let _g = store.lock_write();
    let mut list = load_all(store);
    list.push(batch);
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    save_all(store, &list)
}

/// Remove a batch by id. Returns whether a batch was actually removed.
pub fn remove(store: &Store, id: &str) -> bool {
    let _g = store.lock_write();
    let mut list = load_all(store);
    let before = list.len();
    list.retain(|b| b.id != id);
    if list.len() == before {
        return false;
    }
    let _ = save_all(store, &list);
    true
}

/// Drop all batch metadata (used by the "clear all index" action).
pub fn clear(store: &Store) {
    let _g = store.lock_write();
    store.remove(STORE_KEY);
}

/// Current timestamp in milliseconds since the Unix epoch.
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Process-wide monotonic sequence mixed into batch ids so two scans started
/// within the same millisecond cannot collide (a collision would merge their
/// documents, including on delete).
static ID_SEQ: AtomicU64 = AtomicU64::new(0);

/// Generate a unique batch id from the nanosecond timestamp + a sequence.
pub fn new_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("batch-{nanos:x}-{:04x}", seq & 0xffff)
}

/// Derive a human-readable default batch name from the scanned path.
pub fn default_name(path: &str) -> String {
    let dir = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    format!("{dir} · {}", now_millis())
}
