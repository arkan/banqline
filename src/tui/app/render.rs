use super::*;

impl App {
    pub(super) fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);
        self.render_accounts(f, chunks[0]);
        if self.refresh.open {
            self.render_refresh_modal(f);
        }
        if self.help_open {
            self.render_help_modal(f);
        }
        let status = Span::styled(
            format!(
                " {}  |  ?:help  q:quit  r:refresh  j/k:accounts  ←→:tabs  ↑↓:scroll",
                self.status
            ),
            Style::default().fg(Color::DarkGray),
        );
        f.render_widget(Paragraph::new(Line::from(status)), chunks[1]);
    }

    pub(super) fn render_help_modal(&self, f: &mut Frame) {
        let popup_area = centered_rect(70, 70, f.area());
        f.render_widget(Clear, popup_area);
        let lines = vec![
            Line::from(Span::styled(
                "Banqline TUI shortcuts",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Global"),
            Line::from("  ?         open/close this help"),
            Line::from("  q         quit"),
            Line::from("  r         refresh accounts, balances, transactions, tags and alerts"),
            Line::from("  j / k     switch account"),
            Line::from("  ← / →     switch panel tab"),
            Line::from(""),
            Line::from("Transactions"),
            Line::from("  /         search"),
            Line::from("  f         toggle filters"),
            Line::from("  c / d     cycle category / direction filters"),
            Line::from("  t         tag selected transaction"),
            Line::from("  n         edit note without forcing uppercase"),
            Line::from(""),
            Line::from("Report"),
            Line::from("  m / w / D switch month, week, day"),
            Line::from(""),
            Line::from(Span::styled(
                "Esc or Enter closes this help",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let popup = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(" Help "))
            .style(Style::default().bg(Color::Black))
            .wrap(Wrap { trim: false });
        f.render_widget(popup, popup_area);
    }

    pub(super) fn render_refresh_modal(&self, f: &mut Frame) {
        let popup_area = centered_rect(90, 80, f.area());
        f.render_widget(Clear, popup_area);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            "Refreshing data...",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        let has_error = self
            .refresh
            .steps
            .iter()
            .any(|step| matches!(step.status, RefreshStatus::Error(_)));
        for step in &self.refresh.steps {
            let (icon, color) = match step.status {
                RefreshStatus::Pending => ("○", Color::DarkGray),
                RefreshStatus::Loading => ("⏳", Color::Yellow),
                RefreshStatus::Done => ("✓", Color::Green),
                RefreshStatus::Error(_) => ("✗", Color::Red),
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", icon), Style::default().fg(color)),
                Span::raw(&step.label),
            ]));
            if let RefreshStatus::Error(ref err) = step.status {
                lines.push(Line::from(Span::styled(
                    format!("   {}", err),
                    Style::default().fg(Color::Red),
                )));
            }
        }
        if self.refresh.done {
            let (message, color) = if has_error {
                ("Refresh stopped", Color::Red)
            } else {
                ("Done!", Color::Green)
            };
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                message,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                "Enter / Esc to close",
                Style::default().fg(Color::DarkGray),
            )));
        }
        let popup = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(" Refresh "))
            .style(Style::default().bg(Color::Black))
            .wrap(Wrap { trim: false });
        f.render_widget(popup, popup_area);
    }

    pub(super) fn render_accounts(&mut self, f: &mut Frame, area: Rect) {
        if self.accounts.is_empty() {
            f.render_widget(
                Paragraph::new("No accounts found.\n\nNext steps:\n  1. banqline bank connect --country FR --bank <name>\n  2. banqline tui\n\nPress ? for shortcuts.")
                    .block(Block::default().borders(Borders::ALL).title(" Accounts ")),
                area,
            );
            return;
        }
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(20)])
            .split(area);
        self.render_sidebar(f, chunks[0]);
        self.render_detail(f, chunks[1]);
    }

    pub(super) fn render_sidebar(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .accounts
            .iter()
            .map(|a| {
                let name = if a.alias.is_empty() {
                    a.name.clone()
                } else {
                    a.alias.clone()
                };
                ListItem::new(Line::from(Span::raw(name)))
            })
            .collect();
        let mut list_state = self.account_list_state.clone();
        let list = List::new(items)
            .block(Block::default().borders(Borders::RIGHT).title(" Accounts "))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, area, &mut list_state);
    }

    pub(super) fn render_detail(&mut self, f: &mut Frame, area: Rect) {
        let acct_idx = self.selected_account;
        let acct = match self.accounts.get(acct_idx) {
            Some(a) => a,
            None => return,
        };
        let acct_uid = acct.uid.clone();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(area);
        let display_name = if acct.alias.is_empty() {
            acct.name.clone()
        } else {
            acct.alias.clone()
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                format!("◂ {}", display_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )])),
            chunks[0],
        );
        let pending_count = self
            .all_transactions
            .get(&acct_uid)
            .map(|t| t.iter().filter(|tx| tx.status == "PDNG").count())
            .unwrap_or(0);
        let alert_count = self
            .alert_results
            .get(&acct_uid)
            .map(|r| r.iter().filter(|a| a.status == "TRIGGERED").count())
            .unwrap_or(0);
        let sub_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(chunks[1]);
        let tab_titles: Vec<Line> = DetailTab::all()
            .iter()
            .map(|t| Line::from(Span::raw(t.label(pending_count, alert_count))))
            .collect();
        let tab_bar = Tabs::new(tab_titles)
            .select(
                DetailTab::all()
                    .iter()
                    .position(|t| *t == self.detail_tab)
                    .unwrap_or(0),
            )
            .block(Block::default().borders(Borders::BOTTOM))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::raw("|"));
        f.render_widget(tab_bar, sub_chunks[0]);
        let acct_clone = acct.clone();
        match self.detail_tab {
            DetailTab::General => self.render_general(f, sub_chunks[1], &acct_clone),
            DetailTab::Pending => self.render_pending(f, sub_chunks[1], &acct_clone),
            DetailTab::Transactions => self.render_transactions(f, sub_chunks[1], &acct_clone),
            DetailTab::AlertsByAccount => self.render_alerts(f, sub_chunks[1], &acct_clone),
            DetailTab::Report => self.render_report(f, sub_chunks[1], &acct_clone),
        }
    }

    // ---------- General tab ----------

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

    // ---------- Transactions tab ----------

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
        let mut txns: Vec<&store::TransactionRecord> =
            all.iter().filter(|tx| tx.status != "PDNG").collect();
        if self.txn_filter.active {
            if let Some(ref cat) = self.txn_filter.category {
                txns.retain(|tx| tx.category.eq_ignore_ascii_case(cat));
            }
            if let Some(ref dir) = self.txn_filter.direction {
                txns.retain(|tx| tx.credit_debit_indicator == *dir);
            }
        }
        if !self.txn_filter.search.is_empty() {
            let q = self.txn_filter.search.to_lowercase();
            txns.retain(|tx| {
                let desc = if !tx.remittance_info.is_empty() {
                    tx.remittance_info.join(" ")
                } else if !tx.creditor_name.is_empty() {
                    tx.creditor_name.clone()
                } else {
                    tx.debtor_name.clone()
                };
                let haystack = format!(
                    "{} {} {} {} {} {} {} {}",
                    tx.booking_date,
                    tx.amount,
                    tx.currency,
                    desc,
                    tx.category,
                    tx.creditor_name,
                    tx.debtor_name,
                    tx.note
                );
                haystack.to_lowercase().contains(&q)
            });
        }
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
                tx.booking_date,
                sign,
                amount.round_dp(2),
                tx.category
            );
            let desc_width = row_width.saturating_sub(fixed.len());
            let desc_display = format!("{:<width$}", desc, width = desc_width);
            lines.push(Line::from(vec![
                Span::styled(format!("{}{}  ", prefix, tx.booking_date), selection),
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

    pub(super) fn render_tag_popup(&self, f: &mut Frame) {
        let popup_area = centered_rect(60, 10, f.area());
        f.render_widget(Clear, popup_area);
        let cat_name: String = if !self.tag.new_category_input.is_empty() {
            self.tag.new_category_input.clone()
        } else {
            self.tag
                .categories
                .get(self.tag.selected_category)
                .cloned()
                .unwrap_or_else(|| "uncategorized".to_string())
        };
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            format!(
                "{}  {} {}",
                self.tag.description, self.tag.amount, self.tag.currency
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        let mut cat_line = vec![Span::styled(
            "Category: ",
            Style::default().add_modifier(Modifier::BOLD),
        )];
        cat_line.push(Span::styled(
            format!("[{}]", cat_name),
            Style::default().fg(Color::Cyan),
        ));
        if !self.tag.new_category_input.is_empty() {
            cat_line.push(Span::raw(" (new)"));
        }
        lines.push(Line::from(cat_line));
        lines.push(Line::from(Span::raw(format!(
            "Pattern:  {} (auto)",
            self.tag.pattern
        ))));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter:apply  A:similar  s:skip  ↑↓:category",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Tag Transaction "),
                )
                .style(Style::default().bg(Color::Black)),
            popup_area,
        );
    }

    pub(super) fn render_note_popup(&self, f: &mut Frame) {
        let popup_area = centered_rect(50, 25, f.area());
        f.render_widget(Clear, popup_area);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            "Add a note",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        let before = &self.note_input[..self.note_cursor];
        let after = &self.note_input[self.note_cursor..];
        lines.push(Line::from(vec![
            Span::raw("Note: "),
            Span::raw(before),
            Span::styled("█", Style::default().fg(Color::Yellow)),
            Span::raw(after),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter:save  Esc:cancel  ←→:move",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(
            Paragraph::new(Text::from(lines))
                .block(Block::default().borders(Borders::ALL).title(" Note "))
                .style(Style::default().bg(Color::Black)),
            popup_area,
        );
    }

    // ---------- Alerts tab ----------

    pub(super) fn render_alerts(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let results = self
            .alert_results
            .get(&acct.uid)
            .cloned()
            .unwrap_or_default();
        let mut lines: Vec<Line> = Vec::new();
        let mut triggered = 0;
        for r in &results {
            let status_color = if r.status == "TRIGGERED" {
                triggered += 1;
                Color::Red
            } else {
                Color::Green
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("▸ {}", r.rule.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(&r.status, Style::default().fg(status_color)),
            ]));
            lines.push(Line::from(Span::styled(
                format!("  {}", r.details),
                Style::default().fg(Color::DarkGray),
            )));
            for mt in &r.matched_transactions {
                lines.push(Line::from(Span::raw(format!(
                    "    {}  {} {} {}  {}",
                    mt.date, mt.amount, mt.currency, mt.account_uid, mt.description
                ))));
            }
            lines.push(Line::from(""));
        }
        if results.is_empty() {
            lines.push(Line::from("No alert rules configured."));
        }
        f.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default().borders(Borders::ALL).title(format!(
                    " Alerts ({}/{}) ",
                    triggered,
                    results.len()
                )),
            ),
            area,
        );
    }

    // ---------- Report tab ----------

    pub(super) fn render_report(&self, f: &mut Frame, area: Rect, acct: &store::AccountRecord) {
        let all = self
            .all_transactions
            .get(&acct.uid)
            .cloned()
            .unwrap_or_default();
        let inputs: Vec<SummaryInput> = all
            .iter()
            .filter(|tx| tx.status != "PDNG")
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

    pub(super) fn build_sparkline_data(&self, fc: &AccountForecast) -> Vec<u64> {
        let booked_f64: f64 = fc.booked_balance.to_string().parse().unwrap_or(0.0);
        let projected_f64: f64 = fc
            .projected_balance
            .to_string()
            .parse()
            .unwrap_or(booked_f64);
        let diff = projected_f64 - booked_f64;
        let points = 30;
        let mut data = Vec::with_capacity(points);
        for i in 0..points {
            let t = i as f64 / (points - 1) as f64;
            let val = (booked_f64 + diff * t * t).abs() as u64;
            data.push(val.min(u64::MAX / 2));
        }
        data
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
