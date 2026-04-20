---
id: T04
parent: S04
milestone: M001
key_files:
  - src/cli/sync.rs
  - tests/sync_demo.rs
  - .gsd/milestones/M001/slices/S04/S04-UAT.md
key_decisions:
  - UnifiedSyncReport with Option<Provider> slots + errors array — cleanly encodes both 'not configured' (null) and 'tried and failed' (in errors) without conflating them.
  - run_*_inline helpers that distinguish Ok(None) = not configured from Err = real failure — lets run_unified collect both cleanly and drive its exit-code logic.
  - Exit 0 as long as at least one provider succeeded, even if the other errored — matches the user expectation that a one-off outage at SimpleFIN shouldn't make fintrack sync return non-zero when Pluggy just worked.
  - Mix of integration + service tests rather than trying to stand up a mock HTTP server — tests/sync_demo.rs covers CLI error paths and roundtrips; service-layer tests from T02/T03 cover the sync happy path with FakeAdapter. Live smokes fill the real-API gap via opt-in runs.
  - Documented the credentials-in-DB revision in S04-UAT's roadmap table rather than silently changing the roadmap — keeps the spec change visible to future readers.
duration: 
verification_result: passed
completed_at: 2026-04-19T21:26:49.497Z
blocker_discovered: false
---

# T04: Unified `fintrack sync` runs SimpleFIN + Pluggy with per-provider error isolation and an aggregate SyncReport; tests/sync_demo.rs covers 8 CLI error and roundtrip scenarios + 2 ignored live smokes; S04-UAT.md documents the full user-facing flow.

**Unified `fintrack sync` runs SimpleFIN + Pluggy with per-provider error isolation and an aggregate SyncReport; tests/sync_demo.rs covers 8 CLI error and roundtrip scenarios + 2 ignored live smokes; S04-UAT.md documents the full user-facing flow.**

## What Happened

**Unified CLI (`src/cli/sync.rs`)** — `--provider` now optional. No flag → run both providers in sequence via `run_simplefin_inline` / `run_pluggy_inline` helpers that return `Ok(Some(report))` on success, `Ok(None)` when credentials aren't configured (skip silently), or `Err` on real failures. The `run_unified` handler collects both into a `UnifiedSyncReport { simplefin: Option, pluggy: Option, errors: Vec }` with per-provider error entries. Exit 0 if any provider succeeded; exit 1 with a guiding message if nothing was configured; exit 1 if every configured provider errored.

**Error isolation** drops out of the design naturally: one provider's `Err` becomes an entry in `errors[]` while the other provider still runs. No cross-contamination.

**Integration tests (`tests/sync_demo.rs`, 8 offline + 2 live):**
- `sync_missing_simplefin_credentials_has_clear_message` — before setup, clear *"SimpleFIN not configured — run …"* error.
- `sync_missing_pluggy_credentials_has_clear_message` — same for Pluggy.
- `sync_unknown_provider_errors` — bogus provider name rejected up front.
- `sync_unified_with_nothing_configured_guides_user` — no providers set → guide the user to run `setup` commands.
- `sync_malformed_since_errors` — `--since not-a-date` → structured error with format hint.
- `accounts_link_roundtrips_through_cli` — create → link → relink-rejected-without-force → relink-with-force all via the real binary.
- `accounts_link_rejects_unknown_provider` — bogus provider string rejected at the CLI boundary.
- `pluggy_setup_stores_credentials` — setup command roundtrips through the DB (verified via direct `fintrack::domain::credentials` lookup).
- `#[ignore] live_simplefin_setup_smoke` — real `simplefin setup` with `SIMPLEFIN_SETUP_TOKEN` env. Just asserts the envelope shape; a full sync-against-linked-account flow is beyond the smoke.
- `#[ignore] live_pluggy_sync_smoke` — setup + sync against live Pluggy with `PLUGGY_CLIENT_ID/SECRET/ITEM_ID`. Since no accounts are locally linked during the smoke, asserts `accounts_synced: 0` — confirms the auth flow + HTTP path + DB write don't explode.

**2-year backfill default** is already verified at the service level (T02: `sync_default_since_is_two_years_back`), which asserts the adapter receives `Some(today - ~730 days)` when `since_override` is None and `last_sync_at` is None. Plan called for a new test here but the T02 one covers the invariant at the right layer; didn't duplicate.

**`.gsd/milestones/M001/slices/S04/S04-UAT.md`** mirrors the S03 shape with 12 scenarios including explicit "what happens when X is missing" paths. Roadmap coverage table notes the scope revision: credentials are in the local DB rather than env vars. That's a better UX than the original plan and matches the "no manual workflows" directive the user laid down at S04 scoping time.

**Known limitation documented**: mixing S02 manual imports with S04 sync on the same account creates duplicates (different external_id namespaces). Not a real issue for this user's workflow (sync-only from day one) but flagged.

**Totals:** 262 unit + 3 convert_demo + 1 import_demo + 8 sync_demo = **274 passing, 0 failed, 4 ignored**.

## Verification

`cargo test` → 274 passing across 4 test artifacts, 4 ignored (1 BCB + 1 convert + 2 sync live smokes), 0 failed. `cargo test --test sync_demo` → 8 passed, 2 ignored, 0 failed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 330ms |
| 2 | `cargo test --test sync_demo` | 0 | pass | 640ms |

## Deviations

Plan suggested an in-test mock HTTP server for tests/sync_demo.rs; decided not to \u2014 adds ~100 lines of std::net::TcpListener boilerplate without catching bugs that the service-level FakeAdapter tests don't already catch. The CLI tests we kept (8 offline scenarios) exercise the code paths between the binary and the service layer without needing to mock HTTP. Plan's credentials-in-env-vars was revised to credentials-in-DB at user request during scoping; final design is documented in S04-UAT's roadmap table.

## Known Issues

None from T04 scope. The S04-level known limitations (manual+sync overlap, single-itemId per Pluggy config, plaintext credentials) are documented in S04-UAT.md and SECURITY.md."

## Files Created/Modified

- `src/cli/sync.rs`
- `tests/sync_demo.rs`
- `.gsd/milestones/M001/slices/S04/S04-UAT.md`
