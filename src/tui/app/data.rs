use super::*;

impl App {
    pub(super) async fn new(cfg: Config) -> Result<Self> {
        let db_path = cfg.data_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let db = SqliteStore::open(
            db_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("invalid db path"))?,
        )?;
        let db = std::sync::Arc::new(db);
        let sessions = session::load(&cfg.session_path())
            .ok()
            .flatten()
            .unwrap_or_default();
        let (accounts, balances, forecasts, all_txns, alert_results, alert_rules) =
            Self::load_all_data(&cfg, &db, &sessions).await;
        let mut account_list_state = ListState::default();
        if !accounts.is_empty() {
            account_list_state.select(Some(0));
        }
        let tag = TagState {
            categories: cfg.tag_rules.0.iter().map(|r| r.category.clone()).collect(),
            ..Default::default()
        };
        Ok(App {
            cfg,
            db,
            sessions,
            accounts,
            account_list_state,
            selected_account: 0,
            balances,
            forecasts,
            all_transactions: all_txns,
            alert_results,
            all_alert_rules: alert_rules,
            detail_tab: DetailTab::General,
            txn_cursor: 0,
            txn_scroll: 0,
            pending_scroll: 0,
            txn_filter: TxnFilter::default(),
            tag,
            report_period: ReportPeriod::Month,
            report_compare: false,
            search_open: false,
            search_input: String::new(),
            note_open: false,
            note_input: String::new(),
            note_cursor: 0,
            note_txn_id: String::new(),
            note_account_uid: String::new(),
            refresh: RefreshState {
                open: false,
                done: false,
                steps: Vec::new(),
            },
            help_open: false,
            status: String::new(),
        })
    }

    pub(super) async fn load_all_data(
        cfg: &Config,
        db: &SqliteStore,
        sessions: &session::Store,
    ) -> (
        Vec<store::AccountRecord>,
        HashMap<String, Vec<store::BalanceRecord>>,
        HashMap<String, AccountForecast>,
        HashMap<String, Vec<store::TransactionRecord>>,
        HashMap<String, Vec<crate::alerter::AlertResult>>,
        Vec<crate::alerter::AlertRule>,
    ) {
        let mut all_accounts = Vec::new();
        let mut balances_map = HashMap::new();
        let mut forecasts_map = HashMap::new();
        let mut txns_map = HashMap::new();
        let mut alerts_map = HashMap::new();
        let alert_rules = cfg.alert_rules.clone();
        for bank_name in sessions.keys() {
            if let Ok(records) = db.get_accounts(bank_name).await {
                for rec in &records {
                    if let Ok((bals, _)) = db.get_balances(&rec.uid).await
                        && !bals.is_empty()
                    {
                        balances_map.insert(rec.uid.clone(), bals.clone());
                    }
                    let bal_inputs: Vec<forecast::BalanceInput> = balances_map
                        .get(&rec.uid)
                        .map(|b| {
                            b.iter()
                                .map(|r| forecast::BalanceInput {
                                    balance_type: r.balance_type.clone(),
                                    amount: r.amount.clone(),
                                    currency: r.currency.clone(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let pending_txns: Vec<forecast::TxnInput> = match db
                        .get_transactions(
                            &rec.uid,
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
                    fc.account_uid = rec.uid.clone();
                    fc.iban = rec.iban.clone();
                    fc.bank_name = rec.bank_name.clone();
                    if fc.currency.is_empty() {
                        fc.currency = rec.currency.clone();
                    }
                    forecasts_map.insert(rec.uid.clone(), fc);
                    if let Ok(txns) = db
                        .get_transactions(&rec.uid, &store::QueryOpts::default())
                        .await
                    {
                        txns_map.insert(rec.uid.clone(), txns);
                    } else {
                        txns_map.insert(rec.uid.clone(), Vec::new());
                    }
                }
                all_accounts.extend(records);
            }
        }
        for acct in &all_accounts {
            let txns = txns_map.get(&acct.uid).cloned().unwrap_or_default();
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
            alerts_map.insert(acct.uid.clone(), output.results);
        }
        (
            all_accounts,
            balances_map,
            forecasts_map,
            txns_map,
            alerts_map,
            alert_rules,
        )
    }

    pub(super) fn current_account(&self) -> Option<&store::AccountRecord> {
        self.accounts.get(self.selected_account)
    }
    pub(super) fn current_uid(&self) -> Option<&str> {
        self.current_account().map(|a| a.uid.as_str())
    }

    // ---------- Event loop ----------
}
