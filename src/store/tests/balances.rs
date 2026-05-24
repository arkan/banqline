use super::*;

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
