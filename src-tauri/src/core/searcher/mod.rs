//! Query engine.
//!
//! Preprocesses the advanced search syntax (size/mtime human units), runs the
//! query on the Tantivy index with BM25 relevance scoring, and assembles
//! `SearchResult`s including a content snippet (produced by re-reading the file
//! since `content` is intentionally not stored).

pub mod parser;

use crate::core::extractor::extract_text;
use crate::core::indexer::{Fields, IndexManager};
use serde::Serialize;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::Term;

/// A single search hit returned to the front-end.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub path: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub mtime: u64,
    pub score: f32,
    pub snippet: String,
}

/// Run a search and return the top `limit` results, ranked by BM25 relevance.
///
/// When `batch_id` is given, the results are restricted to documents belonging
/// to that import batch (the search query is ANDed with a batch_id term). Pass
/// `None` (or an empty string) to search across the whole index.
pub fn search(
    indexmgr: &IndexManager,
    raw_query: &str,
    limit: usize,
    batch_id: Option<&str>,
) -> tantivy::Result<Vec<SearchResult>> {
    // Rewrite human-friendly size/date filters into Tantivy-syntax first.
    let pre = parser::preprocess(raw_query);
    let parsed = indexmgr.parse_query(&pre)?;

    let query: Box<dyn Query> = match batch_id {
        Some(bid) if !bid.is_empty() => {
            let term = Term::from_field_text(indexmgr.fields.batch_id, bid);
            let batch_q: Box<dyn Query> =
                Box::new(TermQuery::new(term, IndexRecordOption::Basic));
            Box::new(BooleanQuery::new(vec![
                (Occur::Must, parsed),
                (Occur::Must, batch_q),
            ]))
        }
        _ => parsed,
    };

    let reader = indexmgr.reader()?;
    let searcher = reader.searcher();
    let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

    let highlight = extract_highlight_terms(raw_query);
    let mut results = Vec::with_capacity(top_docs.len());

    for (score, doc_addr) in top_docs {
        let doc = match searcher.doc(doc_addr) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let res = doc_to_result(&indexmgr.fields, &doc, score, &highlight);
        results.push(res);
    }
    Ok(results)
}

/// Map a Tantivy document + score into a `SearchResult`.
fn doc_to_result(fields: &Fields, doc: &tantivy::TantivyDocument, score: f32, highlight: &[String]) -> SearchResult {
    let s = |f: tantivy::schema::Field| {
        doc.get_first(f)
            .and_then(|v| v.as_str())
            .map(|x| x.to_string())
            .unwrap_or_default()
    };
    let u = |f: tantivy::schema::Field| {
        doc.get_first(f).and_then(|v| v.as_u64()).unwrap_or(0)
    };

    let path = s(fields.path);
    let name = s(fields.name);
    let ext = s(fields.ext);
    let snippet = make_snippet(&path, highlight);

    SearchResult {
        id: s(fields.id),
        path,
        name,
        ext,
        size: u(fields.size),
        mtime: u(fields.mtime),
        score,
        snippet,
    }
}

/// Extract simple highlight terms from a raw query (ASCII runs + CJK unigrams),
/// so the snippet can underline whatever the user is searching for.
fn extract_highlight_terms(raw: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut cur = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() || (ch as u32) >= 0x2e80 {
            cur.push(ch.to_lowercase().next().unwrap_or(ch));
        } else if !cur.is_empty() {
            terms.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        terms.push(cur);
    }
    terms
}

/// Snap a byte index down to the nearest UTF-8 char boundary (`i` inclusive).
fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Best-effort snippet: re-read the file's text (content is not stored), find
/// the first highlighted term and show a window around it.
fn make_snippet(path: &str, highlight: &[String]) -> String {
    if highlight.is_empty() {
        return String::new();
    }
    let mut text = match extract_text(Path::new(path)) {
        Some(t) => t,
        None => return String::new(),
    };
    // Collapse whitespace to keep the snippet compact.
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    let lower = text.to_lowercase();
    let mut best: Option<(usize, usize)> = None; // (start, end) of first match
    for term in highlight {
        let t = term.to_lowercase();
        if let Some(pos) = lower.find(&t) {
            best = Some((pos, pos + t.len()));
            break;
        }
    }

    let Some((start, end)) = best else {
        // No term found in content; fall back to the file name.
        return std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
    };

    let window = 160usize;
    let len = text.len();
    // Snap the window edges to UTF-8 char boundaries so we never slice in the
    // middle of a multi-byte character (e.g. CJK), which would panic.
    let from = floor_char_boundary(&text, start.saturating_sub(window / 2).min(len));
    let to = floor_char_boundary(&text, len.min(end + window / 2));
    let mut snip = text[from..to].to_string();
    if from > 0 {
        snip = format!("…{}", snip);
    }
    if to < len {
        snip.push('…');
    }
    snip
}
