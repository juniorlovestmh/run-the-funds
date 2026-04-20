# S04: Bank Sync Adapters (SimpleFIN + Pluggy)

**Goal:** Replace the manual OFX/QFX download-and-import flow with automated, API-driven bank sync. Two providers: SimpleFIN for US banks, Pluggy for Brazilian banks. Run The Funds manages its own credentials in a new `provider_credentials` table — user runs a one-time `rtf simplefin setup <token>` / `rtf pluggy setup ...` per provider, after which `rtf sync` just works with zero env-var management and zero manual steps. First-time sync for a freshly-linked account backfills the last 2 years automatically; subsequent syncs are incremental from `last_sync_at`.
**Demo:** rtf sync pulls US bank transactions via SimpleFIN and Brazilian bank transactions via Pluggy, reconciling them into the local DB with per-account last-sync-at bookkeeping. Secrets (SIMPLEFIN_TOKEN, PLUGGY_CLIENT_ID/SECRET) live in env vars, not the repo. Replaces the manual QFX/OFX download flow from S02 — no manual step required.

## Must-Haves

- `rtf simplefin setup <setup-token>` decodes the base64 setup token, POSTs to SimpleFIN to get the access URL, and persists it in the local DB. Re-running replaces the stored URL (for re-subscription). No env var needed.
- `rtf pluggy setup --client-id <x> --client-secret <y> --item-id <z>` persists Pluggy credentials in the local DB. No env var needed.
- `rtf accounts link --id <local-uuid> --provider <simplefin|pluggy> --external-id <remote-id>` persists the local↔remote account mapping.
- `rtf sync --provider simplefin` reads stored SimpleFIN creds, pulls new transactions since each account's `last_sync_at` (or 2-year backfill on first run), persists with external_id dedup, and updates `last_sync_at`.
- `rtf sync --provider pluggy` does the same via Pluggy.
- `rtf sync` (no flag) runs both providers sequentially with error isolation; one failing provider does not block the other. Aggregate report `{status:"ok", data:{simplefin:{...}, pluggy:{...}, errors:[...]}}`.
- `rtf sync --since 2024-01-01` overrides the per-account `last_sync_at` default.
- Re-running `rtf sync` immediately after a successful run returns `imported:0, duplicates:N` per provider (FITID dedup).
- Missing provider credentials surface as a clear error telling the user which setup command to run (e.g. *"SimpleFIN not configured — run `rtf simplefin setup <token>` first"*).
- `cargo test` passes. Unit tests use mocked HTTP; one integration test in `tests/sync_demo.rs` covers the full flow end-to-end against a local mock HTTP server. Two `#[ignore]`d live smokes (one per provider) exist for manual verification.

## Proof Level

- This slice proves: contract — two real bank-sync providers work end-to-end with zero manual user steps beyond a one-time `setup` subcommand per provider. Credentials live in the local DB, not env vars. Downstream slices can assume transactions arrive automatically via `rtf sync`.

## Integration Closure

- Upstream surfaces consumed: `Account` + `AccountRepository` (S01); `Transaction` + `TransactionRepository` + `find_by_external_id` (S02); `TransactionService::import_from` logic (refactored in T02 into shared `persist_batch`); `HttpClient` trait from `bcb_ptax.rs` (T02 promotes it to a shared module).
- New wiring: migration 003 adds `external_provider`/`external_account_id`/`last_sync_at` to `accounts`; migration 004 creates `provider_credentials` table; new `ProviderCredentialsRepository` + SQLite impl; new `BankSyncAdapter` trait + `SimpleFinAdapter` + `PluggyAdapter`; new `SyncService`; CLI commands `accounts link`, `simplefin setup`, `pluggy setup`, `sync`.
- What remains: S05 (categorization engine) + downstream slices all consume transactions the sync pipeline has populated.

## Verification

- Structured sync envelope: per-provider `{imported, duplicates, accounts_synced, window_start, window_end}`.
- Missing-credentials errors name the exact setup command to run, not just "missing config".
- Per-provider error isolation: one provider's HTTP failure doesn't mask the other's success in unified `rtf sync`.
- First-fetch stderr note per account (`"syncing simplefin account <id> since <date>..."`).
- Credentials are plaintext in the DB, same security level as a gitignored `.env` file. Document: treat `rtf.db` like any other credential store; user-only file permissions assumed.

