mod migrations;
pub mod models;

pub use models::*;

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

/// Store defines the contract for local banking data persistence.
///
/// All methods are async. The SQLite implementation runs synchronous rusqlite
/// calls inside `tokio::task::spawn_blocking`.
pub trait Store: Send + Sync {
    fn upsert_account(
        &self,
        bank_name: &str,
        acct: &AccountRecord,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_accounts(
        &self,
        bank_name: &str,
    ) -> impl std::future::Future<Output = Result<Vec<AccountRecord>>> + Send;
    fn get_all_accounts(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<AccountRecord>>> + Send;
    fn replace_balances(
        &self,
        account_uid: &str,
        balances: &[BalanceRecord],
        fetched_at: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_balances(
        &self,
        account_uid: &str,
    ) -> impl std::future::Future<Output = Result<(Vec<BalanceRecord>, DateTime<Utc>)>> + Send;
    fn upsert_transactions(
        &self,
        account_uid: &str,
        txns: &[TransactionRecord],
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_transactions(
        &self,
        account_uid: &str,
        opts: &QueryOpts,
    ) -> impl std::future::Future<Output = Result<Vec<TransactionRecord>>> + Send;
    fn get_all_transactions(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TransactionRecord>>> + Send;
    fn update_categories(
        &self,
        updates: &[CategoryUpdate],
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn update_category(
        &self,
        account_uid: &str,
        transaction_id: &str,
        category: &str,
        source: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_last_synced(
        &self,
        account_uid: &str,
        data_type: &str,
    ) -> impl std::future::Future<Output = Result<DateTime<Utc>>> + Send;
    fn set_last_synced(
        &self,
        account_uid: &str,
        data_type: &str,
        t: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn set_account_alias(
        &self,
        uid: &str,
        alias: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_account_by_alias(
        &self,
        alias: &str,
    ) -> impl std::future::Future<Output = Result<Option<AccountRecord>>> + Send;
    fn clear_account_alias(
        &self,
        alias: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn set_transaction_note(
        &self,
        account_uid: &str,
        transaction_id: &str,
        note: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    #[allow(dead_code)]
    fn get_metadata(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<String>>> + Send;
    fn set_metadata(
        &self,
        key: &str,
        value: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    #[allow(dead_code)]
    fn close(&self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// SqliteStore
// ---------------------------------------------------------------------------

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path).context("open database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=1;")
            .context("set pragmas")?;
        migrations::migrate(&conn).context("run migrations")?;
        Ok(SqliteStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl Store for SqliteStore {
    async fn upsert_account(&self, bank_name: &str, acct: &AccountRecord) -> Result<()> {
        let conn = self.conn.clone();
        let bank_name = bank_name.to_string();
        let acct = acct.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO accounts (uid, bank_name, iban, name, currency, details, usage_type, account_type, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
                 ON CONFLICT(uid) DO UPDATE SET
                     bank_name=excluded.bank_name,
                     iban=excluded.iban,
                     name=excluded.name,
                     currency=excluded.currency,
                     details=excluded.details,
                     usage_type=excluded.usage_type,
                     account_type=excluded.account_type,
                     updated_at=datetime('now')",
                params![
                    acct.uid,
                    bank_name,
                    acct.iban,
                    acct.name,
                    acct.currency,
                    acct.details,
                    acct.usage_type,
                    acct.account_type,
                ],
            )
            .with_context(|| format!("upsert account {}", acct.uid))?;

            Ok(())
        })
        .await?
    }

    async fn get_accounts(&self, bank_name: &str) -> Result<Vec<AccountRecord>> {
        let conn = self.conn.clone();
        let bank_name = bank_name.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT uid, bank_name, iban, name, currency, details, usage_type, account_type, alias
                     FROM accounts WHERE bank_name = ?1 ORDER BY iban",
                )
                .context("prepare get_accounts")?;

            let rows = stmt
                .query_map(params![bank_name], |row| {
                    Ok(AccountRecord {
                        uid: row.get(0)?,
                        bank_name: row.get(1)?,
                        iban: row.get(2)?,
                        name: row.get(3)?,
                        currency: row.get(4)?,
                        details: row.get(5)?,
                        usage_type: row.get(6)?,
                        account_type: row.get(7)?,
                        alias: row.get(8)?,
                    })
                })
                .context("query get_accounts")?;

            let mut accounts = Vec::new();
            for row in rows {
                accounts.push(row.context("scan account")?);
            }
            Ok(accounts)
        })
        .await?
    }

    async fn get_all_accounts(&self) -> Result<Vec<AccountRecord>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT uid, bank_name, iban, name, currency, details, usage_type, account_type, alias
                     FROM accounts ORDER BY bank_name, iban",
                )
                .context("prepare get_all_accounts")?;

            let rows = stmt
                .query_map([], |row| {
                    Ok(AccountRecord {
                        uid: row.get(0)?,
                        bank_name: row.get(1)?,
                        iban: row.get(2)?,
                        name: row.get(3)?,
                        currency: row.get(4)?,
                        details: row.get(5)?,
                        usage_type: row.get(6)?,
                        account_type: row.get(7)?,
                        alias: row.get(8)?,
                    })
                })
                .context("query get_all_accounts")?;

            let mut accounts = Vec::new();
            for row in rows {
                accounts.push(row.context("scan account")?);
            }
            Ok(accounts)
        })
        .await?
    }

    async fn replace_balances(
        &self,
        account_uid: &str,
        balances: &[BalanceRecord],
        fetched_at: DateTime<Utc>,
    ) -> Result<()> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        let balances = balances.to_vec();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let tx = conn.unchecked_transaction().context("begin replace balances")?;

            tx.execute("DELETE FROM balances WHERE account_uid = ?1", params![account_uid])
                .context("delete old balances")?;

            let fetched_at_str = fetched_at.to_rfc3339();
            for b in &balances {
                tx.execute(
                    "INSERT INTO balances (account_uid, balance_type, amount, currency, reference_date, fetched_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        account_uid,
                        b.balance_type,
                        b.amount,
                        b.currency,
                        b.reference_date,
                        fetched_at_str,
                    ],
                )
                .context("insert balance")?;
            }

            tx.commit().context("commit replace balances")
        })
        .await?
    }

    async fn get_balances(&self, account_uid: &str) -> Result<(Vec<BalanceRecord>, DateTime<Utc>)> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT balance_type, amount, currency, reference_date, fetched_at
                     FROM balances WHERE account_uid = ?1 ORDER BY balance_type",
                )
                .context("prepare get_balances")?;

            let mut balances = Vec::new();
            let mut fetched_at = DateTime::<Utc>::default();

            let rows = stmt
                .query_map(params![account_uid], |row| {
                    let btype: String = row.get(0)?;
                    let amount: String = row.get(1)?;
                    let currency: String = row.get(2)?;
                    let ref_date: String = row.get(3)?;
                    let fa_str: String = row.get(4)?;
                    Ok((btype, amount, currency, ref_date, fa_str))
                })
                .context("query get_balances")?;

            for row in rows {
                let (btype, amount, currency, ref_date, fa_str) = row.context("scan balance")?;
                if fetched_at == DateTime::<Utc>::default() {
                    fetched_at = DateTime::parse_from_rfc3339(&fa_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_default();
                }
                balances.push(BalanceRecord {
                    balance_type: btype,
                    amount,
                    currency,
                    reference_date: ref_date,
                });
            }

            Ok((balances, fetched_at))
        })
        .await?
    }

    async fn upsert_transactions(
        &self,
        account_uid: &str,
        txns: &[TransactionRecord],
    ) -> Result<()> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        let txns = txns.to_vec();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let tx = conn
                .unchecked_transaction()
                .context("begin upsert transactions")?;

            for t in &txns {
                let txn_id = if t.transaction_id.is_empty() {
                    &t.entry_reference
                } else {
                    &t.transaction_id
                };

                let remittance_json = serde_json::to_string(&t.remittance_info)
                    .context("marshal remittance info")?;

                tx.execute(
                    "INSERT INTO transactions (account_uid, transaction_id, entry_reference, amount, currency,
                        booking_date, value_date, transaction_date, remittance_info, creditor_name, debtor_name,
                        status, credit_debit_indicator, note)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                     ON CONFLICT(account_uid, transaction_id) DO UPDATE SET
                        entry_reference=excluded.entry_reference,
                        amount=excluded.amount,
                        currency=excluded.currency,
                        booking_date=excluded.booking_date,
                        value_date=excluded.value_date,
                        transaction_date=excluded.transaction_date,
                        remittance_info=excluded.remittance_info,
                        creditor_name=excluded.creditor_name,
                        debtor_name=excluded.debtor_name,
                        status=excluded.status,
                        credit_debit_indicator=excluded.credit_debit_indicator",
                    params![
                        account_uid,
                        txn_id,
                        t.entry_reference,
                        t.amount,
                        t.currency,
                        t.booking_date,
                        t.value_date,
                        t.transaction_date,
                        remittance_json,
                        t.creditor_name,
                        t.debtor_name,
                        t.status,
                        t.credit_debit_indicator,
                        t.note,
                    ],
                )
                .with_context(|| format!("upsert transaction {txn_id}"))?;
            }

            tx.commit().context("commit upsert transactions")
        })
        .await?
    }

    async fn get_transactions(
        &self,
        account_uid: &str,
        opts: &QueryOpts,
    ) -> Result<Vec<TransactionRecord>> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        let opts = opts.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut query = String::from(
                "SELECT transaction_id, entry_reference, amount, currency, booking_date, value_date,
                 transaction_date, remittance_info, creditor_name, debtor_name, status, credit_debit_indicator, note,
                 category, category_source
                 FROM transactions WHERE account_uid = ?1",
            );
            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            param_values.push(Box::new(account_uid.clone()));

            let mut next_idx = 2;

            if let Some(ref date_from) = opts.date_from {
                query.push_str(&format!(" AND booking_date >= ?{next_idx}"));
                param_values.push(Box::new(date_from.clone()));
                next_idx += 1;
            }
            if let Some(ref date_to) = opts.date_to {
                query.push_str(&format!(" AND booking_date <= ?{next_idx}"));
                param_values.push(Box::new(date_to.clone()));
                next_idx += 1;
            }
            if let Some(ref status) = opts.status {
                query.push_str(&format!(" AND status = ?{next_idx}"));
                param_values.push(Box::new(status.clone()));
                next_idx += 1;
            }

            query.push_str(" ORDER BY booking_date DESC, id DESC");

            if let Some(limit) = opts.limit {
                if limit > 0 {
                    query.push_str(&format!(" LIMIT ?{next_idx}"));
                    param_values.push(Box::new(limit));
                }
            }

            let params: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn.prepare(&query).context("prepare get_transactions")?;

            let rows = stmt
                .query_map(params.as_slice(), |row| {
                    let remittance_json: String = row.get(7)?;
                    let remittance_info: Vec<String> = serde_json::from_str(&remittance_json)
                        .unwrap_or_default();
                    Ok(TransactionRecord {
                        account_uid: account_uid.clone(),
                        transaction_id: row.get(0)?,
                        entry_reference: row.get(1)?,
                        amount: row.get(2)?,
                        currency: row.get(3)?,
                        booking_date: row.get(4)?,
                        value_date: row.get(5)?,
                        transaction_date: row.get(6)?,
                        remittance_info,
                        creditor_name: row.get(8)?,
                        debtor_name: row.get(9)?,
                        status: row.get(10)?,
                        credit_debit_indicator: row.get(11)?,
                        note: row.get(12)?,
                        category: row.get(13)?,
                        category_source: row.get(14)?,
                    })
                })
                .context("query get_transactions")?;

            let mut txns = Vec::new();
            for row in rows {
                txns.push(row.context("scan transaction")?);
            }
            Ok(txns)
        })
        .await?
    }

    async fn get_all_transactions(&self) -> Result<Vec<TransactionRecord>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare(
                    "SELECT account_uid, transaction_id, entry_reference, amount, currency, booking_date, value_date,
                     transaction_date, remittance_info, creditor_name, debtor_name, status, credit_debit_indicator, note,
                     category, category_source
                     FROM transactions ORDER BY booking_date DESC",
                )
                .context("prepare get_all_transactions")?;

            let rows = stmt
                .query_map([], |row| {
                    let remittance_json: String = row.get(8)?;
                    let remittance_info: Vec<String> =
                        serde_json::from_str(&remittance_json).unwrap_or_default();
                    Ok(TransactionRecord {
                        account_uid: row.get(0)?,
                        transaction_id: row.get(1)?,
                        entry_reference: row.get(2)?,
                        amount: row.get(3)?,
                        currency: row.get(4)?,
                        booking_date: row.get(5)?,
                        value_date: row.get(6)?,
                        transaction_date: row.get(7)?,
                        remittance_info,
                        creditor_name: row.get(9)?,
                        debtor_name: row.get(10)?,
                        status: row.get(11)?,
                        credit_debit_indicator: row.get(12)?,
                        note: row.get(13)?,
                        category: row.get(14)?,
                        category_source: row.get(15)?,
                    })
                })
                .context("query get_all_transactions")?;

            let mut txns = Vec::new();
            for row in rows {
                txns.push(row.context("scan transaction")?);
            }
            Ok(txns)
        })
        .await?
    }

    async fn update_categories(&self, updates: &[CategoryUpdate]) -> Result<()> {
        let conn = self.conn.clone();
        let updates = updates.to_vec();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let tx = conn
                .unchecked_transaction()
                .context("begin update categories")?;

            for u in &updates {
                tx.execute(
                    "UPDATE transactions SET category = ?1, category_source = ?2 WHERE account_uid = ?3 AND transaction_id = ?4",
                    params![u.category, u.source, u.account_uid, u.transaction_id],
                )
                .with_context(|| format!("update category for {}", u.transaction_id))?;
            }

            tx.commit().context("commit update categories")
        })
        .await?
    }

    async fn update_category(
        &self,
        account_uid: &str,
        transaction_id: &str,
        category: &str,
        source: &str,
    ) -> Result<()> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        let transaction_id = transaction_id.to_string();
        let category = category.to_string();
        let source = source.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "UPDATE transactions SET category = ?1, category_source = ?2 WHERE account_uid = ?3 AND transaction_id = ?4",
                params![category, source, account_uid, transaction_id],
            )
            .with_context(|| format!("update category for {transaction_id}"))
        })
        .await?
        .map(|_| ())
    }

    async fn get_last_synced(&self, account_uid: &str, data_type: &str) -> Result<DateTime<Utc>> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        let data_type = data_type.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let result: std::result::Result<String, rusqlite::Error> = conn.query_row(
                "SELECT last_synced FROM sync_meta WHERE account_uid = ?1 AND data_type = ?2",
                params![account_uid, data_type],
                |row| row.get(0),
            );

            match result {
                Ok(ts) => {
                    let dt = DateTime::parse_from_rfc3339(&ts)
                        .map(|dt| dt.with_timezone(&Utc))
                        .context("parse last_synced")?;
                    Ok(dt)
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(DateTime::default()),
                Err(e) => Err(e).context("get last_synced"),
            }
        })
        .await?
    }

    async fn set_last_synced(
        &self,
        account_uid: &str,
        data_type: &str,
        t: DateTime<Utc>,
    ) -> Result<()> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        let data_type = data_type.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO sync_meta (account_uid, data_type, last_synced) VALUES (?1, ?2, ?3)
                 ON CONFLICT(account_uid, data_type) DO UPDATE SET last_synced=excluded.last_synced",
                params![account_uid, data_type, t.to_rfc3339()],
            )
            .context("set last_synced")
        })
        .await?
        .map(|_| ())
    }

    async fn set_account_alias(&self, uid: &str, alias: &str) -> Result<()> {
        let conn = self.conn.clone();
        let uid = uid.to_string();
        let alias = alias.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "UPDATE accounts SET alias = ?1 WHERE uid = ?2",
                params![alias, uid],
            )
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("UNIQUE constraint failed: accounts.alias") {
                    anyhow::anyhow!("alias {alias:?} already in use")
                } else {
                    anyhow::Error::from(e).context("set account alias")
                }
            })
        })
        .await?
        .map(|_| ())
    }

    async fn get_account_by_alias(&self, alias: &str) -> Result<Option<AccountRecord>> {
        let conn = self.conn.clone();
        let alias = alias.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let result = conn.query_row(
                "SELECT uid, bank_name, iban, name, currency, details, usage_type, account_type, alias
                 FROM accounts WHERE alias = ?1",
                params![alias],
                |row| {
                    Ok(AccountRecord {
                        uid: row.get(0)?,
                        bank_name: row.get(1)?,
                        iban: row.get(2)?,
                        name: row.get(3)?,
                        currency: row.get(4)?,
                        details: row.get(5)?,
                        usage_type: row.get(6)?,
                        account_type: row.get(7)?,
                        alias: row.get(8)?,
                    })
                },
            );

            match result {
                Ok(acct) => Ok(Some(acct)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e).context("get account by alias"),
            }
        })
        .await?
    }

    async fn clear_account_alias(&self, alias: &str) -> Result<()> {
        let conn = self.conn.clone();
        let alias = alias.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let n = conn
                .execute(
                    "UPDATE accounts SET alias = '' WHERE alias = ?1",
                    params![alias],
                )
                .context("clear account alias")?;

            if n == 0 {
                anyhow::bail!("alias {alias:?} not found");
            }
            Ok(())
        })
        .await?
    }

    async fn set_transaction_note(
        &self,
        account_uid: &str,
        transaction_id: &str,
        note: &str,
    ) -> Result<()> {
        let conn = self.conn.clone();
        let account_uid = account_uid.to_string();
        let transaction_id = transaction_id.to_string();
        let note = note.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let n = conn
                .execute(
                    "UPDATE transactions SET note = ?1 WHERE account_uid = ?2 AND transaction_id = ?3",
                    params![note, account_uid, transaction_id],
                )
                .context("set transaction note")?;
            if n == 0 {
                anyhow::bail!("transaction {transaction_id:?} not found");
            }
            Ok(())
        })
        .await?
    }

    async fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let result = conn.query_row(
                "SELECT value FROM app_metadata WHERE key = ?1",
                params![key],
                |row| row.get(0),
            );

            match result {
                Ok(val) => Ok(Some(val)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e).with_context(|| format!("get metadata {key}")),
            }
        })
        .await?
    }

    async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.clone();
        let key = key.to_string();
        let value = value.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO app_metadata (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, value],
            )
            .with_context(|| format!("set metadata {key}"))
        })
        .await?
        .map(|_| ())
    }

    fn close(&self) -> Result<()> {
        // rusqlite Connection close is implicit when dropped.
        // We keep the method for API completeness.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn open_test_store() -> SqliteStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        // Keep tempdir alive by leaking it — tests are short-lived anyway.
        std::mem::forget(dir);
        SqliteStore::open(path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn test_open_creates_schema() {
        let store = open_test_store();
        let conn = store.conn.lock().unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"schema_version".to_string()));
        assert!(tables.contains(&"accounts".to_string()));
        assert!(tables.contains(&"balances".to_string()));
        assert!(tables.contains(&"transactions".to_string()));
        assert!(tables.contains(&"sync_meta".to_string()));
        assert!(tables.contains(&"app_metadata".to_string()));
    }

    #[test]
    fn test_open_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let path_str = path.to_str().unwrap();

        let _s1 = SqliteStore::open(path_str).unwrap();
        let _s2 = SqliteStore::open(path_str).unwrap();
    }

    #[tokio::test]
    async fn test_upsert_account() {
        let store = open_test_store();
        let acct = AccountRecord {
            uid: "acc-001".into(),
            iban: "FR7612345".into(),
            name: "Compte Courant".into(),
            currency: "EUR".into(),
            details: "Main account".into(),
            usage_type: "PRIV".into(),
            account_type: "CACC".into(),
            bank_name: String::new(),
            alias: String::new(),
        };
        store.upsert_account("MY_BANK", &acct).await.unwrap();

        let accounts = store.get_accounts("MY_BANK").await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].uid, "acc-001");
        assert_eq!(accounts[0].bank_name, "MY_BANK");
        assert_eq!(accounts[0].iban, "FR7612345");
        assert_eq!(accounts[0].name, "Compte Courant");
        assert_eq!(accounts[0].currency, "EUR");
    }

    #[tokio::test]
    async fn test_upsert_account_update() {
        let store = open_test_store();
        let acct = AccountRecord {
            uid: "acc-001".into(),
            iban: "FR76OLD".into(),
            name: "Old Name".into(),
            currency: "EUR".into(),
            ..Default::default()
        };
        store.upsert_account("MY_BANK", &acct).await.unwrap();

        let acct2 = AccountRecord {
            uid: "acc-001".into(),
            iban: "FR76NEW".into(),
            name: "New Name".into(),
            currency: "EUR".into(),
            ..Default::default()
        };
        store.upsert_account("MY_BANK", &acct2).await.unwrap();

        let accounts = store.get_accounts("MY_BANK").await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].iban, "FR76NEW");
        assert_eq!(accounts[0].name, "New Name");
    }

    #[tokio::test]
    async fn test_get_accounts_filter_by_bank() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK_A",
                &AccountRecord {
                    uid: "a1".into(),
                    iban: "FR76A".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_account(
                "BANK_B",
                &AccountRecord {
                    uid: "b1".into(),
                    iban: "FR76B".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let accounts = store.get_accounts("BANK_A").await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].uid, "a1");
    }

    #[tokio::test]
    async fn test_replace_balances() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let fetched_at = Utc.with_ymd_and_hms(2026, 3, 27, 10, 0, 0).unwrap();
        let balances = vec![BalanceRecord {
            balance_type: "CLBD".into(),
            amount: "1234.56".into(),
            currency: "EUR".into(),
            reference_date: "2026-03-27".into(),
        }];
        store
            .replace_balances("acc-001", &balances, fetched_at)
            .await
            .unwrap();

        let (got, got_time) = store.get_balances("acc-001").await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].balance_type, "CLBD");
        assert_eq!(got[0].amount, "1234.56");
        assert_eq!(got[0].currency, "EUR");
        assert_eq!(got_time, fetched_at);
    }

    #[tokio::test]
    async fn test_replace_balances_overwrite() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let t1 = Utc.with_ymd_and_hms(2026, 3, 26, 10, 0, 0).unwrap();
        store
            .replace_balances(
                "acc-001",
                &[BalanceRecord {
                    balance_type: "CLBD".into(),
                    amount: "100.00".into(),
                    currency: "EUR".into(),
                    reference_date: String::new(),
                }],
                t1,
            )
            .await
            .unwrap();

        let t2 = Utc.with_ymd_and_hms(2026, 3, 27, 10, 0, 0).unwrap();
        store
            .replace_balances(
                "acc-001",
                &[BalanceRecord {
                    balance_type: "ITBD".into(),
                    amount: "200.00".into(),
                    currency: "EUR".into(),
                    reference_date: String::new(),
                }],
                t2,
            )
            .await
            .unwrap();

        let (got, got_time) = store.get_balances("acc-001").await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].balance_type, "ITBD");
        assert_eq!(got[0].amount, "200.00");
        assert_eq!(got_time, t2);
    }

    #[tokio::test]
    async fn test_upsert_transactions() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let txns = vec![TransactionRecord {
            transaction_id: "txn-001".into(),
            amount: "42.50".into(),
            currency: "EUR".into(),
            booking_date: "2026-03-27".into(),
            creditor_name: "Amazon".into(),
            credit_debit_indicator: "DBIT".into(),
            remittance_info: vec!["Order #123".into()],
            ..Default::default()
        }];
        store.upsert_transactions("acc-001", &txns).await.unwrap();

        let got = store
            .get_transactions("acc-001", &QueryOpts::default())
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].transaction_id, "txn-001");
        assert_eq!(got[0].amount, "42.50");
        assert_eq!(got[0].creditor_name, "Amazon");
        assert_eq!(got[0].remittance_info, vec!["Order #123"]);
    }

    #[tokio::test]
    async fn test_upsert_transactions_update() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let txn = TransactionRecord {
            transaction_id: "txn-001".into(),
            amount: "10.00".into(),
            currency: "EUR".into(),
            status: "PDNG".into(),
            ..Default::default()
        };
        store.upsert_transactions("acc-001", &[txn]).await.unwrap();

        let txn2 = TransactionRecord {
            transaction_id: "txn-001".into(),
            amount: "10.50".into(),
            currency: "EUR".into(),
            status: "BOOK".into(),
            ..Default::default()
        };
        store.upsert_transactions("acc-001", &[txn2]).await.unwrap();

        let got = store
            .get_transactions("acc-001", &QueryOpts::default())
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].amount, "10.50");
        assert_eq!(got[0].status, "BOOK");
    }

    #[tokio::test]
    async fn test_upsert_transactions_empty_id_fallback() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let txn = TransactionRecord {
            transaction_id: String::new(),
            entry_reference: "ref-001".into(),
            amount: "5.00".into(),
            currency: "EUR".into(),
            ..Default::default()
        };
        store.upsert_transactions("acc-001", &[txn]).await.unwrap();

        let got = store
            .get_transactions("acc-001", &QueryOpts::default())
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].transaction_id, "ref-001");
    }

    #[tokio::test]
    async fn test_get_transactions_date_filter() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_transactions(
                "acc-001",
                &[
                    TransactionRecord {
                        transaction_id: "t1".into(),
                        amount: "1.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-25".into(),
                        ..Default::default()
                    },
                    TransactionRecord {
                        transaction_id: "t2".into(),
                        amount: "2.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-26".into(),
                        ..Default::default()
                    },
                    TransactionRecord {
                        transaction_id: "t3".into(),
                        amount: "3.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-27".into(),
                        ..Default::default()
                    },
                ],
            )
            .await
            .unwrap();

        let got = store
            .get_transactions(
                "acc-001",
                &QueryOpts {
                    date_from: Some("2026-03-26".into()),
                    date_to: Some("2026-03-26".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].transaction_id, "t2");
    }

    #[tokio::test]
    async fn test_get_transactions_limit() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_transactions(
                "acc-001",
                &[
                    TransactionRecord {
                        transaction_id: "t1".into(),
                        amount: "1.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-25".into(),
                        ..Default::default()
                    },
                    TransactionRecord {
                        transaction_id: "t2".into(),
                        amount: "2.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-26".into(),
                        ..Default::default()
                    },
                    TransactionRecord {
                        transaction_id: "t3".into(),
                        amount: "3.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-27".into(),
                        ..Default::default()
                    },
                ],
            )
            .await
            .unwrap();

        let got = store
            .get_transactions(
                "acc-001",
                &QueryOpts {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(got.len(), 2);
    }

    #[tokio::test]
    async fn test_set_get_last_synced() {
        let store = open_test_store();
        let ts = Utc.with_ymd_and_hms(2026, 3, 27, 12, 0, 0).unwrap();
        store
            .set_last_synced("acc-001", "transactions", ts)
            .await
            .unwrap();

        let got = store
            .get_last_synced("acc-001", "transactions")
            .await
            .unwrap();
        assert_eq!(got, ts);
    }

    #[tokio::test]
    async fn test_get_last_synced_zero() {
        let store = open_test_store();
        let got = store
            .get_last_synced("nonexistent", "transactions")
            .await
            .unwrap();
        assert_eq!(got, DateTime::<Utc>::default());
    }

    #[tokio::test]
    async fn test_get_transactions_status_filter() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_transactions(
                "acc-001",
                &[
                    TransactionRecord {
                        transaction_id: "t1".into(),
                        amount: "10.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-27".into(),
                        status: "PDNG".into(),
                        credit_debit_indicator: "DBIT".into(),
                        ..Default::default()
                    },
                    TransactionRecord {
                        transaction_id: "t2".into(),
                        amount: "20.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-27".into(),
                        status: "BOOK".into(),
                        credit_debit_indicator: "DBIT".into(),
                        ..Default::default()
                    },
                ],
            )
            .await
            .unwrap();

        let got = store
            .get_transactions(
                "acc-001",
                &QueryOpts {
                    status: Some("PDNG".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].transaction_id, "t1");
        assert_eq!(got[0].status, "PDNG");

        let got = store
            .get_transactions(
                "acc-001",
                &QueryOpts {
                    status: Some("BOOK".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].transaction_id, "t2");

        let got = store
            .get_transactions("acc-001", &QueryOpts::default())
            .await
            .unwrap();
        assert_eq!(got.len(), 2);
    }

    #[tokio::test]
    async fn test_get_all_transactions() {
        let store = open_test_store();

        let txns = store.get_all_transactions().await.unwrap();
        assert!(txns.is_empty());

        store
            .upsert_account(
                "BANK_A",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_account(
                "BANK_B",
                &AccountRecord {
                    uid: "acc-002".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        store
            .upsert_transactions(
                "acc-001",
                &[TransactionRecord {
                    transaction_id: "t1".into(),
                    amount: "10.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-27".into(),
                    creditor_name: "Shop A".into(),
                    ..Default::default()
                }],
            )
            .await
            .unwrap();
        store
            .upsert_transactions(
                "acc-002",
                &[TransactionRecord {
                    transaction_id: "t2".into(),
                    amount: "20.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-26".into(),
                    creditor_name: "Shop B".into(),
                    ..Default::default()
                }],
            )
            .await
            .unwrap();

        let txns = store.get_all_transactions().await.unwrap();
        assert_eq!(txns.len(), 2);
        for tx in &txns {
            assert_eq!(tx.category, "uncategorized");
            assert_eq!(tx.category_source, "");
        }
    }

    #[tokio::test]
    async fn test_update_categories() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_transactions(
                "acc-001",
                &[
                    TransactionRecord {
                        transaction_id: "t1".into(),
                        amount: "10.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-27".into(),
                        ..Default::default()
                    },
                    TransactionRecord {
                        transaction_id: "t2".into(),
                        amount: "20.00".into(),
                        currency: "EUR".into(),
                        booking_date: "2026-03-26".into(),
                        ..Default::default()
                    },
                ],
            )
            .await
            .unwrap();

        let updates = vec![
            CategoryUpdate {
                account_uid: "acc-001".into(),
                transaction_id: "t1".into(),
                category: "groceries".into(),
                source: "auto".into(),
            },
            CategoryUpdate {
                account_uid: "acc-001".into(),
                transaction_id: "t2".into(),
                category: "transport".into(),
                source: "auto".into(),
            },
        ];
        store.update_categories(&updates).await.unwrap();

        let txns = store.get_all_transactions().await.unwrap();
        assert_eq!(txns.len(), 2);

        let cat_map: std::collections::HashMap<&str, &str> = txns
            .iter()
            .map(|t| (t.transaction_id.as_str(), t.category.as_str()))
            .collect();
        assert_eq!(cat_map["t1"], "groceries");
        assert_eq!(cat_map["t2"], "transport");

        let src_map: std::collections::HashMap<&str, &str> = txns
            .iter()
            .map(|t| (t.transaction_id.as_str(), t.category_source.as_str()))
            .collect();
        assert_eq!(src_map["t1"], "auto");
        assert_eq!(src_map["t2"], "auto");
    }

    #[tokio::test]
    async fn test_update_category() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_transactions(
                "acc-001",
                &[TransactionRecord {
                    transaction_id: "t1".into(),
                    amount: "10.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-27".into(),
                    ..Default::default()
                }],
            )
            .await
            .unwrap();

        store
            .update_category("acc-001", "t1", "dining", "manual")
            .await
            .unwrap();

        let txns = store
            .get_transactions("acc-001", &QueryOpts::default())
            .await
            .unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].category, "dining");
        assert_eq!(txns[0].category_source, "manual");
    }

    #[tokio::test]
    async fn test_upsert_preserves_category() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let txn = TransactionRecord {
            transaction_id: "t1".into(),
            amount: "10.00".into(),
            currency: "EUR".into(),
            booking_date: "2026-03-27".into(),
            ..Default::default()
        };
        store
            .upsert_transactions("acc-001", &[txn.clone()])
            .await
            .unwrap();
        store
            .update_category("acc-001", "t1", "groceries", "manual")
            .await
            .unwrap();

        let txn2 = TransactionRecord {
            amount: "15.00".into(),
            ..txn
        };
        store.upsert_transactions("acc-001", &[txn2]).await.unwrap();

        let txns = store
            .get_transactions("acc-001", &QueryOpts::default())
            .await
            .unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].amount, "15.00");
        assert_eq!(txns[0].category, "groceries");
        assert_eq!(txns[0].category_source, "manual");
    }

    #[tokio::test]
    async fn test_upsert_preserves_note() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let txn = TransactionRecord {
            transaction_id: "t1".into(),
            amount: "10.00".into(),
            currency: "EUR".into(),
            status: "PDNG".into(),
            note: "user note".into(),
            ..Default::default()
        };
        store
            .upsert_transactions("acc-001", &[txn.clone()])
            .await
            .unwrap();

        let refreshed = TransactionRecord {
            amount: "11.00".into(),
            status: "BOOK".into(),
            note: "api note".into(),
            ..txn
        };
        store
            .upsert_transactions("acc-001", &[refreshed])
            .await
            .unwrap();

        let txns = store
            .get_transactions("acc-001", &QueryOpts::default())
            .await
            .unwrap();
        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].amount, "11.00");
        assert_eq!(txns[0].status, "BOOK");
        assert_eq!(txns[0].note, "user note");
    }

    #[tokio::test]
    async fn test_metadata() {
        let store = open_test_store();

        let val = store.get_metadata("alert_last_checked").await.unwrap();
        assert_eq!(val, None);

        store
            .set_metadata("alert_last_checked", "2026-03-27T12:00:00Z")
            .await
            .unwrap();
        let val = store.get_metadata("alert_last_checked").await.unwrap();
        assert_eq!(val, Some("2026-03-27T12:00:00Z".into()));

        store
            .set_metadata("alert_last_checked", "2026-03-28T08:00:00Z")
            .await
            .unwrap();
        let val = store.get_metadata("alert_last_checked").await.unwrap();
        assert_eq!(val, Some("2026-03-28T08:00:00Z".into()));
    }

    #[tokio::test]
    async fn test_set_account_alias() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    iban: "FR7612345678901234565133".into(),
                    name: "Compte Courant".into(),
                    currency: "EUR".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        store
            .set_account_alias("acc-001", "principal")
            .await
            .unwrap();

        let accounts = store.get_accounts("BANK").await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].alias, "principal");
    }

    #[tokio::test]
    async fn test_get_account_by_alias() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    iban: "FR7612345678901234565133".into(),
                    currency: "EUR".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .set_account_alias("acc-001", "principal")
            .await
            .unwrap();

        let acct = store.get_account_by_alias("principal").await.unwrap();
        assert!(acct.is_some());
        let acct = acct.unwrap();
        assert_eq!(acct.uid, "acc-001");
        assert_eq!(acct.iban, "FR7612345678901234565133");
        assert_eq!(acct.alias, "principal");
    }

    #[tokio::test]
    async fn test_set_account_alias_duplicate() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK_A",
                &AccountRecord {
                    uid: "acc-001".into(),
                    iban: "FR76A".into(),
                    currency: "EUR".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .upsert_account(
                "BANK_B",
                &AccountRecord {
                    uid: "acc-002".into(),
                    iban: "FR76B".into(),
                    currency: "EUR".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        store
            .set_account_alias("acc-001", "principal")
            .await
            .unwrap();
        let err = store
            .set_account_alias("acc-002", "principal")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already in use"));
    }

    #[tokio::test]
    async fn test_clear_account_alias() {
        let store = open_test_store();
        store
            .upsert_account(
                "BANK",
                &AccountRecord {
                    uid: "acc-001".into(),
                    iban: "FR76A".into(),
                    currency: "EUR".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        store
            .set_account_alias("acc-001", "principal")
            .await
            .unwrap();

        store.clear_account_alias("principal").await.unwrap();

        let acct = store.get_account_by_alias("principal").await.unwrap();
        assert!(acct.is_none());

        let accounts = store.get_accounts("BANK").await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].alias, "");
    }

    #[tokio::test]
    async fn test_upsert_account_preserves_alias() {
        let store = open_test_store();
        let acct = AccountRecord {
            uid: "acc-001".into(),
            iban: "FR76OLD".into(),
            name: "Old".into(),
            currency: "EUR".into(),
            ..Default::default()
        };
        store.upsert_account("BANK", &acct).await.unwrap();
        store
            .set_account_alias("acc-001", "principal")
            .await
            .unwrap();

        let acct2 = AccountRecord {
            uid: "acc-001".into(),
            iban: "FR76NEW".into(),
            name: "New".into(),
            currency: "EUR".into(),
            ..Default::default()
        };
        store.upsert_account("BANK", &acct2).await.unwrap();

        let accounts = store.get_accounts("BANK").await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].iban, "FR76NEW");
        assert_eq!(accounts[0].alias, "principal");
    }

    #[tokio::test]
    async fn test_get_account_by_alias_not_found() {
        let store = open_test_store();
        let acct = store.get_account_by_alias("nonexistent").await.unwrap();
        assert!(acct.is_none());
    }
}

impl Default for AccountRecord {
    fn default() -> Self {
        AccountRecord {
            uid: String::new(),
            bank_name: String::new(),
            iban: String::new(),
            name: String::new(),
            currency: String::new(),
            details: String::new(),
            usage_type: String::new(),
            account_type: String::new(),
            alias: String::new(),
        }
    }
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

impl Default for BalanceRecord {
    fn default() -> Self {
        BalanceRecord {
            balance_type: String::new(),
            amount: String::new(),
            currency: String::new(),
            reference_date: String::new(),
        }
    }
}
