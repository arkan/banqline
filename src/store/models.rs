use serde::{Deserialize, Serialize};

/// A locally stored bank account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountRecord {
    pub uid: String,
    pub bank_name: String,
    pub iban: String,
    pub name: String,
    pub currency: String,
    pub details: String,
    pub usage_type: String,
    pub account_type: String,
    pub alias: String,
}

/// A locally stored account balance snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceRecord {
    pub balance_type: String,
    pub amount: String,
    pub currency: String,
    pub reference_date: String,
}

/// A locally stored transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub account_uid: String,
    pub transaction_id: String,
    pub entry_reference: String,
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

/// A category update for a single transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryUpdate {
    pub account_uid: String,
    pub transaction_id: String,
    pub category: String,
    pub source: String,
}

/// Optional filters and pagination for transaction queries.
#[derive(Debug, Clone, Default)]
pub struct QueryOpts {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<i32>,
    pub status: Option<String>,
}
