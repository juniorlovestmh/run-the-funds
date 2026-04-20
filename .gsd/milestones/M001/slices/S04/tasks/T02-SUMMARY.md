---
id: T02
parent: S04
milestone: M001
key_files:
  - src/infrastructure/http.rs
  - src/infrastructure/mod.rs
  - src/infrastructure/exchange/bcb_ptax.rs
  - src/infrastructure/exchange/mod.rs
  - src/infrastructure/sync_adapter/mod.rs
  - src/infrastructure/sync_adapter/simplefin.rs
  - src/application/sync_service.rs
  - src/application/transaction_service.rs
  - src/application/mod.rs
  - src/cli/simplefin.rs
  - src/cli/sync.rs
  - src/cli/mod.rs
  - src/main.rs
  - Cargo.toml
key_decisions:
  - Three-method HttpClient trait (get, get_with_basic_auth, post) with default panic impls on the last two — small enough for easy fakes, covers every adapter shape.
  - persist_batch as a method on TransactionService, with import_from kept as a thin wrapper. SyncService duplicates ~20 lines of the persist-and-dedup logic rather than trying to share a service instance; duplication beat the generic gymnastics of constructing a TransactionService from references inside SyncService.
  - 2-year default backfill via a named FIRST_SYNC_BACKFILL_DAYS constant — explicit, easy to tune.
  - Per-account effective-since + single adapter.sync(earliest_since) — one HTTP call per provider per sync regardless of how many accounts are linked, with service-side filtering to respect each account's own cutoff.
  - SimpleFIN URL parser is hand-rolled (not url crate) — simpler given we just need to split on `://` and `@`; avoids a proc-macro-ish dep surface for a 15-line operation.
  - Two base64 alphabets (standard + URL-safe) tried in sequence — SimpleFIN's documented format is standard but we don't know every edge case the real bridge emits; URL-safe fallback is cheap insurance.
  - simplefin_setup validates the decoded URL starts with `http` — catches the common 'user pasted an access URL instead of a setup token' mistake early with a clear message.
duration: 
verification_result: passed
completed_at: 2026-04-19T21:18:46.331Z
blocker_discovered: false
---

# T02: Shared HttpClient trait in src/infrastructure/http.rs; persist_batch extracted from import_from; BankSyncAdapter trait + RemoteTransaction; SimpleFinAdapter with basic-auth URL parsing; SyncService with 2-year default backfill, per-account since, currency guard, FITID dedup, last_sync_at advancement; `rtf simplefin setup <token>` exchanges the base64 token and stores the access URL in the DB; `rtf sync --provider simplefin [--since]` runs the pipeline.

**Shared HttpClient trait in src/infrastructure/http.rs; persist_batch extracted from import_from; BankSyncAdapter trait + RemoteTransaction; SimpleFinAdapter with basic-auth URL parsing; SyncService with 2-year default backfill, per-account since, currency guard, FITID dedup, last_sync_at advancement; `rtf simplefin setup <token>` exchanges the base64 token and stores the access URL in the DB; `rtf sync --provider simplefin [--since]` runs the pipeline.**

## What Happened

Five coordinated pieces that land SimpleFIN end-to-end.

