use super::*;

#[tokio::test]
async fn test_upsert_transactions() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let txns = vec![TransactionRecord {
        transaction_id: "txn-001".into(),
        amount: "42.50".into(),
        currency: "EUR".into(),
        booking_date: "2026-03-27".into(),
        creditor_name: "Amazon".into(),
        credit_debit_indicator: "DBIT".into(),
        remittance_info: vec!["Order #123".into()],
        ..Default::default()
    }];
    store.upsert_transactions("acc-001", &txns).await.unwrap();

    let got = store
        .get_transactions("acc-001", &QueryOpts::default())
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].transaction_id, "txn-001");
    assert_eq!(got[0].amount, "42.50");
    assert_eq!(got[0].creditor_name, "Amazon");
    assert_eq!(got[0].remittance_info, vec!["Order #123"]);
}

#[tokio::test]
async fn test_upsert_transactions_update() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let txn = TransactionRecord {
        transaction_id: "txn-001".into(),
        amount: "10.00".into(),
        currency: "EUR".into(),
        status: "PDNG".into(),
        ..Default::default()
    };
    store.upsert_transactions("acc-001", &[txn]).await.unwrap();

    let txn2 = TransactionRecord {
        transaction_id: "txn-001".into(),
        amount: "10.50".into(),
        currency: "EUR".into(),
        status: "BOOK".into(),
        ..Default::default()
    };
    store.upsert_transactions("acc-001", &[txn2]).await.unwrap();

    let got = store
        .get_transactions("acc-001", &QueryOpts::default())
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].amount, "10.50");
    assert_eq!(got[0].status, "BOOK");
}

#[tokio::test]
async fn test_upsert_transactions_empty_id_fallback() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let txn = TransactionRecord {
        transaction_id: String::new(),
        entry_reference: "ref-001".into(),
        amount: "5.00".into(),
        currency: "EUR".into(),
        ..Default::default()
    };
    store.upsert_transactions("acc-001", &[txn]).await.unwrap();

    let got = store
        .get_transactions("acc-001", &QueryOpts::default())
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].transaction_id, "ref-001");
}

#[tokio::test]
async fn test_get_transactions_date_filter() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .upsert_transactions(
            "acc-001",
            &[
                TransactionRecord {
                    transaction_id: "t1".into(),
                    amount: "1.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-25".into(),
                    ..Default::default()
                },
                TransactionRecord {
                    transaction_id: "t2".into(),
                    amount: "2.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-26".into(),
                    ..Default::default()
                },
                TransactionRecord {
                    transaction_id: "t3".into(),
                    amount: "3.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-27".into(),
                    ..Default::default()
                },
            ],
        )
        .await
        .unwrap();

    let got = store
        .get_transactions(
            "acc-001",
            &QueryOpts {
                date_from: Some("2026-03-26".into()),
                date_to: Some("2026-03-26".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].transaction_id, "t2");
}

