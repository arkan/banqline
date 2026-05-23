use super::*;

impl SqliteStore {
    pub(crate) async fn get_last_synced(
        &self,
        account_uid: &str,
        data_type: &str,
    ) -> Result<DateTime<Utc>> {
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

    pub(crate) async fn set_last_synced(
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

    pub(crate) async fn set_account_alias(&self, uid: &str, alias: &str) -> Result<()> {
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

    pub(crate) async fn get_account_by_alias(&self, alias: &str) -> Result<Option<AccountRecord>> {
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

    pub(crate) async fn clear_account_alias(&self, alias: &str) -> Result<()> {
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
}

impl SqliteStore {
    #[allow(dead_code)]
    pub(crate) async fn get_metadata(&self, key: &str) -> Result<Option<String>> {
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

    pub(crate) async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
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

    #[allow(dead_code)]
    pub(crate) fn close(&self) -> Result<()> {
        // rusqlite Connection close is implicit when dropped.
        // We keep the method for API completeness.
        Ok(())
    }
}
