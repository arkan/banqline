use super::*;

pub(crate) async fn cmd_forecast(
    args: &ForecastArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    if args.all {
        return forecast_all_banks(args, cfg, pr).await;
    }

    if args.bank.is_some() || !args.all {
        return forecast_single_bank(args, cfg, pr).await;
    }

    let sessions = load_sessions(cfg)?;
    if sessions.len() == 1 {
        return forecast_single_bank(args, cfg, pr).await;
    }
    forecast_all_banks(args, cfg, pr).await
}

pub(crate) async fn forecast_single_bank(
    args: &ForecastArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    let sessions = load_sessions(cfg)?;
    let (bank_name, sess) = resolve_bank(&sessions, args.bank.as_deref())?;
    let db = open_store(cfg)?;

    let account_uids: Vec<String> = match args.account.as_deref() {
        Some(flag) => {
            let resolved = resolve_alias(Some(&db), flag).await;
            vec![resolved]
        }
        None => sess.accounts.iter().map(|a| a.uid.clone()).collect(),
    };

    let mut forecasts: Vec<aggregator::forecast::AccountForecast> = Vec::new();
    let mut oldest_synced: Option<DateTime<Utc>> = None;

    for uid in &account_uids {
        let account = sess
            .accounts
            .iter()
            .find(|a| a.uid == *uid)
            .ok_or_else(|| anyhow::anyhow!("account {uid} not found in session"))?;

        let fc = compute_account_forecast(&db, uid, &account.iban, bank_name).await?;

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

        forecasts.push(fc);
    }

    if pr.json {
        print_forecast_json(pr, &forecasts, oldest_synced, bank_name)?;
    } else if args.detail {
        print_forecast_detail(&forecasts, oldest_synced);
    } else {
        print_forecast_table(pr, &forecasts, oldest_synced)?;
    }

    Ok(())
}

pub(crate) async fn forecast_all_banks(
    args: &ForecastArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    let sessions = load_sessions(cfg)?;
    let db = open_store(cfg)?;

    let mut all_forecasts: Vec<aggregator::forecast::AccountForecast> = Vec::new();
    let mut oldest_synced: Option<DateTime<Utc>> = None;

    for (bank_name, sess) in &sessions {
        if !sess.is_valid() {
            eprintln!("Warning: session for '{bank_name}' has expired");
            continue;
        }

        for acct in &sess.accounts {
            let fc = compute_account_forecast(&db, &acct.uid, &acct.iban, bank_name).await?;

            let synced = db
                .get_last_synced(&acct.uid, "transactions")
                .await
                .unwrap_or_default();
            if synced != DateTime::<Utc>::default() {
                oldest_synced = Some(match oldest_synced {
                    None => synced,
                    Some(existing) => existing.min(synced),
                });
            }

            all_forecasts.push(fc);
        }
    }

    if pr.json {
        print_forecast_json(pr, &all_forecasts, oldest_synced, "all")?;
    } else if args.detail {
        print_forecast_detail(&all_forecasts, oldest_synced);
    } else {
        print_forecast_table(pr, &all_forecasts, oldest_synced)?;
    }

    Ok(())
}

