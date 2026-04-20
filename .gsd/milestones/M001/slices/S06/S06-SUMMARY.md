---
id: S06
parent: M001
milestone: M001
provides:
  - ["Tags domain (Tag + TagRepository + transaction_tags CASCADE junction)", "MonarchAdapter via mmoney subprocess + MmoneyRunner trait", "MonarchSyncService with 3-phase external_id upsert orchestration", "Migration 009 + 010", "AccountType: Brokerage + Other", "CLI: `rtf sync --provider monarch` + `rtf tags list`"]
requires:
  - slice: S01
    provides: Domain + storage + migration framework
  - slice: S02
    provides: Transaction + repository + unique index
  - slice: S05
    provides: Category + CategoryRepository
affects:
  []
key_files:
  - ["migrations/009_tags.sql", "migrations/010_category_external_ids.sql", "src/domain/tag/*", "src/domain/category/category.rs", "src/domain/account/account_type.rs", "src/infrastructure/storage/tag_repo.rs", "src/infrastructure/sync_adapter/monarch.rs", "src/application/monarch_sync_service.rs", "src/cli/sync.rs", ".gsd/milestones/M001/slices/S06/S06-UAT.md"]
key_decisions:
  - ["Subprocess to mmoney vs native Rust GraphQL \u2014 reuses installed+authed tool.", "external_id columns on existing tables vs side table \u2014 simpler joins.", "MonarchSyncService separate from SyncService \u2014 Monarch surface too different from BankSyncAdapter.", "Opt-in via --provider monarch, not auto in unified \u2014 mmoney holds global auth.", "AccountType extended rather than rejecting Monarch subtypes \u2014 import with best-guess type.", "S05 sign limitation resolved by upstream (Monarch normalization), not by fintrack-side logic."]
patterns_established:
  - ["MmoneyRunner trait + FakeRunner \u2014 testable subprocess adapter pattern.", "Three-phase upstream-owned sync with external_id upsert \u2014 template for any hosted provider.", "Optional external_id + external_provider on domain types \u2014 locally-created rows untouched by sync."]
observability_surfaces:
  - ["MonarchSyncReport with per-phase counts + duplicates + updates + window.", "Clear missing-binary and not-authenticated error paths.", "Idempotency visible in duplicates_skipped."]
drill_down_paths:
  []
duration: ""
verification_result: passed
completed_at: 2026-04-20T12:33:24.674Z
blocker_discovered: false
---

# S06: Monarch adapter + tags domain

**rtf becomes Monarch-ingestable: `rtf sync --provider monarch` pulls accounts + categories + tags + transactions with idempotent upsert in ~11 seconds (19 accounts / 2185 txns exact parity with mmoney).**

## What Happened

S06 shipped across four tasks. T01 added tags as a first-class fintrack domain (migration 009 + Tag + TagRepository + CASCADE junction + 10 tests). T02 built MonarchAdapter shelling out to mmoney CLI via MmoneyRunner trait, 12 tests + live smoke. T03 built MonarchSyncService with 3-phase taxonomy→accounts→transactions import + external_id upsert resolution; migration 010 added external_id to categories; AccountType gained Brokerage + Other. T04 wired CLI and ran live demo. 441 tests passing (+35 over S05). Monarch normalizes owner-perspective signs, resolving S05-UAT's sign-convention limitation automatically.

## Verification

cargo test 441 passing / 6 ignored / 0 failed. cargo build clean. Live: `rtf sync --provider monarch` on fresh DB completes in ~11s with 19 accounts + 15 groups + 66 categories + 13 tags + 2185 txns. mmoney paginated count (2185) = fintrack COUNT(*) (2185). Re-run idempotent.

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

Original 5-task plan reduced to 4 (retirement task pulled out of scope mid-slice; see T05 administrative completion). Migration 010 + AccountType expansion discovered mid-T03 \u2014 additive, no breaking impact.

## Known Limitations

All Monarch accounts treated as USD. transactions_updated doesn't track tag-set changes. Unified sync error message refers only to dormant providers. Monarch's unofficial GraphQL can break on upstream release.

## Follow-ups

S07 investments / holdings (Monarch holdings is read-only, needs native schema). Pin mmoney version if upstream breaks parsing.

## Files Created/Modified

None.
