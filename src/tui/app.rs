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
use crate::store::{self, SqliteStore};
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
    help_open: bool,
    status: String,
}

mod data;
mod events;
mod notes;
mod refresh;
mod render;
mod tagging;
mod txn_view;

#[cfg(test)]
mod tests;

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
}
