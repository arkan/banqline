// Banqline — CLI + TUI for Enable Banking API
// Rust migration from Go codebase.

mod aggregator;
mod alerter;
mod auth;
mod client;
mod config;
mod output;
mod session;
mod store;
mod tagger;
mod tui;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rust_decimal::Decimal;
use serde_json::json;
use store::Store as _;

// ---------------------------------------------------------------------------
// CLI structs
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "banqline",
    about = "Terminal-first personal banking via Enable Banking API"
)]
struct Cli {
    #[arg(short = 'c', long = "config", help = "Path to config file")]
    config: Option<PathBuf>,

    #[arg(
        long = "format",
        value_enum,
        default_value_t = OutputFormat::Table,
        help = "Output format"
    )]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Csv,
}

#[derive(Subcommand)]
enum Commands {
    Version,
    /// Diagnose local configuration, sessions and storage.
    Doctor,
    /// Bank discovery and connections.
    Bank(BankArgs),
    /// Account listing and aliases.
    Account(AccountArgs),
    /// Balance inspection.
    Balance(BalanceArgs),
    /// Transaction workflows.
    Tx(TxArgs),
    /// Reports and forecasts.
    Report(ReportArgs),
    /// Alert rules and checks.
    Alert(AlertsArgs),
    /// Synchronize the local cache from bank APIs.
    Sync(SyncArgs),
    /// Launch the interactive TUI dashboard.
    Tui,
}

#[derive(Args)]
struct BankArgs {
    #[command(subcommand)]
    action: BankAction,
}

#[derive(Subcommand)]
enum BankAction {
    /// List supported banks for a country.
    List(BanksArgs),
    /// Connect a bank account through OAuth.
    Connect(AuthArgs),
    /// Show locally stored bank sessions.
    Status,
}

#[derive(Args)]
struct AccountArgs {
    #[command(subcommand)]
    action: AccountAction,
}

#[derive(Subcommand)]
enum AccountAction {
    /// List accounts from the local cache.
    List(AccountsListArgs),
    /// Manage account aliases.
    Alias(AccountAliasArgs),
}

#[derive(Args)]
struct AccountsListArgs {
    #[arg(long)]
    bank: Option<String>,
}

#[derive(Args)]
struct SyncArgs {
    #[command(subcommand)]
    target: Option<SyncTarget>,
}

#[derive(Subcommand)]
enum SyncTarget {
    /// Synchronize accounts, balances and transactions.
    All(SyncAllArgs),
    /// Synchronize transactions.
    Tx(SyncTxArgs),
    /// Synchronize balances.
    Balances(SyncBalancesArgs),
    /// Synchronize accounts and their balances.
    Accounts(SyncAccountsArgs),
}

#[derive(Args, Default)]
struct SyncAllArgs {
    #[arg(long)]
    bank: Option<String>,
    #[arg(long)]
    account: Option<String>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args)]
struct SyncTxArgs {
    #[arg(long)]
    bank: Option<String>,
    #[arg(long)]
    account: Option<String>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args)]
struct SyncBalancesArgs {
    #[arg(long)]
    bank: Option<String>,
    #[arg(long)]
    account: Option<String>,
}

#[derive(Args)]
struct SyncAccountsArgs {
    #[arg(long)]
    bank: Option<String>,
}

#[derive(Args)]
struct AccountAliasArgs {
    #[command(subcommand)]
    action: AccountAliasAction,
}

#[derive(Subcommand)]
enum AccountAliasAction {
    Set {
        #[arg(long)]
        alias: String,
        #[arg(long)]
        uid: String,
    },
    Get {
        alias: String,
    },
    Remove {
        alias: String,
    },
    List,
}

#[derive(Args)]
struct BalanceArgs {
    #[command(subcommand)]
    action: BalanceAction,
}

#[derive(Subcommand)]
enum BalanceAction {
    /// List balances for an account.
    List(BalancesArgs),
}

#[derive(Args)]
struct TxArgs {
    #[command(subcommand)]
    action: TxAction,
}

#[derive(Subcommand)]
enum TxAction {
    /// List transactions.
    List(TransactionsArgs),
    /// Manage transaction tags.
    Tag(TagArgs),
}

#[derive(Args)]
struct ReportArgs {
    #[command(subcommand)]
    action: ReportAction,
}

