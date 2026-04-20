---
id: S05
parent: M001
milestone: M001
provides:
  - ["Rule domain + regex validation at construction", "SqliteRuleRepository (UPSERT, priority-ordered)", "CategorizationService (dry-run, reset, per-rule counts, transfer pair detection)", "transactions.save ON CONFLICT(id) DO UPDATE", "TransactionSplit + SplitService (sum-invariant, currency-match)", "SpendingService (per-currency rollup, split expansion, transfer exclusion)", "CLI: rules, categorize, transaction-split, category-groups, categories, spending", "migrations 007_rules, 008_transaction_splits"]
requires:
  - slice: S01
    provides: Domain foundation + SQLite storage + migration framework
  - slice: S02
    provides: Transaction + TransactionRepository + partial unique index
affects:
  []
key_files:
  - ["migrations/007_rules.sql", "migrations/008_transaction_splits.sql", "src/application/categorization_service.rs", "src/application/split_service.rs", "src/application/spending_service.rs", "src/infrastructure/storage/rules_repo.rs", "src/infrastructure/storage/transaction_split_repo.rs", "src/cli/rules.rs", "src/cli/categorize.rs", "src/cli/spending.rs", ".gsd/milestones/M001/slices/S05/S05-UAT.md"]
key_decisions:
  - ["Regex validated at Rule construction.", "transactions.save ON CONFLICT(id) DO UPDATE (not REPLACE).", "Transfer pairing same-currency only.", "detect_transfers skips rows with category_id OR transfer_pair_id (idempotent).", "SpendingService per-currency only (cross-currency deferred to S07).", "Rolled S04C pluggy_connect.html drift fix into S05 commit."]
patterns_established:
  - ["Service generic over repository traits (template for S08 goals/debt/health).", "Dry-run + apply envelope on mutating CLIs.", "Per-rule fire counts observable in reports.", "Sum-invariant validation at domain construction."]
observability_surfaces:
  - ["CategorizeReport + TransferPairingReport + SpendingReport with standardized CliResponse/ErrorResponse envelope."]
drill_down_paths:
  []
duration: ""
verification_result: passed
completed_at: 2026-04-20T11:05:56.371Z
blocker_discovered: false
---

# S05: Categorization Engine

**Rules + transfer pair detection + transaction splits + per-currency spending rollup, validated on the user's live 1,508-txn database.**

## What Happened

S05 shipped the categorization engine across six tasks (see T01-T06 summaries). Critical storage fix in T02 (transactions.save UPSERT by id) unblocked re-categorization. Greedy same-currency transfer pair detection in T04 is idempotent by design. SpendingService in T05 is per-currency only (cross-currency rollup deferred to S07). Single commit fd4e662 rolled in the uncommitted S04C pluggy_connect.html drift fix alongside S05 work. 406 tests passing (+24 over S04D).

## Verification

406 tests passing across lib + 4 integration test artifacts. cargo build clean (3 pre-existing categorization_service warnings only). Live DB demo against the user's real 1,508-txn database: 17 rules applied, 8 transfer pairs, spending rollup in table + JSON.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

Rolled uncommitted S04C pluggy_connect.html drift fix into the S05 commit. Preserves one-commit-per-slice cadence.

## Known Limitations

Importer sign convention inconsistent (S06 fixes). No cross-currency rollup (S07). No split query/delete CLI. Transfer pairing same-currency only.

## Follow-ups

S06 makes Monarch the upstream source and resolves sign convention at ingest. S07 adds cross-currency rollup via holdings + PTAX.

## Files Created/Modified

None.
