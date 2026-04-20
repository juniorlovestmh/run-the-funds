# S06: Monarch adapter + tags domain — UAT

**Milestone:** M001
**Written:** 2026-04-20T12:33:24.674Z

# S06: Monarch adapter + tags domain — UAT

**Milestone:** M001
**Written:** 2026-04-20

## Migrations auto-apply (009, 010)
Fresh DB migrates 0 → 10. **PASS.**

## First sync against live Monarch
`rtf sync --provider monarch` → 19 accounts, 15 groups, 66 categories, 13 tags, 2185 txns in ~11s. **PASS.**

## Idempotent re-run
Second invocation: 0 new, 2185 duplicates. **PASS.**

## Parity with mmoney direct count
mmoney pagination sum = 2185. fintrack COUNT(*) = 2185. **PASS.**

## Account display names preserved
All 19 Monarch-side renames come through ("Schwab Individual (...947)" etc.). **PASS.**

## Category + tag ID resolution
Monarch external IDs resolve to fintrack local ids via external_id upsert. transaction_tags junction populated. **PASS.**

## Spending rollup on Monarch data
March 2026 shows Paychecks +$6878, Groceries -$774, Transfer -$1911. Sign convention now consistent (resolves S05 limitation). **PASS.**

## `rtf sync` unified does NOT auto-run Monarch
Opt-in only via `--provider monarch`. **PASS.**

## Full test suite
441 passing.

## Known limitations
All Monarch accounts treated as USD. transactions_updated doesn't track tag-set-only changes. Unified sync message still refers to dormant providers only.
