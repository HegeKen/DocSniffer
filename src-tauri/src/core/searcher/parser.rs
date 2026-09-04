//! Query preprocessing for the advanced search syntax documented in the README.
//!
//! Tantivy's `QueryParser` already understands quoted phrases, `OR`/`AND`,
//! `-term` negation and `field:value`/`field:range` syntax. The only forms we
//! must rewrite are human-friendly size and date filters, e.g.
//! `size:>10mb` -> `size:>10485760` and `mtime:>2026-01-01` -> `mtime:>1767225600`.

use chrono::{NaiveDate, TimeZone, Utc};
use regex::Regex;
use std::sync::OnceLock;

/// Convert `size:>10mb` / `size:<1gb` / `mtime:>2026-01-01` into forms the
/// Tantivy query parser accepts.
pub fn preprocess(raw: &str) -> String {
    let mut out = raw.to_string();
    out = map_sizes(&out);
    out = map_mtimes(&out);
    out
}

fn map_sizes(input: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)size:(>=|<=|>|<|=)?(\d+(?:\.\d+)?)\s*([bkmgtp]?b?)").unwrap()
    });
    if !re.is_match(input) {
        return input.to_string();
    }
    regex_map(input, re, |c| {
        let op = match &c[1] {
            ">" | ">=" | "<" | "<=" => c[1].to_string(),
            "=" | "" => String::new(),
            _ => String::new(),
        };
        let bytes = to_bytes(&c[2], &c[3]);
        format!("size:{op}{bytes}")
    })
}

fn map_mtimes(input: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)mtime:(>=|<=|>|<)?(\d{4}-\d{2}-\d{2})").unwrap()
    });
    if !re.is_match(input) {
        return input.to_string();
    }
    regex_map(input, re, |c| {
        let op = match &c[1] {
            ">" | ">=" | "<" | "<=" => c[1].to_string(),
            _ => String::new(),
        };
        let epoch = date_to_epoch(&c[2]).unwrap_or(0);
        format!("mtime:{op}{epoch}")
    })
}

fn to_bytes(num: &str, unit: &str) -> u64 {
    let n: f64 = num.parse().unwrap_or(0.0);
    let u = unit.to_lowercase();
    let m: f64 = match u.as_str() {
        "" | "b" => 1.0,
        "k" | "kb" => 1024.0,
        "m" | "mb" => 1024.0 * 1024.0,
        "g" | "gb" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" => 1024.0_f64.powi(4),
        "p" | "pb" => 1024.0_f64.powi(5),
        _ => 1.0,
    };
    (n * m) as u64
}

fn date_to_epoch(ymd: &str) -> Option<u64> {
    let d = NaiveDate::parse_from_str(ymd, "%Y-%m-%d").ok()?;
    let dt = d.and_hms_opt(0, 0, 0)?;
    let ts = Utc.from_utc_datetime(&dt).timestamp();
    if ts < 0 {
        None
    } else {
        Some(ts as u64)
    }
}

/// Apply `f` to each match of `re`, rebuilding the string with replacements.
fn regex_map(input: &str, re: &Regex, f: impl Fn(&regex::Captures<'_>) -> String) -> String {
    let mut out = String::new();
    let mut last = 0;
    for cap in re.captures_iter(input) {
        let m = cap.get(0).unwrap();
        out.push_str(&input[last..m.start()]);
        out.push_str(&f(&cap));
        last = m.end();
    }
    out.push_str(&input[last..]);
    out
}
