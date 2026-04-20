---
id: T01
parent: S04C
milestone: M001
key_files:
  - migrations/005_provider_connections.sql
  - migrations/006_migrate_connections.sql
  - src/infrastructure/storage/migrations.rs
  - src/infrastructure/storage/connections_repo.rs
  - src/infrastructure/storage/mod.rs
  - src/infrastructure/storage/database.rs
  - src/domain/connections/mod.rs
  - src/domain/connections/connection.rs
  - src/domain/connections/repository.rs
  - src/domain/mod.rs
  - src/cli/connections.rs
  - src/cli/mod.rs
  - src/main.rs
key_decisions:
  - Access-token-as-synthetic-external-id for the legacy migration — preserves continuity without needing to call Teller's API during migration to fetch a real enrollment.id. Fresh `teller connect` later writes the real id.
  - UPSERT on (provider, external_id) — re-running connect refreshes the row rather than creating duplicates; matches the user expectation of 'this is MY Chase enrollment, not a different one'.
  - `data` blob is opaque in the domain layer — each provider's adapter parses its own shape. Same pattern as `provider_credentials`.
  - `connections list` redacts `data` by design — operator-facing listing should never print access_tokens.
  - RFC3339 timestamps written by the migration via strftime, not datetime('now'), to avoid the timezone-less default format drifting into the table.
duration: 
verification_result: passed
completed_at: 2026-04-19T22:56:34.976Z
blocker_discovered: false
---

# T01: provider_connections table + domain + SQLite repo + `fintrack connections list` CLI; migration 006 auto-splits existing Teller access_token into a connection row without data loss; verified against user's real DB.

**provider_connections table + domain + SQLite repo + `fintrack connections list` CLI; migration 006 auto-splits existing Teller access_token into a connection row without data loss; verified against user's real DB.**

## What Happened

Foundation layer for S04C multi-bank support.

**Schema:**
- Migration 005 creates `provider_connections (id, provider, external_id, data, institution_name, created_at, updated_at)` with `UNIQUE (provider, external_id)` and an index on `provider` for fast per-provider enumeration.
- Migration 006 data-migration: for each Teller row in `provider_credentials` that carried an `access_token`, insert a new `provider_connections` row using the access_token itself as a synthetic external_id, then strip the access_token out of the credentials row. Same pattern for Pluggy's `item_id`. Preserves the user's existing single-enrollment setup so sync keeps working between this commit and the T03 refactor.
- Both migrations use `strftime('%Y-%m-%dT%H:%M:%SZ', 'now')` for RFC3339 timestamps (cleaner than SQLite's default `datetime('now')` which drops the timezone and sometimes carries fractional seconds).

**Domain (`src/domain/connections/`):**
- `ProviderConnection { id, provider, external_id, data: String, institution_name: Option<String>, created_at, updated_at }`.
- Validation in `new()` — empty provider or external_id rejected.
- `ProviderConnectionRepository` trait: `save` (UPSERT), `find_by_provider`, `find_by_external_id`, `find_all`, `delete`.

**SQLite impl (`src/infrastructure/storage/connections_repo.rs`):**
- `save` uses `INSERT ... ON CONFLICT(provider, external_id) DO UPDATE` so re-running connect for the same bank refreshes `data` + `institution_name` + `updated_at` while `created_at` stays pinned to the first insert.
- Timestamp parser accepts both RFC3339 (what our repo writes) and naive `YYYY-MM-DD HH:MM:SS` (what older SQLite `datetime('now')` produces) — defensive for any data that was written pre-migration-fix.
- Tests: save+find_by_provider across both providers, UPSERT behavior, find_by_external_id per-provider scoping, find_all ordering, delete removal, and a migration-006 roundtrip that seeds a legacy-shaped credentials row and verifies the split.

**CLI — `fintrack connections list [--format json|table]`** (`src/cli/connections.rs`):
- Read-only view over `provider_connections`. Never exposes `data` (secrets live there).
- `ConnectionView` projection: `{id, provider, external_id, institution_name, created_at, updated_at}`.
- Table form truncates long external_ids so the human-readable output stays on-screen; JSON form prints full values.

**Live DB verification:** ran the new `fintrack connections list --format json` against the user's real `~/fintrack.db` post-migration. One Teller connection row present with the legacy access_token as external_id, labeled "Legacy (re-run `fintrack teller connect`)". The 524 transactions from S04B remain in the DB (transaction rows are unaffected — linkage is via `account_id`, not through `provider_connections`).

**Test totals:** 287 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = **302 passing**, 4 ignored (live smokes), 0 failed. Up from 293 after S04B — +9 new (6 repo + 3 domain).

## Verification

`cargo test` → 302 passing, 4 ignored, 0 failed. `cargo build --release` → clean. Real-DB smoke: `fintrack connections list --format json` against `~/fintrack.db` returns one Teller row auto-migrated from S04B's single-enrollment storage.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 370ms |
| 2 | `fintrack connections list --format json` | 0 | pass | 50ms |

## Deviations

None. Caught one migration-timestamp parsing issue during testing (datetime('now') returns no TZ offset and occasionally fractional seconds) and fixed it by forcing RFC3339 output via strftime. Added defensive naive-datetime parsing in the repo for any legacy rows that slipped through."

## Known Issues

None."

## Files Created/Modified

- `migrations/005_provider_connections.sql`
- `migrations/006_migrate_connections.sql`
- `src/infrastructure/storage/migrations.rs`
- `src/infrastructure/storage/connections_repo.rs`
- `src/infrastructure/storage/mod.rs`
- `src/infrastructure/storage/database.rs`
- `src/domain/connections/mod.rs`
- `src/domain/connections/connection.rs`
- `src/domain/connections/repository.rs`
- `src/domain/mod.rs`
- `src/cli/connections.rs`
- `src/cli/mod.rs`
- `src/main.rs`
