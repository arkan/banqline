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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn txn(
    transaction_id: &str,
    booking_date: &str,
    value_date: &str,
    status: &str,
) -> store::TransactionRecord {
    store::TransactionRecord {
        transaction_id: transaction_id.into(),
        booking_date: booking_date.into(),
        value_date: value_date.into(),
        status: status.into(),
        credit_debit_indicator: "DBIT".into(),
        category: "food".into(),
        ..Default::default()
    }
}

fn open_test_store() -> (store::SqliteStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    (
        store::SqliteStore::open(path.to_str().unwrap()).unwrap(),
        dir,
    )
}

fn test_app_with_transactions(
    db: std::sync::Arc<store::SqliteStore>,
    txns: Vec<store::TransactionRecord>,
) -> App {
    let uid: String = "test-uid".into();
    let mut all_txns = HashMap::new();
    all_txns.insert(uid.clone(), txns);
    App {
        cfg: Config::default(),
        db,
        sessions: session::Store::default(),
        accounts: vec![store::AccountRecord {
            uid,
            name: "Test Account".into(),
            ..Default::default()
        }],
        account_list_state: ListState::default(),
        selected_account: 0,
        balances: HashMap::new(),
        forecasts: HashMap::new(),
        all_transactions: all_txns,
        alert_results: HashMap::new(),
        all_alert_rules: Vec::new(),
        detail_tab: DetailTab::General,
        txn_cursor: 0,
        txn_scroll: 0,
        pending_scroll: 0,
        txn_filter: TxnFilter::default(),
        tag: TagState::default(),
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
    }
}

// ---------------------------------------------------------------------------
// visible / selected helpers (unit-level)
// ---------------------------------------------------------------------------

#[test]
fn visible_transactions_excludes_pending_and_sorts_like_cli() {
    let all = vec![
        txn("booked-old", "2026-05-01", "", "BOOK"),
        txn("pending-new", "2026-05-09", "", "PDNG"),
        txn("value-date", "", "2026-05-08", "BOOK"),
    ];

    let visible = txn_view::visible_transactions(&all, &TxnFilter::default());

    assert_eq!(visible.len(), 2);
    assert_eq!(visible[0].transaction_id, "value-date");
    assert_eq!(visible[1].transaction_id, "booked-old");
}

#[test]
fn selected_transaction_uses_visible_transaction_order() {
    let all = vec![
        txn("raw-first-but-visible-second", "2026-05-01", "", "BOOK"),
        txn("raw-second-but-pending", "2026-05-09", "", "PDNG"),
        txn("raw-third-but-visible-first", "", "2026-05-08", "BOOK"),
    ];

    let selected = txn_view::selected_transaction(&all, &TxnFilter::default(), 0).unwrap();

    assert_eq!(selected.transaction_id, "raw-third-but-visible-first");
}

#[test]
fn open_note_popup_uses_visible_transaction_order() {
    let (store, _dir) = open_test_store();
    let db = std::sync::Arc::new(store);
    let txns = vec![
        txn("booked-old", "2026-05-01", "", "BOOK"),
        txn("pending-new", "2026-05-09", "", "PDNG"),
        txn("value-date", "", "2026-05-08", "BOOK"),
    ];

    let mut app = test_app_with_transactions(db, txns);
    app.txn_cursor = 0;
    app.open_note_popup();

    assert!(app.note_open);
    assert_eq!(
        app.note_txn_id, "value-date",
        "expected the first visible transaction, not the raw first element"
    );
}

#[tokio::test]
async fn apply_tag_uses_filtered_transaction() {
    let (store, _dir) = open_test_store();
    let uid = "test-uid";

    store
        .upsert_account(
            "BANK",
            &store::AccountRecord {
                uid: uid.into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    store
        .upsert_transactions(
            uid,
            &[
                store::TransactionRecord {
                    transaction_id: "tx-food".into(),
                    amount: "10.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-05-01".into(),
                    ..Default::default()
                },
                store::TransactionRecord {
                    transaction_id: "tx-transport".into(),
                    amount: "20.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-05-09".into(),
                    ..Default::default()
                },
            ],
        )
        .await
        .unwrap();

    store
        .update_category(uid, "tx-food", "food", "")
        .await
        .unwrap();
    store
        .update_category(uid, "tx-transport", "transport", "")
        .await
        .unwrap();

    let db = std::sync::Arc::new(store);
    let db_txns = db
        .get_transactions(uid, &store::QueryOpts::default())
        .await
        .unwrap();

    let mut app = test_app_with_transactions(db, db_txns);

    app.txn_filter = TxnFilter {
        active: true,
        category: Some("transport".into()),
        direction: None,
        search: String::new(),
    };
    app.txn_cursor = 0;

    app.open_tag_popup();
    assert!(app.tag.open);
    assert_eq!(
        app.tag.transaction_id, "tx-transport",
        "expected the visible filtered transaction, not raw[0]"
    );

    app.tag.new_category_input = "newcat".into();
    app.apply_tag().await.unwrap();

    let updated = app.all_transactions.get(uid).cloned().unwrap_or_default();

    let tx_food = updated
        .iter()
        .find(|t| t.transaction_id == "tx-food")
        .unwrap();
    let tx_transport = updated
        .iter()
        .find(|t| t.transaction_id == "tx-transport")
        .unwrap();

    assert_eq!(
        tx_food.category, "food",
        "food transaction must remain unchanged"
    );
    assert_eq!(
        tx_transport.category, "newcat",
        "transport must have the new category"
    );
    assert_eq!(
        tx_transport.category_source, "manual",
        "transport must be marked manual"
    );
}
