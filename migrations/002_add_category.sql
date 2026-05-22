ALTER TABLE transactions ADD COLUMN category TEXT NOT NULL DEFAULT 'uncategorized';
ALTER TABLE transactions ADD COLUMN category_source TEXT NOT NULL DEFAULT '';
