use super::*;

pub(crate) async fn cmd_summary(
    args: &SummaryArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    let period = aggregator::summary::parse_period(&args.period).context("invalid --period")?;

    let (from, to) = compute_date_range(
        period,
        args.from.as_deref(),
        args.to.as_deref(),
        args.compare,
    );

    let db = open_store(cfg)?;

    let inputs = if let Some(ref bank) = args.bank {
        let sessions = load_sessions(cfg)?;
        let (_bank_name, sess) = resolve_bank(&sessions, Some(bank.as_str()))?;
        fetch_summary_bank_txns(&db, sess, &from, &to).await?
    } else {
        fetch_summary_all_txns(&db, &from, &to).await?
    };

    let opts = aggregator::summary::SummaryOpts {
        period,
        compare: args.compare,
    };

    let result = aggregator::summary::summarize(&inputs, &opts);

    if pr.json {
        format_summary_json(pr, &result, period, args.compare, &from, &to)?;
    } else {
        format_summary_table(pr, &result, args.compare)?;
    }

    Ok(())
}

pub(crate) fn compute_date_range(
    period: aggregator::summary::Period,
    from: Option<&str>,
    to: Option<&str>,
    compare: bool,
) -> (String, String) {
    let now = Utc::now();
    let to_date = to
        .map(|s| s.to_string())
        .unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    let from_date = from.map(|s| s.to_string()).unwrap_or_else(|| match period {
        aggregator::summary::Period::Month => {
            let mut start = now.date_naive().with_day(1).unwrap();
            if compare {
                start = start - chrono::Months::new(1);
            }
            start.format("%Y-%m-%d").to_string()
        }
        aggregator::summary::Period::Week => {
            let weekday = now.weekday().num_days_from_monday();
            let mut start = now.date_naive() - chrono::Days::new(weekday as u64);
            if compare {
                start = start - chrono::Days::new(7);
            }
            start.format("%Y-%m-%d").to_string()
        }
        aggregator::summary::Period::Day => {
            let mut start = now.date_naive();
            if compare {
                start = start - chrono::Days::new(1);
            }
            start.format("%Y-%m-%d").to_string()
        }
    });
    (from_date, to_date)
}

pub(crate) async fn fetch_summary_bank_txns(
    db: &store::SqliteStore,
    sess: &session::StoredSession,
    from: &str,
    to: &str,
) -> Result<Vec<aggregator::summary::SummaryInput>> {
    let mut inputs = Vec::new();
    for acct in &sess.accounts {
        let opts = store::QueryOpts {
            date_from: Some(from.to_string()),
            date_to: Some(to.to_string()),
            status: Some("BOOK".to_string()),
            ..Default::default()
        };
        let txns = db.get_transactions(&acct.uid, &opts).await?;
        for t in &txns {
            inputs.push(aggregator::summary::SummaryInput {
                booking_date: t.booking_date.clone(),
                amount: t.amount.clone(),
                currency: t.currency.clone(),
                credit_debit_indicator: t.credit_debit_indicator.clone(),
                category: t.category.clone(),
            });
        }
    }
    Ok(inputs)
}

pub(crate) async fn fetch_summary_all_txns(
    db: &store::SqliteStore,
    from: &str,
    to: &str,
) -> Result<Vec<aggregator::summary::SummaryInput>> {
    let all = db.get_all_transactions().await?;
    let inputs: Vec<aggregator::summary::SummaryInput> = all
        .iter()
        .filter(|t| t.status == "BOOK")
        .filter(|t| t.booking_date.as_str() >= from && t.booking_date.as_str() <= to)
        .map(|t| aggregator::summary::SummaryInput {
            booking_date: t.booking_date.clone(),
            amount: t.amount.clone(),
            currency: t.currency.clone(),
            credit_debit_indicator: t.credit_debit_indicator.clone(),
            category: t.category.clone(),
        })
        .collect();
    Ok(inputs)
}

pub(crate) fn format_summary_table(
    pr: &output::Printer,
    result: &aggregator::summary::SummaryResult,
    compare: bool,
) -> Result<()> {
    let num_cols = if compare { 4 } else { 3 };
    let mut headers = vec![
        "Period".to_string(),
        "Category".to_string(),
        "Amount".to_string(),
    ];
    if compare {
        headers.push("Delta".to_string());
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut last_currency = String::new();

    for ps in &result.periods {
        if ps.currency != last_currency {
            if !last_currency.is_empty() {
                let mut sep = Vec::with_capacity(num_cols);
                for _ in 0..num_cols {
                    sep.push(String::new());
                }
                rows.push(sep);
            }
            let mut header = Vec::with_capacity(num_cols);
            header.push(format!("== {} ==", ps.currency));
            for _ in 1..num_cols {
                header.push(String::new());
            }
            rows.push(header);
            last_currency = ps.currency.clone();
        }

        for cat in &ps.categories {
            let mut row = Vec::with_capacity(num_cols);
            row.push(ps.key.clone());
            row.push(cat.name.clone());
            row.push(format!("{:.2}", cat.amount.round_dp(2)));
            if compare {
                row.push(format_delta(&cat.delta, &ps.currency));
            }
            rows.push(row);
        }

        let mut total_row = Vec::with_capacity(num_cols);
        total_row.push(ps.key.clone());
        total_row.push("TOTAL".into());
        total_row.push(format!("{:.2}", ps.total.round_dp(2)));
        if compare {
            total_row.push(format_delta(&ps.delta, &ps.currency));
        }
        rows.push(total_row);
    }

    let mut footer_parts: Vec<String> = Vec::new();
    for ps in &result.periods {
        footer_parts.push(format!(
            "{} {}: {}",
            ps.currency,
            ps.key,
            ps.coverage.display()
        ));
    }
    let mut footer = footer_parts.join(" | ");

    if !result.warnings.is_empty() {
        if !footer.is_empty() {
            footer.push_str("\n\n");
        }
        footer.push_str("Warnings:\n");
        for w in &result.warnings {
            footer.push_str(&format!("  - {w}\n"));
        }
    }

    pr.print_table_with_footer(headers, rows, &footer)
}

pub(crate) fn format_summary_json(
    pr: &output::Printer,
    result: &aggregator::summary::SummaryResult,
    period: aggregator::summary::Period,
    compare: bool,
    from: &str,
    to: &str,
) -> Result<()> {
    let data = json!({
        "period": period.to_string(),
        "compare": compare,
        "from": from,
        "to": to,
        "results": &result.periods,
    });

    let mut envelope = serde_json::Map::new();
    envelope.insert("data".into(), data);
    if !result.warnings.is_empty() {
        envelope.insert(
            "warnings".into(),
            serde_json::Value::Array(
                result
                    .warnings
                    .iter()
                    .map(|w| serde_json::Value::String(w.clone()))
                    .collect(),
            ),
        );
    }

    pr.print_json(&envelope)
}

pub(crate) fn format_delta(d: &Option<aggregator::summary::DeltaInfo>, currency: &str) -> String {
    match d {
        None => String::new(),
        Some(d) => {
            let sign = if d.absolute_diff.is_sign_negative() {
                ""
            } else {
                "+"
            };
            format!(
                "{}{}{} ({})",
                sign,
                currency,
                d.absolute_diff.round_dp(2),
                d.percentage
            )
        }
    }
}
