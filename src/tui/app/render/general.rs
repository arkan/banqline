use super::*;

impl App {
    pub(super) fn render_general(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::raw(acct.iban.clone()),
            Span::raw(" — "),
            Span::styled(acct.bank_name.clone(), Style::default().fg(Color::Cyan)),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "═══ Balances ═══",
            Style::default().fg(Color::Yellow),
        )));
        if let Some(bals) = self.balances.get(&acct.uid) {
            for b in bals {
                lines.push(Line::from(Span::raw(format!(
                    "{}: {} {}",
                    balance_type_name(&b.balance_type),
                    b.amount,
                    b.currency
                ))));
            }
        } else {
            lines.push(Line::from("No balances available"));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "═══ Forecast ═══",
            Style::default().fg(Color::Yellow),
        )));
        if let Some(fc) = self.forecasts.get(&acct.uid) {
            lines.push(Line::from(Span::raw(format!(
                "Booked: {}  |  Pending: {}  |  Projected: {}  {}",
                fc.booked_balance.round_dp(2),
                format_pending(&fc.pending_delta, fc.has_pending_data),
                fc.projected_balance.round_dp(2),
                fc.currency
            ))));
            let spark_data = self.build_sparkline_data(fc);
            if !spark_data.is_empty() {
                let spark = Sparkline::default()
                    .data(&spark_data)
                    .max(5)
                    .style(Style::default().fg(Color::Cyan));
                let spark_rect = Rect {
                    x: area.x + 1,
                    y: area.y + lines.len() as u16,
                    width: (area.width - 2).min(60),
                    height: 1,
                };
                f.render_widget(spark, spark_rect);
                lines.push(Line::from(""));
            }
        } else {
            lines.push(Line::from("No forecast data"));
        }
        lines.push(Line::from(""));
        let usage = if acct.usage_type.is_empty() {
            "N/A"
        } else {
            &acct.usage_type
        };
        lines.push(Line::from(Span::raw(format!(
            "Usage: {}  ·  Currency: {}",
            usage, acct.currency
        ))));
        f.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            area,
        );
    }

    // ---------- Pending tab ----------

    pub(super) fn render_pending(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let pending: Vec<&store::TransactionRecord> = self
            .all_transactions
            .get(&acct.uid)
            .map(|t| t.iter().filter(|tx| tx.status == "PDNG").collect())
            .unwrap_or_default();
        if pending.is_empty() {
            f.render_widget(Paragraph::new("No pending transactions."), area);
            return;
        }
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "Date        ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("Amount    ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("Description", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from("─".repeat(60)));
        let mut total = Decimal::ZERO;
        let mut curr = String::new();
        for tx in pending.iter().skip(self.pending_scroll) {
            let desc = if !tx.note.is_empty() {
                format!("📝 {}", tx.note)
            } else if !tx.remittance_info.is_empty() {
                tx.remittance_info.join(" ")
            } else if !tx.creditor_name.is_empty() {
                tx.creditor_name.clone()
            } else {
                tx.debtor_name.clone()
            };
            let amount: Decimal = tx.amount.parse().unwrap_or(Decimal::ZERO);
            let sign = if tx.credit_debit_indicator == "CRDT" {
                "+"
            } else {
                "-"
            };
            let signed = if tx.credit_debit_indicator == "DBIT" {
                -amount
            } else {
                amount
            };
            total += signed;
            curr = tx.currency.clone();
            let color = if tx.credit_debit_indicator == "CRDT" {
                Color::Green
            } else {
                Color::Red
            };
            lines.push(Line::from(vec![
                Span::raw(format!("{}  ", tx.value_date)),
                Span::styled(
                    format!("{:>7}{}", sign, amount.round_dp(2)),
                    Style::default().fg(color),
                ),
                Span::raw(format!("  {}", desc)),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Total pending: {} {}", total.round_dp(2), curr),
            Style::default().fg(if total.is_sign_negative() {
                Color::Red
            } else {
                Color::Green
            }),
        )));
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .block(Block::default().borders(Borders::ALL).title(" Pending ")),
            area,
        );
    }
}

fn format_pending(delta: &Decimal, has_data: bool) -> String {
    if !has_data {
        "(no data)".into()
    } else if delta.is_zero() {
        "0.00".into()
    } else if delta.is_sign_positive() {
        format!("+{}", delta.round_dp(2))
    } else {
        format!("{}", delta.round_dp(2))
    }
}