#[tokio::test]
async fn test_get_transactions_limit() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .upsert_transactions(
            "acc-001",
            &[
                TransactionRecord {
                    transaction_id: "t1".into(),
                    amount: "1.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-25".into(),
                    ..Default::default()
                },
                TransactionRecord {
                    transaction_id: "t2".into(),
                    amount: "2.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-26".into(),
                    ..Default::default()
                },
                TransactionRecord {
                    transaction_id: "t3".into(),
                    amount: "3.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-27".into(),
                    ..Default::default()
                },
            ],
        )
        .await
        .unwrap();

    let got = store
        .get_transactions(
            "acc-001",
            &QueryOpts {
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(got.len(), 2);
}

#[tokio::test]
async fn test_set_get_last_synced() {
    let store = open_test_store();
    let ts = Utc.with_ymd_and_hms(2026, 3, 27, 12, 0, 0).unwrap();
    store
        .set_last_synced("acc-001", "transactions", ts)
        .await
        .unwrap();

    let got = store
        .get_last_synced("acc-001", "transactions")
        .await
        .unwrap();
    assert_eq!(got, ts);
}

#[tokio::test]
async fn test_get_last_synced_zero() {
    let store = open_test_store();
    let got = store
        .get_last_synced("nonexistent", "transactions")
        .await
        .unwrap();
    assert_eq!(got, DateTime::<Utc>::default());
}

#[tokio::test]
async fn test_get_transactions_status_filter() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .upsert_transactions(
            "acc-001",
            &[
                TransactionRecord {
                    transaction_id: "t1".into(),
                    amount: "10.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-27".into(),
                    status: "PDNG".into(),
                    credit_debit_indicator: "DBIT".into(),
                    ..Default::default()
                },
                TransactionRecord {
                    transaction_id: "t2".into(),
                    amount: "20.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-27".into(),
                    status: "BOOK".into(),
                    credit_debit_indicator: "DBIT".into(),
                    ..Default::default()
                },
            ],
        )
        .await
        .unwrap();

    let got = store
        .get_transactions(
            "acc-001",
            &QueryOpts {
                status: Some("PDNG".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].transaction_id, "t1");
    assert_eq!(got[0].status, "PDNG");

    let got = store
        .get_transactions(
            "acc-001",
            &QueryOpts {
                status: Some("BOOK".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].transaction_id, "t2");

    let got = store
        .get_transactions("acc-001", &QueryOpts::default())
        .await
        .unwrap();
    assert_eq!(got.len(), 2);
}

#[tokio::test]
async fn test_get_all_transactions() {
    let store = open_test_store();

    let txns = store.get_all_transactions().await.unwrap();
    assert!(txns.is_empty());

    store
        .upsert_account(
            "BANK_A",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .upsert_account(
            "BANK_B",
            &AccountRecord {
                uid: "acc-002".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    store
        .upsert_transactions(
            "acc-001",
            &[TransactionRecord {
                transaction_id: "t1".into(),
                amount: "10.00".into(),
                currency: "EUR".into(),
                booking_date: "2026-03-27".into(),
                creditor_name: "Shop A".into(),
                ..Default::default()
            }],
        )
        .await
        .unwrap();
    store
        .upsert_transactions(
            "acc-002",
            &[TransactionRecord {
                transaction_id: "t2".into(),
                amount: "20.00".into(),
                currency: "EUR".into(),
                booking_date: "2026-03-26".into(),
                creditor_name: "Shop B".into(),
                ..Default::default()
            }],
        )
        .await
        .unwrap();

    let txns = store.get_all_transactions().await.unwrap();
    assert_eq!(txns.len(), 2);
    for tx in &txns {
        assert_eq!(tx.category, "uncategorized");
        assert_eq!(tx.category_source, "");
    }
}
#[tokio::test]
async fn test_upsert_preserves_category() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let txn = TransactionRecord {
        transaction_id: "t1".into(),
        amount: "10.00".into(),
        currency: "EUR".into(),
        booking_date: "2026-03-27".into(),
        ..Default::default()
    };
    store
        .upsert_transactions("acc-001", std::slice::from_ref(&txn))
        .await
        .unwrap();
    store
        .update_category("acc-001", "t1", "groceries", "manual")
        .await
        .unwrap();

    let txn2 = TransactionRecord {
        amount: "15.00".into(),
        ..txn
    };
    store.upsert_transactions("acc-001", &[txn2]).await.unwrap();

    let txns = store
        .get_transactions("acc-001", &QueryOpts::default())
        .await
        .unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].amount, "15.00");
    assert_eq!(txns[0].category, "groceries");
    assert_eq!(txns[0].category_source, "manual");
}

#[tokio::test]
async fn test_upsert_preserves_note() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let txn = TransactionRecord {
        transaction_id: "t1".into(),
        amount: "10.00".into(),
        currency: "EUR".into(),
        status: "PDNG".into(),
        note: "user note".into(),
        ..Default::default()
    };
    store
        .upsert_transactions("acc-001", std::slice::from_ref(&txn))
        .await
        .unwrap();

    let refreshed = TransactionRecord {
        amount: "11.00".into(),
        status: "BOOK".into(),
        note: "api note".into(),
        ..txn
    };
    store
        .upsert_transactions("acc-001", &[refreshed])
        .await
        .unwrap();

    let txns = store
        .get_transactions("acc-001", &QueryOpts::default())
        .await
        .unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].amount, "11.00");
    assert_eq!(txns[0].status, "BOOK");
    assert_eq!(txns[0].note, "user note");
}