#[derive(Subcommand)]
enum ReportAction {
    Summary(SummaryArgs),
    Forecast(ForecastArgs),
}

#[derive(Args)]
struct BanksArgs {
    #[arg(long, required = true)]
    country: String,
    #[arg(long)]
    filter: Option<String>,
}

impl From<AccountsListArgs> for AccountsArgs {
    fn from(value: AccountsListArgs) -> Self {
        AccountsArgs {
            bank: value.bank,
            alias: None,
        }
    }
}

impl From<AccountAliasAction> for AliasCommand {
    fn from(value: AccountAliasAction) -> Self {
        match value {
            AccountAliasAction::Set { alias, uid } => AliasCommand::Set { alias, uid },
            AccountAliasAction::Get { alias } => AliasCommand::Get { alias },
            AccountAliasAction::Remove { alias } => AliasCommand::Remove { alias },
            AccountAliasAction::List => AliasCommand::List,
        }
    }
}

#[derive(Args)]
struct AuthArgs {
    #[arg(long)]
    bank: Option<String>,
    #[arg(long, required = true)]
    country: String,
}

#[derive(Args)]
struct AccountsArgs {
    #[arg(long)]
    bank: Option<String>,
    #[command(subcommand)]
    alias: Option<AliasCommand>,
}

#[derive(Subcommand)]
enum AliasCommand {
    #[command(name = "alias-set")]
    Set {
        #[arg(long)]
        alias: String,
        #[arg(long)]
        uid: String,
    },
    #[command(name = "alias-get")]
    Get { alias: String },
    #[command(name = "alias-remove")]
    Remove { alias: String },
    #[command(name = "alias-list")]
    List,
}

#[derive(Args)]
struct BalancesArgs {
    #[arg(long)]
    bank: Option<String>,
    #[arg(long)]
    account: Option<String>,
}

#[derive(Args)]
struct TransactionsArgs {
    #[arg(long)]
    bank: Option<String>,
    #[arg(long)]
    account: Option<String>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long, default_value = "50")]
    limit: i32,
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    direction: Option<String>,
}

#[derive(Args)]
struct ForecastArgs {
    #[arg(long)]
    bank: Option<String>,
    #[arg(long)]
    account: Option<String>,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    detail: bool,
}

#[derive(Args)]
struct SummaryArgs {
    #[arg(long, default_value = "month")]
    period: String,
    #[arg(long)]
    compare: bool,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    bank: Option<String>,
}

#[derive(Args)]
struct TagArgs {
    #[command(subcommand)]
    action: TagAction,
}

#[derive(Subcommand)]
enum TagAction {
    Apply,
    Preview,
    Override {
        #[arg(long)]
        id: String,
        #[arg(long)]
        category: String,
    },
    Interactive,
}

#[derive(Args)]
struct AlertsArgs {
    #[command(subcommand)]
    action: AlertsAction,
}

#[derive(Subcommand)]
enum AlertsAction {
    Add(AlertsAddArgs),
    Remove { name: String },
    List,
    Check(AlertsCheckArgs),
}

#[derive(Args)]
struct AlertsAddArgs {
    #[arg(long)]
    name: Option<String>,
    #[arg(long = "type")]
    rule_type: String,
    #[arg(long)]
    amount_gte: Option<String>,
    #[arg(long)]
    merchant_contains: Option<String>,
    #[arg(long)]
    direction: Option<String>,
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    threshold: Option<String>,
    #[arg(long)]
    period: Option<String>,
    #[arg(long)]
    bank: Option<String>,
}

