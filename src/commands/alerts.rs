use super::*;

pub(crate) fn alert_rule_row(r: &alerter::AlertRule) -> Vec<String> {
    let (criteria, extra) = match r.rule_type.as_str() {
        "transaction" => {
            if let Some(t) = &r.transaction {
                let mut parts = Vec::new();
                if !t.amount_gte.is_empty() {
                    parts.push(format!("amount >= {}", t.amount_gte));
                }
                if !t.merchant_contains.is_empty() {
                    parts.push(format!("merchant contains {}", t.merchant_contains));
                }
                (parts.join(", "), t.direction.clone())
            } else {
                (String::new(), String::new())
            }
        }
        "category" => {
            if let Some(c) = &r.category {
                (
                    format!("{} >= {}", c.category, c.threshold),
                    c.period.clone(),
                )
            } else {
                (String::new(), String::new())
            }
        }
        _ => (String::new(), String::new()),
    };
    vec![r.name.clone(), r.rule_type.clone(), criteria, extra]
}

pub(crate) async fn cmd_alerts_add(
    args: &AlertsAddArgs,
    cfg: &config::Config,
    config_path: &Path,
) -> Result<()> {
    use alerter::types::{AlertRule, CategoryAlertCriteria, TransactionAlertCriteria};

    if args.rule_type == "category"
        && let Some(ref cat) = args.category
    {
        let valid = cfg.tag_rules.0.iter().any(|r| r.category == *cat);
        if !valid {
            anyhow::bail!(
                "category '{cat}' not found in tag_rules; add it first or check spelling"
            );
        }
    }

    let mut rule = match args.rule_type.as_str() {
        "transaction" => AlertRule {
            name: String::new(),
            rule_type: "transaction".into(),
            transaction: Some(TransactionAlertCriteria {
                amount_gte: args.amount_gte.clone().unwrap_or_default(),
                merchant_contains: args.merchant_contains.clone().unwrap_or_default(),
                direction: args.direction.clone().unwrap_or_default(),
            }),
            category: None,
        },
        "category" => AlertRule {
            name: String::new(),
            rule_type: "category".into(),
            transaction: None,
            category: Some(CategoryAlertCriteria {
                category: args.category.clone().unwrap_or_default(),
                threshold: args.threshold.clone().unwrap_or_default(),
                period: args.period.clone().unwrap_or_default(),
                bank: args.bank.clone().unwrap_or_default(),
            }),
        },
        other => anyhow::bail!("unknown rule type: {other}; expected 'transaction' or 'category'"),
    };

    rule.name = match &args.name {
        Some(n) if !n.is_empty() => n.clone(),
        _ => rule.auto_name(),
    };

    if rule.name.is_empty() {
        anyhow::bail!("could not auto-generate alert name; provide --name");
    }

    let mut cfg = cfg.clone();
    cfg.alert_rules.push(rule.clone());
    cfg.save(config_path)?;

    println!("Alert rule '{}' (type={}) added", rule.name, rule.rule_type);
    Ok(())
}

pub(crate) async fn cmd_alerts_remove(
    name: &str,
    cfg: &config::Config,
    config_path: &Path,
) -> Result<()> {
    let mut cfg = cfg.clone();
    let len_before = cfg.alert_rules.len();
    cfg.alert_rules.retain(|r| r.name != name);

    if cfg.alert_rules.len() == len_before {
        anyhow::bail!("alert rule '{name}' not found");
    }

    cfg.save(config_path)?;
    println!("Alert rule '{name}' removed");
    Ok(())
}

pub(crate) async fn cmd_alerts_list(cfg: &config::Config, pr: &output::Printer) -> Result<()> {
    if cfg.alert_rules.is_empty() {
        println!("No alert rules configured.");
        return Ok(());
    }

    if pr.json {
        pr.print_json(&cfg.alert_rules)?;
    } else {
        let headers = vec![
            "NAME".into(),
            "TYPE".into(),
            "CRITERIA".into(),
            "PERIOD / DIRECTION".into(),
        ];
        let rows: Vec<Vec<String>> = cfg.alert_rules.iter().map(alert_rule_row).collect();
        pr.print_table(headers, rows)?;
    }
    Ok(())
}

