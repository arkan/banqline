use serde::{Deserialize, Serialize};

/// AlertRule defines a single alert rule with its type and criteria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction: Option<TransactionAlertCriteria>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CategoryAlertCriteria>,
}

/// TransactionAlertCriteria holds criteria for transaction-based alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionAlertCriteria {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub amount_gte: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub merchant_contains: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub direction: String,
}

/// CategoryAlertCriteria holds criteria for category-based spending alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryAlertCriteria {
    pub category: String,
    pub threshold: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub period: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bank: String,
}

/// MatchedTransaction holds details of a single transaction that matched an alert rule.
#[derive(Debug, Clone, Serialize)]
pub struct MatchedTransaction {
    pub account_uid: String,
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub description: String,
}

/// AlertResult represents the evaluation result of a single alert rule.
#[derive(Debug, Clone, Serialize)]
pub struct AlertResult {
    pub rule: AlertRule,
    pub status: String,
    pub details: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matched_transactions: Vec<MatchedTransaction>,
}

/// CheckOutput holds the full results of evaluating all alert rules.
#[derive(Debug, Clone, Serialize)]
pub struct CheckOutput {
    pub results: Vec<AlertResult>,
    pub exit_code: i32,
}

impl AlertRule {
    /// Generates a default name for a rule based on its type and criteria.
    pub fn auto_name(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }

        match self.rule_type.as_str() {
            "transaction" => {
                let tx = match &self.transaction {
                    Some(t) => t,
                    None => return "transaction-unknown".to_string(),
                };
                let mut parts = vec!["transaction".to_string()];
                if !tx.amount_gte.is_empty() {
                    parts.push(tx.amount_gte.clone());
                }
                if !tx.merchant_contains.is_empty() {
                    parts.push(tx.merchant_contains.to_uppercase());
                }
                parts.join("-")
            }
            "category" => {
                let cat = match &self.category {
                    Some(c) => c,
                    None => return "category-unknown".to_string(),
                };
                let mut parts = vec![
                    "category".to_string(),
                    cat.category.clone(),
                    cat.threshold.clone(),
                ];
                if !cat.period.is_empty() {
                    parts.push(cat.period.clone());
                }
                parts.join("-")
            }
            other => format!("unknown-{other}"),
        }
    }
}
