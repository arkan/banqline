use super::*;

impl App {
    pub(super) async fn event_loop(&mut self) -> Result<()> {
        let stdout = std::io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("create terminal")?;
        loop {
            if self.refresh.open {
                self.do_refresh(&mut terminal).await?;
            }
            terminal.draw(|f| self.render(f)).context("draw frame")?;
            if let Event::Key(key) = event::read().context("read event")? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Help overlay is intentionally global and read-only.
                if self.help_open {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter => {
                            self.help_open = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                // If refresh modal is open and done, Enter/Esc/Escape closes it.
                if self.refresh.open && self.refresh.done {
                    match key.code {
                        KeyCode::Enter | KeyCode::Esc => {
                            self.refresh.open = false;
                            self.refresh.done = false;
                            continue;
                        }
                        _ => continue,
                    }
                }

                // Global shortcuts (skipped when search/note modal is active).
                let input_active = self.search_open || self.note_open;
                if !input_active {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('?') => {
                            self.help_open = true;
                            continue;
                        }
                        KeyCode::Esc => {
                            if self.tag.open {
                                self.tag.open = false;
                            }
                            continue;
                        }
                        KeyCode::Char('r') => {
                            self.start_refresh();
                            continue;
                        }
                        _ => {}
                    }
                }
                self.handle_accounts_key(key.code).await?;
            }
        }
        Ok(())
    }

    // ---------- Key handling ----------

