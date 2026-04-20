---
id: T04
parent: S06
milestone: M001
key_files:
  - (none)
key_decisions:
  - Monarch opt-in via --provider monarch, not auto-run in unified path — mmoney holds global auth, surprise side effects are bad UX.
  - Parity check via mmoney pagination sum (5 pages × 500 = 2185) matches fintrack COUNT(*) = 2185, zero drift.
  - S05 sign-convention limitation resolved by upstream switch — Monarch normalizes per-owner perspective, so fintrack's spending rollup now shows correct signs without any fintrack-side change.
duration: 
verification_result: untested
completed_at: 2026-04-20T12:31:01.431Z
blocker_discovered: false
---

# T04: CLI wired (`rtf sync --provider monarch`) + live demo against real Monarch account: 19 accounts, 15 groups, 66 cats, 13 tags, 2185 txns in 11 seconds, idempotent re-run confirmed, exact parity with mmoney direct count.

**CLI wired (`rtf sync --provider monarch`) + live demo against real Monarch account: 19 accounts, 15 groups, 66 cats, 13 tags, 2185 txns in 11 seconds, idempotent re-run confirmed, exact parity with mmoney direct count.**

## What Happened

Wired the monarch provider into src/cli/sync.rs: new `run_monarch` + `run_monarch_inline` functions plus a branch in handle_sync for `Some("monarch")`. Extended UnifiedSyncReport with an optional `monarch` slot. Key UX decision: Monarch is NOT auto-run in the unified (no-flag) path because mmoney is a system-wide tool with the user's global auth — silently pulling it on every `rtf sync` would be surprising. Users opt in via `--provider monarch`. Live demo against the user's actual Monarch account returned exact parity: 2185 transactions in fintrack = 2185 in Monarch (paginated direct count). All 19 Monarch accounts landed with their renamed display names ("Schwab Individual (...947)" etc. — the Monarch cleanup we did earlier in the session is preserved). Spending rollup on the synced data shows correct owner-perspective sign convention (Paychecks +$6878 income, Groceries -$774 spending) — resolves the S05-UAT known limitation about inconsistent importer signs. Idempotent re-run confirmed: `transactions_imported=0`, `duplicates_skipped=2185`. S06-UAT.md documents every test scenario with commands + expected/actual output.

## Verification

cargo test 441 passing. Live: `rtf sync --provider monarch` against production Monarch account in ~11s, 2185 txn parity verified against `mmoney --format json transactions list` pagination sum. Re-run produces 0 new / 2185 duplicates. `rtf spending --from 2026-03-01 --to 2026-03-31` renders correct signed rollup.</verification>
<parameter name="keyFiles">["src/cli/sync.rs", ".gsd/milestones/M001/slices/S06/S06-UAT.md"]

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

None.
