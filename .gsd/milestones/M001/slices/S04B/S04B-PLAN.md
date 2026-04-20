# S04B: Teller Adapter (third bank-sync provider)

**Goal:** Add Teller as a third bank-sync provider alongside SimpleFIN and Pluggy. Free developer tier covers personal-scale usage. One-time `rtf teller setup --access-token <TOKEN>` persists credentials; subsequent `rtf sync` runs all three configured providers with per-provider error isolation. One adapter + one setup CLI + one dispatch branch — no changes to SyncService, storage, or other adapters.
**Demo:** rtf teller setup --access-token <TOKEN> persists credentials; rtf sync --provider teller pulls US bank transactions via Teller's API; unified rtf sync runs simplefin + pluggy + teller with error isolation. No changes to service layer or storage.

## Must-Haves

- `rtf teller setup --access-token <TOKEN>` persists `{"access_token":"..."}` into `provider_credentials` under `provider = "teller"`. Re-running upserts.
- `rtf sync --provider teller` reads stored credentials, hits `api.teller.io` with `Authorization: Bearer <TOKEN>`, fetches accounts + per-account transactions (cursor pagination via `from_id`), persists with FITID dedup, updates `last_sync_at`.
- `rtf sync` (no flag) now runs all three providers; error isolation unchanged.
- `rtf accounts link --provider teller --external-id <teller-acct-id>` works.
- Missing Teller creds → clear "run `rtf teller setup --access-token <TOKEN>` first" error.
- `cargo test` passes. TellerAdapter unit tests with FakeHttpClient. Integration tests in `tests/sync_demo.rs` for the new provider's error + setup paths. One `#[ignore]`d live smoke.
- Teller's Development/Production tiers require mTLS; this slice ships Bearer-only. If the user is on a tier requiring mTLS, first live sync errors with a clear TLS-rejection body (improved HTTP error handler included). Adding mTLS is a follow-up.

## Proof Level

- This slice proves: contract — Teller is a fully-configured provider in the unified sync pipeline with identical UX to SimpleFIN/Pluggy. No changes to domain/service contracts.

## Integration Closure

- Upstream surfaces consumed: everything from S04 — BankSyncAdapter trait, HttpClient, SyncService, ProviderCredentialsRepository, Account::link, the unified sync CLI handler. Also: the S04 live-test bug-fix to UreqHttpClient (include HTTP body on 4xx/5xx) which is currently uncommitted in the working tree.
- New wiring: TellerAdapter, `rtf teller setup`, `sync --provider teller` dispatch branch, accounts-link provider validation extended to "teller", TELLER_SETUP.md, UnifiedSyncReport gets a third `teller` Option slot.
- What remains: S05 (categorization) onward — provider-agnostic.

## Verification

- Same per-provider envelope as SimpleFIN/Pluggy. Unified sync now has three optional slots.
- Missing-cred error names the setup command.
- HTTP-body-on-error fix (from S04 live-test) rides along in this slice's commit.
- Clear TLS-rejection surface if Teller tier requires mTLS.

## Tasks

- [x] **T01: TellerAdapter + `teller setup` CLI + unified sync dispatch + tests** `est:3h`
  Follows the SimpleFIN template — Teller's Bearer-token + REST JSON is very similar.

**TellerAdapter** (`src/infrastructure/sync_adapter/teller.rs`):
- Constructed with HttpClient + access_token + optional base URL (`https://api.teller.io` default, `#[cfg(test)]`-gated override).
- `sync(since)`:
  1. `GET <base>/accounts` with `Authorization: Bearer <TOKEN>` + `Accept: application/json` → JSON array of accounts (raw array at top level).
  2. For each account parse `id`, `currency` (3-letter ISO). Skip unsupported currencies silently (only USD/BRL in our domain).
  3. For each account: `GET <base>/accounts/{id}/transactions?from_date=YYYY-MM-DD&count=500&from_id=<cursor>`. Cursor-based pagination: on first page, omit `from_id`; on each subsequent page, pass the last transaction id. Stop when fewer than `count` transactions come back.
  4. Map each txn: `id → external_id`, `date` (ISO) → NaiveDate, `amount` (signed string decimal) → Decimal (no sign inversion; Teller pre-signs), `description` → description, `details.counterparty.name` → payee (fallback to description).

