pub mod group;

use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
pub use group::*;

/// Maps a category name to a list of substring patterns for matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRule {
    pub category: String,
    pub patterns: Vec<String>,
}

/// Normalize concatenates remittance info, creditor name, debtor name, and note
/// into a trimmed uppercase single-spaced string for pattern matching.
pub fn normalize(
    remittance_info: &[String],
    creditor_name: &str,
    debtor_name: &str,
    note: &str,
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for r in remittance_info {
        parts.push(r);
    }
    parts.push(creditor_name);
    parts.push(debtor_name);
    parts.push(note);
    let joined = parts.join(" ");
    collapse_spaces(&joined.to_uppercase())
}

fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns the first matching category from rules, or "uncategorized" if none match.
pub fn match_category(normalized: &str, rules: &[CategoryRule]) -> String {
    if normalized.is_empty() {
        return "uncategorized".to_string();
    }
    for rule in rules {
        for pattern in &rule.patterns {
            if pattern.is_empty() {
                continue;
            }
            if normalized.contains(&pattern.to_uppercase()) {
                return rule.category.clone();
            }
        }
    }
    "uncategorized".to_string()
}