## Tasks

- [x] **T01: Account linkage + provider credentials storage (migrations 003 + 004, repos, `accounts link` CLI)** `est:2h`
  Foundation for both sync providers: schema + domain + repos for account↔remote mapping AND for stored credentials.

**Migration 003** (`migrations/003_account_external_link.sql`):
```sql
ALTER TABLE accounts ADD COLUMN external_provider TEXT;
ALTER TABLE accounts ADD COLUMN external_account_id TEXT;
ALTER TABLE accounts ADD COLUMN last_sync_at TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_external_link
    ON accounts(external_provider, external_account_id)
    WHERE external_provider IS NOT NULL;
```
Partial unique index — unlinked accounts (NULL provider) coexist freely; linked accounts cannot duplicate a (provider, external_id) pair.

**Migration 004** (`migrations/004_provider_credentials.sql`):
```sql
CREATE TABLE IF NOT EXISTS provider_credentials (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL UNIQUE,
    data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```
`data` is a JSON blob whose shape depends on the provider (SimpleFIN: `{"access_url":"https://user:pass@host/path"}`; Pluggy: `{"client_id":...,"client_secret":...,"item_id":...}`). UNIQUE on `provider` so there's only one cred set per provider.

Register both migrations in `src/infrastructure/storage/migrations.rs`.

**Domain** (`src/domain/account/account.rs`): extend `Account` with `external_provider: Option<String>`, `external_account_id: Option<String>`, `last_sync_at: Option<DateTime<Utc>>`. Helpers: `link(provider, external_id)`, `mark_synced(at)`.

**Domain** (`src/domain/credentials/`, new module): new `ProviderCredentials` domain struct `{ id, provider, data: String (JSON), created_at, updated_at }`. Trait `ProviderCredentialsRepository` with `save`, `find_by_provider(provider) -> Option<ProviderCredentials>`, `delete(provider)`. Keep the JSON opaque at this layer — each adapter parses its own shape in T02/T03.

**Repos** (`src/infrastructure/storage/`):
- `account_repo.rs`: update row mapping + save SQL for the three new columns. Add `find_by_external_link(provider, external_id)` and `find_by_provider(provider)` methods to the trait + impl.
- New `credentials_repo.rs`: `SqliteProviderCredentialsRepository` implementing the trait. `save` uses INSERT OR REPLACE on the (provider) UNIQUE to upsert.

**CLI** (`src/cli/accounts.rs` + wiring): new `accounts link` subcommand:
- `rtf accounts link --id <local-uuid> --provider <simplefin|pluggy> --external-id <remote-id>`
- Validates provider string, looks up local account, calls `link`, saves. Rejects a second link on an already-linked account unless `--force`.
- `{status:"ok", data: <account>}` envelope.

**Security posture** — add a one-paragraph note to `.gsd/milestones/M001/slices/S04/SECURITY.md`: credentials are plaintext in the SQLite DB; treat `rtf.db` like `.aws/credentials` (user-only file perms, not checked in, encrypted-volume recommended). Keychain integration is an out-of-scope future enhancement.

**TDD**:
- Domain: `link` sets both fields; `mark_synced` updates timestamp; serde round-trips.
- Account repo: migration 003 applied; `find_by_external_link` roundtrip; `find_by_provider` returns only matching rows; partial unique index rejects dup (provider, external_id); multiple NULL-provider accounts coexist.
- Credentials repo: migration 004 applied; save/find_by_provider roundtrip; upsert on duplicate provider; delete removes the row.
- CLI `accounts link`: success path, unknown provider rejected, double-link rejected without `--force`, `--force` overrides.
  - Files: `migrations/003_account_external_link.sql`, `migrations/004_provider_credentials.sql`, `src/infrastructure/storage/migrations.rs`, `src/infrastructure/storage/account_repo.rs`, `src/infrastructure/storage/credentials_repo.rs`, `src/infrastructure/storage/mod.rs`, `src/domain/account/account.rs`, `src/domain/credentials/mod.rs`, `src/domain/credentials/credentials.rs`, `src/domain/credentials/repository.rs`, `src/domain/mod.rs`, `src/application/account_service.rs`, `src/cli/accounts.rs`, `src/cli/mod.rs`, `src/main.rs`, `.gsd/milestones/M001/slices/S04/SECURITY.md`
  - Verify: cargo test -- domain::account && cargo test -- domain::credentials && cargo test -- infrastructure::storage::account_repo && cargo test -- infrastructure::storage::credentials_repo && cargo test -- cli::accounts

