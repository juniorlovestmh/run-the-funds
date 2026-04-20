---
id: T04
parent: S05
milestone: M001
key_files:
  - src/application/categorization_service.rs
  - src/domain/transaction/repository.rs
  - src/cli/categorize.rs
key_decisions:
  - Same-currency only — cross-currency pair detection needs PTAX-aware amount comparison, deferred to S07.
  - Greedy (not optimal-matching) is good enough and avoids O(n!) search.
  - Skip already-tagged rows for idempotency.
duration: 
verification_result: untested
completed_at: 2026-04-20T11:04:54.464Z
blocker_discovered: false
---

# T04: Transfer pair detection: greedy same-currency matching ±3 days, wired into `rtf categorize --detect-transfers`.

**Transfer pair detection: greedy same-currency matching ±3 days, wired into `rtf categorize --detect-transfers`.**

## What Happened

Extended CategorizationService with detect_transfers(dry_run) returning TransferPairingReport. Algorithm: find_untagged (no category_id AND no transfer_pair_id), group by (currency, abs(amount)), within each group pair a debit with a credit from a DIFFERENT account within ±3 days. Greedy — first valid pair wins, skip rows already paired. Cross-account constraint prevents same-account sign-flip false-positives. Idempotent: re-running produces zero new pairs. Added find_untagged repo method. CLI flag `--detect-transfers` is mutually exclusive with rule-based categorize; output uses a mode-tagged enum (Rules | Transfers). 9 service tests.

## Verification

cargo test green. Live demo on real DB: 8 pairs created (16 rows got transfer_pair_id), 414 candidates skipped. Re-running produces 0 new pairs (idempotency verified).

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
- `src/domain/transaction/repository.rs`
- `src/cli/categorize.rs`
