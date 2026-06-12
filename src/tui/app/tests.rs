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
