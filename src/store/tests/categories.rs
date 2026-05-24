use super::*;

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