#[derive(Args)]
struct AlertsCheckArgs {
    #[arg(long)]
    json: bool,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn load_config(config_path: Option<&Path>) -> Result<config::Config> {
    match config_path {
        Some(p) => config::Config::load(p),
        None => {
            let cfg = config::default_config();
            let default_path = cfg.config_path();
            if default_path.exists() {
                config::Config::load(&default_path)
            } else {
                Ok(cfg)
            }
        }
    }
}

fn new_client(cfg: &config::Config) -> Result<client::Client> {
    let key_path = cfg.key_abs_path()?;
    let key = auth::key::load_private_key(&key_path.to_string_lossy())?;
    let app_id = cfg.application_id.clone();
    let jwt_fn: client::JwtProvider = Arc::new(move || {
        auth::jwt::generate_jwt(&key, &app_id).map_err(|e| anyhow::anyhow!("{e}"))
    });
    Ok(client::Client::new(None, Some(jwt_fn)))
}

fn load_sessions(cfg: &config::Config) -> Result<session::Store> {
    session::load(&cfg.session_path())?
        .ok_or_else(|| anyhow::anyhow!("no sessions found; run bank connect first"))
}

fn resolve_bank<'a>(
    store: &'a session::Store,
    bank_flag: Option<&str>,
) -> Result<(&'a str, &'a session::StoredSession)> {
    match bank_flag {
        Some(name) => {
            let (key, sess) = store.get_key_value(name).ok_or_else(|| {
                anyhow::anyhow!("no session for bank '{name}'; run bank connect first")
            })?;
            if !sess.is_valid() {
                anyhow::bail!("session for '{name}' has expired; run bank connect again");
            }
            Ok((key.as_str(), sess))
        }
        None => {
            if store.len() == 1 {
                let (key, sess) = store.iter().next().unwrap();
                if !sess.is_valid() {
                    anyhow::bail!("session for '{}' has expired; run bank connect again", key);
                }
                Ok((key.as_str(), sess))
            } else {
                let banks: Vec<&str> = store.keys().map(|s| s.as_str()).collect();
                anyhow::bail!(
                    "multiple banks configured ({}); specify --bank",
                    banks.join(", ")
                );
            }
        }
    }
}

async fn resolve_account<'a>(
    sess: &'a session::StoredSession,
    db: Option<&store::SqliteStore>,
    account_flag: Option<&str>,
) -> Result<&'a session::StoredAccount> {
    let flag = match account_flag {
        Some(f) => f,
        None => {
            if sess.accounts.len() == 1 {
                return Ok(&sess.accounts[0]);
            }
            anyhow::bail!(
                "multiple accounts; specify --account (available: {})",
                sess.accounts
                    .iter()
                    .map(|a| output::iban_suffix(&a.iban, 6))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    };

    let resolved_uid = resolve_alias(db, flag).await;

    for a in &sess.accounts {
        if a.uid == resolved_uid {
            return Ok(a);
        }
        if a.iban == resolved_uid {
            return Ok(a);
        }
        if resolved_uid.len() >= 6 && a.iban.ends_with(&resolved_uid) {
            return Ok(a);
        }
    }

    anyhow::bail!("account '{}' not found in session for this bank", flag);
}

async fn resolve_alias(db: Option<&store::SqliteStore>, account_flag: &str) -> String {
    if let Some(db) = db
        && let Ok(Some(acct)) = db.get_account_by_alias(account_flag).await
    {
        return acct.uid;
    }
    account_flag.to_string()
}

fn open_store(cfg: &config::Config) -> Result<store::SqliteStore> {
    let db_path = cfg.data_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let path_str = db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid db path"))?;
    store::SqliteStore::open(path_str)
}

fn to_transaction_records(
    account_uid: &str,
    txns: Vec<client::Transaction>,
) -> Vec<store::TransactionRecord> {
    txns.into_iter()
        .map(|t| store::TransactionRecord {
            account_uid: account_uid.to_string(),
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
            category: "uncategorized".into(),
            category_source: String::new(),
        })
        .collect()
}

fn to_account_record(bank_name: &str, a: &client::Account) -> store::AccountRecord {
    store::AccountRecord {
        uid: a.uid.clone(),
        bank_name: bank_name.to_string(),
        iban: a.account_id.iban.clone(),
        name: a.name.clone(),
        currency: a.currency.clone(),
        details: a.details.clone(),
        usage_type: a.usage.clone(),
        account_type: a.cash_account_type.clone(),
        alias: String::new(),
    }
}

fn to_balance_records(balances: Vec<client::Balance>) -> Vec<store::BalanceRecord> {
    balances
        .into_iter()
        .map(|b| store::BalanceRecord {
            balance_type: b.balance_type,
            amount: b.balance_amount.amount,
            currency: b.balance_amount.currency,
            reference_date: b.reference_date,
        })
        .collect()
}

fn last_synced_footer(t: Option<DateTime<Utc>>) -> String {
    match t {
        Some(dt) if dt != DateTime::<Utc>::default() => {
            format!("Last synced: {}", dt.format("%Y-%m-%d %H:%M:%S UTC"))
        }
        _ => "Last synced: never".to_string(),
    }
}

fn last_synced_meta(t: Option<DateTime<Utc>>) -> Option<HashMap<String, serde_json::Value>> {
    t.filter(|dt| *dt != DateTime::<Utc>::default()).map(|dt| {
        let mut m = HashMap::new();
        m.insert(
            "last_synced".to_string(),
            serde_json::Value::String(dt.to_rfc3339()),
        );
        m
    })
}

async fn build_alias_map(db: &store::SqliteStore) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    if let Ok(all) = db.get_all_accounts().await {
        for acct in &all {
            if !acct.alias.is_empty() {
                aliases.insert(acct.alias.clone(), acct.uid.clone());
            }
        }
    }
    aliases
}

