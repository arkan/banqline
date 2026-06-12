use super::*;

impl App {
    pub(super) fn render_report(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let all = self
            .all_transactions
            .get(&acct.uid)
            .cloned()
            .unwrap_or_default();
        let inputs: Vec<SummaryInput> = all
            .iter()
            .filter(|tx| !tx.is_pending())
            .map(|tx| SummaryInput {
                booking_date: tx.booking_date.clone(),
                amount: tx.amount.clone(),
                currency: tx.currency.clone(),
                credit_debit_indicator: tx.credit_debit_indicator.clone(),
                category: tx.category.clone(),
            })
            .collect();
        let result = summary::summarize(
            &inputs,
            &SummaryOpts {
                period: self.report_period.to_aggregator(),
                compare: self.report_compare,
            },
        );
        let period_name = match self.report_period {
            ReportPeriod::Day => "day",
            ReportPeriod::Week => "week",
            ReportPeriod::Month => "month",
        };

        // Income per period.
        let mut income_by_period: HashMap<String, Decimal> = HashMap::new();
        for inp in &inputs {
            if inp.credit_debit_indicator != "CRDT" {
                continue;
            }
            let key =
                match summary::bucket_key(&inp.booking_date, self.report_period.to_aggregator()) {
                    Ok(k) => k,
                    Err(_) => continue,
                };
            let amt: Decimal = inp.amount.parse().unwrap_or(Decimal::ZERO);
            *income_by_period.entry(key).or_default() += amt;
        }

        let inner = area.inner(Margin::new(1, 1));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        let ctrl = format!(
            "m:month  w:week  D:day  Period: {}  Compare: {}",
            period_name,
            if self.report_compare {
                "on (vs prior)"
            } else {
                "off"
            }
        );
        f.render_widget(
            Paragraph::new(Span::styled(ctrl, Style::default().fg(Color::DarkGray))),
            chunks[0],
        );

        let mut lines: Vec<Line> = Vec::new();
        if result.periods.is_empty() {
            lines.push(Line::from("No spending data found."));
        } else {
            let header = "Category          Amount        %";
            lines.push(Line::from(Span::styled(
                header,
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from("─".repeat(55)));
            for ps in &result.periods {
                let period_label = match self.report_period {
                    ReportPeriod::Day => ps.key.clone(),
                    ReportPeriod::Week => format!("Week {}", ps.key),
                    ReportPeriod::Month => {
                        if let Ok(d) =
                            NaiveDate::parse_from_str(&format!("{}-01", ps.key), "%Y-%m-%d")
                        {
                            d.format("%B %Y").to_string()
                        } else {
                            ps.key.clone()
                        }
                    }
                };
                let currency_info = format!("{} {}", period_label, ps.currency);
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("── {} ──", currency_info),
                    Style::default().fg(Color::Yellow),
                )));
                if let Some(inc) = income_by_period.get(&ps.key) {
                    lines.push(Line::from(Span::styled(
                        format!("  Income            +{:>8}", inc.round_dp(2)),
                        Style::default().fg(Color::Green),
                    )));
                }
                for cat in &ps.categories {
                    let pct = if ps.total.is_zero() {
                        0
                    } else {
                        (cat.amount / ps.total * Decimal::from(100))
                            .round_dp(0)
                            .to_string()
                            .parse::<i64>()
                            .unwrap_or(0)
                    };
                    let bar_len = (pct as usize * 30 / 100).min(30);
                    let bar = "█".repeat(bar_len);
                    lines.push(Line::from(vec![
                        Span::raw(format!(
                            "{:<16} ",
                            cat.name.chars().take(16).collect::<String>()
                        )),
                        Span::styled(
                            format!("-{:>7}", cat.amount.round_dp(2)),
                            Style::default().fg(Color::Red),
                        ),
                        Span::raw(format!(" {:>3}%  {}", pct, bar)),
                    ]));
                }
                lines.push(Line::from("─".repeat(55)));
                lines.push(Line::from(Span::styled(
                    format!("{:<16} -{:>7}", "Spending", ps.total.round_dp(2)),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
                let inc = income_by_period
                    .get(&ps.key)
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                let net = inc - ps.total;
                let net_color = if net.is_sign_negative() {
                    Color::Red
                } else {
                    Color::Green
                };
                let net_sign = if net.is_sign_positive() { "+" } else { "" };
                lines.push(Line::from(Span::styled(
                    format!("{:<16} {}{}", "Net", net_sign, net.round_dp(2)),
                    Style::default().fg(net_color).add_modifier(Modifier::BOLD),
                )));
            }
        }
        if let Some(last) = result.periods.last() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(
                    "Coverage: {}% ({}/{})",
                    last.coverage.percentage(),
                    last.coverage.categorized,
                    last.coverage.total
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
        f.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Spending Summary "),
            ),
            chunks[1],
        );
    }
}
