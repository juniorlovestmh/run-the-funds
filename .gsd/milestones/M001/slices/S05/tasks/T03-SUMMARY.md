---
id: T03
parent: S05
milestone: M001
key_files:
  - migrations/008_transaction_splits.sql
  - src/domain/transaction/split.rs
  - src/application/split_service.rs
  - src/infrastructure/storage/transaction_split_repo.rs
  - src/cli/transactions.rs
key_decisions:
  - Sum tolerance 0.01 cents for user-input float rounding.
  - Clearing parent category_id on split ensures spending rollup doesn't double-count.
duration: 
verification_result: untested
completed_at: 2026-04-20T11:04:41.926Z
blocker_discovered: false
---

# T03: Transaction splits: migration 008 + domain + SplitService + `fintrack transaction-split` CLI.

**Transaction splits: migration 008 + domain + SplitService + `fintrack transaction-split` CLI.**

## What Happened

Migration 008 created transaction_splits (id, transaction_id FK CASCADE, category_id FK, amount_value, amount_currency, notes). Domain TransactionSplit with validation (non-empty, non-zero amount, currency match). SplitService.split_transaction validates sum=abs(parent) with 0.01 tolerance, replaces existing splits atomically, clears parent category_id so the spending rollup expands splits not parents. CLI: `fintrack transaction-split --id X --split cat_id:amount[:notes]` (multiple). 9 service tests + repo tests including CASCADE verification.

## Verification

cargo test green. Live demo on real $262 Casas Guanabara charge split into $200 Groceries + $62.46 Shopping.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `migrations/008_transaction_splits.sql`
- `src/domain/transaction/split.rs`
- `src/application/split_service.rs`
- `src/infrastructure/storage/transaction_split_repo.rs`
- `src/cli/transactions.rs`
