use anyhow::{Context, Result};
use chrono::NaiveDate;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Sparkline, Tabs, Wrap},
};
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::aggregator::{
    forecast::{self, AccountForecast},
    summary::{self, Period, SummaryInput, SummaryOpts},
};
use crate::auth;
use crate::client;
use crate::config::Config;
use crate::session;
use crate::store::{self, SqliteStore, Store};
use crate::tagger;

fn balance_type_name(btype: &str) -> &str {
    match btype {
        "ITBD" => "Interim Booked",
        "CLBD" => "Closing Booked",
        "OPNT" => "Opening Booked",
        "XPCD" => "Expected",
        "VALU" => "Value Dated",
        other => other,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    General,
    Pending,
    Transactions,
    AlertsByAccount,
    Report,
}
impl DetailTab {
    fn all() -> &'static [DetailTab] {
        &[
            DetailTab::General,
            DetailTab::Pending,
            DetailTab::Transactions,
            DetailTab::AlertsByAccount,
            DetailTab::Report,
        ]
    }
    fn label(&self, pending_count: usize, alert_count: usize) -> String {
        match self {
            DetailTab::General => "General".into(),
            DetailTab::Pending => format!("Pending({})", pending_count),
            DetailTab::Transactions => "Transactions".into(),
            DetailTab::AlertsByAccount => format!("Alerts({})", alert_count),
            DetailTab::Report => "Report".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReportPeriod {
    Day,
    Week,
    Month,
}
impl ReportPeriod {
    fn to_aggregator(self) -> Period {
        match self {
            ReportPeriod::Day => Period::Day,
            ReportPeriod::Week => Period::Week,
            ReportPeriod::Month => Period::Month,
        }
    }
}

#[derive(Clone, Default)]
struct TxnFilter {
    category: Option<String>,
    direction: Option<String>,
    active: bool,
    search: String,
}

struct TagState {
    open: bool,
    transaction_id: String,
    description: String,
    amount: String,
    currency: String,
    selected_category: usize,
    categories: Vec<String>,
    new_category_input: String,
    pattern: String,
    apply_similar: bool,
}
impl Default for TagState {
    fn default() -> Self {
        TagState {
            open: false,
            transaction_id: String::new(),
            description: String::new(),
            amount: String::new(),
            currency: String::new(),
            selected_category: 0,
            categories: vec!["uncategorized".into()],
            new_category_input: String::new(),
            pattern: String::new(),
            apply_similar: false,
        }
    }
}

#[derive(Clone)]
struct RefreshStep {
    label: String,
    status: RefreshStatus,
}
#[derive(Clone, PartialEq)]
enum RefreshStatus {
    Pending,
    Loading,
    Done,
    Error(String),
}
struct RefreshState {
    open: bool,
    done: bool,
    steps: Vec<RefreshStep>,
}

pub struct App {
    cfg: Config,
    db: std::sync::Arc<SqliteStore>,
    #[allow(dead_code)]
    sessions: session::Store,
    accounts: Vec<store::AccountRecord>,
    account_list_state: ListState,
    selected_account: usize,
    balances: HashMap<String, Vec<store::BalanceRecord>>,
    forecasts: HashMap<String, AccountForecast>,
    all_transactions: HashMap<String, Vec<store::TransactionRecord>>,
    alert_results: HashMap<String, Vec<crate::alerter::AlertResult>>,
    all_alert_rules: Vec<crate::alerter::AlertRule>,
    detail_tab: DetailTab,
    txn_cursor: usize,
    txn_scroll: usize,
    pending_scroll: usize,
    txn_filter: TxnFilter,
    tag: TagState,
    report_period: ReportPeriod,
    report_compare: bool,
    search_open: bool,
    search_input: String,
    note_open: bool,
    note_input: String,
    note_cursor: usize,
    note_txn_id: String,
    note_account_uid: String,
    refresh: RefreshState,
    status: String,
}

impl App {
    pub async fn run(cfg: Config) -> Result<()> {
        let mut app = Self::new(cfg).await?;

        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let result = app.event_loop().await;
        disable_raw_mode()?;
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
        result
    }

    async fn new(cfg: Config) -> Result<Self> {
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
            status: String::new(),
        })
    }

    async fn load_all_data(
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

    fn current_account(&self) -> Option<&store::AccountRecord> {
        self.accounts.get(self.selected_account)
    }
    fn current_uid(&self) -> Option<&str> {
        self.current_account().map(|a| a.uid.as_str())
    }

    // ---------- Event loop ----------

    async fn event_loop(&mut self) -> Result<()> {
        let stdout = std::io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("create terminal")?;
        loop {
            if self.refresh.open {
                self.do_refresh(&mut terminal).await?;
            }
            terminal.draw(|f| self.render(f)).context("draw frame")?;
            if let Event::Key(key) = event::read().context("read event")? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // If refresh modal is open and done, Enter/Esc/Escape closes it.
                if self.refresh.open && self.refresh.done {
                    match key.code {
                        KeyCode::Enter | KeyCode::Esc => {
                            self.refresh.open = false;
                            self.refresh.done = false;
                            continue;
                        }
                        _ => continue,
                    }
                }

                // Global shortcuts (skipped when search/note modal is active).
                let input_active = self.search_open || self.note_open;
                if !input_active {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Esc => {
                            if self.tag.open {
                                self.tag.open = false;
                            }
                            continue;
                        }
                        KeyCode::Char('r') => {
                            self.start_refresh();
                            continue;
                        }
                        _ => {}
                    }
                }
                self.handle_accounts_key(key.code).await?;
            }
        }
        Ok(())
    }

    // ---------- Key handling ----------

    async fn handle_accounts_key(&mut self, code: KeyCode) -> Result<()> {
        // Note modal.
        if self.note_open {
            match code {
                KeyCode::Enter => {
                    self.save_note().await?;
                    return Ok(());
                }
                KeyCode::Esc => {
                    self.note_open = false;
                    return Ok(());
                }
                KeyCode::Left => {
                    if self.note_cursor > 0 {
                        self.note_cursor -= 1;
                    }
                    return Ok(());
                }
                KeyCode::Right => {
                    if self.note_cursor < self.note_input.len() {
                        self.note_cursor += 1;
                    }
                    return Ok(());
                }
                KeyCode::Backspace => {
                    if self.note_cursor > 0 {
                        self.note_cursor -= 1;
                        self.note_input.remove(self.note_cursor);
                    }
                    return Ok(());
                }
                KeyCode::Delete => {
                    if self.note_cursor < self.note_input.len() {
                        self.note_input.remove(self.note_cursor);
                    }
                    return Ok(());
                }
                KeyCode::Home => {
                    self.note_cursor = 0;
                    return Ok(());
                }
                KeyCode::End => {
                    self.note_cursor = self.note_input.len();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    self.note_input
                        .insert(self.note_cursor, c.to_ascii_uppercase());
                    self.note_cursor += 1;
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
        // Search modal.
        if self.search_open {
            match code {
                KeyCode::Enter => {
                    self.txn_filter.search = self.search_input.clone();
                    self.txn_filter.active = !self.search_input.is_empty();
                    self.search_open = false;
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                    return Ok(());
                }
                KeyCode::Esc => {
                    self.search_open = false;
                    return Ok(());
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
        // Tag popup.
        if self.tag.open {
            match code {
                KeyCode::Enter => {
                    self.apply_tag().await?;
                    self.tag.open = false;
                    return Ok(());
                }
                KeyCode::Char('s') => {
                    self.tag.open = false;
                    return Ok(());
                }
                KeyCode::Char('A') => {
                    self.tag.apply_similar = true;
                    self.apply_tag().await?;
                    self.tag.open = false;
                    return Ok(());
                }
                KeyCode::Up => {
                    if self.tag.selected_category > 0 {
                        self.tag.selected_category -= 1;
                    }
                    return Ok(());
                }
                KeyCode::Down => {
                    if self.tag.selected_category + 1 < self.tag.categories.len() {
                        self.tag.selected_category += 1;
                    }
                    return Ok(());
                }
                KeyCode::Backspace => {
                    self.tag.new_category_input.pop();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    self.tag.new_category_input.push(c);
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }

        match code {
            // j/k: navigate sidebar accounts.
            KeyCode::Char('j') => {
                if self.selected_account + 1 < self.accounts.len() {
                    self.selected_account += 1;
                    self.account_list_state.select(Some(self.selected_account));
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                    self.pending_scroll = 0;
                }
            }
            KeyCode::Char('k') => {
                if self.selected_account > 0 {
                    self.selected_account -= 1;
                    self.account_list_state.select(Some(self.selected_account));
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                    self.pending_scroll = 0;
                }
            }
            // ↑/↓: move cursor with scroll-at-edges.
            KeyCode::Up => {
                if self.detail_tab == DetailTab::Transactions {
                    if self.txn_cursor > 0 {
                        self.txn_cursor -= 1;
                        if self.txn_cursor < self.txn_scroll {
                            self.txn_scroll = self.txn_cursor;
                        }
                    }
                } else if self.detail_tab == DetailTab::Pending && self.pending_scroll > 0 {
                    self.pending_scroll -= 1;
                }
            }
            KeyCode::Down => {
                if self.detail_tab == DetailTab::Transactions {
                    self.txn_cursor += 1;
                } else if self.detail_tab == DetailTab::Pending {
                    self.pending_scroll += 1;
                }
            }
            // ←/→: switch sub-tabs.
            KeyCode::Left => {
                let all = DetailTab::all();
                let pos = all.iter().position(|t| *t == self.detail_tab).unwrap_or(0);
                self.detail_tab = all[(pos + all.len() - 1) % all.len()];
            }
            KeyCode::Right => {
                let all = DetailTab::all();
                let pos = all.iter().position(|t| *t == self.detail_tab).unwrap_or(0);
                self.detail_tab = all[(pos + 1) % all.len()];
            }
            KeyCode::Char('/') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.search_open = true;
                    self.search_input.clear();
                }
            }
            KeyCode::Char('n') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.open_note_popup();
                }
            }
            KeyCode::Char('t') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.open_tag_popup();
                }
            }
            KeyCode::Char('f') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.txn_filter.active = !self.txn_filter.active;
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                }
            }
            KeyCode::Char('c') => {
                if self.detail_tab == DetailTab::Transactions {
                    let mut cats: Vec<String> = self
                        .cfg
                        .tag_rules
                        .0
                        .iter()
                        .map(|r| r.category.clone())
                        .collect();
                    cats.push("uncategorized".into());
                    let current = self.txn_filter.category.clone();
                    if let Some(pos) = cats.iter().position(|c| Some(c) == current.as_ref()) {
                        let next = (pos + 1) % (cats.len() + 1);
                        self.txn_filter.category = if next < cats.len() {
                            Some(cats[next].clone())
                        } else {
                            None
                        };
                    } else {
                        self.txn_filter.category = Some(cats[0].clone());
                    }
                    self.txn_filter.active = self.txn_filter.category.is_some();
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                }
            }
            KeyCode::Char('d') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.txn_filter.direction = match self.txn_filter.direction.as_deref() {
                        None => Some("DBIT".into()),
                        Some("DBIT") => Some("CRDT".into()),
                        _ => None,
                    };
                    self.txn_filter.active = true;
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                }
            }
            KeyCode::Char('m') => {
                if self.detail_tab == DetailTab::Report {
                    self.report_period = ReportPeriod::Month;
                }
            }
            KeyCode::Char('w') => {
                if self.detail_tab == DetailTab::Report {
                    self.report_period = ReportPeriod::Week;
                }
            }
            KeyCode::Char('D') => {
                if self.detail_tab == DetailTab::Report {
                    self.report_period = ReportPeriod::Day;
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ---------- Refresh ----------

    fn start_refresh(&mut self) {
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

    fn stop_refresh_with_error(&mut self, step_index: usize, message: String) {
        if let Some(step) = self.refresh.steps.get_mut(step_index) {
            step.status = RefreshStatus::Error(message.clone());
        }
        self.status = message;
        self.refresh.done = true;
    }

    fn create_api_client(&self) -> Result<client::Client> {
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

    async fn do_refresh(
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

    fn open_tag_popup(&mut self) {
        let uid = match self.current_uid() {
            Some(u) => u.to_string(),
            None => return,
        };
        let txns = self.all_transactions.get(&uid).cloned().unwrap_or_default();
        let idx = self.txn_cursor;
        if idx >= txns.len() {
            return;
        }
        let tx = &txns[idx];
        let desc = if !tx.remittance_info.is_empty() {
            tx.remittance_info.join(" ")
        } else if !tx.creditor_name.is_empty() {
            tx.creditor_name.clone()
        } else {
            tx.debtor_name.clone()
        };
        let pattern = desc.split_whitespace().next().unwrap_or("").to_lowercase();
        self.tag = TagState {
            open: true,
            transaction_id: tx.transaction_id.clone(),
            description: desc,
            amount: tx.amount.clone(),
            currency: tx.currency.clone(),
            selected_category: 0,
            categories: self
                .cfg
                .tag_rules
                .0
                .iter()
                .map(|r| r.category.clone())
                .collect(),
            pattern,
            apply_similar: false,
            ..Default::default()
        };
    }

    async fn apply_tag(&mut self) -> Result<()> {
        let uid = match self.current_uid() {
            Some(u) => u.to_string(),
            None => return Ok(()),
        };
        let cat = if !self.tag.new_category_input.is_empty() {
            self.tag.new_category_input.clone()
        } else {
            self.tag
                .categories
                .get(self.tag.selected_category)
                .cloned()
                .unwrap_or_else(|| "uncategorized".to_string())
        };
        if self.tag.apply_similar {
            let pattern = self.tag.pattern.to_uppercase();
            let txns = self.all_transactions.get(&uid).cloned().unwrap_or_default();
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
                if norm.contains(&pattern) {
                    updates.push(store::CategoryUpdate {
                        account_uid: uid.clone(),
                        transaction_id: tx.transaction_id.clone(),
                        category: cat.clone(),
                        source: "auto".to_string(),
                    });
                }
            }
            if !updates.is_empty() {
                self.db.update_categories(&updates).await?;
            }
        } else {
            self.db
                .update_category(&uid, &self.tag.transaction_id, &cat, "manual")
                .await?;
        }
        if let Ok(txns) = self
            .db
            .get_transactions(&uid, &store::QueryOpts::default())
            .await
        {
            self.all_transactions.insert(uid, txns);
        }
        self.status = format!("Tagged as {}", cat);
        Ok(())
    }

    // ---------- Note ----------

    fn open_note_popup(&mut self) {
        let uid = match self.current_uid() {
            Some(u) => u.to_string(),
            None => return,
        };
        let txns = self.all_transactions.get(&uid).cloned().unwrap_or_default();
        let idx = self.txn_cursor;
        if idx >= txns.len() {
            return;
        }
        let tx = &txns[idx];
        self.note_open = true;
        self.note_input = tx.note.clone();
        self.note_cursor = tx.note.len();
        self.note_txn_id = tx.transaction_id.clone();
        self.note_account_uid = uid;
    }

    async fn save_note(&mut self) -> Result<()> {
        self.db
            .set_transaction_note(&self.note_account_uid, &self.note_txn_id, &self.note_input)
            .await
            .context("save note")?;
        if let Ok(txns) = self
            .db
            .get_transactions(&self.note_account_uid, &store::QueryOpts::default())
            .await
        {
            self.all_transactions
                .insert(self.note_account_uid.clone(), txns);
        }
        self.note_open = false;
        self.status = "Note saved".to_string();
        Ok(())
    }

    // ---------- Render ----------

    fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);
        self.render_accounts(f, chunks[0]);
        if self.refresh.open {
            self.render_refresh_modal(f);
        }
        let status = Span::styled(
            format!(
                " {}  |  q:quit  r:refresh  j/k:accounts  ←→:tabs  ↑↓:scroll",
                self.status
            ),
            Style::default().fg(Color::DarkGray),
        );
        f.render_widget(Paragraph::new(Line::from(status)), chunks[1]);
    }

    fn render_refresh_modal(&self, f: &mut Frame) {
        let popup_area = centered_rect(90, 80, f.area());
        f.render_widget(Clear, popup_area);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            "Refreshing data...",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        let has_error = self
            .refresh
            .steps
            .iter()
            .any(|step| matches!(step.status, RefreshStatus::Error(_)));
        for step in &self.refresh.steps {
            let (icon, color) = match step.status {
                RefreshStatus::Pending => ("○", Color::DarkGray),
                RefreshStatus::Loading => ("⏳", Color::Yellow),
                RefreshStatus::Done => ("✓", Color::Green),
                RefreshStatus::Error(_) => ("✗", Color::Red),
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", icon), Style::default().fg(color)),
                Span::raw(&step.label),
            ]));
            if let RefreshStatus::Error(ref err) = step.status {
                lines.push(Line::from(Span::styled(
                    format!("   {}", err),
                    Style::default().fg(Color::Red),
                )));
            }
        }
        if self.refresh.done {
            let (message, color) = if has_error {
                ("Refresh stopped", Color::Red)
            } else {
                ("Done!", Color::Green)
            };
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                message,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                "Enter / Esc to close",
                Style::default().fg(Color::DarkGray),
            )));
        }
        let popup = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(" Refresh "))
            .style(Style::default().bg(Color::Black))
            .wrap(Wrap { trim: false });
        f.render_widget(popup, popup_area);
    }

    fn render_accounts(&mut self, f: &mut Frame, area: Rect) {
        if self.accounts.is_empty() {
            f.render_widget(
                Paragraph::new("No accounts found.\nRun 'banqline auth' first.")
                    .block(Block::default().borders(Borders::ALL).title(" Accounts ")),
                area,
            );
            return;
        }
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(20)])
            .split(area);
        self.render_sidebar(f, chunks[0]);
        self.render_detail(f, chunks[1]);
    }

    fn render_sidebar(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .accounts
            .iter()
            .map(|a| {
                let name = if a.alias.is_empty() {
                    a.name.clone()
                } else {
                    a.alias.clone()
                };
                ListItem::new(Line::from(Span::raw(name)))
            })
            .collect();
        let mut list_state = self.account_list_state.clone();
        let list = List::new(items)
            .block(Block::default().borders(Borders::RIGHT).title(" Accounts "))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, area, &mut list_state);
    }

    fn render_detail(&mut self, f: &mut Frame, area: Rect) {
        let acct_idx = self.selected_account;
        let acct = match self.accounts.get(acct_idx) {
            Some(a) => a,
            None => return,
        };
        let acct_uid = acct.uid.clone();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(area);
        let display_name = if acct.alias.is_empty() {
            acct.name.clone()
        } else {
            acct.alias.clone()
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                format!("◂ {}", display_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )])),
            chunks[0],
        );
        let pending_count = self
            .all_transactions
            .get(&acct_uid)
            .map(|t| t.iter().filter(|tx| tx.status == "PDNG").count())
            .unwrap_or(0);
        let alert_count = self
            .alert_results
            .get(&acct_uid)
            .map(|r| r.iter().filter(|a| a.status == "TRIGGERED").count())
            .unwrap_or(0);
        let sub_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(chunks[1]);
        let tab_titles: Vec<Line> = DetailTab::all()
            .iter()
            .map(|t| Line::from(Span::raw(t.label(pending_count, alert_count))))
            .collect();
        let tab_bar = Tabs::new(tab_titles)
            .select(
                DetailTab::all()
                    .iter()
                    .position(|t| *t == self.detail_tab)
                    .unwrap_or(0),
            )
            .block(Block::default().borders(Borders::BOTTOM))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::raw("|"));
        f.render_widget(tab_bar, sub_chunks[0]);
        let acct_clone = acct.clone();
        match self.detail_tab {
            DetailTab::General => self.render_general(f, sub_chunks[1], &acct_clone),
            DetailTab::Pending => self.render_pending(f, sub_chunks[1], &acct_clone),
            DetailTab::Transactions => self.render_transactions(f, sub_chunks[1], &acct_clone),
            DetailTab::AlertsByAccount => self.render_alerts(f, sub_chunks[1], &acct_clone),
            DetailTab::Report => self.render_report(f, sub_chunks[1], &acct_clone),
        }
    }

    // ---------- General tab ----------

    fn render_general(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::raw(acct.iban.clone()),
            Span::raw(" — "),
            Span::styled(acct.bank_name.clone(), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "═══ Balances ═══",
            Style::default().fg(Color::Yellow),
        )));
        if let Some(bals) = self.balances.get(&acct.uid) {
            for b in bals {
                lines.push(Line::from(Span::raw(format!(
                    "{}: {} {}",
                    balance_type_name(&b.balance_type),
                    b.amount,
                    b.currency
                ))));
            }
        } else {
            lines.push(Line::from("No balances available"));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "═══ Forecast ═══",
            Style::default().fg(Color::Yellow),
        )));
        if let Some(fc) = self.forecasts.get(&acct.uid) {
            lines.push(Line::from(Span::raw(format!(
                "Booked: {}  |  Pending: {}  |  Projected: {}  {}",
                fc.booked_balance.round_dp(2),
                format_pending(&fc.pending_delta, fc.has_pending_data),
                fc.projected_balance.round_dp(2),
                fc.currency
            ))));
            let spark_data = self.build_sparkline_data(fc);
            if !spark_data.is_empty() {
                let spark = Sparkline::default()
                    .data(&spark_data)
                    .max(5)
                    .style(Style::default().fg(Color::Cyan));
                let spark_rect = Rect {
                    x: area.x + 1,
                    y: area.y + lines.len() as u16,
                    width: (area.width - 2).min(60),
                    height: 1,
                };
                f.render_widget(spark, spark_rect);
                lines.push(Line::from(""));
            }
        } else {
            lines.push(Line::from("No forecast data"));
        }
        lines.push(Line::from(""));
        let usage = if acct.usage_type.is_empty() {
            "N/A"
        } else {
            &acct.usage_type
        };
        lines.push(Line::from(Span::raw(format!(
            "Usage: {}  ·  Currency: {}",
            usage, acct.currency
        ))));
        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            area,
        );
    }

    // ---------- Pending tab ----------

    fn render_pending(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let pending: Vec<&store::TransactionRecord> = self
            .all_transactions
            .get(&acct.uid)
            .map(|t| t.iter().filter(|tx| tx.status == "PDNG").collect())
            .unwrap_or_default();
        if pending.is_empty() {
            f.render_widget(Paragraph::new("No pending transactions."), area);
            return;
        }
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "Date        ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("Amount    ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("Description", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from("─".repeat(60)));
        let mut total = Decimal::ZERO;
        let mut curr = String::new();
        for tx in pending.iter().skip(self.pending_scroll) {
            let desc = if !tx.note.is_empty() {
                format!("📝 {}", tx.note)
            } else if !tx.remittance_info.is_empty() {
                tx.remittance_info.join(" ")
            } else if !tx.creditor_name.is_empty() {
                tx.creditor_name.clone()
            } else {
                tx.debtor_name.clone()
            };
            let amount: Decimal = tx.amount.parse().unwrap_or(Decimal::ZERO);
            let sign = if tx.credit_debit_indicator == "CRDT" {
                "+"
            } else {
                "-"
            };
            let signed = if tx.credit_debit_indicator == "DBIT" {
                -amount
            } else {
                amount
            };
            total += signed;
            curr = tx.currency.clone();
            let color = if tx.credit_debit_indicator == "CRDT" {
                Color::Green
            } else {
                Color::Red
            };
            lines.push(Line::from(vec![
                Span::raw(format!("{}  ", tx.value_date)),
                Span::styled(
                    format!("{:>7}{}", sign, amount.round_dp(2)),
                    Style::default().fg(color),
                ),
                Span::raw(format!("  {}", desc)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Total pending: {} {}", total.round_dp(2), curr),
            Style::default().fg(if total.is_sign_negative() {
                Color::Red
            } else {
                Color::Green
            }),
        )));
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .block(Block::default().borders(Borders::ALL).title(" Pending ")),
            area,
        );
    }

    // ---------- Transactions tab ----------

    fn render_transactions(&mut self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let all: Vec<store::TransactionRecord> = self
            .all_transactions
            .get(&acct.uid)
            .cloned()
            .unwrap_or_default();
        let mut txns: Vec<&store::TransactionRecord> =
            all.iter().filter(|tx| tx.status != "PDNG").collect();
        if self.txn_filter.active {
            if let Some(ref cat) = self.txn_filter.category {
                txns.retain(|tx| tx.category.eq_ignore_ascii_case(cat));
            }
            if let Some(ref dir) = self.txn_filter.direction {
                txns.retain(|tx| tx.credit_debit_indicator == *dir);
            }
        }
        if !self.txn_filter.search.is_empty() {
            let q = self.txn_filter.search.to_lowercase();
            txns.retain(|tx| {
                let desc = if !tx.remittance_info.is_empty() {
                    tx.remittance_info.join(" ")
                } else if !tx.creditor_name.is_empty() {
                    tx.creditor_name.clone()
                } else {
                    tx.debtor_name.clone()
                };
                let haystack = format!(
                    "{} {} {} {} {} {} {} {}",
                    tx.booking_date,
                    tx.amount,
                    tx.currency,
                    desc,
                    tx.category,
                    tx.creditor_name,
                    tx.debtor_name,
                    tx.note
                );
                haystack.to_lowercase().contains(&q)
            });
        }
        let inner = area.inner(Margin::new(1, 1));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        let filter_text = if self.search_open {
            format!("/{}█", self.search_input)
        } else if !self.txn_filter.search.is_empty() {
            format!(
                "Search: \"{}\"  |  f:filter c:cat d:dir  /:search  n:note",
                self.txn_filter.search
            )
        } else if self.txn_filter.active {
            let mut parts = Vec::new();
            if let Some(ref c) = self.txn_filter.category {
                parts.push(format!("cat:{}", c));
            }
            if let Some(ref d) = self.txn_filter.direction {
                parts.push(format!("dir:{}", d));
            }
            format!("Filters: {}", parts.join(" "))
        } else {
            "/:search  f:filter  c:category  d:direction  n:note".to_string()
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                filter_text,
                Style::default().fg(Color::DarkGray),
            )),
            chunks[0],
        );
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "Date        ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("Amount  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                "Category           ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("Description", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from("─".repeat(70)));
        let visible_height = (chunks[1].height as usize).saturating_sub(4); // border(2) + header+sep(2)
        self.txn_cursor = self.txn_cursor.min(txns.len().saturating_sub(1));
        // Scroll down: only when cursor moves past the last visible item.
        if self.txn_cursor > self.txn_scroll + visible_height.saturating_sub(1) {
            self.txn_scroll = self
                .txn_cursor
                .saturating_sub(visible_height.saturating_sub(1));
        }
        // Scroll up: when cursor is before the first visible item.
        if self.txn_cursor < self.txn_scroll {
            self.txn_scroll = self.txn_cursor;
        }
        for (i, tx) in txns.iter().skip(self.txn_scroll).enumerate() {
            if i >= visible_height && !self.tag.open && !self.note_open {
                break;
            }
            let desc = if !tx.note.is_empty() {
                format!("📝 {}", tx.note)
            } else if !tx.remittance_info.is_empty() {
                tx.remittance_info.join(" ")
            } else if !tx.creditor_name.is_empty() {
                tx.creditor_name.clone()
            } else {
                tx.debtor_name.clone()
            };
            let amount: Decimal = tx.amount.parse().unwrap_or(Decimal::ZERO);
            let sign = if tx.credit_debit_indicator == "DBIT" {
                "-"
            } else {
                "+"
            };
            let amt_color = if tx.credit_debit_indicator == "DBIT" {
                Color::Red
            } else {
                Color::Green
            };
            let is_current = self.txn_scroll + i == self.txn_cursor;
            let prefix = if is_current { "▸" } else { " " };
            let cat_color = if tx.category_source == "manual" {
                Color::Magenta
            } else if tx.category == "uncategorized" {
                Color::Yellow
            } else {
                Color::Green
            };
            let selection = if is_current {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            // Pad the selected row to full width.
            let row_width = chunks[1].width as usize - 2; // minus border
            let fixed = format!(
                "{}{}  {}{:<8} {:<18}",
                prefix,
                tx.booking_date,
                sign,
                amount.round_dp(2),
                tx.category
            );
            let desc_width = row_width.saturating_sub(fixed.len());
            let desc_display = format!("{:<width$}", desc, width = desc_width);
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}  ", prefix, tx.booking_date), selection),
                Span::styled(
                    format!("{}{:<8} ", sign, amount.round_dp(2)),
                    Style::default().fg(amt_color).patch(selection),
                ),
                Span::styled(
                    format!("{:<18}", tx.category),
                    Style::default().fg(cat_color).patch(selection),
                ),
                Span::styled(desc_display, selection),
            ]));
        }
        f.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Transactions ({}) ", txns.len())),
            ),
            chunks[1],
        );
        if self.tag.open {
            self.render_tag_popup(f);
        }
        if self.note_open {
            self.render_note_popup(f);
        }
    }

    fn render_tag_popup(&self, f: &mut Frame) {
        let popup_area = centered_rect(60, 10, f.area());
        f.render_widget(Clear, popup_area);
        let cat_name: String = if !self.tag.new_category_input.is_empty() {
            self.tag.new_category_input.clone()
        } else {
            self.tag
                .categories
                .get(self.tag.selected_category)
                .cloned()
                .unwrap_or_else(|| "uncategorized".to_string())
        };
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            format!(
                "{}  {} {}",
                self.tag.description, self.tag.amount, self.tag.currency
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        let mut cat_line = vec![Span::styled(
            "Category: ",
            Style::default().add_modifier(Modifier::BOLD),
        )];
        cat_line.push(Span::styled(
            format!("[{}]", cat_name),
            Style::default().fg(Color::Cyan),
        ));
        if !self.tag.new_category_input.is_empty() {
            cat_line.push(Span::raw(" (new)"));
        }
        lines.push(Line::from(cat_line));
        lines.push(Line::from(Span::raw(format!(
            "Pattern:  {} (auto)",
            self.tag.pattern
        ))));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter:apply  A:similar  s:skip  ↑↓:category",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Tag Transaction "),
                )
                .style(Style::default().bg(Color::Black)),
            popup_area,
        );
    }

    fn render_note_popup(&self, f: &mut Frame) {
        let popup_area = centered_rect(50, 25, f.area());
        f.render_widget(Clear, popup_area);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            "Add a note",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        let before = &self.note_input[..self.note_cursor];
        let after = &self.note_input[self.note_cursor..];
        lines.push(Line::from(vec![
            Span::raw("Note: "),
            Span::raw(before),
            Span::styled("█", Style::default().fg(Color::Yellow)),
            Span::raw(after),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter:save  Esc:cancel  ←→:move",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .block(Block::default().borders(Borders::ALL).title(" Note "))
                .style(Style::default().bg(Color::Black)),
            popup_area,
        );
    }

    // ---------- Alerts tab ----------

    fn render_alerts(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let results = self
            .alert_results
            .get(&acct.uid)
            .cloned()
            .unwrap_or_default();
        let mut lines: Vec<Line> = Vec::new();
        let mut triggered = 0;
        for r in &results {
            let status_color = if r.status == "TRIGGERED" {
                triggered += 1;
                Color::Red
            } else {
                Color::Green
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("▸ {}", r.rule.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(&r.status, Style::default().fg(status_color)),
            ]));
            lines.push(Line::from(Span::styled(
                format!("  {}", r.details),
                Style::default().fg(Color::DarkGray),
            )));
            for mt in &r.matched_transactions {
                lines.push(Line::from(Span::raw(format!(
                    "    {}  {} {} {}  {}",
                    mt.date, mt.amount, mt.currency, mt.account_uid, mt.description
                ))));
            }
            lines.push(Line::from(""));
        }
        if results.is_empty() {
            lines.push(Line::from("No alert rules configured."));
        }
        f.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default().borders(Borders::ALL).title(format!(
                    " Alerts ({}/{}) ",
                    triggered,
                    results.len()
                )),
            ),
            area,
        );
    }

    // ---------- Report tab ----------

    fn render_report(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let all = self
            .all_transactions
            .get(&acct.uid)
            .cloned()
            .unwrap_or_default();
        let inputs: Vec<SummaryInput> = all
            .iter()
            .filter(|tx| tx.status != "PDNG")
            .map(|tx| SummaryInput {
                booking_date: tx.booking_date.clone(),
                amount: tx.amount.clone(),
                currency: tx.currency.clone(),
                credit_debit_indicator: tx.credit_debit_indicator.clone(),
                category: tx.category.clone(),
            })
            .collect();
        let result = summary::summarize(
            &inputs,
            &SummaryOpts {
                period: self.report_period.to_aggregator(),
                compare: self.report_compare,
            },
        );
        let period_name = match self.report_period {
            ReportPeriod::Day => "day",
            ReportPeriod::Week => "week",
            ReportPeriod::Month => "month",
        };

        // Income per period.
        let mut income_by_period: HashMap<String, Decimal> = HashMap::new();
        for inp in &inputs {
            if inp.credit_debit_indicator != "CRDT" {
                continue;
            }
            let key =
                match summary::bucket_key(&inp.booking_date, self.report_period.to_aggregator()) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
            let amt: Decimal = inp.amount.parse().unwrap_or(Decimal::ZERO);
            *income_by_period.entry(key).or_default() += amt;
        }

        let inner = area.inner(Margin::new(1, 1));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        let ctrl = format!(
            "m:month  w:week  D:day  Period: {}  Compare: {}",
            period_name,
            if self.report_compare {
                "on (vs prior)"
            } else {
                "off"
            }
        );
        f.render_widget(
            Paragraph::new(Span::styled(ctrl, Style::default().fg(Color::DarkGray))),
            chunks[0],
        );

        let mut lines: Vec<Line> = Vec::new();
        if result.periods.is_empty() {
            lines.push(Line::from("No spending data found."));
        } else {
            let header = "Category          Amount        %";
            lines.push(Line::from(Span::styled(
                header,
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from("─".repeat(55)));
            for ps in &result.periods {
                let period_label = match self.report_period {
                    ReportPeriod::Day => ps.key.clone(),
                    ReportPeriod::Week => format!("Week {}", ps.key),
                    ReportPeriod::Month => {
                        if let Ok(d) =
                            NaiveDate::parse_from_str(&format!("{}-01", ps.key), "%Y-%m-%d")
                        {
                            d.format("%B %Y").to_string()
                        } else {
                            ps.key.clone()
                        }
                    }
                };
                let currency_info = format!("{} {}", period_label, ps.currency);
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("── {} ──", currency_info),
                    Style::default().fg(Color::Yellow),
                )));
                if let Some(inc) = income_by_period.get(&ps.key) {
                    lines.push(Line::from(Span::styled(
                        format!("  Income            +{:>8}", inc.round_dp(2)),
                        Style::default().fg(Color::Green),
                    )));
                }
                for cat in &ps.categories {
                    let pct = if ps.total.is_zero() {
                        0
                    } else {
                        (cat.amount / ps.total * Decimal::from(100))
                            .round_dp(0)
                            .to_string()
                            .parse::<i64>()
                            .unwrap_or(0)
                    };
                    let bar_len = (pct as usize * 30 / 100).min(30);
                    let bar = "█".repeat(bar_len);
                    lines.push(Line::from(vec![
                        Span::raw(format!(
                            "{:<16} ",
                            cat.name.chars().take(16).collect::<String>()
                        )),
                        Span::styled(
                            format!("-{:>7}", cat.amount.round_dp(2)),
                            Style::default().fg(Color::Red),
                        ),
                        Span::raw(format!(" {:>3}%  {}", pct, bar)),
                    ]));
                }
                lines.push(Line::from("─".repeat(55)));
                lines.push(Line::from(Span::styled(
                    format!("{:<16} -{:>7}", "Spending", ps.total.round_dp(2)),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                let inc = income_by_period
                    .get(&ps.key)
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                let net = inc - ps.total;
                let net_color = if net.is_sign_negative() {
                    Color::Red
                } else {
                    Color::Green
                };
                let net_sign = if net.is_sign_positive() { "+" } else { "" };
                lines.push(Line::from(Span::styled(
                    format!("{:<16} {}{}", "Net", net_sign, net.round_dp(2)),
                    Style::default().fg(net_color).add_modifier(Modifier::BOLD),
                )));
            }
        }
        if let Some(last) = result.periods.last() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "Coverage: {}% ({}/{})",
                    last.coverage.percentage(),
                    last.coverage.categorized,
                    last.coverage.total
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
        f.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Spending Summary "),
            ),
            chunks[1],
        );
    }

    fn build_sparkline_data(&self, fc: &AccountForecast) -> Vec<u64> {
        let booked_f64: f64 = fc.booked_balance.to_string().parse().unwrap_or(0.0);
        let projected_f64: f64 = fc
            .projected_balance
            .to_string()
            .parse()
            .unwrap_or(booked_f64);
        let diff = projected_f64 - booked_f64;
        let points = 30;
        let mut data = Vec::with_capacity(points);
        for i in 0..points {
            let t = i as f64 / (points - 1) as f64;
            let val = (booked_f64 + diff * t * t).abs() as u64;
            data.push(val.min(u64::MAX / 2));
        }
        data
    }
}

fn format_pending(delta: &Decimal, has_data: bool) -> String {
    if !has_data {
        "(no data)".into()
    } else if delta.is_zero() {
        "0.00".into()
    } else if delta.is_sign_positive() {
        format!("+{}", delta.round_dp(2))
    } else {
        format!("{}", delta.round_dp(2))
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_creates_data_dir_before_opening_database() {
        let cfg = Config::default();
        let data_path = cfg.data_path();

        let app = App::new(cfg).await;

        if let Err(err) = &app {
            panic!("expected App::new to create data dir: {err:#}");
        }
        assert!(data_path.exists());
    }
}
