//! Sensitive-information rule model and matching.
//!
//! Rules are either `regex` patterns or `keyword` sets (pipe-separated). The
//! default library is embedded via `rust-embed` from `resources/rules/default_rules.json`; user
//! overrides are persisted in the portable-aware store and take precedence.

use crate::core::storage::Store;
use serde::{Deserialize, Serialize};

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
    /// Return the first fragment of `text` matched by this rule, if any.
    pub fn first_match(&self, text: &str) -> Option<String> {
        match self.rule_type.as_str() {
            "regex" => {
                let re = regex::Regex::new(&self.pattern).ok()?;
                re.find(text).map(|m| m.as_str().to_string())
            }
            // keyword: pattern is a `|` separated list of literal keywords
            _ => {
                for kw in self.pattern.split('|') {
                    let kw = kw.trim();
                    if !kw.is_empty() && text.contains(kw) {
                        return Some(kw.to_string());
                    }
                }
                None
            }
        }
    }

    /// Whether this rule should inspect the file content.
    pub fn scans_content(&self) -> bool {
        self.scope.iter().any(|s| s == "content")
    }

    /// Whether this rule should inspect the file name.
    pub fn scans_filename(&self) -> bool {
        self.scope.iter().any(|s| s == "filename" || s == "name")
    }
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
