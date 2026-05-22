use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct BalanceInput {
    pub balance_type: String,
    pub amount: String,
    pub currency: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TxnInput {
    pub transaction_id: String,
    pub amount: String,
    pub currency: String,
    pub credit_debit_indicator: String,
    pub description: String,
    pub value_date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingTxn {
    pub amount: Decimal,
    pub description: String,
    pub value_date: String,
    pub is_credit: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountForecast {
    pub account_uid: String,
    pub iban: String,
    pub bank_name: String,
    pub currency: String,
    pub booked_balance: Decimal,
    pub booked_balance_type: String,
    pub pending_delta: Decimal,
    pub projected_balance: Decimal,
    pub has_pending_data: bool,
    pub pending_txns: Vec<PendingTxn>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CurrencyTotal {
    pub currency: String,
    pub total_booked: Decimal,
    pub total_pending: Decimal,
    pub total_projected: Decimal,
}

const BALANCE_PRIORITY: &[&str] = &["ITBD", "CLBD"];

/// Compute a projected balance from booked balance and pending transactions.
/// projected = booked + pending_delta where CRDT is positive and DBIT is negative.
pub fn forecast(balances: &[BalanceInput], pending_txns: &[TxnInput]) -> AccountForecast {
    let (booked, bal_type, warnings) = select_balance(balances);

    let currency = balances
        .first()
        .map(|b| b.currency.clone())
        .unwrap_or_default();

    let mut result = AccountForecast {
        account_uid: String::new(),
        iban: String::new(),
        bank_name: String::new(),
        currency,
        booked_balance: booked,
        booked_balance_type: bal_type,
        pending_delta: Decimal::ZERO,
        projected_balance: Decimal::ZERO,
        has_pending_data: false,
        pending_txns: Vec::new(),
        warnings,
    };

    let mut pending_delta = Decimal::ZERO;
    let mut parsed_txns: Vec<PendingTxn> = Vec::new();

    for txn in pending_txns {
        let amt = match Decimal::from_str(&txn.amount) {
            Ok(v) => v,
            Err(_) => {
                result.warnings.push(format!(
                    "skipping transaction {}: invalid amount {:?}",
                    txn.transaction_id, txn.amount
                ));
                continue;
            }
        };

        let is_credit = txn.credit_debit_indicator == "CRDT";
        let signed_amt = if is_credit { amt } else { -amt };

        pending_delta += signed_amt;
        parsed_txns.push(PendingTxn {
            amount: amt.abs(),
            description: txn.description.clone(),
            value_date: txn.value_date.clone(),
            is_credit,
        });
    }

    if parsed_txns.is_empty() {
        result.has_pending_data = false;
        result.pending_delta = Decimal::ZERO;
        result.projected_balance = booked;
    } else {
        result.has_pending_data = true;
        result.pending_delta = pending_delta;
        result.projected_balance = booked + pending_delta;
        result.pending_txns = parsed_txns;
    }

    result
}

/// Pick the best available balance using fallback chain: ITBD > CLBD > first.
fn select_balance(balances: &[BalanceInput]) -> (Decimal, String, Vec<String>) {
    if balances.is_empty() {
        return (
            Decimal::ZERO,
            String::new(),
            vec!["no balance data available".to_string()],
        );
    }

    for prio in BALANCE_PRIORITY {
        for b in balances {
            if b.balance_type == *prio {
                return match Decimal::from_str(&b.amount) {
                    Ok(amt) => (amt, b.balance_type.clone(), Vec::new()),
                    Err(_) => (
                        Decimal::ZERO,
                        b.balance_type.clone(),
                        vec![format!(
                            "invalid {} balance amount {:?}",
                            b.balance_type, b.amount
                        )],
                    ),
                };
            }
        }
    }

    let b = &balances[0];
    match Decimal::from_str(&b.amount) {
        Ok(amt) => (
            amt,
            b.balance_type.clone(),
            vec![format!(
                "using {} balance (ITBD/CLBD unavailable)",
                b.balance_type
            )],
        ),
        Err(_) => (
            Decimal::ZERO,
            b.balance_type.clone(),
            vec![format!(
                "invalid {} balance amount {:?}",
                b.balance_type, b.amount
            )],
        ),
    }
}

/// Group account forecasts by currency and sum their totals.
pub fn aggregate_by_currency(forecasts: &[AccountForecast]) -> Vec<CurrencyTotal> {
    let mut totals: BTreeMap<&str, CurrencyTotal> = BTreeMap::new();

    for f in forecasts {
        let ct = totals.entry(&f.currency).or_insert(CurrencyTotal {
            currency: f.currency.clone(),
            total_booked: Decimal::ZERO,
            total_pending: Decimal::ZERO,
            total_projected: Decimal::ZERO,
        });
        ct.total_booked += f.booked_balance;
        ct.total_pending += f.pending_delta;
        ct.total_projected += f.projected_balance;
    }

    totals.into_values().collect()
}
