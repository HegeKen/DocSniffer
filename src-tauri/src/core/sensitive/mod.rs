//! Sensitive-information detection engine.
//!
//! Exposes the rule model + matcher (`rules`) and the directory/file scanner
//! (`scanner`). This is the "敏感信息检测引擎" in the README architecture.

pub mod rules;
pub mod scanner;

pub use rules::{load_rules, save_rules, Rule, RuleSet};
pub use scanner::{scan_dir, scan_file, Hit};
