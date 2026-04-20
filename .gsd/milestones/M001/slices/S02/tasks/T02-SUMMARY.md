---
id: T02
parent: S02
milestone: M001
key_files:
  - migrations/002_transaction_external_id_unique.sql
  - src/infrastructure/storage/migrations.rs
  - src/infrastructure/storage/database.rs
  - src/infrastructure/storage/transaction_repo.rs
  - src/domain/transaction/repository.rs
key_decisions:
  - Partial unique index (WHERE external_id IS NOT NULL) rather than a full unique index — keeps manually-entered txns without FITIDs permissive.
  - Dedup key is (account_id, external_id), not just external_id — two banks can legitimately issue the same FITID, so the scope is per-account.
  - Flipped save from INSERT OR REPLACE to INSERT. REPLACE would silently delete on unique-index conflict, defeating the index. Transactions are append-only in this domain (UUID PK), so plain INSERT matches the semantics and lets the constraint enforce.
  - find_by_external_id returns Option<Transaction> rather than Option<Id>, because the service layer will want the full row in some follow-up flows.
duration: 
verification_result: passed
completed_at: 2026-04-19T19:13:30.231Z
blocker_discovered: false
---

# T02: Migration 002 adds a partial unique index on (account_id, external_id); find_by_external_id added to trait and SQLite impl; save changed from INSERT OR REPLACE to plain INSERT so the constraint actually enforces.

**Migration 002 adds a partial unique index on (account_id, external_id); find_by_external_id added to trait and SQLite impl; save changed from INSERT OR REPLACE to plain INSERT so the constraint actually enforces.**

## What Happened

Three coordinated changes to give the import pipeline real FITID-backed dedup at the storage layer:

**1. Migration 002** (`migrations/002_transaction_external_id_unique.sql`) — creates a **partial unique index** `idx_transactions_account_external_id` on `(account_id, external_id) WHERE external_id IS NOT NULL`. Partial predicate is the key detail: it skips NULL rows so manually-entered transactions (no FITID) can coexist under the same account without the index clashing. Registered in `src/infrastructure/storage/migrations.rs` as the second element of `MIGRATIONS`; `Database::in_memory()` and `::open()` now apply both migrations on fresh DBs.

**2. `TransactionRepository::find_by_external_id(account_id, external_id)`** — new trait method + SQLite impl (`SELECT * FROM transactions WHERE account_id = ?1 AND external_id = ?2 LIMIT 1` returning `Option<Transaction>`). Scoped by account because two different banks can legitimately issue the same FITID; dedup must be per-account.

**3. `SqliteTransactionRepository::save` changed from `INSERT OR REPLACE` to plain `INSERT`.** The REPLACE semantics silently deleted the conflicting row on unique-constraint violation — which neutered the partial index we just added. Transactions are append-only in this domain (UUID PK, never collide), so plain INSERT is semantically correct AND lets the unique index actually enforce. The service layer (T03) will call `find_by_external_id` first and skip duplicates before calling save; if a caller ever races past that check, the constraint surfaces a `DomainError::Storage`.

**Tests (6 new):**
- `migration_002_creates_external_id_unique_index` — verifies the index exists in `sqlite_master` after migration.
- `migration_is_idempotent` — updated from `version == 1` to `version == 2`.
- `find_by_external_id_returns_none_when_absent`.
- `find_by_external_id_roundtrips`.
- `find_by_external_id_scoped_to_account` — two accounts share a FITID; each lookup returns the right row (proves per-account scoping).
- `unique_index_rejects_duplicate_external_id_in_same_account` — second insert with the same `(account_id, external_id)` but a different UUID errors out with `DomainError::Storage`.
- `null_external_id_not_constrained` — two rows with `external_id = None` under the same account both insert.

S01 regressions: none. All 158 prior tests still pass; total climbs to 164 (+6).

## Verification

`cargo test` → 164 passed, 0 failed, 0 ignored. (Up from 157 after T01 + 1 ignored Nubank test now live = 158 baseline; +6 new T02 tests.)

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 90ms |

## Deviations

Added `find_by_external_id_scoped_to_account` beyond the minimum plan — caught a real design question (shared FITIDs across banks) and pins the contract. The INSERT-vs-INSERT-OR-REPLACE flip wasn't explicitly in the plan but was a necessary implication of the partial index; noted here for traceability.

## Known Issues

None.

## Files Created/Modified

- `migrations/002_transaction_external_id_unique.sql`
- `src/infrastructure/storage/migrations.rs`
- `src/infrastructure/storage/database.rs`
- `src/infrastructure/storage/transaction_repo.rs`
- `src/domain/transaction/repository.rs`
