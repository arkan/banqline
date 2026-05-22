CREATE TABLE IF NOT EXISTS accounts (
    uid          TEXT PRIMARY KEY,
    bank_name    TEXT NOT NULL,
    iban         TEXT NOT NULL DEFAULT '',
    name         TEXT NOT NULL DEFAULT '',
    currency     TEXT NOT NULL DEFAULT '',
    details      TEXT NOT NULL DEFAULT '',
    usage_type   TEXT NOT NULL DEFAULT '',
    account_type TEXT NOT NULL DEFAULT '',
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_accounts_bank ON accounts(bank_name);

CREATE TABLE IF NOT EXISTS balances (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    account_uid    TEXT NOT NULL,
    balance_type   TEXT NOT NULL,
    amount         TEXT NOT NULL,
    currency       TEXT NOT NULL,
    reference_date TEXT NOT NULL DEFAULT '',
    fetched_at     TEXT NOT NULL,
    FOREIGN KEY (account_uid) REFERENCES accounts(uid)
);

CREATE INDEX IF NOT EXISTS idx_balances_account ON balances(account_uid);

CREATE TABLE IF NOT EXISTS transactions (
    id                     INTEGER PRIMARY KEY AUTOINCREMENT,
    account_uid            TEXT NOT NULL,
    transaction_id         TEXT NOT NULL,
    entry_reference        TEXT NOT NULL DEFAULT '',
    amount                 TEXT NOT NULL,
    currency               TEXT NOT NULL,
    booking_date           TEXT NOT NULL DEFAULT '',
    value_date             TEXT NOT NULL DEFAULT '',
    transaction_date       TEXT NOT NULL DEFAULT '',
    remittance_info        TEXT NOT NULL DEFAULT '[]',
    creditor_name          TEXT NOT NULL DEFAULT '',
    debtor_name            TEXT NOT NULL DEFAULT '',
    status                 TEXT NOT NULL DEFAULT '',
    credit_debit_indicator TEXT NOT NULL DEFAULT '',
    note                   TEXT NOT NULL DEFAULT '',
    UNIQUE(account_uid, transaction_id)
);

CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_uid);
CREATE INDEX IF NOT EXISTS idx_transactions_booking ON transactions(booking_date);

CREATE TABLE IF NOT EXISTS sync_meta (
    account_uid TEXT NOT NULL,
    data_type   TEXT NOT NULL,
    last_synced TEXT NOT NULL,
    PRIMARY KEY (account_uid, data_type)
);
