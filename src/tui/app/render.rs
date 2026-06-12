use super::*;

mod alerts;
mod general;
mod modals;
mod report;
mod transactions;

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
            .map(|t| t.iter().filter(|tx| tx.is_pending()).count())
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
