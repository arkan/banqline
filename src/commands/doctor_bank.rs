use super::*;

pub(crate) fn cmd_version(json_output: bool) -> Result<()> {
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
pub(crate) struct DoctorCheck {
    name: &'static str,
    status: &'static str,
    detail: String,
    suggestion: Option<String>,
}

pub(crate) fn check_status(ok: bool) -> &'static str {
    if ok { "OK" } else { "WARN" }
}

pub(crate) fn cmd_doctor(config_path: Option<&Path>, pr: &output::Printer) -> Result<()> {
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

pub(crate) async fn cmd_bank_status(cfg: &config::Config, pr: &output::Printer) -> Result<()> {
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

pub(crate) async fn cmd_banks(
    args: &BanksArgs,
    cfg: &config::Config,
    pr: &output::Printer,
) -> Result<()> {
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

pub(crate) async fn cmd_auth(args: &AuthArgs, cfg: &config::Config) -> Result<()> {
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

pub(crate) async fn poll_session(
    client: &client::Client,
    session_id: &str,
) -> Result<client::Session> {
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
