use super::*;

impl SqliteStore {
    pub(crate) async fn upsert_transactions(
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

    pub(crate) async fn get_transactions(
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

            if let Some(limit) = opts.limit
                && limit > 0 {
                    query.push_str(&format!(" LIMIT ?{next_idx}"));
                    param_values.push(Box::new(limit));
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

    pub(crate) async fn get_all_transactions(&self) -> Result<Vec<TransactionRecord>> {
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
}