**`rtf teller setup --access-token <TOKEN>`** (`src/cli/teller.rs`):
- Non-empty validation.
- Stores `{"access_token":"<TOKEN>"}` JSON in `provider_credentials` via upsert.
- No network call at setup (lazy auth, like Pluggy).

**CLI glue:**
- New top-level `Teller { command: TellerCommands }` subcommand + `TellerCommands::Setup { access_token: String }`.
- `sync --provider teller` dispatches to `run_teller` (and `run_teller_inline` for unified).
- `UnifiedSyncReport` gains a `teller: Option<ProviderSyncReport>` slot; `run_unified` adds a third try-and-collect step.
- `accounts link` provider validator extended to accept `teller`.

**Unit tests:**
- Happy path: FakeHttpClient with canned `/accounts` + `/accounts/{id}/transactions` responses; adapter yields expected RemoteTransaction list with correct signs + currency + dates.
- Cursor pagination: first call returns `count` txns, second returns fewer → adapter walks twice and stops.
- Authorization header sent on every call — assert FakeHttpClient captured it.
- Unsupported currency skipped silently (account with `"currency":"EUR"` doesn't appear in output tuples).
- HTTP 4xx propagates with body preview (validates the S04 live-test fix).

**Integration tests (tests/sync_demo.rs):**
- `sync_missing_teller_credentials_has_clear_message`
- `teller_setup_stores_credentials`
- `accounts_link_accepts_teller_provider`
- `#[ignore] live_teller_sync_smoke` using `TELLER_ACCESS_TOKEN` env.

**TELLER_SETUP.md** (`.gsd/milestones/M001/slices/S04B/TELLER_SETUP.md`): sign up at teller.io → create Application → obtain enrollment access token via Teller Connect widget → `rtf teller setup --access-token ...` → `accounts link` → `sync`. Note mTLS tier requirement.
  - Files: `src/infrastructure/sync_adapter/teller.rs`, `src/infrastructure/sync_adapter/mod.rs`, `src/cli/teller.rs`, `src/cli/sync.rs`, `src/cli/mod.rs`, `src/cli/accounts.rs`, `src/main.rs`, `tests/sync_demo.rs`, `.gsd/milestones/M001/slices/S04B/TELLER_SETUP.md`
  - Verify: cargo test -- infrastructure::sync_adapter::teller && cargo test -- cli::teller && cargo test --test sync_demo && cargo test

- [x] **T02: Live-test Teller + S04B-UAT.md + commit (includes S04 leftover HTTP fix)** `est:1h`
  Manual verification + slice docs + rollup commit.

**Live flow** (user-driven):
1. User obtains an access token via Teller Connect (one-time browser step).
2. `rtf teller setup --access-token <TOKEN>`.
3. Discover account IDs via direct API call or curl (similar to our S04 discovery step).
4. `rtf accounts link --provider teller --external-id <id>` for each local account.
5. `rtf sync --provider teller` — real transactions come back (or clear error if mTLS is required by the tier).
6. Re-run — dedup.

**S04B-UAT.md**: mirrors S04's shape with setup/link/sync/unified/dedup/error scenarios.

**Commit**: one rollup containing T01 + T02 changes + the uncommitted S04 live-test HTTP fix. Message notes the HTTP improvement rides along.
  - Files: `.gsd/milestones/M001/slices/S04B/S04B-UAT.md`
  - Verify: cargo test && manual live-sync once credentials are in place

## Files Likely Touched

- src/infrastructure/sync_adapter/teller.rs
- src/infrastructure/sync_adapter/mod.rs
- src/cli/teller.rs
- src/cli/sync.rs
- src/cli/mod.rs
- src/cli/accounts.rs
- src/main.rs
- tests/sync_demo.rs
- .gsd/milestones/M001/slices/S04B/TELLER_SETUP.md
- .gsd/milestones/M001/slices/S04B/S04B-UAT.md
