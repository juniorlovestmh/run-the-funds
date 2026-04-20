---
estimated_steps: 29
estimated_files: 9
skills_used: []
---

# T01: TellerAdapter + `teller setup` CLI + unified sync dispatch + tests

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

## Inputs

- `src/infrastructure/sync_adapter/simplefin.rs (template)`
- `src/infrastructure/sync_adapter/pluggy.rs (template)`
- `src/cli/simplefin.rs (template)`
- `src/cli/sync.rs (unified run to extend)`

## Expected Output

- `TellerAdapter with cursor pagination + Bearer auth`
- `rtf teller setup CLI`
- `rtf sync --provider teller dispatch + unified 3-provider run`
- `UnifiedSyncReport with third slot`
- `TELLER_SETUP.md`
- `+~12 unit tests and +3 integration tests`

## Verification

cargo test -- infrastructure::sync_adapter::teller && cargo test -- cli::teller && cargo test --test sync_demo && cargo test