fn printer(format: OutputFormat) -> output::Printer {
    output::Printer {
        json: format == OutputFormat::Json,
        csv: format == OutputFormat::Csv,
    }
}

async fn fetch_all_transactions(
    client: &client::Client,
    account_uid: &str,
    from: Option<&str>,
    to: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<client::Transaction>> {
    let opts = client::TransactionOpts {
        date_from: from.map(String::from),
        date_to: to.map(String::from),
        status: status.map(String::from),
        continuation_key: None,
    };
    client
        .get_all_transactions(account_uid, &opts)
        .await
        .context("fetch transactions")
}

fn best_transaction_date(t: &store::TransactionRecord) -> &str {
    if !t.booking_date.is_empty() {
        &t.booking_date
    } else if !t.value_date.is_empty() {
        &t.value_date
    } else {
        &t.transaction_date
    }
}

fn prepare_transaction_output(
    txns: &mut Vec<store::TransactionRecord>,
    direction: Option<&str>,
    category: Option<&str>,
    limit: i32,
) {
    if let Some(dir) = direction {
        let normalized: String = match dir.to_lowercase().as_str() {
            "credit" | "crdt" => "CRDT".into(),
            "debit" | "dbit" => "DBIT".into(),
            other => other.to_uppercase(),
        };
        txns.retain(|t| t.credit_debit_indicator == normalized);
    }

    if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        txns.retain(|t| t.category.to_lowercase() == cat_lower);
    }

    txns.sort_by(|a, b| {
        let a_key = (
            best_transaction_date(a),
            a.booking_date.as_str(),
            a.value_date.as_str(),
            a.transaction_date.as_str(),
            a.transaction_id.as_str(),
            a.account_uid.as_str(),
        );
        let b_key = (
            best_transaction_date(b),
            b.booking_date.as_str(),
            b.value_date.as_str(),
            b.transaction_date.as_str(),
            b.transaction_id.as_str(),
            b.account_uid.as_str(),
        );
        b_key.cmp(&a_key)
    });

    if limit >= 0 {
        txns.truncate(limit as usize);
    }
}

fn select_main_balance(balances: &[store::BalanceRecord]) -> Option<&store::BalanceRecord> {
    for prio in &["ITBD", "CLBD"] {
        for b in balances {
            if b.balance_type == *prio {
                return Some(b);
            }
        }
    }
    balances.first()
}