pub(crate) async fn cmd_alerts_check(
    args: &AlertsCheckArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
    use alerter::TransactionRecord as AlertTxn;

    if cfg.alert_rules.is_empty() {
        anyhow::bail!("no alert rules configured; use 'alert add' first");
    }

    let db = open_store(cfg)?;

    let sessions = session::load(&cfg.session_path())?.unwrap_or_default();
    let session_valid = sessions.values().any(|s| s.is_valid());

    let store_txns = db.get_all_transactions().await?;

    let alert_txns: Vec<AlertTxn> = store_txns
        .iter()
        .map(|t| AlertTxn {
            account_uid: t.account_uid.clone(),
            transaction_id: t.transaction_id.clone(),
            amount: t.amount.clone(),
            currency: t.currency.clone(),
            booking_date: t.booking_date.clone(),
            value_date: t.value_date.clone(),
            transaction_date: t.transaction_date.clone(),
            remittance_info: t.remittance_info.clone(),
            creditor_name: t.creditor_name.clone(),
            debtor_name: t.debtor_name.clone(),
            status: t.status.clone(),
            credit_debit_indicator: t.credit_debit_indicator.clone(),
            note: t.note.clone(),
            category: t.category.clone(),
            category_source: t.category_source.clone(),
        })
        .collect();

    let output = alerter::check(&cfg.alert_rules, &alert_txns, session_valid);

    db.set_metadata("alert_last_checked", &Utc::now().to_rfc3339())
        .await?;

    let accounts = db.get_all_accounts().await?;
    let account_map: HashMap<String, String> = accounts
        .iter()
        .map(|a| {
            (
                a.uid.clone(),
                output::account_display_name(&a.alias, &a.iban),
            )
        })
        .collect();

    let use_json = args.json || pr.json;

    if use_json {
        let triggered_count = output
            .results
            .iter()
            .filter(|r| r.status == "TRIGGERED")
            .count();
        let envelope = json!({
            "exit_code": output.exit_code,
            "triggered_count": triggered_count,
            "total_count": output.results.len(),
            "results": output.results,
        });
        serde_json::to_writer_pretty(std::io::stdout(), &envelope).context("write json")?;
        println!();
    } else {
        let headers = vec![
            "NAME".into(),
            "TYPE".into(),
            "STATUS".into(),
            "DETAILS".into(),
        ];

        let mut rows: Vec<Vec<String>> = Vec::new();
        for r in &output.results {
            rows.push(vec![
                r.rule.name.clone(),
                r.rule.rule_type.clone(),
                r.status.clone(),
                r.details.clone(),
            ]);
        }
        pr.print_table(headers, rows)?;

        if output.exit_code == 2 {
            println!("\nSession expired — run bank connect to refresh.");
        } else {
            let triggered = output
                .results
                .iter()
                .filter(|r| r.status == "TRIGGERED")
                .count();
            let total = output.results.len();
            println!("\n{triggered}/{total} rules triggered");

            for r in &output.results {
                if r.status == "TRIGGERED" && !r.matched_transactions.is_empty() {
                    println!("\n-- {} --", r.rule.name);
                    let txn_headers = vec![
                        "ACCOUNT".into(),
                        "DATE".into(),
                        "AMOUNT".into(),
                        "CUR".into(),
                        "DESCRIPTION".into(),
                    ];
                    let txn_rows: Vec<Vec<String>> = r
                        .matched_transactions
                        .iter()
                        .map(|mt| {
                            let display = account_map
                                .get(&mt.account_uid)
                                .cloned()
                                .unwrap_or_else(|| output::iban_suffix(&mt.account_uid, 6));
                            vec![
                                display,
                                mt.date.clone(),
                                format_amount(&mt.amount),
                                mt.currency.clone(),
                                mt.description.clone(),
                            ]
                        })
                        .collect();
                    pr.print_table(txn_headers, txn_rows)?;
                }
            }
        }
    }

    std::process::exit(output.exit_code);
}

pub(crate) async fn cmd_alerts(
    args: &AlertsArgs,
    cfg: &config::Config,
    config_path: Option<&Path>,
    pr: &output::Printer,
) -> Result<()> {
    let save_path = config_save_path(cfg, config_path);
    match &args.action {
        AlertsAction::Add(add_args) => cmd_alerts_add(add_args, cfg, &save_path).await,
        AlertsAction::Remove { name } => cmd_alerts_remove(name, cfg, &save_path).await,
        AlertsAction::List => cmd_alerts_list(cfg, pr).await,
        AlertsAction::Check(check_args) => cmd_alerts_check(check_args, cfg, pr).await,
    }
}
