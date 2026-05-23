use super::*;

impl App {
    pub(super) fn open_note_popup(&mut self) {
        let uid = match self.current_uid() {
            Some(u) => u.to_string(),
            None => return,
        };
        let txns = self.all_transactions.get(&uid).cloned().unwrap_or_default();
        let idx = self.txn_cursor;
        if idx >= txns.len() {
            return;
        }
        let tx = &txns[idx];
        self.note_open = true;
        self.note_input = tx.note.clone();
        self.note_cursor = tx.note.len();
        self.note_txn_id = tx.transaction_id.clone();
        self.note_account_uid = uid;
    }

    pub(super) async fn save_note(&mut self) -> Result<()> {
        self.db
            .set_transaction_note(&self.note_account_uid, &self.note_txn_id, &self.note_input)
            .await
            .context("save note")?;
        if let Ok(txns) = self
            .db
            .get_transactions(&self.note_account_uid, &store::QueryOpts::default())
            .await
        {
            self.all_transactions
                .insert(self.note_account_uid.clone(), txns);
        }
        self.note_open = false;
        self.status = "Note saved".to_string();
        Ok(())
    }

    // ---------- Render ----------
}
