CREATE TABLE IF NOT EXISTS persons (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    relationship TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL,
    currency TEXT NOT NULL,
    owner TEXT NOT NULL,
    institution TEXT,
    account_number_last4 TEXT,
    balance_amount TEXT NOT NULL DEFAULT '0',
    balance_currency TEXT NOT NULL,
    credit_limit_amount TEXT,
    credit_limit_currency TEXT,
    interest_rate TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS category_groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY,
    group_id TEXT NOT NULL REFERENCES category_groups(id),
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_categories_group_id ON categories(group_id);

CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id),
    date TEXT NOT NULL,
    amount_value TEXT NOT NULL,
    amount_currency TEXT NOT NULL,
    payee TEXT,
    description TEXT,
    category_id TEXT REFERENCES categories(id),
    beneficiary_id TEXT REFERENCES persons(id),
    transfer_pair_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    external_id TEXT,
    imported_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transactions_account_id ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
CREATE INDEX IF NOT EXISTS idx_transactions_category_id ON transactions(category_id);
CREATE INDEX IF NOT EXISTS idx_transactions_beneficiary_id ON transactions(beneficiary_id);
CREATE INDEX IF NOT EXISTS idx_transactions_external_id ON transactions(external_id);

CREATE TABLE IF NOT EXISTS exchange_rates (
    id TEXT PRIMARY KEY,
    from_currency TEXT NOT NULL,
    to_currency TEXT NOT NULL,
    rate TEXT NOT NULL,
    date TEXT NOT NULL,
    source TEXT NOT NULL,
    fetched_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_exchange_rates_pair_date
    ON exchange_rates(from_currency, to_currency, date);

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);
