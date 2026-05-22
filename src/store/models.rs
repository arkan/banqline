use serde::{Deserialize, Serialize};

/// A locally stored bank account.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

impl Default for TransactionRecord {
    fn default() -> Self {
        TransactionRecord {
            account_uid: String::new(),
            transaction_id: String::new(),
            entry_reference: String::new(),
            amount: String::new(),
            currency: String::new(),
            booking_date: String::new(),
            value_date: String::new(),
            transaction_date: String::new(),
            remittance_info: Vec::new(),
            creditor_name: String::new(),
            debtor_name: String::new(),
            status: String::new(),
            credit_debit_indicator: String::new(),
            note: String::new(),
            category: "uncategorized".into(),
            category_source: String::new(),
        }
    }
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
