use super::*;

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PreviewChange {
    transaction_id: String,
    booking_date: String,
    description: String,
    current_category: String,
    new_category: String,
}

pub(crate) async fn apply_tag_rules(
    db: &store::SqliteStore,
    rules: &[tagger::CategoryRule],
) -> Result<(usize, usize, HashMap<String, usize>)> {
    let txns = db.get_all_transactions().await?;
    let mut updates = Vec::new();
    let mut tagged = 0usize;
    let mut skipped_manual = 0usize;
    let mut by_category: HashMap<String, usize> = HashMap::new();

    for tx in &txns {
        if tx.category_source == "manual" {
            skipped_manual += 1;
            continue;
        }
        let normalized = tagger::normalize(
            &tx.remittance_info,
            &tx.creditor_name,
            &tx.debtor_name,
            &tx.note,
        );
        let category = tagger::match_category(&normalized, rules);

        updates.push(store::CategoryUpdate {
            account_uid: tx.account_uid.clone(),
            transaction_id: tx.transaction_id.clone(),
            category: category.clone(),
            source: "auto".to_string(),
        });
        tagged += 1;
        *by_category.entry(category).or_insert(0) += 1;
    }

    if !updates.is_empty() {
        db.update_categories(&updates).await?;
    }

    Ok((tagged, skipped_manual, by_category))
}

pub(crate) async fn preview_tag_rules(
    db: &store::SqliteStore,
    rules: &[tagger::CategoryRule],
) -> Result<Vec<PreviewChange>> {
    let txns = db.get_all_transactions().await?;
    let mut changes = Vec::new();
    for tx in &txns {
        if tx.category_source == "manual" {
            continue;
        }
        let normalized = tagger::normalize(
            &tx.remittance_info,
            &tx.creditor_name,
            &tx.debtor_name,
            &tx.note,
        );
        let category = tagger::match_category(&normalized, rules);
        if category != tx.category {
            let desc = if normalized.len() > 40 {
                format!("{}...", &normalized[..37])
            } else {
                normalized.clone()
            };
            changes.push(PreviewChange {
                transaction_id: tx.transaction_id.clone(),
                booking_date: tx.booking_date.clone(),
                description: desc,
                current_category: tx.category.clone(),
                new_category: category,
            });
        }
    }
    Ok(changes)
}

pub(crate) async fn override_category(
    db: &store::SqliteStore,
    transaction_id: &str,
    category: &str,
) -> Result<()> {
    let txns = db.get_all_transactions().await?;
    for tx in &txns {
        if tx.transaction_id == transaction_id {
            return db
                .update_category(&tx.account_uid, transaction_id, category, "manual")
                .await;
        }
    }
    anyhow::bail!("transaction {transaction_id:?} not found")
}

pub(crate) async fn cmd_tag(args: &TagArgs, cfg: &config::Config) -> Result<()> {
    let db = open_store(cfg)?;
    let rules = &cfg.tag_rules.0;

    match &args.action {
        TagAction::Apply => {
            if rules.is_empty() {
                anyhow::bail!("no tag rules configured; add them to config.yaml under tag_rules:");
            }
            println!(
                "Applying {} tag rules to all non-manual transactions...",
                rules.len()
            );
            let (tagged, skipped_manual, by_category) = apply_tag_rules(&db, rules).await?;

            println!("Done. {tagged} transactions tagged, {skipped_manual} skipped (manual).");
            if !by_category.is_empty() {
                let mut sorted: Vec<_> = by_category.iter().collect();
                sorted.sort_by(|a, b| b.1.cmp(a.1));
                println!("\nCategory breakdown:");
                for (cat, count) in sorted {
                    println!("  {cat}: {count}");
                }
            }
        }
        TagAction::Preview => {
            if rules.is_empty() {
                anyhow::bail!("no tag rules configured; add them to config.yaml under tag_rules:");
            }
            println!(
                "Previewing {} tag rules (no changes will be made)...",
                rules.len()
            );
            let changes = preview_tag_rules(&db, rules).await?;
            if changes.is_empty() {
                println!("No category changes would be made.");
            } else {
                let headers = vec![
                    "DATE".into(),
                    "ID".into(),
                    "FROM".into(),
                    "TO".into(),
                    "DESC".into(),
                ];
                let rows: Vec<Vec<String>> = changes
                    .iter()
                    .map(|c| {
                        vec![
                            c.booking_date.clone(),
                            c.transaction_id.clone(),
                            c.current_category.clone(),
                            c.new_category.clone(),
                            c.description.clone(),
                        ]
                    })
                    .collect();
                let pr = output::Printer {
                    json: false,
                    csv: false,
                };
                pr.print_table_with_footer(
                    headers,
                    rows,
                    &format!("{} transactions would be updated", changes.len()),
                )?;
            }
        }
        TagAction::Override { id, category } => {
            println!("Applying manual override: transaction {id} → {category}...");
            override_category(&db, id, category).await?;
            println!("Category set to '{category}' for transaction {id}");
        }
        TagAction::Interactive => {
            println!("interactive tagging: not yet implemented, requires TUI");
        }
    }

    Ok(())
}
