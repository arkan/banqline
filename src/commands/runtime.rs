use super::*;

pub(crate) fn load_config(config_path: Option<&Path>) -> Result<config::Config> {
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

pub(crate) fn config_save_path(cfg: &config::Config, config_path: Option<&Path>) -> PathBuf {
    config_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cfg.config_path())
}

pub(crate) fn new_client(cfg: &config::Config) -> Result<client::Client> {
    let key_path = cfg.key_abs_path()?;
    let key = auth::key::load_private_key(&key_path.to_string_lossy())?;
    let app_id = cfg.application_id.clone();
    let jwt_fn: client::JwtProvider = Arc::new(move || {
        auth::jwt::generate_jwt(&key, &app_id).map_err(|e| anyhow::anyhow!("{e}"))
    });
    Ok(client::Client::new(None, Some(jwt_fn)))
}

pub(crate) fn load_sessions(cfg: &config::Config) -> Result<session::Store> {
    session::load(&cfg.session_path())?
        .ok_or_else(|| anyhow::anyhow!("no sessions found; run bank connect first"))
}

pub(crate) fn resolve_bank<'a>(
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

pub(crate) async fn resolve_account<'a>(
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

pub(crate) async fn resolve_alias(db: Option<&store::SqliteStore>, account_flag: &str) -> String {
    if let Some(db) = db
        && let Ok(Some(acct)) = db.get_account_by_alias(account_flag).await
    {
        return acct.uid;
    }
    account_flag.to_string()
}

pub(crate) fn open_store(cfg: &config::Config) -> Result<store::SqliteStore> {
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

pub(crate) fn to_transaction_records(
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

pub(crate) fn to_account_record(bank_name: &str, a: &client::Account) -> store::AccountRecord {
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

pub(crate) fn to_balance_records(balances: Vec<client::Balance>) -> Vec<store::BalanceRecord> {
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

pub(crate) fn last_synced_footer(t: Option<DateTime<Utc>>) -> String {
    match t {
        Some(dt) if dt != DateTime::<Utc>::default() => {
            format!("Last synced: {}", dt.format("%Y-%m-%d %H:%M:%S UTC"))
        }
        _ => "Last synced: never".to_string(),
    }
}

pub(crate) fn last_synced_meta(
    t: Option<DateTime<Utc>>,
) -> Option<HashMap<String, serde_json::Value>> {
    t.filter(|dt| *dt != DateTime::<Utc>::default()).map(|dt| {
        let mut m = HashMap::new();
        m.insert(
            "last_synced".to_string(),
            serde_json::Value::String(dt.to_rfc3339()),
        );
        m
    })
}

pub(crate) async fn build_alias_map(db: &store::SqliteStore) -> HashMap<String, String> {
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

pub(crate) fn printer(format: OutputFormat) -> output::Printer {
    output::Printer {
        json: format == OutputFormat::Json,
        csv: format == OutputFormat::Csv,
    }
}

pub(crate) async fn fetch_all_transactions(
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

pub(crate) fn best_transaction_date(t: &store::TransactionRecord) -> &str {
    if !t.booking_date.is_empty() {
        &t.booking_date
    } else if !t.value_date.is_empty() {
        &t.value_date
    } else {
        &t.transaction_date
    }
}

pub(crate) fn prepare_transaction_output(
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

pub(crate) fn select_main_balance(
    balances: &[store::BalanceRecord],
) -> Option<&store::BalanceRecord> {
    for prio in &["ITBD", "CLBD"] {
        for b in balances {
            if b.balance_type == *prio {
                return Some(b);
            }
        }
    }
    balances.first()
}

pub(crate) fn format_amount(amount: &str) -> String {
    match Decimal::from_str_exact(amount) {
        Ok(d) => format!("{:.2}", d.round_dp(2)),
        Err(_) => amount.to_string(),
    }
}
