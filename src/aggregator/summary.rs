use chrono::{Datelike, Duration, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day = 1,
    Week = 2,
    Month = 3,
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Period::Day => write!(f, "day"),
            Period::Week => write!(f, "week"),
            Period::Month => write!(f, "month"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SummaryInput {
    pub booking_date: String,
    pub amount: String,
    pub currency: String,
    pub credit_debit_indicator: String,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct SummaryOpts {
    pub period: Period,
    pub compare: bool,
}

#[derive(Debug, Clone)]
pub struct SummaryResult {
    pub periods: Vec<PeriodSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodSummary {
    pub key: String,
    pub currency: String,
    pub total: Decimal,
    pub categories: Vec<CategoryAmount>,
    pub delta: Option<DeltaInfo>,
    pub coverage: CoverageInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryAmount {
    pub name: String,
    pub amount: Decimal,
    pub delta: Option<DeltaInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeltaInfo {
    pub absolute_diff: Decimal,
    pub percentage: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageInfo {
    pub categorized: i32,
    pub total: i32,
}

impl CoverageInfo {
    pub fn percentage(&self) -> i32 {
        if self.total == 0 {
            0
        } else {
            self.categorized * 100 / self.total
        }
    }

    pub fn display(&self) -> String {
        format!(
            "{}% of transactions categorized ({}/{})",
            self.percentage(),
            self.categorized,
            self.total
        )
    }
}

struct CategoryAccum {
    amount: Decimal,
    count: i32,
}

/// Parse a string into a Period. Accepts "day", "week", or "month".
pub fn parse_period(s: &str) -> anyhow::Result<Period> {
    match s {
        "day" => Ok(Period::Day),
        "week" => Ok(Period::Week),
        "month" => Ok(Period::Month),
        _ => anyhow::bail!("unknown period {s:?}: must be day, week, or month"),
    }
}

/// Compute spending summaries from transaction inputs.
/// Only DBIT transactions are counted as spending.
/// Errors (invalid dates, invalid amounts) are collected as warnings.
pub fn summarize(inputs: &[SummaryInput], opts: &SummaryOpts) -> SummaryResult {
    if inputs.is_empty() {
        return SummaryResult {
            periods: Vec::new(),
            warnings: Vec::new(),
        };
    }

    let mut warnings: Vec<String> = Vec::new();

    struct BucketData {
        total: Decimal,
        categories: HashMap<String, CategoryAccum>,
        total_count: i32,
        cat_count: i32,
    }

    let mut buckets: HashMap<(String, String), BucketData> = HashMap::new();
    let mut bucket_order: Vec<(String, String)> = Vec::new();

    for inp in inputs {
        if inp.credit_debit_indicator != "DBIT" {
            continue;
        }

        let key = match bucket_key(&inp.booking_date, opts.period) {
            Ok(k) => k,
            Err(e) => {
                warnings.push(format!(
                    "skipping transaction with date {:?}: {e}",
                    inp.booking_date
                ));
                continue;
            }
        };

        let amt = match Decimal::from_str(&inp.amount) {
            Ok(v) => v,
            Err(_) => {
                warnings.push(format!(
                    "skipping transaction with invalid amount {:?}",
                    inp.amount
                ));
                continue;
            }
        };

        let bk = (inp.currency.clone(), key);
        let bd = buckets.entry(bk.clone()).or_insert_with(|| {
            bucket_order.push(bk.clone());
            BucketData {
                total: Decimal::ZERO,
                categories: HashMap::new(),
                total_count: 0,
                cat_count: 0,
            }
        });

        bd.total += amt;
        bd.total_count += 1;

        let cat = if inp.category.is_empty() {
            "uncategorized".to_string()
        } else {
            inp.category.clone()
        };

        if cat != "uncategorized" {
            bd.cat_count += 1;
        }

        let ca = bd.categories.entry(cat).or_insert(CategoryAccum {
            amount: Decimal::ZERO,
            count: 0,
        });
        ca.amount += amt;
        ca.count += 1;
    }

    bucket_order.sort_by(|a, b| match a.0.cmp(&b.0) {
        std::cmp::Ordering::Equal => a.1.cmp(&b.1),
        other => other,
    });

    let mut periods: Vec<PeriodSummary> = Vec::with_capacity(bucket_order.len());
    for (currency, key) in &bucket_order {
        let bd = buckets.get(&(currency.clone(), key.clone())).unwrap();

        let mut cats: Vec<CategoryAmount> = bd
            .categories
            .iter()
            .map(|(name, ca)| CategoryAmount {
                name: name.clone(),
                amount: ca.amount,
                delta: None,
            })
            .collect();

        cats.sort_by(|a, b| match (a.name.as_str(), b.name.as_str()) {
            ("uncategorized", _) => std::cmp::Ordering::Greater,
            (_, "uncategorized") => std::cmp::Ordering::Less,
            _ => b.amount.cmp(&a.amount),
        });

        periods.push(PeriodSummary {
            key: key.clone(),
            currency: currency.clone(),
            total: bd.total,
            categories: cats,
            delta: None,
            coverage: CoverageInfo {
                categorized: bd.cat_count,
                total: bd.total_count,
            },
        });
    }

    if opts.compare && periods.len() > 1 {
        let period_map: HashMap<(String, String), usize> = periods
            .iter()
            .enumerate()
            .map(|(i, ps)| ((ps.currency.clone(), ps.key.clone()), i))
            .collect();

        for i in 0..periods.len() {
            let prior_key = match prior_period_key(&periods[i].key, opts.period) {
                Ok(k) => k,
                Err(_) => continue,
            };

            let lookup = (periods[i].currency.clone(), prior_key);
            let prior_idx = match period_map.get(&lookup) {
                Some(&idx) => idx,
                None => continue,
            };

            let prior_total = periods[prior_idx].total;
            let prior_categories: Vec<(String, Decimal)> = periods[prior_idx]
                .categories
                .iter()
                .map(|c| (c.name.clone(), c.amount))
                .collect();

            periods[i].delta = Some(compute_delta(periods[i].total, prior_total));

            for cat in &mut periods[i].categories {
                let prior_amt = prior_categories
                    .iter()
                    .find(|(name, _)| name == &cat.name)
                    .map(|(_, amt)| *amt)
                    .unwrap_or(Decimal::ZERO);
                cat.delta = Some(compute_delta(cat.amount, prior_amt));
            }
        }
    }

    SummaryResult { periods, warnings }
}

pub fn bucket_key(date: &str, period: Period) -> anyhow::Result<String> {
    if date.is_empty() {
        anyhow::bail!("empty date");
    }

    let t = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("parse date {date:?}: {e}"))?;

    match period {
        Period::Day => Ok(date.to_string()),
        Period::Month => Ok(t.format("%Y-%m").to_string()),
        Period::Week => {
            let iso = t.iso_week();
            Ok(format!("{:04}-W{:02}", iso.year(), iso.week()))
        }
    }
}

fn prior_period_key(key: &str, period: Period) -> anyhow::Result<String> {
    match period {
        Period::Day => {
            let t = NaiveDate::parse_from_str(key, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("parse date {key:?}: {e}"))?;
            Ok((t - Duration::days(1)).format("%Y-%m-%d").to_string())
        }
        Period::Month => {
            let t = NaiveDate::parse_from_str(&format!("{key}-01"), "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("parse month {key:?}: {e}"))?;
            let prior = t
                .pred_opt()
                .ok_or_else(|| anyhow::anyhow!("cannot compute prior month for {key:?}"))?;
            Ok(prior.format("%Y-%m").to_string())
        }
        Period::Week => {
            let (year_str, week_str) = key
                .split_once("-W")
                .ok_or_else(|| anyhow::anyhow!("invalid week key format: {key:?}"))?;
            let year: i32 = year_str
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid year in week key: {key:?}"))?;
            let week: u32 = week_str
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid week in week key: {key:?}"))?;
            let monday = iso_week_start(year, week);
            let prior = monday - Duration::days(7);
            let prior_iso = prior.iso_week();
            Ok(format!("{:04}-W{:02}", prior_iso.year(), prior_iso.week()))
        }
    }
}

/// Returns the Monday of the given ISO year/week (Jan 4 method).
fn iso_week_start(year: i32, week: u32) -> NaiveDate {
    let jan4 = NaiveDate::from_ymd_opt(year, 1, 4).unwrap();
    let days_from_monday = jan4.weekday().num_days_from_monday() as i64;
    let monday_of_week1 = jan4 - Duration::days(days_from_monday);
    monday_of_week1 + Duration::days(((week as i64) - 1) * 7)
}

fn compute_delta(current: Decimal, prior: Decimal) -> DeltaInfo {
    let diff = current - prior;

    let percentage = if prior.is_zero() && current.is_zero() {
        String::new()
    } else if prior.is_zero() {
        "new".to_string()
    } else {
        let ratio = (diff / prior) * Decimal::from(100);
        let rounded = ratio.round_dp(0).to_i32().unwrap_or(0);
        if rounded > 0 {
            format!("+{rounded}%")
        } else {
            format!("{rounded}%")
        }
    };

    DeltaInfo {
        absolute_diff: diff,
        percentage,
    }
}
