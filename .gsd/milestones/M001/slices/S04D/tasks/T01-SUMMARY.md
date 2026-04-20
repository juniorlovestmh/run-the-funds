---
id: T01
parent: S04D
milestone: M001
key_files:
  - src/cli/connections.rs
  - src/cli/mod.rs
  - src/main.rs
  - src/domain/connections/repository.rs
  - src/infrastructure/storage/connections_repo.rs
key_decisions:
  - Single handler with mutually-exclusive arg groups (not two separate subcommands). Smaller surface, clearer errors.
  - Deletion doesn't cascade to transactions — removing a connection unlinks future syncs but preserves historical data. User-friendly since re-linking the same bank would dedup via FITID anyway.
  - `find_by_id` added to the repo trait (not just a one-off SQL in the CLI) so the lookup is consistent with the other find_* methods.
duration: 
verification_result: passed
completed_at: 2026-04-20T00:03:38.248Z
blocker_discovered: false
---

# T01: `fintrack connections remove --id <uuid>` and `--provider <p> --external-id <id>` CLI commands; added find_by_id to the repo; 7 unit tests; live-verified by cleaning the Legacy Teller row from the user's real DB.

**`fintrack connections remove --id <uuid>` and `--provider <p> --external-id <id>` CLI commands; added find_by_id to the repo; 7 unit tests; live-verified by cleaning the Legacy Teller row from the user's real DB.**

## What Happened

Single-task follow-up slice closing the `connections remove` gap flagged in S04C-UAT.

**Repository:** added `find_by_id(id)` to `ProviderConnectionRepository` trait + `SqliteProviderConnectionRepository` impl. Complements the existing `find_by_external_id` and `delete`.

**CLI:** new `ConnectionsCommands::Remove { id, provider, external_id }` variant. `handle_remove` delegates to `resolve_target` which enforces mutually-exclusive arg groups:
- `--id <uuid>` → find_by_id path
- `--provider <p> --external-id <id>` → find_by_external_id path
- Both arg groups → Validation error
- Neither → Validation error with guidance
- Partial (provider without external_id or vice versa) → Validation error

Not-found → `DomainError::NotFound`, exit 1.

**Tests (+7):** resolve_target happy paths (by id, by provider+external_id), 2× not-found cases, 3× validation errors (both groups, no args, partial provider pair).

**Live verification:** ran `fintrack connections remove --provider teller --external-id token_2czxt73goplp3kmpfogkbj53vm` against user's real DB to delete the Legacy row from S04C's migration 006. Confirmed via `connections list` that only the fresh Capital One enrollment remains. 1,508 real transactions across all 6 linked accounts remained intact — deletion of a provider_connections row does NOT cascade to transactions (they're keyed to the local account_id, independent of the enrollment metadata).

**User follow-up noted:** sync for the Chase account will need a fresh `fintrack teller connect` for Chase, since the Legacy-token-based connection is now gone. Existing Chase transactions in the DB stay put; only future syncs need the new enrollment.

**Totals:** 318 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = **333 passing**, 5 ignored, 0 failed. Up from 326 after S04C (+7 new).

## Verification

`cargo test` → 333 passing, 5 ignored, 0 failed. Live: removed the Legacy Teller row from user's real DB; transactions preserved; connections list shows only the Capital One enrollment.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 1200ms |
| 2 | `fintrack connections remove --provider teller --external-id token_2czxt73goplp3kmpfogkbj53vm` | 0 | pass | 50ms |
| 3 | `fintrack connections list` | 0 | pass | 30ms |

## Deviations

None.

## Known Issues

None."

## Files Created/Modified

- `src/cli/connections.rs`
- `src/cli/mod.rs`
- `src/main.rs`
- `src/domain/connections/repository.rs`
- `src/infrastructure/storage/connections_repo.rs`
