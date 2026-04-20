---
estimated_steps: 21
estimated_files: 6
skills_used: []
---

# T04: Unified `rtf sync` + 2-year backfill verification + tests/sync_demo.rs + S04-UAT.md

Pull the two providers together behind a single command and lock the slice with an integration test + UAT doc.

**CLI update** (`src/cli/sync.rs`): `--provider` becomes optional. No flag → run both providers in order (simplefin, then pluggy):
- Skip a provider if its credentials are missing from the DB (log a one-line stderr note, do not error).
- Otherwise call `SyncService::sync_provider`.
- Aggregate into `SyncReport { simplefin: Option<ProviderSyncReport>, pluggy: Option<ProviderSyncReport>, errors: Vec<{provider, message}> }`. Serialize-able.
- Exit 0 if any provider succeeded OR all were unconfigured. Exit 1 only if every configured provider errored.
- Output: `{status:"ok", data: SyncReport}`.

**Backfill verification**: one service-level test asserting that when an account has `last_sync_at == None` and no `--since`, the adapter's `sync(since)` receives `Some(today - 730 days)`.

**Integration test** (`tests/sync_demo.rs`): shells to the compiled binary, spins up a tiny local mock HTTP server using only `std::net::TcpListener` (no new deps). Mock responds to SimpleFIN's `/accounts` and Pluggy's `/auth` + `/accounts` + `/transactions` endpoints with canned bodies. Scenarios:
1. Create US and BR accounts, run `accounts link` for each.
2. `simplefin setup` / `pluggy setup` with URLs pointing at the mock server (override via an env var or injectable config — design choice: add a hidden `RTF_SIMPLEFIN_BASE_URL_OVERRIDE` / `RTF_PLUGGY_BASE_URL_OVERRIDE` env var read only by the adapters, used only in test flows).
3. `rtf sync --provider simplefin` → transactions inserted, `last_sync_at` updated.
4. `rtf sync --provider pluggy` → same.
5. `rtf sync` (unified) → both providers ran, aggregate report shape asserted.
6. Re-run `rtf sync` → `imported: 0` per provider (dedup).
7. Backfill: fresh linked account, run sync, assert mock received `start-date` / `from` ~2 years back.
8. Missing creds on one provider: delete Pluggy creds, run `rtf sync` → SimpleFIN succeeds, Pluggy skipped with note, exit 0.
9. Currency mismatch: link a BRL local account to SimpleFIN (which returns USD) → CurrencyMismatch on that account; other simplefin accounts unaffected.

If the in-test mock HTTP server is too much scope, fall back to testing SyncService at the service level for scenarios 6–9 and using `tests/sync_demo.rs` only for the CLI-level setup + sync flows (1–5) via pre-seeded credentials pointing at a never-called base URL. Prefer to keep scope tight.

**Two `#[ignore]`d live smokes** in `tests/sync_demo.rs` invoking the real binary against real APIs. Run manually when credentials are set up.

**`.gsd/milestones/M001/slices/S04/S04-UAT.md`**: mirror S03's shape. Scenarios: `simplefin setup`, `pluggy setup`, `accounts link`, `sync --provider simplefin`, `sync --provider pluggy`, unified `sync`, backfill-on-first-run, incremental re-sync, missing-credentials graceful handling, currency mismatch, full test suite. Roadmap coverage table. Documented limitation: mixing manual S02 imports with sync will create dupes; user's real workflow is sync-only so not a blocker.

## Inputs

- `target/debug/rtf`
- `.gsd/milestones/M001/slices/S03/S03-UAT.md (template)`
- `src/cli/sync.rs (T02)`
- `src/application/sync_service.rs (T02)`

## Expected Output

- `Unified rtf sync CLI running both providers with error isolation`
- `2-year backfill default verified via service test`
- `tests/sync_demo.rs with offline scenarios + 2 ignored live smokes`
- `S04-UAT.md with full demo commands and Roadmap coverage table`

## Verification

cargo test && cargo test --test sync_demo
