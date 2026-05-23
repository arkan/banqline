use super::*;
use chrono::TimeZone;

fn open_test_store() -> SqliteStore {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    // Keep tempdir alive by leaking it — tests are short-lived anyway.
    std::mem::forget(dir);
    SqliteStore::open(path.to_str().unwrap()).unwrap()
}

#[test]
fn test_open_creates_schema() {
    let store = open_test_store();
    let conn = store.conn.lock().unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"schema_version".to_string()));
    assert!(tables.contains(&"accounts".to_string()));
    assert!(tables.contains(&"balances".to_string()));
    assert!(tables.contains(&"transactions".to_string()));
    assert!(tables.contains(&"sync_meta".to_string()));
    assert!(tables.contains(&"app_metadata".to_string()));
}

#[test]
fn test_open_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let path_str = path.to_str().unwrap();

    let _s1 = SqliteStore::open(path_str).unwrap();
    let _s2 = SqliteStore::open(path_str).unwrap();
}

#[tokio::test]
async fn test_upsert_account() {
    let store = open_test_store();
    let acct = AccountRecord {
        uid: "acc-001".into(),
        iban: "FR7612345".into(),
        name: "Compte Courant".into(),
        currency: "EUR".into(),
        details: "Main account".into(),
        usage_type: "PRIV".into(),
        account_type: "CACC".into(),
        bank_name: String::new(),
        alias: String::new(),
    };
    store.upsert_account("MY_BANK", &acct).await.unwrap();

    let accounts = store.get_accounts("MY_BANK").await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].uid, "acc-001");
    assert_eq!(accounts[0].bank_name, "MY_BANK");
    assert_eq!(accounts[0].iban, "FR7612345");
    assert_eq!(accounts[0].name, "Compte Courant");
    assert_eq!(accounts[0].currency, "EUR");
}

#[tokio::test]
async fn test_upsert_account_update() {
    let store = open_test_store();
    let acct = AccountRecord {
        uid: "acc-001".into(),
        iban: "FR76OLD".into(),
        name: "Old Name".into(),
        currency: "EUR".into(),
        ..Default::default()
    };
    store.upsert_account("MY_BANK", &acct).await.unwrap();

    let acct2 = AccountRecord {
        uid: "acc-001".into(),
        iban: "FR76NEW".into(),
        name: "New Name".into(),
        currency: "EUR".into(),
        ..Default::default()
    };
    store.upsert_account("MY_BANK", &acct2).await.unwrap();

    let accounts = store.get_accounts("MY_BANK").await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].iban, "FR76NEW");
    assert_eq!(accounts[0].name, "New Name");
}

#[tokio::test]
async fn test_get_accounts_filter_by_bank() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK_A",
            &AccountRecord {
                uid: "a1".into(),
                iban: "FR76A".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .upsert_account(
            "BANK_B",
            &AccountRecord {
                uid: "b1".into(),
                iban: "FR76B".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let accounts = store.get_accounts("BANK_A").await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].uid, "a1");
}

#[tokio::test]
async fn test_replace_balances() {
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

    let fetched_at = Utc.with_ymd_and_hms(2026, 3, 27, 10, 0, 0).unwrap();
    let balances = vec![BalanceRecord {
        balance_type: "CLBD".into(),
        amount: "1234.56".into(),
        currency: "EUR".into(),
        reference_date: "2026-03-27".into(),
    }];
    store
        .replace_balances("acc-001", &balances, fetched_at)
        .await
        .unwrap();

    let (got, got_time) = store.get_balances("acc-001").await.unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].balance_type, "CLBD");
    assert_eq!(got[0].amount, "1234.56");
    assert_eq!(got[0].currency, "EUR");
    assert_eq!(got_time, fetched_at);
}

#[tokio::test]
async fn test_replace_balances_overwrite() {
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

    let t1 = Utc.with_ymd_and_hms(2026, 3, 26, 10, 0, 0).unwrap();
    store
        .replace_balances(
            "acc-001",
            &[BalanceRecord {
                balance_type: "CLBD".into(),
                amount: "100.00".into(),
                currency: "EUR".into(),
                reference_date: String::new(),
            }],
            t1,
        )
        .await
        .unwrap();

    let t2 = Utc.with_ymd_and_hms(2026, 3, 27, 10, 0, 0).unwrap();
    store
        .replace_balances(
            "acc-001",
            &[BalanceRecord {
                balance_type: "ITBD".into(),
                amount: "200.00".into(),
                currency: "EUR".into(),
                reference_date: String::new(),
            }],
            t2,
        )
        .await
        .unwrap();

    let (got, got_time) = store.get_balances("acc-001").await.unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].balance_type, "ITBD");
    assert_eq!(got[0].amount, "200.00");
    assert_eq!(got_time, t2);
}

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
async fn test_update_categories() {
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
                    ..Default::default()
                },
                TransactionRecord {
                    transaction_id: "t2".into(),
                    amount: "20.00".into(),
                    currency: "EUR".into(),
                    booking_date: "2026-03-26".into(),
                    ..Default::default()
                },
            ],
        )
        .await
        .unwrap();

    let updates = vec![
        CategoryUpdate {
            account_uid: "acc-001".into(),
            transaction_id: "t1".into(),
            category: "groceries".into(),
            source: "auto".into(),
        },
        CategoryUpdate {
            account_uid: "acc-001".into(),
            transaction_id: "t2".into(),
            category: "transport".into(),
            source: "auto".into(),
        },
    ];
    store.update_categories(&updates).await.unwrap();

    let txns = store.get_all_transactions().await.unwrap();
    assert_eq!(txns.len(), 2);

    let cat_map: std::collections::HashMap<&str, &str> = txns
        .iter()
        .map(|t| (t.transaction_id.as_str(), t.category.as_str()))
        .collect();
    assert_eq!(cat_map["t1"], "groceries");
    assert_eq!(cat_map["t2"], "transport");

    let src_map: std::collections::HashMap<&str, &str> = txns
        .iter()
        .map(|t| (t.transaction_id.as_str(), t.category_source.as_str()))
        .collect();
    assert_eq!(src_map["t1"], "auto");
    assert_eq!(src_map["t2"], "auto");
}

