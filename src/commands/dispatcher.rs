use super::*;

pub(crate) async fn run(cli: Cli) -> anyhow::Result<()> {
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
            cmd_alerts(&args, &cfg, config_path.as_deref(), &pr).await
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
