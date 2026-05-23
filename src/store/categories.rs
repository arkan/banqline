use super::*;

impl SqliteStore {
    pub(crate) async fn update_categories(&self, updates: &[CategoryUpdate]) -> Result<()> {
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

    pub(crate) async fn update_category(
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
}

impl SqliteStore {
    pub(crate) async fn set_transaction_note(
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
}
