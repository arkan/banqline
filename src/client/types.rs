use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_default_on_null<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthRequest {
    pub access: Access,
    pub aspsp: Aspsp,
    pub state: String,
    pub redirect_url: String,
    #[serde(rename = "psu_type")]
    pub psu_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Access {
    pub valid_until: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aspsp {
    pub name: String,
    pub country: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListAspspsResponse {
    pub aspsps: Vec<Aspsp>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedSession {
    pub session_id: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Session {
    pub session_id: String,
    pub status: String,
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountId {
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub iban: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Account {
    pub uid: String,
    pub account_id: AccountId,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub details: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub usage: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub currency: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub cash_account_type: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub psu_status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BalancesResponse {
    pub balances: Vec<Balance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Balance {
    pub balance_type: String,
    pub balance_amount: Amount,
    pub reference_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    pub amount: String,
    pub currency: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Party {
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transaction {
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub entry_reference: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub transaction_id: String,
    #[serde(rename = "transaction_amount")]
    pub amount: Amount,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub booking_date: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub value_date: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub transaction_date: String,
    #[serde(
        default,
        rename = "remittance_information",
        deserialize_with = "deserialize_default_on_null"
    )]
    pub remittance_info: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub creditor: Party,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub debtor: Party,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub status: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub credit_debit_indicator: String,
    #[serde(default, deserialize_with = "deserialize_default_on_null")]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionList {
    pub transactions: Vec<Transaction>,
    #[serde(default)]
    pub continuation_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TransactionOpts {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub status: Option<String>,
    pub continuation_key: Option<String>,
}