    pub(super) async fn handle_accounts_key(&mut self, code: KeyCode) -> Result<()> {
        // Note modal.
        if self.note_open {
            match code {
                KeyCode::Enter => {
                    self.save_note().await?;
                    return Ok(());
                }
                KeyCode::Esc => {
                    self.note_open = false;
                    return Ok(());
                }
                KeyCode::Left => {
                    if self.note_cursor > 0 {
                        self.note_cursor -= 1;
                    }
                    return Ok(());
                }
                KeyCode::Right => {
                    if self.note_cursor < self.note_input.len() {
                        self.note_cursor += 1;
                    }
                    return Ok(());
                }
                KeyCode::Backspace => {
                    if self.note_cursor > 0 {
                        self.note_cursor -= 1;
                        self.note_input.remove(self.note_cursor);
                    }
                    return Ok(());
                }
                KeyCode::Delete => {
                    if self.note_cursor < self.note_input.len() {
                        self.note_input.remove(self.note_cursor);
                    }
                    return Ok(());
                }
                KeyCode::Home => {
                    self.note_cursor = 0;
                    return Ok(());
                }
                KeyCode::End => {
                    self.note_cursor = self.note_input.len();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    self.note_input.insert(self.note_cursor, c);
                    self.note_cursor += 1;
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
        // Search modal.
        if self.search_open {
            match code {
                KeyCode::Enter => {
                    self.txn_filter.search = self.search_input.clone();
                    self.txn_filter.active = !self.search_input.is_empty();
                    self.search_open = false;
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                    return Ok(());
                }
                KeyCode::Esc => {
                    self.search_open = false;
                    return Ok(());
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
        // Tag popup.
        if self.tag.open {
            match code {
                KeyCode::Enter => {
                    self.apply_tag().await?;
                    self.tag.open = false;
                    return Ok(());
                }
                KeyCode::Char('s') => {
                    self.tag.open = false;
                    return Ok(());
                }
                KeyCode::Char('A') => {
                    self.tag.apply_similar = true;
                    self.apply_tag().await?;
                    self.tag.open = false;
                    return Ok(());
                }
                KeyCode::Up => {
                    if self.tag.selected_category > 0 {
                        self.tag.selected_category -= 1;
                    }
                    return Ok(());
                }
                KeyCode::Down => {
                    if self.tag.selected_category + 1 < self.tag.categories.len() {
                        self.tag.selected_category += 1;
                    }
                    return Ok(());
                }
                KeyCode::Backspace => {
                    self.tag.new_category_input.pop();
                    return Ok(());
                }
                KeyCode::Char(c) => {
                    self.tag.new_category_input.push(c);
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }

        match code {
            // j/k: navigate sidebar accounts.
            KeyCode::Char('j') => {
                if self.selected_account + 1 < self.accounts.len() {
                    self.selected_account += 1;
                    self.account_list_state.select(Some(self.selected_account));
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                    self.pending_scroll = 0;
                }
            }
            KeyCode::Char('k') => {
                if self.selected_account > 0 {
                    self.selected_account -= 1;
                    self.account_list_state.select(Some(self.selected_account));
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                    self.pending_scroll = 0;
                }
            }
            // ↑/↓: move cursor with scroll-at-edges.
            KeyCode::Up => {
                if self.detail_tab == DetailTab::Transactions {
                    if self.txn_cursor > 0 {
                        self.txn_cursor -= 1;
                        if self.txn_cursor < self.txn_scroll {
                            self.txn_scroll = self.txn_cursor;
                        }
                    }
                } else if self.detail_tab == DetailTab::Pending && self.pending_scroll > 0 {
                    self.pending_scroll -= 1;
                }
            }
            KeyCode::Down => {
                if self.detail_tab == DetailTab::Transactions {
                    self.txn_cursor += 1;
                } else if self.detail_tab == DetailTab::Pending {
                    self.pending_scroll += 1;
                }
            }
            // ←/→: switch sub-tabs.
            KeyCode::Left => {
                let all = DetailTab::all();
                let pos = all.iter().position(|t| *t == self.detail_tab).unwrap_or(0);
                self.detail_tab = all[(pos + all.len() - 1) % all.len()];
            }
            KeyCode::Right => {
                let all = DetailTab::all();
                let pos = all.iter().position(|t| *t == self.detail_tab).unwrap_or(0);
                self.detail_tab = all[(pos + 1) % all.len()];
            }
            KeyCode::Char('/') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.search_open = true;
                    self.search_input.clear();
                }
            }
            KeyCode::Char('n') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.open_note_popup();
                }
            }
            KeyCode::Char('t') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.open_tag_popup();
                }
            }
            KeyCode::Char('f') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.txn_filter.active = !self.txn_filter.active;
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                }
            }
            KeyCode::Char('c') => {
                if self.detail_tab == DetailTab::Transactions {
                    let mut cats: Vec<String> = self
                        .cfg
                        .tag_rules
                        .0
                        .iter()
                        .map(|r| r.category.clone())
                        .collect();
                    cats.push("uncategorized".into());
                    let current = self.txn_filter.category.clone();
                    if let Some(pos) = cats.iter().position(|c| Some(c) == current.as_ref()) {
                        let next = (pos + 1) % (cats.len() + 1);
                        self.txn_filter.category = if next < cats.len() {
                            Some(cats[next].clone())
                        } else {
                            None
                        };
                    } else {
                        self.txn_filter.category = Some(cats[0].clone());
                    }
                    self.txn_filter.active = self.txn_filter.category.is_some();
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                }
            }
            KeyCode::Char('d') => {
                if self.detail_tab == DetailTab::Transactions {
                    self.txn_filter.direction = match self.txn_filter.direction.as_deref() {
                        None => Some("DBIT".into()),
                        Some("DBIT") => Some("CRDT".into()),
                        _ => None,
                    };
                    self.txn_filter.active = true;
                    self.txn_cursor = 0;
                    self.txn_scroll = 0;
                }
            }
            KeyCode::Char('m') => {
                if self.detail_tab == DetailTab::Report {
                    self.report_period = ReportPeriod::Month;
                }
            }
            KeyCode::Char('w') => {
                if self.detail_tab == DetailTab::Report {
                    self.report_period = ReportPeriod::Week;
                }
            }
            KeyCode::Char('D') => {
                if self.detail_tab == DetailTab::Report {
                    self.report_period = ReportPeriod::Day;
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ---------- Refresh ----------
}
