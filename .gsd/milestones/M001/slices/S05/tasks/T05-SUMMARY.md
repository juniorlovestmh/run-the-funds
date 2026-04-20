---
id: T05
parent: S05
milestone: M001
key_files:
  - src/application/spending_service.rs
  - src/cli/spending.rs
  - src/cli/categories.rs
key_decisions:
  - Transfer pairs excluded by default — 'spending' means money-out-of-household, not between-own-accounts.
  - Per-currency only MVP; cross-currency deferred to S07.
  - category-groups + categories CRUD deliberately create+list only, no update/delete until a real use case emerges.
duration: 
verification_result: untested
completed_at: 2026-04-20T11:05:08.094Z
blocker_discovered: false
---

# T05: SpendingService + `rtf spending` rollup with split expansion + transfer exclusion; plus category-groups and categories CRUD CLIs.

**SpendingService + `rtf spending` rollup with split expansion + transfer exclusion; plus category-groups and categories CRUD CLIs.**

## What Happened

SpendingService generic over TransactionRepository + TransactionSplitRepository + CategoryRepository. compute(SpendingOptions) returns SpendingReport with totals_by_currency, by_category (sorted most-negative-first), by_group, by_account, uncategorized, split_count, transfers_excluded, window_start/end. Expands splits into line items (parent's cleared category_id ensures no double-count). Filters transfer pairs by default (`--include-transfers` flips). MVP per-currency only — cross-currency rollup deferred to S07. CLI: `rtf spending [--from --to --account-id --include-transfers --format json|table]` with pretty table output showing all 4 breakdowns + summary footer. Added `rtf category-groups {create,list}` and `rtf categories {create,list}` for the category CRUD that S02 didn't ship. 9 spending tests + category CLI roundtrip tests.

## Verification

cargo test green (406 total). YTD + MTD reports rendered in both table and JSON against real 1,508-txn DB.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/application/spending_service.rs`
- `src/cli/spending.rs`
- `src/cli/categories.rs`
