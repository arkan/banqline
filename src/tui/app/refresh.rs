use super::*;

impl App {
    pub(super) fn start_refresh(&mut self) {
        self.refresh.open = true;
        self.refresh.done = false;
        self.refresh.steps = vec![
            RefreshStep {
                label: "Accounts".into(),
                status: RefreshStatus::Pending,
            },
            RefreshStep {
                label: "API Sync".into(),
                status: RefreshStatus::Pending,
            },
            RefreshStep {
                label: "Balances".into(),
                status: RefreshStatus::Pending,
            },
            RefreshStep {
                label: "Transactions".into(),
                status: RefreshStatus::Pending,
            },
            RefreshStep {
                label: "Tagging".into(),
                status: RefreshStatus::Pending,
            },
            RefreshStep {
                label: "Alerts".into(),
                status: RefreshStatus::Pending,
            },
        ];
    }

    pub(super) fn stop_refresh_with_error(&mut self, step_index: usize, message: String) {
        if let Some(step) = self.refresh.steps.get_mut(step_index) {
            step.status = RefreshStatus::Error(message.clone());
        }
        self.status = message;
        self.refresh.done = true;
    }

    pub(super) fn create_api_client(&self) -> Result<client::Client> {
        let key_path = self.cfg.key_abs_path().context("resolve key path")?;
        let key =
            auth::key::load_private_key(&key_path.to_string_lossy()).context("load private key")?;
        let app_id = self.cfg.application_id.clone();
        let jwt_fn: std::sync::Arc<dyn Fn() -> anyhow::Result<String> + Send + Sync> =
            std::sync::Arc::new(move || {
                auth::jwt::generate_jwt(&key, &app_id).map_err(|e| anyhow::anyhow!("{}", e))
            });
        Ok(client::Client::new(None, Some(jwt_fn)))
    }

