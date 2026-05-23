use super::*;

impl App {
    pub(super) fn open_tag_popup(&mut self) {
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
        let desc = if !tx.remittance_info.is_empty() {
            tx.remittance_info.join(" ")
        } else if !tx.creditor_name.is_empty() {
            tx.creditor_name.clone()
        } else {
            tx.debtor_name.clone()
        };
        let pattern = desc.split_whitespace().next().unwrap_or("").to_lowercase();
        self.tag = TagState {
            open: true,
            transaction_id: tx.transaction_id.clone(),
            description: desc,
            amount: tx.amount.clone(),
            currency: tx.currency.clone(),
            selected_category: 0,
            categories: self
                .cfg
                .tag_rules
                .0
                .iter()
                .map(|r| r.category.clone())
                .collect(),
            pattern,
            apply_similar: false,
            ..Default::default()
        };
    }

    pub(super) async fn apply_tag(&mut self) -> Result<()> {
        let uid = match self.current_uid() {
            Some(u) => u.to_string(),
            None => return Ok(()),
        };
        let cat = if !self.tag.new_category_input.is_empty() {
            self.tag.new_category_input.clone()
        } else {
            self.tag
                .categories
                .get(self.tag.selected_category)
                .cloned()
                .unwrap_or_else(|| "uncategorized".to_string())
        };
        if self.tag.apply_similar {
            let pattern = self.tag.pattern.to_uppercase();
            let txns = self.all_transactions.get(&uid).cloned().unwrap_or_default();
            let mut updates = Vec::new();
            for tx in &txns {
                if tx.category_source == "manual" {
                    continue;
                }
                let norm = tagger::normalize(
                    &tx.remittance_info,
                    &tx.creditor_name,
                    &tx.debtor_name,
                    &tx.note,
                );
                if norm.contains(&pattern) {
                    updates.push(store::CategoryUpdate {
                        account_uid: uid.clone(),
                        transaction_id: tx.transaction_id.clone(),
                        category: cat.clone(),
                        source: "auto".to_string(),
                    });
                }
            }
            if !updates.is_empty() {
                self.db.update_categories(&updates).await?;
            }
        } else {
            self.db
                .update_category(&uid, &self.tag.transaction_id, &cat, "manual")
                .await?;
        }
        if let Ok(txns) = self
            .db
            .get_transactions(&uid, &store::QueryOpts::default())
            .await
        {
            self.all_transactions.insert(uid, txns);
        }
        self.status = format!("Tagged as {}", cat);
        Ok(())
    }

    // ---------- Note ----------
}
