ALTER TABLE accounts ADD COLUMN alias TEXT NOT NULL DEFAULT '';

CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_alias
    ON accounts(alias) WHERE alias != '';
