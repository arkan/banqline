use super::*;

pub(crate) async fn cmd_accounts(
    args: &AccountsArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    if let Some(ref alias_cmd) = args.alias {
        return cmd_accounts_alias(alias_cmd, cfg, pr).await;
    }

    let sessions = load_sessions(cfg)?;
    let (bank_name, _sess) = resolve_bank(&sessions, args.bank.as_deref())?;
    let db = open_store(cfg)?;
    let aliases = build_alias_map(&db).await;

    let stored = db.get_accounts(bank_name).await?;
    if stored.is_empty() {
        println!("No accounts found for {bank_name}");
        return Ok(());
    }

    let mut rows = Vec::new();
    let mut total_booked = Decimal::ZERO;

    for acct in &stored {
        let display_name = match aliases.get(&acct.uid) {
            Some(alias) => format!("{alias} ({})", output::iban_suffix(&acct.iban, 6)),
            None => output::iban_suffix(&acct.iban, 6),
        };

        let (bals, _) = db.get_balances(&acct.uid).await.unwrap_or_default();
        let booked = select_main_balance(&bals);
        let booked_str = booked
            .map(|b| format_amount(&b.amount))
            .unwrap_or_else(|| "-".to_string());

        if let Some(b) = booked
            && let Ok(v) = Decimal::from_str_exact(&b.amount)
        {
            total_booked += v;
        }

        rows.push(vec![
            display_name,
            acct.iban.clone(),
            acct.currency.clone(),
            booked_str,
        ]);
    }

    let headers = vec![
        "ACCOUNT".into(),
        "IBAN".into(),
        "CURRENCY".into(),
        "BALANCE".into(),
    ];

    let footer = format!("Total booked: {:.2}", total_booked.round_dp(2));

    if pr.json {
        let data: Vec<serde_json::Value> = stored
            .iter()
            .map(|a| {
                serde_json::json!({
                    "uid": a.uid,
                    "iban": a.iban,
                    "name": a.name,
                    "currency": a.currency,
                    "alias": a.alias,
                })
            })
            .collect();
        let mut meta = HashMap::new();
        meta.insert(
            "bank".to_string(),
            serde_json::Value::String(bank_name.to_string()),
        );
        meta.insert(
            "total_booked".to_string(),
            serde_json::json!(total_booked.round_dp(2)),
        );
        pr.print_json_with_meta(&data, meta)?;
    } else {
        pr.print_table_with_footer(headers, rows, &footer)?;
    }

    Ok(())
}

pub(crate) async fn cmd_accounts_alias(
    alias_cmd: &AliasCommand,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    let db = open_store(cfg)?;

    match alias_cmd {
        AliasCommand::Set { alias, uid } => {
            db.set_account_alias(uid, alias).await?;
            println!("Alias '{alias}' set for account '{uid}'");
        }
        AliasCommand::Get { alias } => {
            let acct = db.get_account_by_alias(alias).await?;
            match acct {
                Some(a) => {
                    if pr.json {
                        pr.print_json(&a)?;
                    } else {
                        println!(
                            "Alias: {alias} -> UID: {} | IBAN: {} | Name: {}",
                            a.uid, a.iban, a.name
                        );
                    }
                }
                None => {
                    anyhow::bail!("alias '{alias}' not found");
                }
            }
        }
        AliasCommand::Remove { alias } => {
            db.clear_account_alias(alias).await?;
            println!("Alias '{alias}' removed");
        }
        AliasCommand::List => {
            let all = db.get_all_accounts().await?;
            let with_alias: Vec<_> = all.iter().filter(|a| !a.alias.is_empty()).collect();
            if pr.json {
                pr.print_json(&with_alias)?;
            } else if with_alias.is_empty() {
                println!("No aliases configured");
            } else {
                let headers = vec!["ALIAS".into(), "UID".into(), "IBAN".into()];
                let rows: Vec<Vec<String>> = with_alias
                    .iter()
                    .map(|a| vec![a.alias.clone(), a.uid.clone(), a.iban.clone()])
                    .collect();
                pr.print_table(headers, rows)?;
            }
        }
    }

    Ok(())
}