- [x] **T02: SimpleFIN adapter + `simplefin setup` + shared HttpClient + `persist_batch` refactor + `sync --provider simplefin`** `est:3.5h`
  First concrete provider. Establishes all the shared patterns (HTTP seam, BankSyncAdapter trait, persist_batch service method, setup→DB flow) that T03 reuses for Pluggy.

**Refactor: extract `HttpClient`** into `src/infrastructure/http.rs`:
- Move the trait + `UreqHttpClient` impl out of `bcb_ptax.rs`.
- Extend with `get_with_basic_auth(url, user, pass) -> Result<String, DomainError>` (needed for SimpleFIN access URL) and `post(url, body: &str, headers: &[(&str, &str)]) -> Result<String, DomainError>` (needed for setup-token exchange and Pluggy auth).
- `bcb_ptax.rs` re-imports from `crate::infrastructure::http`. Existing BCB tests must pass unchanged.

**Refactor: split `TransactionService::import_from`**:
- New `persist_batch(&self, account_id: &str, transactions: Vec<Transaction>) -> Result<ImportReport, DomainError>` — the account-lookup + currency-validation + dedup + save logic, extracted verbatim.
- `import_from<I: Importer>(...)` becomes a thin wrapper calling `persist_batch`.
- All existing S02 tests pass unchanged.

**New abstraction** (`src/infrastructure/sync_adapter/mod.rs`):
```rust
pub struct RemoteTransaction {
    pub external_id: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub currency: CurrencyCode,
    pub payee: Option<String>,
    pub description: Option<String>,
}

pub trait BankSyncAdapter {
    fn provider_name(&self) -> &'static str;
    fn sync(&self, since: Option<NaiveDate>)
        -> Result<Vec<(String, Vec<RemoteTransaction>)>, DomainError>;
}
```

**SimpleFIN adapter** (`src/infrastructure/sync_adapter/simplefin.rs`):
- Constructed with an HttpClient + access URL string (loaded from the credentials repo by the service layer).
- Access URL contains embedded basic auth (`https://user:pass@host/path`). Parse with `url::Url`.
- `sync(since)` → `GET <base>/accounts?start-date=<unix>&pending=0` with Basic auth header. `start-date` defaults to today − 730 days if `since` is None.
- Parse `response.accounts[]`: each account has `id`, `currency`, `transactions[]`. Each transaction has `id`, `posted` (unix timestamp), `amount` (string), `description`. Map to RemoteTransaction.
- Currency: parse `CurrencyCode::from_str(account.currency)`.
- SimpleFIN's sign convention matches ours (negative for debits) — no inversion needed.

**`rtf simplefin setup <setup-token>` subcommand** (`src/cli/simplefin.rs`):
- Setup token is base64-encoded URL. Decode it.
- `POST <decoded-url>` with empty body; response body IS the access URL.
- Wrap `{"access_url": "<url>"}` as JSON, upsert into `provider_credentials` via the T01 repo.
- Output: `{status:"ok", data: {"message":"SimpleFIN configured; run 'rtf sync' to pull transactions."}}`.
- Supports re-running with a new token (upsert replaces old credentials).
- `url` crate already in dep tree via `ureq`; base64 comes from a new thin dep `base64 = "0.22"`.

**`SyncService`** (`src/application/sync_service.rs`):
- Generic over `AccountRepository` + `TransactionRepository` + `ProviderCredentialsRepository`.
- `sync_provider<A: BankSyncAdapter>(adapter: &A, since_override: Option<NaiveDate>) -> Result<ProviderSyncReport, DomainError>`:
  1. Enumerate linked accounts via `account_repo.find_by_provider(adapter.provider_name())`.
  2. If none, return `ProviderSyncReport { accounts_synced: 0, ... }` with no error.
  3. Compute effective start date per account: `since_override > account.last_sync_at > today - 730d`. Use the earliest of those to minimize per-provider HTTP calls.
  4. Call `adapter.sync(effective_since)` once.
  5. For each (external_account_id, remote_txns) from the adapter: find matching local account (skip if not linked); filter txns to `date >= account_effective_since`; map to domain Transactions; call `txn_svc.persist_batch`. Update `account.last_sync_at = Utc::now()`.
  6. Aggregate into `ProviderSyncReport { provider, imported, duplicates, accounts_synced, window_start, window_end }`.

