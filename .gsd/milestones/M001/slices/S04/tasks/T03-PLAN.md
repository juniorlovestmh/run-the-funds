---
estimated_steps: 32
estimated_files: 7
skills_used: []
---

# T03: Pluggy adapter + `pluggy setup` + `sync --provider pluggy`

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

## Inputs

- `src/infrastructure/sync_adapter/mod.rs (T02)`
- `src/infrastructure/http.rs (T02)`
- `src/application/sync_service.rs (T02)`
- `src/cli/simplefin.rs (T02 template)`

## Expected Output

- `PluggyAdapter with lazy auth cache, item-scoped fetching, pagination, sign inversion`
- ``rtf pluggy setup --client-id --client-secret --item-id` CLI`
- `CLI: rtf sync --provider pluggy`
- `PLUGGY_SETUP.md with the one-time browser-based itemId setup`

## Verification

cargo test -- infrastructure::sync_adapter::pluggy && cargo test -- cli::pluggy && cargo test -- application::sync_service && cargo test -- infrastructure::http
