use super::*;

#[derive(serde::Serialize)]
pub(crate) struct SyncReport {
    account: String,
    bank: String,
    tx_before: usize,
    tx_added: usize,
    balances: usize,
    status: String,
    synced_at: DateTime<Utc>,
}

pub(crate) fn sync_bank_names<'a>(
    sessions: &'a session::Store,
    bank: Option<&'a str>,
) -> Result<Vec<(&'a str, &'a session::StoredSession)>> {
    if let Some(name) = bank {
        let (_, sess) = resolve_bank(sessions, Some(name))?;
        return Ok(vec![(name, sess)]);
    }

    let mut selected = Vec::new();
    for (name, sess) in sessions {
        if sess.is_valid() {
            selected.push((name.as_str(), sess));
        }
    }
    if selected.is_empty() {
        anyhow::bail!("no valid sessions; run bank connect first");
    }
    Ok(selected)
}

pub(crate) async fn sync_account_uids(
    db: &store::SqliteStore,
    sess: &session::StoredSession,
    account: Option<&str>,
) -> Result<Vec<String>> {
    match account {
        Some(flag) => {
            let uid = resolve_alias(Some(db), flag).await;
            if !sess.accounts.iter().any(|acct| acct.uid == uid) {
                anyhow::bail!("account '{flag}' not found in selected bank session");
            }
            Ok(vec![uid])
        }
        None => Ok(sess.accounts.iter().map(|acct| acct.uid.clone()).collect()),
    }
}

pub(crate) async fn sync_accounts_for_bank(
    client: &client::Client,
    db: &store::SqliteStore,
    bank_name: &str,
    sess: &session::StoredSession,
) -> Result<Vec<SyncReport>> {
    let mut reports = Vec::new();

    for acct in &sess.accounts {
        let details = client
            .get_account_details(&acct.uid)
            .await
            .context("get account details")?;
        let rec = to_account_record(bank_name, &details);
        db.upsert_account(bank_name, &rec).await?;

        let api_balances = client
            .get_balances(&acct.uid)
            .await
            .context("get balances")?;
        let balance_recs = to_balance_records(api_balances);
        let balances = balance_recs.len();
        db.replace_balances(&acct.uid, &balance_recs, Utc::now())
            .await?;

        let tx_before = cached_transaction_count(db, &acct.uid).await?;
        reports.push(SyncReport {
            account: account_label(db, bank_name, acct).await,
            bank: bank_name.into(),
            tx_before,
            tx_added: 0,
            balances,
            status: "OK".into(),
            synced_at: Utc::now(),
        });
    }

    Ok(reports)
}

pub(crate) async fn sync_balances_for_bank(
    client: &client::Client,
    db: &store::SqliteStore,
    bank_name: &str,
    sess: &session::StoredSession,
    account: Option<&str>,
) -> Result<Vec<SyncReport>> {
    let account_uids = sync_account_uids(db, sess, account).await?;
    let mut reports = Vec::new();

    for uid in &account_uids {
        let acct = session_account_by_uid(sess, uid)?;
        let api_balances = client.get_balances(uid).await.context("get balances")?;
        let recs = to_balance_records(api_balances);
        let balances = recs.len();
        db.replace_balances(uid, &recs, Utc::now()).await?;
        let tx_before = cached_transaction_count(db, uid).await?;
        reports.push(SyncReport {
            account: account_label(db, bank_name, acct).await,
            bank: bank_name.into(),
            tx_before,
            tx_added: 0,
            balances,
            status: "OK".into(),
            synced_at: Utc::now(),
        });
    }

    Ok(reports)
}

