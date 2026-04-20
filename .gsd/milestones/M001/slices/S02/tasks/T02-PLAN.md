---
estimated_steps: 18
estimated_files: 4
skills_used: []
---

# T02: External_id dedup: repo lookup, unique index, migration 002

Today `SqliteTransactionRepository::save` uses `INSERT OR REPLACE`, which overwrites rows that share an id PK but doesn't prevent two rows with the same `external_id` under different UUIDs from coexisting. For real FITID-backed dedup we need a uniqueness constraint and a lookup helper.

Changes:
1. Add `migrations/002_transaction_external_id_unique.sql`:
   ```sql
   CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_account_external_id
       ON transactions(account_id, external_id)
       WHERE external_id IS NOT NULL;
   ```
   Update `infrastructure/storage/migrations.rs` to load and apply 002 after 001; bump `schema_version` row.
2. Extend `TransactionRepository` trait with `fn find_by_external_id(&self, account_id: &str, external_id: &str) -> Result<Option<Transaction>, DomainError>`.
3. Implement it in `SqliteTransactionRepository` via `SELECT * FROM transactions WHERE account_id = ?1 AND external_id = ?2 LIMIT 1`.
4. Leave `save` semantics unchanged — the service layer (T03) will call `find_by_external_id` first and skip on hit.

TDD:
- `find_by_external_id_returns_none_when_absent`.
- `find_by_external_id_roundtrips`.
- `unique_index_enforced`: insert two rows under the same `(account_id, external_id)` but different PKs → second insert fails with a UNIQUE constraint violation surfaced as `DomainError::Storage`.
- `null_external_id_not_constrained`: two rows under the same account with `external_id = None` both insert successfully (partial index keeps NULLs permissive).
- Regression: existing S01 tests still pass (notably `roundtrip_optional_fields`).

## Inputs

- `src/infrastructure/storage/transaction_repo.rs`
- `migrations/001_initial.sql`
- `src/infrastructure/storage/migrations.rs`

## Expected Output

- `Migration 002 applied on fresh DBs, idempotent on existing ones`
- ``find_by_external_id` on trait + SQLite impl`
- `Unit test coverage for uniqueness, lookup, and NULL permissiveness`

## Verification

cargo test -- infrastructure::storage::transaction_repo
