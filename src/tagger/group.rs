use std::collections::BTreeSet;

/// A pre-normalized transaction for grouping.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NormalizedTransaction {
    pub description: String,
    pub amount: String,
    pub currency: String,
    pub booking_date: String,
    pub credit_debit_indicator: String,
    pub creditor_name: String,
    pub debtor_name: String,
}

/// Holds display info for a sample transaction in a group.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransactionSample {
    pub description: String,
    pub amount: String,
    pub currency: String,
    pub booking_date: String,
    pub credit_debit_indicator: String,
    pub creditor_name: String,
    pub debtor_name: String,
}

/// Represents a group of similar uncategorized transactions.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TransactionGroup {
    pub key: String,
    pub count: usize,
    pub samples: Vec<TransactionSample>,
    pub suggested_pattern: String,
    pub total_amount: f64,
    pub average_amount: f64,
}

#[allow(dead_code)]
struct Accumulator {
    count: usize,
    samples: Vec<TransactionSample>,
    total_amount: f64,
}

/// Groups normalized transactions by their exact normalized description.
/// Returns groups sorted by count descending.
#[allow(dead_code)]
pub fn group_by_description(txns: &[NormalizedTransaction]) -> Vec<TransactionGroup> {
    if txns.is_empty() {
        return Vec::new();
    }

    let mut grouped: std::collections::BTreeMap<usize, Accumulator> =
        std::collections::BTreeMap::new();
    let mut key_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for tx in txns {
        let key = tx.description.trim().to_uppercase();
        let key = if key.is_empty() {
            "UNKNOWN".to_string()
        } else {
            key
        };

        let idx = match key_index.get(&key) {
            Some(&i) => i,
            None => {
                let i = key_index.len();
                key_index.insert(key.clone(), i);
                order.push(key.clone());
                grouped.insert(
                    i,
                    Accumulator {
                        count: 0,
                        samples: Vec::new(),
                        total_amount: 0.0,
                    },
                );
                i
            }
        };

        if let Some(acc) = grouped.get_mut(&idx) {
            acc.count += 1;
            if let Ok(v) = tx.amount.parse::<f64>() {
                acc.total_amount += v.abs();
            }
            if acc.samples.len() < 3 {
                acc.samples.push(TransactionSample {
                    description: tx.description.clone(),
                    amount: tx.amount.clone(),
                    currency: tx.currency.clone(),
                    booking_date: tx.booking_date.clone(),
                    credit_debit_indicator: tx.credit_debit_indicator.clone(),
                    creditor_name: tx.creditor_name.clone(),
                    debtor_name: tx.debtor_name.clone(),
                });
            }
        }
    }

    let mut groups: Vec<TransactionGroup> = order
        .iter()
        .filter_map(|key| {
            let idx = key_index.get(key)?;
            let acc = grouped.get(idx)?;
            let avg = if acc.count > 0 {
                acc.total_amount / acc.count as f64
            } else {
                0.0
            };
            Some(TransactionGroup {
                key: key.clone(),
                count: acc.count,
                samples: acc.samples.clone(),
                suggested_pattern: suggest_pattern(key),
                total_amount: acc.total_amount,
                average_amount: avg,
            })
        })
        .collect();

    groups.sort_by(|a, b| b.count.cmp(&a.count));
    groups
}

/// Extracts the most distinctive word from a description key
/// to use as a default tag pattern. Skips common banking prefixes.
#[allow(dead_code)]
fn suggest_pattern(key: &str) -> String {
    let skip: BTreeSet<&str> = [
        "PAIEMENT",
        "CB",
        "VIR",
        "VIREMENT",
        "PRELEVEMENT",
        "PRLV",
        "CARTE",
        "PAR",
        "DE",
        "DU",
        "LE",
        "LA",
        "AU",
    ]
    .iter()
    .copied()
    .collect();

    for word in key.split_whitespace() {
        if !skip.contains(word) && word.len() >= 3 {
            return word.to_lowercase();
        }
    }
    key.to_lowercase()
}
