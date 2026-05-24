use super::*;

impl App {
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
}