**CLI** (`src/cli/sync.rs`, `cli/mod.rs`, `main.rs`): `rtf sync --provider simplefin [--since YYYY-MM-DD]`:
- Requires `--provider` in this task (T04 makes it optional).
- Loads credentials from DB; if missing, emit a clear error: *"SimpleFIN not configured — run `rtf simplefin setup <token>` first"*.
- Instantiate SimpleFinAdapter + SyncService, run, print envelope.

**TDD**:
- HttpClient extraction: BCB tests still pass.
- SimpleFIN setup: FakeHttpClient with canned access-URL response; setup command parses, stores, verifies via repo.
- SimpleFIN adapter: FakeHttpClient with canned /accounts JSON body (realistic shape, 2 accounts with 3 and 4 transactions); assert correct mapping, sign preservation, currency per account.
- SyncService: locally-link 2 of 3 external accounts; sync; assert only the 2 linked are persisted, last_sync_at updated on both, third ignored. Currency mismatch on one account → that account fails; others succeed.
- Missing-credentials error wording verified.
- `#[ignore]`d live smoke: `simplefin_live_smoke` hits real SimpleFIN with the developer's stored token; asserts non-empty response.
  - Files: `src/infrastructure/http.rs`, `src/infrastructure/mod.rs`, `src/infrastructure/exchange/bcb_ptax.rs`, `src/infrastructure/sync_adapter/mod.rs`, `src/infrastructure/sync_adapter/simplefin.rs`, `src/application/sync_service.rs`, `src/application/transaction_service.rs`, `src/application/mod.rs`, `src/cli/simplefin.rs`, `src/cli/sync.rs`, `src/cli/mod.rs`, `src/main.rs`, `Cargo.toml`
  - Verify: cargo test -- infrastructure::http && cargo test -- infrastructure::sync_adapter::simplefin && cargo test -- application::sync_service && cargo test -- application::transaction_service && cargo test -- cli::simplefin && cargo test -- infrastructure::exchange

- [x] **T03: Pluggy adapter + `pluggy setup` + `sync --provider pluggy`** `est:3h`
  Second provider. Reuses BankSyncAdapter + HttpClient + SyncService + persist_batch from T02.

**`rtf pluggy setup` subcommand** (`src/cli/pluggy.rs`):
- `rtf pluggy setup --client-id <x> --client-secret <y> --item-id <z>`.
- Stores `{"client_id":...,"client_secret":...,"item_id":...}` JSON in `provider_credentials`. No network call at setup time — auth happens lazily on first sync.
- Upsert replaces existing creds (e.g. new item-id after re-linking a bank).

**Pluggy adapter** (`src/infrastructure/sync_adapter/pluggy.rs`):
- Constructed with HttpClient + `{client_id, client_secret, item_id}` (loaded by the service from credentials repo).
- Lazy auth cache: first method call runs `POST https://api.pluggy.ai/auth` with JSON `{"clientId":...,"clientSecret":...}` → `{"apiKey": "..."}`; cache apiKey in the adapter for its lifetime (one process run = one token).
- `sync(since)`:
  1. Resolve auth (cached).
  2. `GET https://api.pluggy.ai/accounts?itemId=<item_id>` with `X-API-KEY` header → list of accounts with `id`, `currencyCode`, `name`.
  3. For each account: `GET https://api.pluggy.ai/transactions?accountId=<id>&from=<YYYY-MM-DD>&pageSize=500&page=1`. Walk pagination via response's `totalPages` field.
  4. Map each transaction: `id → external_id`, `date → NaiveDate`, `amount → Decimal`, `description`, `merchant.name` (fallback to `description`) → payee.
