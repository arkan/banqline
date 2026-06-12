use super::*;

pub(super) fn visible_transactions<'a>(
    all: &'a [store::TransactionRecord],
    filter: &TxnFilter,
) -> Vec<&'a store::TransactionRecord> {
    let mut txns: Vec<&store::TransactionRecord> =
        all.iter().filter(|tx| !tx.is_pending()).collect();
    txns.sort_by(|a, b| store::compare_transactions_for_display(a, b));

    if filter.active {
        if let Some(ref cat) = filter.category {
            txns.retain(|tx| tx.category.eq_ignore_ascii_case(cat));
        }
        if let Some(ref dir) = filter.direction {
            txns.retain(|tx| tx.credit_debit_indicator == *dir);
        }
    }

    if !filter.search.is_empty() {
        let q = filter.search.to_lowercase();
        txns.retain(|tx| searchable_text(tx).to_lowercase().contains(&q));
    }

    txns
}

pub(super) fn selected_transaction<'a>(
    all: &'a [store::TransactionRecord],
    filter: &TxnFilter,
    cursor: usize,
) -> Option<&'a store::TransactionRecord> {
    visible_transactions(all, filter).get(cursor).copied()
}

pub(super) fn description_without_note(tx: &store::TransactionRecord) -> String {
    if !tx.remittance_info.is_empty() {
        tx.remittance_info.join(" ")
    } else if !tx.creditor_name.is_empty() {
        tx.creditor_name.clone()
    } else {
        tx.debtor_name.clone()
    }
}

fn searchable_text(tx: &store::TransactionRecord) -> String {
    format!(
        "{} {} {} {} {} {} {} {}",
        tx.booking_date,
        tx.amount,
        tx.currency,
        description_without_note(tx),
        tx.category,
        tx.creditor_name,
        tx.debtor_name,
        tx.note
    )
}