    pub(super) async fn do_refresh(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        self.refresh.steps[0].status = RefreshStatus::Loading;
        terminal.draw(|f| self.render(f))?;
        let mut accounts = Vec::new();
        let mut count = 0;
        for bank_name in self.sessions.keys() {
            if let Ok(recs) = self.db.get_accounts(bank_name).await {
                count += recs.len();
                accounts.extend(recs);
            }
        }
        self.refresh.steps[0].status = RefreshStatus::Done;
        self.refresh.steps[0].label = format!("Accounts: {} loaded", count);
        terminal.draw(|f| self.render(f))?;

        // Step 2: API Sync — fetch fresh data from Enable Banking.
        self.refresh.steps[1].status = RefreshStatus::Loading;
        terminal.draw(|f| self.render(f))?;
        let mut api_fetched = 0;
        if !self.cfg.application_id.is_empty() {
            match self.create_api_client() {
                Ok(api_client) => {
                    for acct in &accounts {
                        // Fetch balances.
                        if let Ok(bals) = api_client.get_balances(&acct.uid).await {
                            let records: Vec<store::BalanceRecord> = bals
                                .into_iter()
                                .map(|b| store::BalanceRecord {
                                    balance_type: b.balance_type,
                                    amount: b.balance_amount.amount,
                                    currency: b.balance_amount.currency,
                                    reference_date: b.reference_date,
                                })
                                .collect();
                            let now = chrono::Utc::now();
                            if self
                                .db
                                .replace_balances(&acct.uid, &records, now)
                                .await
                                .is_ok()
                            {
                                api_fetched += 1;
                            }
                        }
                        // Fetch transactions (booked + pending) with full pagination.
                        let opts = client::TransactionOpts {
                            date_from: None,
                            date_to: None,
                            status: None,
                            continuation_key: None,
                        };
                        let result = api_client.get_all_transactions(&acct.uid, &opts).await;
                        let api_txns = match result {
                            Ok(txns) => txns,
                            Err(e) => {
                                let msg = format!("fetch transactions for {}: {:#}", acct.uid, e);
                                self.stop_refresh_with_error(1, msg);
                                terminal.draw(|f| self.render(f))?;
                                return Ok(());
                            }
                        };
                        let records: Vec<store::TransactionRecord> = api_txns
                            .into_iter()
                            .map(|t| store::TransactionRecord {
                                account_uid: acct.uid.clone(),
                                transaction_id: t.transaction_id,
                                entry_reference: t.entry_reference,
                                amount: t.amount.amount,
                                currency: t.amount.currency,
                                booking_date: t.booking_date,
                                value_date: t.value_date,
                                transaction_date: t.transaction_date,
                                remittance_info: t.remittance_info,
                                creditor_name: t.creditor.name,
                                debtor_name: t.debtor.name,
                                status: t.status,
                                credit_debit_indicator: t.credit_debit_indicator,
                                note: t.note,
                                category: String::new(),
                                category_source: String::new(),
                            })
                            .collect();
                        if !records.is_empty()
                            && let Err(e) = self
                                .db
                                .upsert_transactions(&acct.uid, &records)
                                .await
                                .with_context(|| format!("upsert transactions for {}", acct.uid))
                        {
                            self.stop_refresh_with_error(1, format!("{:#}", e));
                            terminal.draw(|f| self.render(f))?;
                            return Ok(());
                        }
                        if let Err(e) = self
                            .db
                            .set_last_synced(&acct.uid, "transactions", chrono::Utc::now())
                            .await
                            .with_context(|| format!("set transaction sync time for {}", acct.uid))
                        {
                            self.stop_refresh_with_error(1, format!("{:#}", e));
                            terminal.draw(|f| self.render(f))?;
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("API client: {:#}", e);
                    self.stop_refresh_with_error(1, msg);
                    terminal.draw(|f| self.render(f))?;
                    return Ok(());
                }
            }
        }
        self.refresh.steps[1].status = RefreshStatus::Done;
        self.refresh.steps[1].label = format!("API Sync: {} accounts fetched", api_fetched);
        terminal.draw(|f| self.render(f))?;

        self.refresh.steps[2].status = RefreshStatus::Loading;
        terminal.draw(|f| self.render(f))?;
        let mut balances = HashMap::new();
        let mut forecasts = HashMap::new();
        for acct in &accounts {
            if let Ok((bals, _)) = self.db.get_balances(&acct.uid).await {
                if !bals.is_empty() {
                    balances.insert(acct.uid.clone(), bals.clone());
                }
                let bal_inputs: Vec<forecast::BalanceInput> = bals
                    .iter()
                    .map(|b| forecast::BalanceInput {
                        balance_type: b.balance_type.clone(),
                        amount: b.amount.clone(),
                        currency: b.currency.clone(),
                    })
                    .collect();
                let pending_txns: Vec<forecast::TxnInput> = match self
                    .db
                    .get_transactions(
                        &acct.uid,
                        &store::QueryOpts {
                            status: Some("PDNG".into()),
                            ..Default::default()
                        },
                    )
                    .await
                {
                    Ok(txns) => txns
                        .into_iter()
                        .map(|t| {
                            let desc = if !t.remittance_info.is_empty() {
                                t.remittance_info.join(" ")
                            } else if !t.creditor_name.is_empty() {
                                t.creditor_name.clone()
                            } else {
                                t.debtor_name.clone()
                            };
                            forecast::TxnInput {
                                transaction_id: t.transaction_id,
                                amount: t.amount,
                                currency: t.currency,
                                credit_debit_indicator: t.credit_debit_indicator,
                                description: desc,
                                value_date: t.value_date,
                            }
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                };
                let mut fc = forecast::forecast(&bal_inputs, &pending_txns);
                fc.account_uid = acct.uid.clone();
                fc.iban = acct.iban.clone();
                fc.bank_name = acct.bank_name.clone();
                if fc.currency.is_empty() {
                    fc.currency = acct.currency.clone();
                }
                forecasts.insert(acct.uid.clone(), fc);
            }
        }
        self.refresh.steps[2].status = RefreshStatus::Done;
        self.refresh.steps[2].label = format!("Balances: {} accounts updated", forecasts.len());
        terminal.draw(|f| self.render(f))?;

        self.refresh.steps[3].status = RefreshStatus::Loading;
        terminal.draw(|f| self.render(f))?;
        let mut all_txns = HashMap::new();
        let mut txn_count = 0;
        let mut txn_details: Vec<String> = Vec::new();
        for acct in &accounts {
            let old_count = self
                .all_transactions
                .get(&acct.uid)
                .map(|t| t.len())
                .unwrap_or(0);
            if let Ok(txns) = self
                .db
                .get_transactions(&acct.uid, &store::QueryOpts::default())
                .await
            {
                let new = txns.len().saturating_sub(old_count);
                let name = if acct.alias.is_empty() {
                    &acct.name
                } else {
                    &acct.alias
                };
                if new > 0 {
                    txn_details.push(format!("  {}: +{} new", name, new));
                }
                txn_count += txns.len();
                all_txns.insert(acct.uid.clone(), txns);
            } else {
                all_txns.insert(acct.uid.clone(), Vec::new());
            }
        }
        self.refresh.steps[3].status = RefreshStatus::Done;
        self.refresh.steps[3].label = if txn_details.is_empty() {
            format!("Transactions: {} total (no new)", txn_count)
        } else {
            format!(
                "Transactions: {} total\n{}",
                txn_count,
                txn_details.join("\n")
            )
        };
        terminal.draw(|f| self.render(f))?;

        self.refresh.steps[4].status = RefreshStatus::Loading;
        terminal.draw(|f| self.render(f))?;
        let tag_rules = self.cfg.tag_rules.0.clone();
        let mut tagged = 0;
        if !tag_rules.is_empty() {
            for acct in &accounts {
                let txns = all_txns.get(&acct.uid).cloned().unwrap_or_default();
                let mut updates = Vec::new();
                for tx in &txns {
                    if tx.category_source == "manual" {
                        continue;
                    }
                    let norm = tagger::normalize(
                        &tx.remittance_info,
                        &tx.creditor_name,
                        &tx.debtor_name,
                        &tx.note,
                    );
                    let cat = tagger::match_category(&norm, &tag_rules);
                    if cat != tx.category {
                        updates.push(store::CategoryUpdate {
                            account_uid: acct.uid.clone(),
                            transaction_id: tx.transaction_id.clone(),
                            category: cat,
                            source: "auto".to_string(),
                        });
                    }
                }
                if !updates.is_empty() {
                    if let Err(e) = self.db.update_categories(&updates).await {
                        self.refresh.steps[4].status = RefreshStatus::Error(e.to_string());
                    } else {
                        tagged += updates.len();
                    }
                }
            }
            for acct in &accounts {
                if let Ok(txns) = self
                    .db
                    .get_transactions(&acct.uid, &store::QueryOpts::default())
                    .await
                {
                    all_txns.insert(acct.uid.clone(), txns);
                }
            }
        }
        self.refresh.steps[4].status = RefreshStatus::Done;
        self.refresh.steps[4].label = format!("Tagging: {} transactions auto-tagged", tagged);
        terminal.draw(|f| self.render(f))?;

        self.refresh.steps[5].status = RefreshStatus::Loading;
        terminal.draw(|f| self.render(f))?;
        let alert_rules = self.cfg.alert_rules.clone();
        let mut alert_results = HashMap::new();
        let mut triggered = 0;
        for acct in &accounts {
            let txns = all_txns.get(&acct.uid).cloned().unwrap_or_default();
            let alerter_txns: Vec<crate::alerter::TransactionRecord> = txns
                .iter()
                .map(|t| crate::alerter::TransactionRecord {
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
            let output = crate::alerter::check(&alert_rules, &alerter_txns, true);
            triggered += output
                .results
                .iter()
                .filter(|r| r.status == "TRIGGERED")
                .count();
            alert_results.insert(acct.uid.clone(), output.results);
        }
        self.refresh.steps[5].status = RefreshStatus::Done;
        self.refresh.steps[5].label = format!(
            "Alerts: {} triggered across {} rules",
            triggered,
            alert_rules.len()
        );
        terminal.draw(|f| self.render(f))?;

        self.accounts = accounts;
        self.balances = balances;
        self.forecasts = forecasts;
        self.all_transactions = all_txns;
        self.alert_results = alert_results;
        self.all_alert_rules = alert_rules;
        self.status = "Refreshed".to_string();
        self.refresh.done = true;
        terminal.draw(|f| self.render(f))?;
        Ok(())
    }

    // ---------- Tag ----------
}