pub(crate) async fn cmd_balances(
    args: &BalancesArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    let sessions = load_sessions(cfg)?;
    let (bank_name, sess) = resolve_bank(&sessions, args.bank.as_deref())?;
    let db = open_store(cfg)?;

    let account = resolve_account(sess, Some(&db), args.account.as_deref()).await?;

    let (bals, fetched_at) = db.get_balances(&account.uid).await.unwrap_or_default();

    if pr.json {
        let mut meta = HashMap::new();
        meta.insert(
            "account".to_string(),
            serde_json::Value::String(account.uid.clone()),
        );
        meta.insert(
            "account_name".to_string(),
            serde_json::Value::String(account.name.clone()),
        );
        meta.insert(
            "bank".to_string(),
            serde_json::Value::String(bank_name.to_string()),
        );
        if let Some(m) = last_synced_meta(Some(fetched_at)) {
            meta.extend(m);
        }
        let data = serde_json::to_value(&bals).context("marshal balances")?;
        let mut envelope = serde_json::Map::new();
        envelope.insert("data".into(), data);
        for (k, v) in meta {
            envelope.insert(k, v);
        }
        pr.print_json(&envelope)?;
    } else {
        let headers = vec![
            "TYPE".into(),
            "AMOUNT".into(),
            "CURRENCY".into(),
            "REFERENCE_DATE".into(),
        ];
        let rows: Vec<Vec<String>> = bals
            .iter()
            .map(|b| {
                vec![
                    b.balance_type.clone(),
                    format_amount(&b.amount),
                    b.currency.clone(),
                    b.reference_date.clone(),
                ]
            })
            .collect();
        let footer = last_synced_footer(Some(fetched_at));
        pr.print_table_with_footer(headers, rows, &footer)?;
    }

    Ok(())
}

pub(crate) async fn cmd_transactions(
    args: &TransactionsArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    let sessions = load_sessions(cfg)?;
    let (bank_name, sess) = resolve_bank(&sessions, args.bank.as_deref())?;
    let db = open_store(cfg)?;

    let account_uids: Vec<String> = match args.account.as_deref() {
        Some(flag) => {
            let account = resolve_account(sess, Some(&db), Some(flag)).await?;
            vec![account.uid.clone()]
        }
        None => sess.accounts.iter().map(|a| a.uid.clone()).collect(),
    };

    let mut all_txns: Vec<store::TransactionRecord> = Vec::new();
    let mut oldest_synced: Option<DateTime<Utc>> = None;

    for uid in &account_uids {
        let synced = db
            .get_last_synced(uid, "transactions")
            .await
            .unwrap_or_default();
        if synced != DateTime::<Utc>::default() {
            oldest_synced = Some(match oldest_synced {
                None => synced,
                Some(existing) => existing.min(synced),
            });
        }

        let opts = store::QueryOpts {
            date_from: args.from.clone(),
            date_to: args.to.clone(),
            limit: None,
            status: None,
        };

        let txns = db.get_transactions(uid, &opts).await?;
        all_txns.extend(txns);
    }

    prepare_transaction_output(
        &mut all_txns,
        args.direction.as_deref(),
        args.category.as_deref(),
        args.limit,
    );

    if pr.json {
        let mut meta = HashMap::new();
        meta.insert(
            "bank".to_string(),
            serde_json::Value::String(bank_name.to_string()),
        );
        meta.insert(
            "count".to_string(),
            serde_json::Value::Number(all_txns.len().into()),
        );
        if let Some(m) = last_synced_meta(oldest_synced) {
            meta.extend(m);
        }
        pr.print_json_with_meta(&all_txns, meta)?;
    } else {
        let headers = vec![
            "DATE".into(),
            "AMOUNT".into(),
            "DIR".into(),
            "DESCRIPTION".into(),
        ];
        let mut total_credits = Decimal::ZERO;
        let mut total_debits = Decimal::ZERO;
        let mut rows = Vec::new();

        for t in &all_txns {
            let desc = if !t.creditor_name.is_empty() {
                t.creditor_name.clone()
            } else if !t.debtor_name.is_empty() {
                t.debtor_name.clone()
            } else if let Some(first) = t.remittance_info.first() {
                first.clone()
            } else {
                String::new()
            };

            let amt = format_amount(&t.amount);
            let dir = match t.credit_debit_indicator.as_str() {
                "CRDT" => {
                    if let Ok(v) = Decimal::from_str_exact(&t.amount) {
                        total_credits += v;
                    }
                    "CR"
                }
                "DBIT" => {
                    if let Ok(v) = Decimal::from_str_exact(&t.amount) {
                        total_debits += v;
                    }
                    "DB"
                }
                other => other,
            };

            rows.push(vec![t.best_date().to_string(), amt, dir.to_string(), desc]);
        }

        let footer = format!(
            "Credits: {:.2} | Debits: {:.2} | Net: {:.2}",
            total_credits.round_dp(2),
            total_debits.round_dp(2),
            (total_credits - total_debits).round_dp(2)
        );

        let synced_footer = last_synced_footer(oldest_synced);
        let full_footer = format!("{footer}\n{synced_footer}");

        pr.print_table_with_footer(headers, rows, &full_footer)?;
    }

    Ok(())
}
