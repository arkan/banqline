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

    #[test]
    fn prepare_transaction_output_sorts_globally_before_limiting() {
        let mut txns = vec![
            txn("acc-a", "old", "2026-05-01", "", "", "DBIT", "food"),
            txn("acc-b", "new", "2026-05-08", "", "", "DBIT", "food"),
        ];

        prepare_transaction_output(&mut txns, None, None, 1);

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

        prepare_transaction_output(&mut txns, Some("debit"), Some("food"), 1);

        assert_eq!(txns.len(), 1);
        assert_eq!(txns[0].transaction_id, "old-debit");
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

        prepare_transaction_output(&mut txns, None, None, 2);

        assert_eq!(txns[0].transaction_id, "value-date");
        assert_eq!(txns[1].transaction_id, "transaction-date");
    }
}
