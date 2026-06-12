use super::*;

impl App {
    pub(super) fn render_transactions(
        &mut self,
        f: &mut Frame,
        area: Rect,
        acct: &store::AccountRecord,
    ) {
        let all: Vec<store::TransactionRecord> = self
            .all_transactions
            .get(&acct.uid)
            .cloned()
            .unwrap_or_default();
        let txns = super::super::txn_view::visible_transactions(&all, &self.txn_filter);
        let inner = area.inner(Margin::new(1, 1));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        let filter_text = if self.search_open {
            format!("/{}█", self.search_input)
        } else if !self.txn_filter.search.is_empty() {
            format!(
                "Search: \"{}\"  |  f:filter c:cat d:dir  /:search  n:note",
                self.txn_filter.search
            )
        } else if self.txn_filter.active {
            let mut parts = Vec::new();
            if let Some(ref c) = self.txn_filter.category {
                parts.push(format!("cat:{}", c));
            }
            if let Some(ref d) = self.txn_filter.direction {
                parts.push(format!("dir:{}", d));
            }
            format!("Filters: {}", parts.join(" "))
        } else {
            "/:search  f:filter  c:category  d:direction  n:note".to_string()
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                filter_text,
                Style::default().fg(Color::DarkGray),
            )),
            chunks[0],
        );
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "Date        ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("Amount  ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                "Category           ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled("Description", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from("─".repeat(70)));
        let visible_height = (chunks[1].height as usize).saturating_sub(4); // border(2) + header+sep(2)
        self.txn_cursor = self.txn_cursor.min(txns.len().saturating_sub(1));
        // Scroll down: only when cursor moves past the last visible item.
        if self.txn_cursor > self.txn_scroll + visible_height.saturating_sub(1) {
            self.txn_scroll = self
                .txn_cursor
                .saturating_sub(visible_height.saturating_sub(1));
        }
        // Scroll up: when cursor is before the first visible item.
        if self.txn_cursor < self.txn_scroll {
            self.txn_scroll = self.txn_cursor;
        }
        for (i, tx) in txns.iter().skip(self.txn_scroll).enumerate() {
            if i >= visible_height && !self.tag.open && !self.note_open {
                break;
            }
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
            let sign = if tx.credit_debit_indicator == "DBIT" {
                "-"
            } else {
                "+"
            };
            let amt_color = if tx.credit_debit_indicator == "DBIT" {
                Color::Red
            } else {
                Color::Green
            };
            let is_current = self.txn_scroll + i == self.txn_cursor;
            let prefix = if is_current { "▸" } else { " " };
            let cat_color = if tx.category_source == "manual" {
                Color::Magenta
            } else if tx.category == "uncategorized" {
                Color::Yellow
            } else {
                Color::Green
            };
            let selection = if is_current {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            // Pad the selected row to full width.
            let row_width = chunks[1].width as usize - 2; // minus border
            let fixed = format!(
                "{}{}  {}{:<8} {:<18}",
                prefix,
                tx.best_date(),
                sign,
                amount.round_dp(2),
                tx.category
            );
            let desc_width = row_width.saturating_sub(fixed.len());
            let desc_display = format!("{:<width$}", desc, width = desc_width);
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}  ", prefix, tx.best_date()), selection),
                Span::styled(
                    format!("{}{:<8} ", sign, amount.round_dp(2)),
                    Style::default().fg(amt_color).patch(selection),
                ),
                Span::styled(
                    format!("{:<18}", tx.category),
                    Style::default().fg(cat_color).patch(selection),
                ),
                Span::styled(desc_display, selection),
            ]));
        }
        f.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Transactions ({}) ", txns.len())),
            ),
            chunks[1],
        );
        if self.tag.open {
            self.render_tag_popup(f);
        }
        if self.note_open {
            self.render_note_popup(f);
        }
    }
}
