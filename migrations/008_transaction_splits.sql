-- Transaction splits (S05 T03). A single transaction can be split into
-- multiple category allocations — e.g. a $150 Costco charge split into
-- $100 groceries + $50 household. When splits exist, the parent
-- transaction's `category_id` is NULL (the splits carry the categorization).
-- Deleting the parent cascades to its splits.
CREATE TABLE IF NOT EXISTS transaction_splits (
    id TEXT PRIMARY KEY,
    transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id),
    amount_value TEXT NOT NULL,
    amount_currency TEXT NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transaction_splits_txn ON transaction_splits(transaction_id);
