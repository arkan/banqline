use super::*;

impl SqliteStore {
    pub(crate) async fn replace_balances(
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

    pub(crate) async fn get_balances(
        &self,
        account_uid: &str,
    ) -> Result<(Vec<BalanceRecord>, DateTime<Utc>)> {
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
}
