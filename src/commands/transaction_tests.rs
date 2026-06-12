use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn txn(
        account_uid: &str,
        transaction_id: &str,
        booking_date: &str,
        value_date: &str,
        transaction_date: &str,
        direction: &str,
        category: &str,
    ) -> store::TransactionRecord {
        store::TransactionRecord {
            account_uid: account_uid.into(),
            transaction_id: transaction_id.into(),
            booking_date: booking_date.into(),
            value_date: value_date.into(),
            transaction_date: transaction_date.into(),
            credit_debit_indicator: direction.into(),
            category: category.into(),
            ..Default::default()
        }
    }

    fn session_with_account(uid: &str) -> session::StoredSession {
        session::StoredSession {
            session_id: "session".into(),
            accounts: vec![session::StoredAccount {
                uid: uid.into(),
                iban: "LOCALIBAN123456".into(),
                name: "Local account".into(),
                currency: "EUR".into(),
            }],
            created_at: Utc::now(),
            valid_until: Utc::now() + Duration::days(1),
        }
    }

    #[test]
    fn prepare_transaction_output_sorts_globally_before_limiting() {
        let mut txns = vec![
            txn("acc-a", "old", "2026-05-01", "", "", "DBIT", "food"),
            txn("acc-b", "new", "2026-05-08", "", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, None, None, Some(1));

        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].transaction_id, "new");
    }

    #[test]
    fn prepare_transaction_output_filters_before_limiting() {
        let mut txns = vec![
            txn(
                "acc-a",
                "new-credit",
                "2026-05-08",
                "",
                "",
                "CRDT",
                "salary",
            ),
            txn("acc-a", "old-debit", "2026-05-01", "", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, Some("debit"), Some("food"), Some(1));

        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].transaction_id, "old-debit");
    }

    #[test]
    fn prepare_transaction_output_removes_pending() {
        let mut txns = vec![
            txn("acc-a", "booked", "2026-05-01", "", "", "DBIT", "food"),
            store::TransactionRecord {
                status: "PDNG".into(),
                account_uid: "acc-a".into(),
                transaction_id: "pending-tx".into(),
                booking_date: "2026-05-02".into(),
                credit_debit_indicator: "DBIT".into(),
                category: "food".into(),
                ..Default::default()
            },
        ];

        prepare_transaction_output(&mut txns, None, None, None);

        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].transaction_id, "booked");
    }

    #[test]
    fn prepare_transaction_output_none_limit_leaves_all() {
        let mut txns = vec![
            txn("acc-a", "t1", "2026-05-01", "", "", "DBIT", "food"),
            txn("acc-a", "t2", "2026-05-02", "", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, None, None, None);

        assert_eq!(txns.len(), 2);
    }

    #[test]
    fn prepare_transaction_output_zero_limit_leaves_all() {
        let mut txns = vec![
            txn("acc-a", "t1", "2026-05-01", "", "", "DBIT", "food"),
            txn("acc-a", "t2", "2026-05-02", "", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, None, None, Some(0));

        assert_eq!(txns.len(), 2);
    }

    #[test]
    fn prepare_transaction_output_removes_pending_case_insensitive() {
        let mut txns = vec![
            txn("acc-a", "booked", "2026-05-01", "", "", "DBIT", "food"),
            store::TransactionRecord {
                status: "pdng".into(),
                account_uid: "acc-a".into(),
                transaction_id: "pending-lower".into(),
                booking_date: "2026-05-02".into(),
                credit_debit_indicator: "DBIT".into(),
                category: "food".into(),
                ..Default::default()
            },
        ];

        prepare_transaction_output(&mut txns, None, None, None);

        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].transaction_id, "booked");
    }

    #[test]
    fn prepare_transaction_output_uses_value_and_transaction_date_fallbacks() {
        let mut txns = vec![
            txn(
                "acc-a",
                "transaction-date",
                "",
                "",
                "2026-05-07",
                "DBIT",
                "food",
            ),
            txn("acc-a", "value-date", "", "2026-05-08", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, None, None, Some(2));

        assert_eq!(txns[0].transaction_id, "value-date");
        assert_eq!(txns[1].transaction_id, "transaction-date");
    }

    #[tokio::test]
    async fn resolve_account_rejects_alias_outside_selected_bank_session() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = store::SqliteStore::open(file.path().to_str().unwrap()).unwrap();
        db.upsert_account(
            "OTHER_BANK",
            &store::AccountRecord {
                uid: "foreign-account".into(),
                iban: "FOREIGNIBAN123456".into(),
                name: "Foreign account".into(),
                currency: "EUR".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        db.set_account_alias("foreign-account", "shared-alias")
            .await
            .unwrap();
        let sess = session_with_account("local-account");

        let err = resolve_account(&sess, Some(&db), Some("shared-alias"))
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("not found in session"),
            "unexpected error: {err:#}"
        );
    }
}
