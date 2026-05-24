use super::*;

impl App {
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
