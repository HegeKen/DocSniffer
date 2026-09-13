//! Sensitive-information rule model and matching.
//!
//! Rules are either `regex` patterns or `keyword` sets (pipe-separated). The
//! default library is embedded via `rust-embed` from `resources/rules/default_rules.json`; user
//! overrides are persisted in the portable-aware store and take precedence.

use crate::core::storage::Store;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// A single detection rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    pub risk_level: String,
    #[serde(default)]
    pub scope: Vec<String>,
}

/// A collection of rules (matches the JSON shape in the README).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

impl Rule {
    /// Whether this rule should inspect the file content.
    pub fn scans_content(&self) -> bool {
        self.scope.iter().any(|s| s == "content")
    }

    /// Whether this rule should inspect the file name.
    pub fn scans_filename(&self) -> bool {
        self.scope.iter().any(|s| s == "filename" || s == "name")
    }
}

/// Process-wide cache of compiled regexes, keyed by pattern text. Scanning a
/// folder used to recompile every rule's pattern once *per file*; the
/// automaton is now built once and cheaply `Clone`d (it is reference-counted
/// internally).
static RE_CACHE: Lazy<Mutex<HashMap<String, Regex>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Compile (or fetch from cache) the regex for `pattern`.
///
/// Unlike the previous `Regex::new(...).ok()?`, an invalid pattern returns an
/// explicit error so the UI can point at the broken rule instead of silently
/// never matching.
pub fn compile_regex(pattern: &str) -> Result<Regex, String> {
    if let Some(re) = RE_CACHE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(pattern)
    {
        return Ok(re.clone());
    }
    let re = Regex::new(pattern).map_err(|e| format!("正则表达式无效 `{pattern}`：{e}"))?;
    RE_CACHE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(pattern.to_string(), re.clone());
    Ok(re)
}

/// A rule with its regex compiled once, ready to scan many files.
pub struct CompiledRule {
    pub rule: Rule,
    /// `None` for keyword rules; `Some` for regex rules.
    re: Option<Regex>,
}

impl CompiledRule {
    /// Compile one rule, failing with a user-facing message for bad regexes.
    pub fn compile(rule: Rule) -> Result<Self, String> {
        let re = if rule.rule_type == "regex" {
            Some(compile_regex(&rule.pattern)?)
        } else {
            None
        };
        Ok(Self { rule, re })
    }

    /// Return the first fragment of `text` matched by this rule, if any.
    pub fn first_match(&self, text: &str) -> Option<String> {
        match &self.re {
            Some(re) => re.find(text).map(|m| m.as_str().to_string()),
            // keyword: pattern is a `|` separated list of literal keywords
            None => {
                for kw in self.rule.pattern.split('|') {
                    let kw = kw.trim();
                    if !kw.is_empty() && text.contains(kw) {
                        return Some(kw.to_string());
                    }
                }
                None
            }
        }
    }

    pub fn scans_content(&self) -> bool {
        self.rule.scans_content()
    }

    pub fn scans_filename(&self) -> bool {
        self.rule.scans_filename()
    }
}

/// Compile a full rule set; the first invalid regex aborts with its error.
pub fn compile_rules(rules: Vec<Rule>) -> Result<Vec<CompiledRule>, String> {
    rules.into_iter().map(CompiledRule::compile).collect()
}

/// Load rules for the current store: user overrides first, else embedded defaults.
pub fn load_rules(store: &Store) -> Vec<Rule> {
    if let Some(set) = store.load::<RuleSet>("rules") {
        return set.rules;
    }
    default_rules().rules
}

/// Persist user rules into the store.
pub fn save_rules(store: &Store, rules: Vec<Rule>) -> std::io::Result<()> {
    store.save("rules", &RuleSet { rules })
}

/// The built-in default rule library (embedded in the binary).
pub fn default_rules() -> RuleSet {
    if let Some(file) = Assets::get("rules/default_rules.json") {
        if let Ok(set) = serde_json::from_slice(&file.data) {
            return set;
        }
    }
    RuleSet::default()
}

/// Embedded resources (default_rules.json).
#[derive(rust_embed::RustEmbed)]
#[folder = "../resources/"]
struct Assets;