pub(crate) async fn sync_transactions_for_bank(
    client: &client::Client,
    db: &store::SqliteStore,
    bank_name: &str,
    sess: &session::StoredSession,
    account: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<SyncReport>> {
    let account_uids = sync_account_uids(db, sess, account).await?;
    let mut reports = Vec::new();

    for uid in &account_uids {
        let acct = session_account_by_uid(sess, uid)?;
        let before = cached_transactions(db, uid).await?;
        let before_ids: HashSet<String> = before.iter().map(transaction_record_id).collect();
        let api_txns = fetch_all_transactions(client, uid, from, to, None).await?;
        let recs = to_transaction_records(uid, api_txns);
        let tx_added = recs
            .iter()
            .filter(|t| !before_ids.contains(&transaction_record_id(t)))
            .count();
        db.upsert_transactions(uid, &recs).await?;
        db.set_last_synced(uid, "transactions", Utc::now()).await?;
        reports.push(SyncReport {
            account: account_label(db, bank_name, acct).await,
            bank: bank_name.into(),
            tx_before: before.len(),
            tx_added,
            balances: 0,
            status: "OK".into(),
            synced_at: Utc::now(),
        });
    }

    Ok(reports)
}

pub(crate) async fn sync_all_for_bank(
    client: &client::Client,
    db: &store::SqliteStore,
    bank_name: &str,
    sess: &session::StoredSession,
    account: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<SyncReport>> {
    let account_uids = sync_account_uids(db, sess, account).await?;
    let mut reports = Vec::new();

    for uid in &account_uids {
        let acct = session_account_by_uid(sess, uid)?;
        let details = client
            .get_account_details(uid)
            .await
            .context("get account details")?;
        let rec = to_account_record(bank_name, &details);
        db.upsert_account(bank_name, &rec).await?;

        let api_balances = client.get_balances(uid).await.context("get balances")?;
        let balance_recs = to_balance_records(api_balances);
        let balances = balance_recs.len();
        db.replace_balances(uid, &balance_recs, Utc::now()).await?;

        let before = cached_transactions(db, uid).await?;
        let before_ids: HashSet<String> = before.iter().map(transaction_record_id).collect();
        let api_txns = fetch_all_transactions(client, uid, from, to, None).await?;
        let recs = to_transaction_records(uid, api_txns);
        let tx_added = recs
            .iter()
            .filter(|t| !before_ids.contains(&transaction_record_id(t)))
            .count();
        db.upsert_transactions(uid, &recs).await?;
        db.set_last_synced(uid, "transactions", Utc::now()).await?;

        reports.push(SyncReport {
            account: account_label(db, bank_name, acct).await,
            bank: bank_name.into(),
            tx_before: before.len(),
            tx_added,
            balances,
            status: "OK".into(),
            synced_at: Utc::now(),
        });
    }

    Ok(reports)
}

pub(crate) fn session_account_by_uid<'a>(
    sess: &'a session::StoredSession,
    uid: &str,
) -> Result<&'a session::StoredAccount> {
    sess.accounts
        .iter()
        .find(|acct| acct.uid == uid)
        .ok_or_else(|| anyhow::anyhow!("account '{uid}' not found in selected bank session"))
}

pub(crate) async fn cached_transactions(
    db: &store::SqliteStore,
    account_uid: &str,
) -> Result<Vec<store::TransactionRecord>> {
    db.get_transactions(account_uid, &store::QueryOpts::default())
        .await
}

pub(crate) async fn cached_transaction_count(
    db: &store::SqliteStore,
    account_uid: &str,
) -> Result<usize> {
    Ok(cached_transactions(db, account_uid).await?.len())
}

pub(crate) fn transaction_record_id(t: &store::TransactionRecord) -> String {
    if t.transaction_id.is_empty() {
        t.entry_reference.clone()
    } else {
        t.transaction_id.clone()
    }
}

pub(crate) async fn account_label(
    db: &store::SqliteStore,
    bank_name: &str,
    acct: &session::StoredAccount,
) -> String {
    if let Ok(accounts) = db.get_accounts(bank_name).await
        && let Some(rec) = accounts.iter().find(|a| a.uid == acct.uid)
    {
        if !rec.alias.is_empty() {
            return rec.alias.clone();
        }
        if !rec.name.is_empty() {
            return rec.name.clone();
        }
        if !rec.iban.is_empty() {
            return output::iban_suffix(&rec.iban, 5);
        }
    }
    if !acct.name.is_empty() {
        return acct.name.clone();
    }
    if !acct.iban.is_empty() {
        return output::iban_suffix(&acct.iban, 5);
    }
    acct.uid.clone()
}

