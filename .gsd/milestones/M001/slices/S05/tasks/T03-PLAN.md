---
estimated_steps: 35
estimated_files: 12
skills_used: []
---

# T03: Transaction splits: migration 008 + domain + repo + `rtf transactions split` CLI

**Migration 008** (`migrations/008_transaction_splits.sql`):
```sql
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
```

**Domain** (`src/domain/transaction/split.rs`):
- `TransactionSplit { id, transaction_id, category_id, amount: Money, notes: Option<String>, created_at }`.
- `TransactionSplitRepository` trait: `save`, `find_by_transaction(txn_id) -> Vec`, `delete_by_transaction(txn_id)`, `delete(id)`.

**SQLite impl** (`src/infrastructure/storage/transaction_split_repo.rs`).

**SplitService** (`src/application/split_service.rs`):
- `split_transaction(txn_id: &str, splits: Vec<(CategoryId, Money, Option<Notes>)>) -> Result<Vec<TransactionSplit>, DomainError>`.
- Flow:
  1. Load the transaction. Error if missing.
  2. Validate: sum of split absolute amounts equals transaction absolute amount (±1 cent tolerance for rounding).
  3. Validate: all splits share the transaction's currency.
  4. In a transaction: delete any existing splits for this txn_id; insert the new ones; set transaction.category_id = NULL (it's now represented by splits).
- `delete_splits(txn_id) -> Result<(), DomainError>` — unwind splits, restore behavior.

**CLI** (`src/cli/transactions.rs` extended):
- `rtf transactions split <txn-id> --split <cat-id>:<amount>[:<notes>] [--split ...]`
- Parses each `--split` arg as `cat_id:amount[:notes]` (colon-separated).
- Validates at least 2 splits (single-split makes no sense).
- Prints the resulting splits envelope.

**List command update:** `rtf transactions list --format json` gains a `splits` field on each row (empty array when no splits).

**Tests:**
- Domain: split validates non-zero amount, currency matches txn.
- Service: happy path (2-way split); sum-mismatch rejected; currency-mismatch rejected; unknown txn_id rejected; re-splitting an already-split txn replaces old splits.
- CLI: split happy path via integration test.

## Inputs

- `migrations/001_initial.sql (categories table)`
- `src/domain/transaction/`

## Expected Output

- `Migration 008 + TransactionSplit domain + repo`
- `SplitService with sum/currency validation`
- `rtf transactions split CLI`
- `list --format json includes splits`
- `~12 unit tests + 1 integration`

## Verification

cargo test -- domain::transaction::split && cargo test -- application::split_service && cargo test -- cli::transactions