**1. HttpClient refactor.** Extracted the trait + `UreqHttpClient` impl from `src/infrastructure/exchange/bcb_ptax.rs` into `src/infrastructure/http.rs`. Expanded the trait to three methods: `get`, `get_with_basic_auth(url, user, pass)` (for SimpleFIN's URL-embedded-creds access URL), and `post(url, body, headers)` (for SimpleFIN setup-token exchange and later Pluggy auth). Default panic impls on the last two so test fakes only need to implement what they exercise. `bcb_ptax.rs` now imports from the shared module; all 8 BCB tests pass unchanged. `base64 = "0.22"` added to Cargo.toml — the production impl builds the Basic-auth header with `base64::engine::general_purpose::STANDARD.encode`.

**2. persist_batch refactor.** Split `TransactionService::import_from` into two methods — `import_from<I: Importer>` is now a thin wrapper around `persist_batch(account_id, Vec<Transaction>)` which carries the account-lookup + currency-validation + FITID-dedup + save flow. All S02 tests pass unchanged.

**3. BankSyncAdapter + RemoteTransaction.** New module `src/infrastructure/sync_adapter/`. `BankSyncAdapter` trait: `provider_name()` and `sync(since) -> Vec<(external_account_id, Vec<RemoteTransaction>)>`. `RemoteTransaction` carries `{external_id, date, amount, currency, payee, description}` — the minimum the service layer needs to build a domain Transaction.

**4. SimpleFinAdapter.** Parses the access URL (user:pass@host/path) into base URL + basic-auth creds. Builds `GET <base>/accounts?start-date=<unix>&pending=0`. Parses the JSON response, mapping per-account `id`/`currency`/`transactions[]` into `RemoteTransaction`s. Unix timestamps → `NaiveDate` via `DateTime::from_timestamp`. Amount is string-parsed for Decimal precision. Sign convention matches ours (no inversion). Surfaces the provider's top-level `errors` array as `DomainError::Import` so operator-side bank-connection issues are visible. 12 tests covering URL parsing (3), Basic-auth + start-date wiring (1), default-since-is-2y-back (1), multi-account parsing (1), provider-errors-surfaced (1), malformed-account (1), malformed-transaction (1), sign preservation (1), provider_name (1), HTTP error propagation (1).

**5. SyncService.** Generic over `TransactionRepository` + `AccountRepository`. `sync_provider<B: BankSyncAdapter>(adapter, since_override)` flow:
- Enumerate locally-linked accounts via `find_by_provider(adapter.provider_name())`. Empty list → return `{imported:0, ...}` envelope without calling the adapter.
- Compute per-account effective since: `--since` wins over `account.last_sync_at` which wins over `today - 730 days` (constant `FIRST_SYNC_BACKFILL_DAYS`).
- Call `adapter.sync(earliest_since_across_accounts)` once.
- For each returned `(external_id, remote_txns)`: match to local account via `find_by_external_link`; skip unlinked. Filter txns by per-account effective since (so an account synced-up-to-April-10 doesn't reimport March when another account triggered an earlier fetch window). Convert to domain `Transaction`s with UUID + account_id + Pending status + imported_at. Validate currency (two-pass, abort on mismatch). Dedup + save via `find_by_external_id`. Stamp `account.mark_synced(now)` and persist.
- Returns `ProviderSyncReport { provider, imported, duplicates, accounts_synced, window_start, window_end }`.

8 service tests: no-linked-accounts-no-call (1), persists + last_sync_at (1), default since 2-years (1), since-override-wins (1), only-linked-accounts-persisted (1), dedup-on-rerun (1), currency-mismatch-errors (1), date-filter-per-account (1).

**6. `rtf simplefin setup <token>`** (`src/cli/simplefin.rs`). Decodes the base64 setup token — tries standard, falls back to URL-safe — validates the decoded result starts with `http`, POSTs to it with empty body, takes the response as the access URL. Wraps `{"access_url": "<url>"}` as JSON, upserts into `provider_credentials` via the T01 repo. Re-running replaces the stored credentials (rotation). 6 tests: standard-b64, url-safe-b64, garbage rejected, non-http rejected, exchange-and-store persists, empty-response rejected.

**7. `rtf sync --provider simplefin [--since]`** (`src/cli/sync.rs`). Loads simplefin credentials from the DB; if missing, clear error message telling the user to run `simplefin setup`. Parses the access_url out of the stored JSON, constructs `SimpleFinAdapter` + `SyncService`, runs, prints `{status:"ok", data: ProviderSyncReport}`. `--provider` is required in T02; T04 makes it optional and runs both providers.

**Totals:** 248 unit tests + 3 convert_demo + 1 import_demo = 252 passing, 1 ignored (live BCB). Up from 226 after T01 (+22 new).

## Verification

`cargo test` → 248 unit + 4 integration = 252 passing, 1 ignored, 0 failed. `cargo build` → clean. `base64` dep compiles cleanly; ureq-based basic-auth header building works via the production `UreqHttpClient::get_with_basic_auth`.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 300ms |

## Deviations

None structurally. Added a couple of defensive tests beyond plan (empty-response rejection, URL-safe base64 fallback, malformed access URL) \u2014 all cheap.

## Known Issues

None.

## Files Created/Modified

- `src/infrastructure/http.rs`
- `src/infrastructure/mod.rs`
- `src/infrastructure/exchange/bcb_ptax.rs`
- `src/infrastructure/exchange/mod.rs`
- `src/infrastructure/sync_adapter/mod.rs`
- `src/infrastructure/sync_adapter/simplefin.rs`
- `src/application/sync_service.rs`
- `src/application/transaction_service.rs`
- `src/application/mod.rs`
- `src/cli/simplefin.rs`
- `src/cli/sync.rs`
- `src/cli/mod.rs`
- `src/main.rs`
- `Cargo.toml`