pub(crate) fn print_sync_reports(pr: &output::Printer, reports: &[SyncReport]) -> Result<()> {
    if pr.json {
        let mut meta = HashMap::new();
        meta.insert("count".into(), serde_json::json!(reports.len()));
        return pr.print_json_with_meta(&reports, meta);
    }

    let (headers, rows) = sync_report_table(reports);
    pr.print_table(headers, rows)
}

pub(crate) fn sync_report_table(reports: &[SyncReport]) -> (Vec<String>, Vec<Vec<String>>) {
    let headers = vec![
        "ACCOUNT".into(),
        "BANK".into(),
        "TX BEFORE".into(),
        "TX ADDED".into(),
        "BALANCES".into(),
        "STATUS".into(),
    ];
    let rows = reports
        .iter()
        .map(|r| {
            vec![
                r.account.clone(),
                r.bank.clone(),
                r.tx_before.to_string(),
                r.tx_added.to_string(),
                r.balances.to_string(),
                r.status.clone(),
            ]
        })
        .collect();
    (headers, rows)
}

pub(crate) async fn cmd_sync(
    args: &SyncArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    let sessions = load_sessions(cfg)?;
    let db = open_store(cfg)?;
    let api_client = new_client(cfg)?;
    let mut reports = Vec::new();

    match &args.target {
        None | Some(SyncTarget::All(_)) => {
            let defaults;
            let all_args = match &args.target {
                Some(SyncTarget::All(a)) => a,
                _ => {
                    defaults = SyncAllArgs::default();
                    &defaults
                }
            };
            for (bank_name, sess) in sync_bank_names(&sessions, all_args.bank.as_deref())? {
                reports.extend(
                    sync_all_for_bank(
                        &api_client,
                        &db,
                        bank_name,
                        sess,
                        all_args.account.as_deref(),
                        all_args.from.as_deref(),
                        all_args.to.as_deref(),
                    )
                    .await?,
                );
            }
        }
        Some(SyncTarget::Tx(tx_args)) => {
            for (bank_name, sess) in sync_bank_names(&sessions, tx_args.bank.as_deref())? {
                reports.extend(
                    sync_transactions_for_bank(
                        &api_client,
                        &db,
                        bank_name,
                        sess,
                        tx_args.account.as_deref(),
                        tx_args.from.as_deref(),
                        tx_args.to.as_deref(),
                    )
                    .await?,
                );
            }
        }
        Some(SyncTarget::Balances(balance_args)) => {
            for (bank_name, sess) in sync_bank_names(&sessions, balance_args.bank.as_deref())? {
                reports.extend(
                    sync_balances_for_bank(
                        &api_client,
                        &db,
                        bank_name,
                        sess,
                        balance_args.account.as_deref(),
                    )
                    .await?,
                );
            }
        }
        Some(SyncTarget::Accounts(account_args)) => {
            for (bank_name, sess) in sync_bank_names(&sessions, account_args.bank.as_deref())? {
                reports.extend(sync_accounts_for_bank(&api_client, &db, bank_name, sess).await?);
            }
        }
    }

    print_sync_reports(pr, &reports)
}

#[cfg(test)]
mod sync_report_tests {
    use super::*;

    #[test]
    fn sync_report_table_is_account_centred() {
        let reports = vec![SyncReport {
            account: "main".into(),
            bank: "BNP Paribas".into(),
            tx_before: 321,
            tx_added: 0,
            balances: 2,
            status: "OK".into(),
            synced_at: Utc::now(),
        }];

        let (headers, rows) = sync_report_table(&reports);

        assert_eq!(
            headers,
            vec![
                "ACCOUNT",
                "BANK",
                "TX BEFORE",
                "TX ADDED",
                "BALANCES",
                "STATUS"
            ]
        );
        assert_eq!(
            rows,
            vec![vec!["main", "BNP Paribas", "321", "0", "2", "OK"]]
        );
    }
}
