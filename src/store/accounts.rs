use super::*;

impl SqliteStore {
    pub(crate) async fn upsert_account(&self, bank_name: &str, acct: &AccountRecord) -> Result<()> {
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

    pub(crate) async fn get_accounts(&self, bank_name: &str) -> Result<Vec<AccountRecord>> {
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

    pub(crate) async fn get_all_accounts(&self) -> Result<Vec<AccountRecord>> {
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
}