#[tokio::test]
async fn test_update_category() {
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
            &[TransactionRecord {
                transaction_id: "t1".into(),
                amount: "10.00".into(),
                currency: "EUR".into(),
                booking_date: "2026-03-27".into(),
                ..Default::default()
            }],
        )
        .await
        .unwrap();

    store
        .update_category("acc-001", "t1", "dining", "manual")
        .await
        .unwrap();

    let txns = store
        .get_transactions("acc-001", &QueryOpts::default())
        .await
        .unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].category, "dining");
    assert_eq!(txns[0].category_source, "manual");
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

#[tokio::test]
async fn test_metadata() {
    let store = open_test_store();

    let val = store.get_metadata("alert_last_checked").await.unwrap();
    assert_eq!(val, None);

    store
        .set_metadata("alert_last_checked", "2026-03-27T12:00:00Z")
        .await
        .unwrap();
    let val = store.get_metadata("alert_last_checked").await.unwrap();
    assert_eq!(val, Some("2026-03-27T12:00:00Z".into()));

    store
        .set_metadata("alert_last_checked", "2026-03-28T08:00:00Z")
        .await
        .unwrap();
    let val = store.get_metadata("alert_last_checked").await.unwrap();
    assert_eq!(val, Some("2026-03-28T08:00:00Z".into()));
}

#[tokio::test]
async fn test_set_account_alias() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                iban: "FR7612345678901234565133".into(),
                name: "Compte Courant".into(),
                currency: "EUR".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    store
        .set_account_alias("acc-001", "principal")
        .await
        .unwrap();

    let accounts = store.get_accounts("BANK").await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].alias, "principal");
}

#[tokio::test]
async fn test_get_account_by_alias() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                iban: "FR7612345678901234565133".into(),
                currency: "EUR".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .set_account_alias("acc-001", "principal")
        .await
        .unwrap();

    let acct = store.get_account_by_alias("principal").await.unwrap();
    assert!(acct.is_some());
    let acct = acct.unwrap();
    assert_eq!(acct.uid, "acc-001");
    assert_eq!(acct.iban, "FR7612345678901234565133");
    assert_eq!(acct.alias, "principal");
}

#[tokio::test]
async fn test_set_account_alias_duplicate() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK_A",
            &AccountRecord {
                uid: "acc-001".into(),
                iban: "FR76A".into(),
                currency: "EUR".into(),
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
                iban: "FR76B".into(),
                currency: "EUR".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    store
        .set_account_alias("acc-001", "principal")
        .await
        .unwrap();
    let err = store
        .set_account_alias("acc-002", "principal")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("already in use"));
}

#[tokio::test]
async fn test_clear_account_alias() {
    let store = open_test_store();
    store
        .upsert_account(
            "BANK",
            &AccountRecord {
                uid: "acc-001".into(),
                iban: "FR76A".into(),
                currency: "EUR".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .set_account_alias("acc-001", "principal")
        .await
        .unwrap();

    store.clear_account_alias("principal").await.unwrap();

    let acct = store.get_account_by_alias("principal").await.unwrap();
    assert!(acct.is_none());

    let accounts = store.get_accounts("BANK").await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].alias, "");
}

#[tokio::test]
async fn test_upsert_account_preserves_alias() {
    let store = open_test_store();
    let acct = AccountRecord {
        uid: "acc-001".into(),
        iban: "FR76OLD".into(),
        name: "Old".into(),
        currency: "EUR".into(),
        ..Default::default()
    };
    store.upsert_account("BANK", &acct).await.unwrap();
    store
        .set_account_alias("acc-001", "principal")
        .await
        .unwrap();

    let acct2 = AccountRecord {
        uid: "acc-001".into(),
        iban: "FR76NEW".into(),
        name: "New".into(),
        currency: "EUR".into(),
        ..Default::default()
    };
    store.upsert_account("BANK", &acct2).await.unwrap();

    let accounts = store.get_accounts("BANK").await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].iban, "FR76NEW");
    assert_eq!(accounts[0].alias, "principal");
}

#[tokio::test]
async fn test_get_account_by_alias_not_found() {
    let store = open_test_store();
    let acct = store.get_account_by_alias("nonexistent").await.unwrap();
    assert!(acct.is_none());
}
