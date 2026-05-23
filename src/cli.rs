use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "banqline",
    about = "Terminal-first personal banking via Enable Banking API"
)]
pub(crate) struct Cli {
    #[arg(short = 'c', long = "config", help = "Path to config file")]
    pub(crate) config: Option<PathBuf>,

    #[arg(
        long = "format",
        value_enum,
        default_value_t = OutputFormat::Table,
        help = "Output format"
    )]
    pub(crate) format: OutputFormat,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputFormat {
    Table,
    Json,
    Csv,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
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
pub(crate) struct BankArgs {
    #[command(subcommand)]
    pub(crate) action: BankAction,
}

#[derive(Subcommand)]
pub(crate) enum BankAction {
    /// List supported banks for a country.
    List(BanksArgs),
    /// Connect a bank account through OAuth.
    Connect(AuthArgs),
    /// Show locally stored bank sessions.
    Status,
}

#[derive(Args)]
pub(crate) struct AccountArgs {
    #[command(subcommand)]
    pub(crate) action: AccountAction,
}

#[derive(Subcommand)]
pub(crate) enum AccountAction {
    /// List accounts from the local cache.
    List(AccountsListArgs),
    /// Manage account aliases.
    Alias(AccountAliasArgs),
}

#[derive(Args)]
pub(crate) struct AccountsListArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
}

#[derive(Args)]
pub(crate) struct SyncArgs {
    #[command(subcommand)]
    pub(crate) target: Option<SyncTarget>,
}

#[derive(Subcommand)]
pub(crate) enum SyncTarget {
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
pub(crate) struct SyncAllArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[arg(long)]
    pub(crate) account: Option<String>,
    #[arg(long)]
    pub(crate) from: Option<String>,
    #[arg(long)]
    pub(crate) to: Option<String>,
}

#[derive(Args)]
pub(crate) struct SyncTxArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[arg(long)]
    pub(crate) account: Option<String>,
    #[arg(long)]
    pub(crate) from: Option<String>,
    #[arg(long)]
    pub(crate) to: Option<String>,
}

#[derive(Args)]
pub(crate) struct SyncBalancesArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[arg(long)]
    pub(crate) account: Option<String>,
}

#[derive(Args)]
pub(crate) struct SyncAccountsArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
}

#[derive(Args)]
pub(crate) struct AccountAliasArgs {
    #[command(subcommand)]
    pub(crate) action: AccountAliasAction,
}

#[derive(Subcommand)]
pub(crate) enum AccountAliasAction {
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
pub(crate) struct BalanceArgs {
    #[command(subcommand)]
    pub(crate) action: BalanceAction,
}

#[derive(Subcommand)]
pub(crate) enum BalanceAction {
    /// List balances for an account.
    List(BalancesArgs),
}

#[derive(Args)]
pub(crate) struct TxArgs {
    #[command(subcommand)]
    pub(crate) action: TxAction,
}

#[derive(Subcommand)]
pub(crate) enum TxAction {
    /// List transactions.
    List(TransactionsArgs),
    /// Manage transaction tags.
    Tag(TagArgs),
}

#[derive(Args)]
pub(crate) struct ReportArgs {
    #[command(subcommand)]
    pub(crate) action: ReportAction,
}

#[derive(Subcommand)]
pub(crate) enum ReportAction {
    Summary(SummaryArgs),
    Forecast(ForecastArgs),
}

#[derive(Args)]
pub(crate) struct BanksArgs {
    #[arg(long, required = true)]
    pub(crate) country: String,
    #[arg(long)]
    pub(crate) filter: Option<String>,
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
pub(crate) struct AuthArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[arg(long, required = true)]
    pub(crate) country: String,
}

#[derive(Args)]
pub(crate) struct AccountsArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[command(subcommand)]
    pub(crate) alias: Option<AliasCommand>,
}

#[derive(Subcommand)]
pub(crate) enum AliasCommand {
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
pub(crate) struct BalancesArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[arg(long)]
    pub(crate) account: Option<String>,
}

#[derive(Args)]
pub(crate) struct TransactionsArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[arg(long)]
    pub(crate) account: Option<String>,
    #[arg(long)]
    pub(crate) from: Option<String>,
    #[arg(long)]
    pub(crate) to: Option<String>,
    #[arg(long, default_value = "50")]
    pub(crate) limit: i32,
    #[arg(long)]
    pub(crate) category: Option<String>,
    #[arg(long)]
    pub(crate) direction: Option<String>,
}

#[derive(Args)]
pub(crate) struct ForecastArgs {
    #[arg(long)]
    pub(crate) bank: Option<String>,
    #[arg(long)]
    pub(crate) account: Option<String>,
    #[arg(long)]
    pub(crate) all: bool,
    #[arg(long)]
    pub(crate) detail: bool,
}

#[derive(Args)]
pub(crate) struct SummaryArgs {
    #[arg(long, default_value = "month")]
    pub(crate) period: String,
    #[arg(long)]
    pub(crate) compare: bool,
    #[arg(long)]
    pub(crate) from: Option<String>,
    #[arg(long)]
    pub(crate) to: Option<String>,
    #[arg(long)]
    pub(crate) bank: Option<String>,
}

#[derive(Args)]
pub(crate) struct TagArgs {
    #[command(subcommand)]
    pub(crate) action: TagAction,
}

#[derive(Subcommand)]
pub(crate) enum TagAction {
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
pub(crate) struct AlertsArgs {
    #[command(subcommand)]
    pub(crate) action: AlertsAction,
}

#[derive(Subcommand)]
pub(crate) enum AlertsAction {
    Add(AlertsAddArgs),
    Remove { name: String },
    List,
    Check(AlertsCheckArgs),
}

#[derive(Args)]
pub(crate) struct AlertsAddArgs {
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[arg(long = "type")]
    pub(crate) rule_type: String,
    #[arg(long)]
    pub(crate) amount_gte: Option<String>,
    #[arg(long)]
    pub(crate) merchant_contains: Option<String>,
    #[arg(long)]
    pub(crate) direction: Option<String>,
    #[arg(long)]
    pub(crate) category: Option<String>,
    #[arg(long)]
    pub(crate) threshold: Option<String>,
    #[arg(long)]
    pub(crate) period: Option<String>,
    #[arg(long)]
    pub(crate) bank: Option<String>,
}

#[derive(Args)]
pub(crate) struct AlertsCheckArgs {
    #[arg(long)]
    pub(crate) json: bool,
}
