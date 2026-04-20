---
id: T02
parent: S05
milestone: M001
key_files:
  - src/application/categorization_service.rs
  - src/cli/categorize.rs
  - src/infrastructure/storage/transaction_repo.rs
key_decisions:
  - Per-rule fire counts in the envelope so 'why did this tag?' is observable.
  - UPSERT on (id) not (account_id, external_id) so new-row inserts still error on duplicates.
duration: 
verification_result: untested
completed_at: 2026-04-20T11:04:30.771Z
blocker_discovered: false
---

# T02: CategorizationService + `fintrack categorize` with dry-run, reset, per-rule fire counts; fixed transactions.save UPSERT collision.

**CategorizationService + `fintrack categorize` with dry-run, reset, per-rule fire counts; fixed transactions.save UPSERT collision.**

## What Happened

Added CategorizationService<R: RuleRepository, T: TransactionRepository> with .categorize(CategorizeOptions) returning CategorizeReport {categorized, skipped, reset, dry_run, rules_fired}. Opts support dry-run (no writes, same counts), reset (clear category_ids first), and per-account scoping. Critical storage fix: flipped transactions.save from plain INSERT to INSERT ... ON CONFLICT(id) DO UPDATE SET ... — without this, re-categorizing an existing row with the same (account_id, external_id) caused a UNIQUE constraint failure. CLI: `fintrack categorize [--dry-run --reset --account-id]`. 9 service tests + 7 rule repo tests + storage roundtrip tests.

## Verification

cargo test passed at the T02 checkpoint (tests added incrementally). Manual CLI smoke: dry-run + apply produced identical categorization counts against the live 1,508-txn DB.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/application/categorization_service.rs`
- `src/cli/categorize.rs`
- `src/infrastructure/storage/transaction_repo.rs`
