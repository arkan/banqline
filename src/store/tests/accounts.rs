use super::*;

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