fn format_amount(amount: &str) -> String {
    match Decimal::from_str_exact(amount) {
        Ok(d) => format!("{:.2}", d.round_dp(2)),
        Err(_) => amount.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

fn cmd_version(json_output: bool) -> Result<()> {
    if json_output {
        serde_json::to_writer_pretty(
            std::io::stdout(),
            &json!({
                "name": "banqline",
                "version": env!("CARGO_PKG_VERSION"),
            }),
        )
        .context("write json")?;
        println!();
    } else {
        println!("banqline {}", env!("CARGO_PKG_VERSION"));
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct DoctorCheck {
    name: &'static str,
    status: &'static str,
    detail: String,
    suggestion: Option<String>,
}

fn check_status(ok: bool) -> &'static str {
    if ok { "OK" } else { "WARN" }
}

fn cmd_doctor(config_path: Option<&Path>, pr: &output::Printer) -> Result<()> {
    let default_cfg = config::default_config();
    let path = config_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_cfg.config_path());
    let cfg = if path.exists() {
        config::Config::load(&path).unwrap_or_else(|_| default_cfg.clone())
    } else {
        default_cfg.clone()
    };

    let mut checks = Vec::new();
    checks.push(DoctorCheck {
        name: "config",
        status: check_status(path.exists()),
        detail: path.display().to_string(),
        suggestion: (!path.exists())
            .then(|| "Run `banqline setup` or create the config file".to_string()),
    });

    checks.push(DoctorCheck {
        name: "application_id",
        status: check_status(!cfg.application_id.is_empty()),
        detail: if cfg.application_id.is_empty() {
            "not configured".into()
        } else {
            "configured".into()
        },
        suggestion: cfg
            .application_id
            .is_empty()
            .then(|| "Set application_id in config.yaml".to_string()),
    });

    let key_detail = cfg
        .key_abs_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|e| e.to_string());
    let key_ok = cfg.key_abs_path().map(|p| p.is_file()).unwrap_or(false);
    checks.push(DoctorCheck {
        name: "private_key",
        status: check_status(key_ok),
        detail: key_detail,
        suggestion: (!key_ok)
            .then(|| "Put your PEM private key in ~/.config/banqline and set key_path".to_string()),
    });

    let port_ok = std::net::TcpListener::bind(("127.0.0.1", cfg.callback_port)).is_ok();
    checks.push(DoctorCheck {
        name: "callback_port",
        status: check_status(port_ok),
        detail: cfg.callback_port.to_string(),
        suggestion: (!port_ok).then(|| "Choose a free callback_port in config.yaml".to_string()),
    });

    let session_path = cfg.session_path();
    let session_detail = match session::load(&session_path) {
        Ok(Some(store)) if store.is_empty() => "no sessions".to_string(),
        Ok(Some(store)) => format!("{} session(s)", store.len()),
        Ok(None) => "missing".to_string(),
        Err(e) => e.to_string(),
    };
    let session_ok = matches!(session::load(&session_path), Ok(Some(store)) if !store.is_empty());
    checks.push(DoctorCheck {
        name: "session",
        status: check_status(session_ok),
        detail: session_detail,
        suggestion: (!session_ok)
            .then(|| "Run `banqline bank connect --country FR --bank <name>`".to_string()),
    });

    let db_path = cfg.data_path();
    let db_ok = db_path.parent().map(|p| p.exists()).unwrap_or(false)
        || std::fs::create_dir_all(cfg.app_dir()).is_ok();
    checks.push(DoctorCheck {
        name: "database_dir",
        status: check_status(db_ok),
        detail: db_path.display().to_string(),
        suggestion: (!db_ok).then(|| "Check permissions under ~/.config/banqline".to_string()),
    });

    let ok = checks.iter().all(|check| check.status == "OK");
    if pr.json {
        pr.print_json(&json!({ "ok": ok, "checks": checks }))?;
    } else {
        let rows = checks
            .iter()
            .map(|check| {
                vec![
                    check.name.to_string(),
                    check.status.to_string(),
                    check.detail.clone(),
                    check.suggestion.clone().unwrap_or_default(),
                ]
            })
            .collect();
        pr.print_table(
            vec![
                "CHECK".into(),
                "STATUS".into(),
                "DETAIL".into(),
                "NEXT STEP".into(),
            ],
            rows,
        )?;
    }
    Ok(())
}

async fn cmd_bank_status(cfg: &config::Config, pr: &output::Printer) -> Result<()> {
    let sessions = session::load(&cfg.session_path())?.unwrap_or_default();
    if pr.json {
        pr.print_json(&sessions)?;
        return Ok(());
    }
    if sessions.is_empty() {
        println!("No bank connected. Run: banqline bank connect --country FR --bank <name>");
        return Ok(());
    }
    let rows = sessions
        .iter()
        .map(|(bank, sess)| {
            vec![
                bank.clone(),
                sess.accounts.len().to_string(),
                sess.valid_until.to_rfc3339(),
                if sess.is_valid() { "OK" } else { "EXPIRED" }.to_string(),
            ]
        })
        .collect();
    pr.print_table(
        vec![
            "BANK".into(),
            "ACCOUNTS".into(),
            "VALID_UNTIL".into(),
            "STATUS".into(),
        ],
        rows,
    )
}

async fn cmd_banks(args: &BanksArgs, cfg: &config::Config, pr: &output::Printer) -> Result<()> {
    let api_client = new_client(cfg)?;
    let mut aspsps = api_client
        .list_aspsps(&args.country)
        .await
        .context("list aspsps")?;

    if let Some(ref filter) = args.filter {
        let f = filter.to_lowercase();
        aspsps.retain(|a| a.name.to_lowercase().contains(&f));
    }

    if pr.json {
        pr.print_json(&aspsps)?;
    } else {
        let headers = vec!["NAME".into(), "COUNTRY".into()];
        let rows: Vec<Vec<String>> = aspsps
            .iter()
            .map(|a| vec![a.name.clone(), a.country.clone()])
            .collect();
        pr.print_table(headers, rows)?;
    }
    Ok(())
}

async fn cmd_auth(args: &AuthArgs, cfg: &config::Config) -> Result<()> {
    let bank_name = args.bank.as_deref().unwrap_or(&cfg.default_bank);
    if bank_name.is_empty() {
        anyhow::bail!("no bank specified; use --bank or set default_bank in config");
    }

    let api_client = new_client(cfg)?;

    let valid_until = (Utc::now() + Duration::days(90)).to_rfc3339();
    let auth_req = client::AuthRequest {
        access: client::Access { valid_until },
        aspsp: client::Aspsp {
            name: bank_name.to_string(),
            country: args.country.clone(),
        },
        state: uuid::Uuid::new_v4().to_string(),
        redirect_url: cfg.redirect_url.clone(),
        psu_type: "personal".to_string(),
    };

    let auth_resp = api_client.authorize(&auth_req).await?;

    let receiver = auth::callback::start_callback_server(cfg.callback_port)?;

    println!("Opening browser for authorization...");
    open::that(&auth_resp.url).map_err(|e| anyhow::anyhow!("open browser: {e}"))?;

    let callback = receiver
        .recv()
        .map_err(|e| anyhow::anyhow!("callback server: {e}"))?;
    if let Some(err) = &callback.error {
        anyhow::bail!("authorization error: {err}");
    }
    let code = callback
        .code
        .ok_or_else(|| anyhow::anyhow!("no authorization code received"))?;

    println!("Authorization code received. Creating session...");
    let session = api_client.create_session(&code).await?;

    let session = poll_session(&api_client, &session.session_id).await?;

    let mut stored_accounts = Vec::new();
    for acct_id in &session.accounts {
        let details = api_client
            .get_account_details(acct_id)
            .await
            .context("get account details")?;
        stored_accounts.push(session::StoredAccount {
            uid: details.uid.clone(),
            iban: details.account_id.iban.clone(),
            name: details.name.clone(),
            currency: details.currency.clone(),
        });
    }

    let stored_session = session::StoredSession {
        session_id: session.session_id,
        accounts: stored_accounts,
        created_at: Utc::now(),
        valid_until: Utc::now() + Duration::days(90),
    };

    let mut store = session::load(&cfg.session_path())?.unwrap_or_default();
    store.insert(bank_name.to_string(), stored_session);
    session::save(&cfg.session_path(), &store)?;

    println!("Authorization successful for {bank_name}");
    Ok(())
}

async fn poll_session(client: &client::Client, session_id: &str) -> Result<client::Session> {
    for i in 0..8 {
        let delay = tokio::time::sleep(std::time::Duration::from_secs(3));
        if i > 0 {
            println!("Waiting for accounts...");
        }
        delay.await;
        let s = client.get_session(session_id).await?;
        if !s.accounts.is_empty() {
            return Ok(s);
        }
    }
    anyhow::bail!("session did not return accounts after polling")
}

#[derive(serde::Serialize)]
struct SyncReport {
    account: String,
    bank: String,
    tx_before: usize,
    tx_added: usize,
    balances: usize,
    status: String,
    synced_at: DateTime<Utc>,
}

fn sync_bank_names<'a>(
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

async fn sync_account_uids(
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

async fn sync_accounts_for_bank(
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

async fn sync_balances_for_bank(
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

async fn sync_transactions_for_bank(
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

async fn sync_all_for_bank(
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

fn session_account_by_uid<'a>(
    sess: &'a session::StoredSession,
    uid: &str,
) -> Result<&'a session::StoredAccount> {
    sess.accounts
        .iter()
        .find(|acct| acct.uid == uid)
        .ok_or_else(|| anyhow::anyhow!("account '{uid}' not found in selected bank session"))
}

async fn cached_transactions(
    db: &store::SqliteStore,
    account_uid: &str,
) -> Result<Vec<store::TransactionRecord>> {
    db.get_transactions(account_uid, &store::QueryOpts::default())
        .await
}

async fn cached_transaction_count(db: &store::SqliteStore, account_uid: &str) -> Result<usize> {
    Ok(cached_transactions(db, account_uid).await?.len())
}

fn transaction_record_id(t: &store::TransactionRecord) -> String {
    if t.transaction_id.is_empty() {
        t.entry_reference.clone()
    } else {
        t.transaction_id.clone()
    }
}

async fn account_label(
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

fn print_sync_reports(pr: &output::Printer, reports: &[SyncReport]) -> Result<()> {
    if pr.json {
        let mut meta = HashMap::new();
        meta.insert("count".into(), serde_json::json!(reports.len()));
        return pr.print_json_with_meta(&reports, meta);
    }

    let (headers, rows) = sync_report_table(reports);
    pr.print_table(headers, rows)
}

fn sync_report_table(reports: &[SyncReport]) -> (Vec<String>, Vec<Vec<String>>) {
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

async fn cmd_sync(args: &SyncArgs, cfg: &config::Config, pr: &output::Printer) -> Result<()> {
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

async fn cmd_accounts(
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

async fn cmd_accounts_alias(
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

async fn cmd_balances(
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

async fn cmd_transactions(
    args: &TransactionsArgs,
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

            rows.push(vec![
                if !t.booking_date.is_empty() {
                    t.booking_date.clone()
                } else {
                    t.transaction_date.clone()
                },
                amt,
                dir.to_string(),
                desc,
            ]);
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

async fn cmd_forecast(
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

async fn forecast_single_bank(
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

async fn forecast_all_banks(
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

async fn compute_account_forecast(
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

fn print_forecast_table(
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

fn print_forecast_detail(
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

fn print_forecast_json(
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

fn format_pending_decimal(d: Decimal) -> String {
    if d == Decimal::ZERO {
        "    0.00".to_string()
    } else if d > Decimal::ZERO {
        format!("+{:.2}", d.round_dp(2))
    } else {
        format!("{:.2}", d.round_dp(2))
    }
}

async fn cmd_summary(args: &SummaryArgs, cfg: &config::Config, pr: &output::Printer) -> Result<()> {
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

fn compute_date_range(
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

async fn fetch_summary_bank_txns(
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

async fn fetch_summary_all_txns(
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

fn format_summary_table(
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

fn format_summary_json(
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

fn format_delta(d: &Option<aggregator::summary::DeltaInfo>, currency: &str) -> String {
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

// ---------------------------------------------------------------------------
// Tag helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
struct PreviewChange {
    transaction_id: String,
    booking_date: String,
    description: String,
    current_category: String,
    new_category: String,
}

async fn apply_tag_rules(
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

async fn preview_tag_rules(
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

async fn override_category(
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

async fn cmd_tag(args: &TagArgs, cfg: &config::Config) -> Result<()> {
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

fn alert_rule_row(r: &alerter::AlertRule) -> Vec<String> {
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

async fn cmd_alerts_add(args: &AlertsAddArgs, cfg: &config::Config) -> Result<()> {
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
    cfg.save(&cfg.config_path())?;

    println!("Alert rule '{}' (type={}) added", rule.name, rule.rule_type);
    Ok(())
}

async fn cmd_alerts_remove(name: &str, cfg: &config::Config) -> Result<()> {
    let mut cfg = cfg.clone();
    let len_before = cfg.alert_rules.len();
    cfg.alert_rules.retain(|r| r.name != name);

    if cfg.alert_rules.len() == len_before {
        anyhow::bail!("alert rule '{name}' not found");
    }

    cfg.save(&cfg.config_path())?;
    println!("Alert rule '{name}' removed");
    Ok(())
}

async fn cmd_alerts_list(cfg: &config::Config, pr: &output::Printer) -> Result<()> {
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

async fn cmd_alerts_check(
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

async fn cmd_alerts(args: &AlertsArgs, cfg: &config::Config, pr: &output::Printer) -> Result<()> {
    match &args.action {
        AlertsAction::Add(add_args) => cmd_alerts_add(add_args, cfg).await,
        AlertsAction::Remove { name } => cmd_alerts_remove(name, cfg).await,
        AlertsAction::List => cmd_alerts_list(cfg, pr).await,
        AlertsAction::Check(check_args) => cmd_alerts_check(check_args, cfg, pr).await,
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let json_output = cli.format == OutputFormat::Json;
    let output_format = cli.format;
    let config_path = cli.config.clone();

    match cli.command {
        Commands::Version => cmd_version(json_output),
        Commands::Doctor => {
            let pr = printer(output_format);
            cmd_doctor(config_path.as_deref(), &pr)
        }
        Commands::Bank(args) => {
            let cfg = load_config(config_path.as_deref())?;
            let pr = printer(output_format);
            match args.action {
                BankAction::List(list_args) => cmd_banks(&list_args, &cfg, &pr).await,
                BankAction::Connect(connect_args) => cmd_auth(&connect_args, &cfg).await,
                BankAction::Status => cmd_bank_status(&cfg, &pr).await,
            }
        }
        Commands::Account(args) => {
            let cfg = load_config(config_path.as_deref())?;
            let pr = printer(output_format);
            match args.action {
                AccountAction::List(list_args) => cmd_accounts(&list_args.into(), &cfg, &pr).await,
                AccountAction::Alias(alias_args) => {
                    let alias_command = AliasCommand::from(alias_args.action);
                    cmd_accounts_alias(&alias_command, &cfg, &pr).await
                }
            }
        }
        Commands::Balance(args) => {
            let cfg = load_config(config_path.as_deref())?;
            let pr = printer(output_format);
            match args.action {
                BalanceAction::List(list_args) => cmd_balances(&list_args, &cfg, &pr).await,
            }
        }
        Commands::Tx(args) => {
            let cfg = load_config(config_path.as_deref())?;
            let pr = printer(output_format);
            match args.action {
                TxAction::List(list_args) => cmd_transactions(&list_args, &cfg, &pr).await,
                TxAction::Tag(tag_args) => cmd_tag(&tag_args, &cfg).await,
            }
        }
        Commands::Report(args) => {
            let cfg = load_config(config_path.as_deref())?;
            let pr = printer(output_format);
            match args.action {
                ReportAction::Summary(summary_args) => cmd_summary(&summary_args, &cfg, &pr).await,
                ReportAction::Forecast(forecast_args) => {
                    cmd_forecast(&forecast_args, &cfg, &pr).await
                }
            }
        }
        Commands::Alert(args) => {
            let cfg = load_config(config_path.as_deref())?;
            let pr = printer(output_format);
            cmd_alerts(&args, &cfg, &pr).await
        }
        Commands::Sync(args) => {
            let cfg = load_config(config_path.as_deref())?;
            let pr = printer(output_format);
            cmd_sync(&args, &cfg, &pr).await
        }
        Commands::Tui => {
            let cfg = load_config(config_path.as_deref())?;
            tui::run(cfg).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txn(
        account_uid: &str,
        transaction_id: &str,
        booking_date: &str,
        value_date: &str,
        transaction_date: &str,
        direction: &str,
        category: &str,
    ) -> store::TransactionRecord {
        store::TransactionRecord {
            account_uid: account_uid.into(),
            transaction_id: transaction_id.into(),
            booking_date: booking_date.into(),
            value_date: value_date.into(),
            transaction_date: transaction_date.into(),
            credit_debit_indicator: direction.into(),
            category: category.into(),
            ..Default::default()
        }
    }

    #[test]
    fn prepare_transaction_output_sorts_globally_before_limiting() {
        let mut txns = vec![
            txn("acc-a", "old", "2026-05-01", "", "", "DBIT", "food"),
            txn("acc-b", "new", "2026-05-08", "", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, None, None, 1);

        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].transaction_id, "new");
    }

    #[test]
    fn prepare_transaction_output_filters_before_limiting() {
        let mut txns = vec![
            txn(
                "acc-a",
                "new-credit",
                "2026-05-08",
                "",
                "",
                "CRDT",
                "salary",
            ),
            txn("acc-a", "old-debit", "2026-05-01", "", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, Some("debit"), Some("food"), 1);

        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].transaction_id, "old-debit");
    }

    #[test]
    fn prepare_transaction_output_uses_value_and_transaction_date_fallbacks() {
        let mut txns = vec![
            txn(
                "acc-a",
                "transaction-date",
                "",
                "",
                "2026-05-07",
                "DBIT",
                "food",
            ),
            txn("acc-a", "value-date", "", "2026-05-08", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, None, None, 2);

        assert_eq!(txns[0].transaction_id, "value-date");
        assert_eq!(txns[1].transaction_id, "transaction-date");
    }
}
