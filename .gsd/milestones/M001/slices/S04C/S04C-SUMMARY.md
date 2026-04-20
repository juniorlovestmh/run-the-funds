---
id: S04C
parent: M001
milestone: M001
provides:
  - ["provider_connections multi-bank schema", "ConnectServer + launch_browser infrastructure", "rtf teller connect / pluggy connect browser flows", "rtf connections list redacted view", "Multi-enrollment sync aggregation", "Migration 005/006 with legacy-data preservation", "S04 HTTP body-on-error fix (cross-cutting)"]
requires:
  - slice: S04
    provides: BankSyncAdapter + HttpClient + SyncService + adapters
  - slice: S04B
    provides: Teller adapter + mTLS
affects:
  []
key_files:
  - ["migrations/005_provider_connections.sql", "migrations/006_migrate_connections.sql", "src/domain/connections/", "src/infrastructure/connect/mod.rs", "src/infrastructure/storage/connections_repo.rs", "src/cli/teller.rs", "src/cli/teller_connect.html", "src/cli/pluggy.rs", "src/cli/pluggy_connect.html", "src/cli/connections.rs", "src/cli/sync.rs", "tests/sync_demo.rs", ".gsd/milestones/M001/slices/S04C/S04C-UAT.md"]
key_decisions:
  - (none)
patterns_established:
  - (none)
observability_surfaces:
  - none
drill_down_paths:
  []
duration: ""
verification_result: passed
completed_at: 2026-04-19T23:57:03.636Z
blocker_discovered: false
---

# S04C: Browser Connect flow + multi-bank support (Teller + Pluggy)

**Browser-driven `teller connect` / `pluggy connect` commands eliminate manual-token-paste; new provider_connections table supports N enrollments per provider; live-verified with real Teller banks (984 + 524 transactions in one sync call).**

## What Happened

Five-task slice that transforms the user-facing setup experience for bank-sync providers. See task summaries for details.

**Live Teller verification** (T05): user linked Capital One through `rtf teller connect` browser flow; `rtf sync --provider teller --since 2024-04-19` returned `{imported: 984, duplicates: 524, accounts_synced: 6, window: 2024-04-19 → 2026-04-19}` across two enrollments (Chase Legacy via migration + fresh Capital One) in one call.

**Pluggy code-complete, live verification pending** — user doesn't yet have Pluggy credentials. Full unit + integration test coverage; first live run will surface any SDK-version drift.

**Riding along:** S04 HTTP-body-on-error fix that was uncommitted since the SimpleFIN live-test; now every provider's 4xx/5xx surfaces the actual response body.

**Tests:** 311 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = 326 passing, 5 ignored, 0 failed. +34 since S04B.

## Verification

`cargo test` → 326 passing, 5 ignored, 0 failed. Live Teller sync: 984 imported + 524 duplicates across 2 enrollments, 6 accounts.

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

None.

## Known Limitations

["Pluggy live verification pending user credentials.", "Legacy enrollment row persists until manually cleaned (SQL); no connections remove CLI yet.", "Credentials at rest plaintext (unchanged)."]

## Follow-ups

["rtf connections remove --id <uuid>", "Pluggy live smoke", "Novo CSV adapter + Wise QFX path", "Keychain integration"]

## Files Created/Modified

None.
