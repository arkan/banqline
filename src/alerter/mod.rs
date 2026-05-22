pub mod types;

use rust_decimal::Decimal;

use crate::tagger;
pub use types::*;

/// Local TransactionRecord with the fields needed by alert evaluators.
/// The CLI layer converts from store::TransactionRecord into this type.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransactionRecord {
    pub account_uid: String,
    pub transaction_id: String,
    pub amount: String,
    pub currency: String,
    pub booking_date: String,
    pub value_date: String,
    pub transaction_date: String,
    pub remittance_info: Vec<String>,
    pub creditor_name: String,
    pub debtor_name: String,
    pub status: String,
    pub credit_debit_indicator: String,
    pub note: String,
    pub category: String,
    pub category_source: String,
}

/// Evaluates a transaction alert rule against a set of transactions.
/// All criteria (amount, merchant, direction) are AND-combined.
/// Returns TRIGGERED if any transaction matches all criteria.
pub fn evaluate_transaction(rule: &AlertRule, txns: &[TransactionRecord]) -> AlertResult {
    let crit = match &rule.transaction {
        Some(c) => c,
        None => {
            return AlertResult {
                rule: rule.clone(),
                status: "OK".to_string(),
                details: "no transaction criteria".to_string(),
                matched_transactions: Vec::new(),
            };
        }
    };

    let threshold = if crit.amount_gte.is_empty() {
        Decimal::ZERO
    } else {
        match Decimal::from_str_exact(&crit.amount_gte) {
            Ok(d) => d,
            Err(_) => {
                return AlertResult {
                    rule: rule.clone(),
                    status: "OK".to_string(),
                    details: format!("invalid threshold: {}", crit.amount_gte),
                    matched_transactions: Vec::new(),
                };
            }
        }
    };

    let merchant_upper = crit.merchant_contains.to_uppercase();

    let mut matches: Vec<MatchedTransaction> = Vec::new();

    for txn in txns {
        // Direction filter
        if !crit.direction.is_empty() && txn.credit_debit_indicator != crit.direction {
            continue;
        }

        // Amount check
        if !crit.amount_gte.is_empty() {
            let amt = match Decimal::from_str_exact(&txn.amount) {
                Ok(a) => a,
                Err(_) => continue,
            };
            if amt < threshold {
                continue;
            }
        }

        // Merchant check
        if !merchant_upper.is_empty() {
            let normalized = tagger::normalize(
                &txn.remittance_info,
                &txn.creditor_name,
                &txn.debtor_name,
                &txn.note,
            );
            if !normalized.contains(&merchant_upper) {
                continue;
            }
        }

        // Build matched transaction detail
        let date = if txn.booking_date.is_empty() {
            txn.transaction_date.clone()
        } else {
            txn.booking_date.clone()
        };

        let desc = if !txn.creditor_name.is_empty() {
            txn.creditor_name.clone()
        } else if !txn.debtor_name.is_empty() {
            txn.debtor_name.clone()
        } else if let Some(first) = txn.remittance_info.first() {
            first.clone()
        } else {
            String::new()
        };

        matches.push(MatchedTransaction {
            account_uid: txn.account_uid.clone(),
            date,
            amount: txn.amount.clone(),
            currency: txn.currency.clone(),
            description: desc,
        });
    }

    if !matches.is_empty() {
        let first_match = format!(
            "{} {} {}",
            matches[0].amount, matches[0].currency, matches[0].description
        );
        let details = format!("{} transaction(s) matched: {}", matches.len(), first_match);
        return AlertResult {
            rule: rule.clone(),
            status: "TRIGGERED".to_string(),
            details,
            matched_transactions: matches,
        };
    }

    AlertResult {
        rule: rule.clone(),
        status: "OK".to_string(),
        details: "no matching transactions".to_string(),
        matched_transactions: Vec::new(),
    }
}

/// Evaluates a category spending alert rule against a set of transactions.
/// Net spending = sum of DBIT amounts - sum of CRDT amounts for the category.
/// Returns TRIGGERED if net spending >= threshold.
pub fn evaluate_category(rule: &AlertRule, txns: &[TransactionRecord]) -> AlertResult {
    let crit = match &rule.category {
        Some(c) => c,
        None => {
            return AlertResult {
                rule: rule.clone(),
                status: "OK".to_string(),
                details: "no category criteria".to_string(),
                matched_transactions: Vec::new(),
            };
        }
    };

    let threshold = match Decimal::from_str_exact(&crit.threshold) {
        Ok(d) => d,
        Err(_) => {
            return AlertResult {
                rule: rule.clone(),
                status: "OK".to_string(),
                details: format!("invalid threshold: {}", crit.threshold),
                matched_transactions: Vec::new(),
            };
        }
    };

    let mut net = Decimal::ZERO;
    let mut currency = String::new();

    for txn in txns {
        if txn.category != crit.category {
            continue;
        }

        let amt = match Decimal::from_str_exact(&txn.amount) {
            Ok(a) => a,
            Err(_) => continue,
        };

        if currency.is_empty() {
            currency = txn.currency.clone();
        }

        match txn.credit_debit_indicator.as_str() {
            "DBIT" => net += amt,
            "CRDT" => net -= amt,
            _ => {}
        }
    }

    if net >= threshold {
        let detail_str = format!(
            "{}: {}/{} {}",
            crit.category,
            net.round_dp(2),
            threshold.round_dp(2),
            currency
        );
        return AlertResult {
            rule: rule.clone(),
            status: "TRIGGERED".to_string(),
            details: detail_str,
            matched_transactions: Vec::new(),
        };
    }

    let detail_str = format!(
        "{}: {}/{} {}",
        crit.category,
        net.round_dp(2),
        threshold.round_dp(2),
        currency
    );
    AlertResult {
        rule: rule.clone(),
        status: "OK".to_string(),
        details: detail_str,
        matched_transactions: Vec::new(),
    }
}

/// Evaluates all alert rules against the provided transactions.
/// If session_valid is false, returns ExitCode 2 without evaluating rules.
/// ExitCode 1 if any rule is TRIGGERED, 0 if all OK.
pub fn check(rules: &[AlertRule], txns: &[TransactionRecord], session_valid: bool) -> CheckOutput {
    if !session_valid {
        return CheckOutput {
            results: Vec::new(),
            exit_code: 2,
        };
    }

    let mut results: Vec<AlertResult> = Vec::new();
    let mut triggered = false;

    for rule in rules {
        let result = match rule.rule_type.as_str() {
            "transaction" => evaluate_transaction(rule, txns),
            "category" => evaluate_category(rule, txns),
            other => AlertResult {
                rule: rule.clone(),
                status: "OK".to_string(),
                details: format!("unknown rule type: {other}"),
                matched_transactions: Vec::new(),
            },
        };
        if result.status == "TRIGGERED" {
            triggered = true;
        }
        results.push(result);
    }

    let exit_code = if triggered { 1 } else { 0 };

    CheckOutput { results, exit_code }
}
