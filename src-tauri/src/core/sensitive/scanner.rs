//! Sensitive-information scanner.
//!
//! Walks a directory (or a single file), checks each file's name and optionally
//! its extracted content against the active rules, and returns a flat `Hit`
//! report that can be rendered and exported by the front-end.

use super::rules::Rule;
use crate::core::extractor::extract_text;
use crate::core::scanner::ScanOptions;
use serde::Serialize;
use std::path::Path;

/// One detected sensitive hit.
#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub path: String,
    pub file_name: String,
    pub rule_id: String,
    pub rule_name: String,
    pub rule_type: String,
    pub risk_level: String,
    /// Where the match was found: "filename" or "content".
    pub match_type: String,
    /// The matched fragment (or surrounding text for content matches).
    pub matched: String,
    pub file_size: u64,
    pub mtime: u64,
}

/// Scan every file under `root` against `rules` and return all hits.
pub fn scan_dir(root: &Path, rules: &[Rule], include_content: bool) -> Vec<Hit> {
    let files = crate::core::scanner::collect_files(root, &ScanOptions::default());
    let mut hits = Vec::new();

    // Keep content extraction on one thread pool for speed; collections are
    // cheap so a single pass is enough for this feature.
    for fe in files {
        scan_file(&fe.path, &fe.name, rules, include_content, &mut hits);
    }
    hits
}

/// Scan one file and push any hits into `out`.
pub fn scan_file(path: &str, name: &str, rules: &[Rule], include_content: bool, out: &mut Vec<Hit>) {
    let meta = std::fs::metadata(path);
    let (size, mtime) = match &meta {
        Ok(m) => (
            m.len(),
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        ),
        Err(_) => (0, 0),
    };

    // Content snapshot is only extracted when at least one rule needs it.
    let content = if include_content && rules.iter().any(|r| r.scans_content()) {
        extract_text(Path::new(path))
    } else {
        None
    };

    for rule in rules {
        // Filename scope
        if rule.scans_filename() {
            if let Some(m) = rule.first_match(name) {
                out.push(build_hit(path, name, rule, "filename", m, size, mtime));
            }
        }
        // Content scope
        if rule.scans_content() {
            if let Some(text) = &content {
                if let Some(m) = rule.first_match(text) {
                    let fragment = snippet_around(text, &m);
                    out.push(build_hit(path, name, rule, "content", fragment, size, mtime));
                }
            }
        }
    }
}

fn build_hit(
    path: &str,
    name: &str,
    rule: &Rule,
    match_type: &str,
    matched: String,
    file_size: u64,
    mtime: u64,
) -> Hit {
    Hit {
        path: path.to_string(),
        file_name: name.to_string(),
        rule_id: rule.id.clone(),
        rule_name: rule.name.clone(),
        rule_type: rule.rule_type.clone(),
        risk_level: rule.risk_level.clone(),
        match_type: match_type.to_string(),
        matched,
        file_size,
        mtime,
    }
}

/// Show a short window of text around `needle` for the report.
fn snippet_around(text: &str, needle: &str) -> String {
    let window = 80usize;
    let lower = text.to_lowercase();
    let n = needle.to_lowercase();
    let Some(pos) = lower.find(&n) else {
        return needle.to_string();
    };
    let from = pos.saturating_sub(window / 2);
    let mut s = text[from..text.len().min(pos + n.len() + window / 2)].to_string();
    if from > 0 {
        s = format!("…{}", s);
    }
    s
}
