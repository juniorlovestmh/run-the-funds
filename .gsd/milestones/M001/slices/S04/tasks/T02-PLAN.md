---
estimated_steps: 59
estimated_files: 13
skills_used: []
---

# T02: SimpleFIN adapter + `simplefin setup` + shared HttpClient + `persist_batch` refactor + `sync --provider simplefin`

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

## Inputs

- `src/infrastructure/exchange/bcb_ptax.rs (HttpClient to extract)`
- `src/application/transaction_service.rs (import_from to refactor)`
- `src/domain/credentials/ (T01)`
- `src/domain/account/account.rs (with T01 fields)`

## Expected Output

- `Shared HttpClient trait + UreqHttpClient in src/infrastructure/http.rs`
- `BankSyncAdapter trait + RemoteTransaction in src/infrastructure/sync_adapter/mod.rs`
- `SimpleFinAdapter parsing access URL from stored credentials`
- ``rtf simplefin setup <token>` one-time exchange + DB persistence`
- `persist_batch extracted from import_from, behaviors preserved`
- `SyncService::sync_provider with 2-year backfill default`
- `CLI: rtf sync --provider simplefin [--since YYYY-MM-DD]`
- `base64 dep added for setup-token decoding`

## Verification

cargo test -- infrastructure::http && cargo test -- infrastructure::sync_adapter::simplefin && cargo test -- application::sync_service && cargo test -- application::transaction_service && cargo test -- cli::simplefin && cargo test -- infrastructure::exchange