- **Sign inversion**: Pluggy emits positive amounts with a separate `type` field (`DEBIT`/`CREDIT`). Our convention: debits negative, credits positive. Invert when `type == "DEBIT"`.
- Currency: `CurrencyCode::from_str(account.currencyCode)`.

**HttpClient** grows `post_json(url, body, headers)` if not already added in T02; otherwise use the existing `post`.

**CLI**: extend `rtf sync --provider pluggy [--since YYYY-MM-DD]`.

**Docs** (`.gsd/milestones/M001/slices/S04/PLUGGY_SETUP.md`): one-time steps to get clientId/clientSecret/itemId:
1. Register at pluggy.ai and create an Application → get `clientId` + `clientSecret`.
2. Run Pluggy's Connect UI in a browser (they provide a hosted demo + client sandbox), link the Brazilian bank, copy the resulting `itemId` from the response.
3. `rtf pluggy setup --client-id <x> --client-secret <y> --item-id <z>`.
4. `rtf sync --provider pluggy`.
Short (<1 page). Include note: itemId is per-bank; re-run setup with a new itemId to switch banks.

**TDD**:
- Setup command persists the three-field JSON blob.
- Auth step: FakeHttpClient with canned `{apiKey}` response; adapter stores the key on first call, reuses on second.
- Auth failure (401) surfaces clear error.
- Happy path: 2 accounts, 2 pages each, 5 + 3 transactions → adapter yields 2 (account_id, Vec[5]) / (account_id, Vec[3]) tuples. Sign inversion verified on DEBIT entries.
- Missing creds error wording.
- Pagination: totalPages=3 → 3 transaction calls per account. totalPages=1 → one call.
- Rate limit (429) surfaces an Import error preserving the retry-after hint if present.
- `#[ignore]`d live smoke: `pluggy_live_smoke`; runs against real Pluggy with stored creds; asserts non-empty response.
  - Files: `src/infrastructure/sync_adapter/pluggy.rs`, `src/infrastructure/http.rs`, `src/cli/pluggy.rs`, `src/cli/sync.rs`, `src/cli/mod.rs`, `src/main.rs`, `.gsd/milestones/M001/slices/S04/PLUGGY_SETUP.md`
  - Verify: cargo test -- infrastructure::sync_adapter::pluggy && cargo test -- cli::pluggy && cargo test -- application::sync_service && cargo test -- infrastructure::http

- [x] **T04: Unified `rtf sync` + 2-year backfill verification + tests/sync_demo.rs + S04-UAT.md** `est:2.5h`
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
  - Files: `src/cli/sync.rs`, `src/application/sync_service.rs`, `src/infrastructure/sync_adapter/simplefin.rs`, `src/infrastructure/sync_adapter/pluggy.rs`, `tests/sync_demo.rs`, `.gsd/milestones/M001/slices/S04/S04-UAT.md`
  - Verify: cargo test && cargo test --test sync_demo

## Files Likely Touched

- migrations/003_account_external_link.sql
- migrations/004_provider_credentials.sql
- src/infrastructure/storage/migrations.rs
- src/infrastructure/storage/account_repo.rs
- src/infrastructure/storage/credentials_repo.rs
- src/infrastructure/storage/mod.rs
- src/domain/account/account.rs
- src/domain/credentials/mod.rs
- src/domain/credentials/credentials.rs
- src/domain/credentials/repository.rs
- src/domain/mod.rs
- src/application/account_service.rs
- src/cli/accounts.rs
- src/cli/mod.rs
- src/main.rs
- .gsd/milestones/M001/slices/S04/SECURITY.md
- src/infrastructure/http.rs
- src/infrastructure/mod.rs
- src/infrastructure/exchange/bcb_ptax.rs
- src/infrastructure/sync_adapter/mod.rs
- src/infrastructure/sync_adapter/simplefin.rs
- src/application/sync_service.rs
- src/application/transaction_service.rs
- src/application/mod.rs
- src/cli/simplefin.rs
- src/cli/sync.rs
- Cargo.toml
- src/infrastructure/sync_adapter/pluggy.rs
- src/cli/pluggy.rs
- .gsd/milestones/M001/slices/S04/PLUGGY_SETUP.md
- tests/sync_demo.rs
- .gsd/milestones/M001/slices/S04/S04-UAT.md