pub(crate) async fn compute_account_forecast(
    db: &store::SqliteStore,
    account_uid: &str,
    iban: &str,
    bank_name: &str,
) -> Result<aggregator::forecast::AccountForecast> {
    let (bals, _) = db.get_balances(account_uid).await.unwrap_or_default();

    let balance_inputs: Vec<aggregator::forecast::BalanceInput> = bals
        .iter()
        .map(|b| aggregator::forecast::BalanceInput {
            balance_type: b.balance_type.clone(),
            amount: b.amount.clone(),
            currency: b.currency.clone(),
        })
        .collect();

    let pending_from_db = db
        .get_transactions(
            account_uid,
            &store::QueryOpts {
                status: Some("PDNG".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_or_default();

    let txn_inputs: Vec<aggregator::forecast::TxnInput> = pending_from_db
        .iter()
        .map(|t| aggregator::forecast::TxnInput {
            transaction_id: t.transaction_id.clone(),
            amount: t.amount.clone(),
            currency: t.currency.clone(),
            credit_debit_indicator: t.credit_debit_indicator.clone(),
            description: if !t.creditor_name.is_empty() {
                t.creditor_name.clone()
            } else if !t.debtor_name.is_empty() {
                t.debtor_name.clone()
            } else {
                t.remittance_info.first().cloned().unwrap_or_default()
            },
            value_date: t.value_date.clone(),
        })
        .collect();

    let mut fc = aggregator::forecast::forecast(&balance_inputs, &txn_inputs);
    fc.account_uid = account_uid.to_string();
    fc.iban = iban.to_string();
    fc.bank_name = bank_name.to_string();
    Ok(fc)
}

pub(crate) fn print_forecast_table(
    pr: &output::Printer,
    forecasts: &[aggregator::forecast::AccountForecast],
    oldest_synced: Option<DateTime<Utc>>,
) -> Result<()> {
    let headers = vec![
        "IBAN".into(),
        "BANK".into(),
        "CURRENCY".into(),
        "BOOKED".into(),
        "PENDING".into(),
        "PROJECTED".into(),
    ];

    let mut rows = Vec::new();
    for fc in forecasts {
        rows.push(vec![
            output::iban_suffix(&fc.iban, 8),
            fc.bank_name.clone(),
            fc.currency.clone(),
            format!("{:.2}", fc.booked_balance.round_dp(2)),
            format_pending_decimal(fc.pending_delta),
            format!("{:.2}", fc.projected_balance.round_dp(2)),
        ]);
    }

    let totals = aggregator::forecast::aggregate_by_currency(forecasts);
    let mut footer_parts: Vec<String> = Vec::new();
    for ct in &totals {
        footer_parts.push(format!(
            "{} Total: booked {:.2} | pending {} | projected {:.2}",
            ct.currency,
            ct.total_booked.round_dp(2),
            format_pending_decimal(ct.total_pending),
            ct.total_projected.round_dp(2)
        ));
    }
    let totals_footer = footer_parts.join("\n");

    let warnings: Vec<&str> = forecasts
        .iter()
        .flat_map(|f| f.warnings.iter().map(|s| s.as_str()))
        .collect();

    let mut full_footer = totals_footer;
    if !warnings.is_empty() {
        full_footer.push_str("\n\nWarnings:\n");
        for w in &warnings {
            full_footer.push_str(&format!("  - {w}\n"));
        }
    }
    let synced = last_synced_footer(oldest_synced);
    full_footer.push_str(&format!("\n{synced}"));

    let disclaimer =
        "Projected balance = booked + pending. May not reflect real-time authorizations.";
    full_footer.push_str(&format!("\n{disclaimer}"));

    pr.print_table_with_footer(headers, rows, &full_footer)
}

pub(crate) fn print_forecast_detail(
    forecasts: &[aggregator::forecast::AccountForecast],
    oldest_synced: Option<DateTime<Utc>>,
) {
    for fc in forecasts {
        println!(
            "═══ {} ({}) ═══",
            output::iban_suffix(&fc.iban, 8),
            fc.bank_name
        );
        println!(
            "  Booked balance:   {:>12.2} {}",
            fc.booked_balance.round_dp(2),
            fc.currency
        );
        if fc.has_pending_data {
            println!(
                "  Pending delta:    {:>12} {}",
                format_pending_decimal(fc.pending_delta),
                fc.currency
            );
            println!(
                "  Projected:        {:>12.2} {}",
                fc.projected_balance.round_dp(2),
                fc.currency
            );
        } else {
            println!("  Pending delta:    N/A (no pending data)");
        }
        if !fc.warnings.is_empty() {
            for w in &fc.warnings {
                println!("  ⚠ {w}");
            }
        }
        if fc.has_pending_data && !fc.pending_txns.is_empty() {
            println!("  Pending transactions:");
            for pt in &fc.pending_txns {
                let dir = if pt.is_credit { "+" } else { "-" };
                println!(
                    "    {dir}{:.2} {} — {}",
                    pt.amount.round_dp(2),
                    fc.currency,
                    pt.description
                );
            }
        }
        println!();
    }

    println!("{}", last_synced_footer(oldest_synced));
}

pub(crate) fn print_forecast_json(
    pr: &output::Printer,
    forecasts: &[aggregator::forecast::AccountForecast],
    oldest_synced: Option<DateTime<Utc>>,
    scope: &str,
) -> Result<()> {
    let totals = aggregator::forecast::aggregate_by_currency(forecasts);
    let warnings: Vec<String> = forecasts.iter().flat_map(|f| f.warnings.clone()).collect();

    let data = serde_json::to_value(forecasts).context("marshal forecasts")?;

    let totals_json: Vec<serde_json::Value> = totals
        .iter()
        .map(|ct| {
            json!({
                "currency": ct.currency,
                "total_booked": ct.total_booked.round_dp(2),
                "total_pending": ct.total_pending.round_dp(2),
                "total_projected": ct.total_projected.round_dp(2),
            })
        })
        .collect();

    let mut envelope = serde_json::Map::new();
    envelope.insert("data".into(), data);
    envelope.insert("scope".into(), serde_json::Value::String(scope.to_string()));
    envelope.insert(
        "totals_by_currency".into(),
        serde_json::Value::Array(totals_json),
    );
    if !warnings.is_empty() {
        envelope.insert(
            "warnings".into(),
            serde_json::Value::Array(
                warnings
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if let Some(dt) = oldest_synced
        && dt != DateTime::<Utc>::default()
    {
        envelope.insert(
            "last_synced".into(),
            serde_json::Value::String(dt.to_rfc3339()),
        );
    }

    pr.print_json(&envelope)
}

pub(crate) fn format_pending_decimal(d: Decimal) -> String {
    if d == Decimal::ZERO {
        "    0.00".to_string()
    } else if d > Decimal::ZERO {
        format!("+{:.2}", d.round_dp(2))
    } else {
        format!("{:.2}", d.round_dp(2))
    }
}
