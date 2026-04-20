---
id: T03
parent: S04
milestone: M001
key_files:
  - src/infrastructure/sync_adapter/pluggy.rs
  - src/infrastructure/sync_adapter/mod.rs
  - src/infrastructure/http.rs
  - src/cli/pluggy.rs
  - src/cli/sync.rs
  - src/cli/mod.rs
  - src/main.rs
  - .gsd/milestones/M001/slices/S04/PLUGGY_SETUP.md
key_decisions:
  - Sign derivation ALWAYS from `type` field, using abs(amount). Some Pluggy connectors pre-sign amounts, others don't; deriving from type is deterministic across connectors.
  - Lazy auth via RefCell<Option<apiKey>> — one POST /auth per process run, regardless of how many accounts are synced.
  - Two-layer HTTP trait growth: added get_with_headers (not get_with_header single) — Pluggy needs X-API-KEY + Accept, future adapters may want more.
  - Amount parsed from either JSON string OR f64 (4dp-safe via format/parse) — defensive against connector variance.
  - Base URL overridable via with_base_url(#[cfg(test)]-gated) — keeps production call sites slim and still allows mock HTTP servers in future integration tests.
  - payee fallback chain: merchant.name → description. Merchant data when present is cleaner; description is the raw bank memo.
duration: 
verification_result: passed
completed_at: 2026-04-19T21:23:12.874Z
blocker_discovered: false
---

# T03: PluggyAdapter with lazy-cached OAuth-ish auth, item-scoped account fetch, per-account transaction pagination, and DEBIT/CREDIT sign-from-type; `rtf pluggy setup --client-id --client-secret --item-id` persists credentials; `sync --provider pluggy` wired; PLUGGY_SETUP.md documents the one-time Connect UI flow.

**PluggyAdapter with lazy-cached OAuth-ish auth, item-scoped account fetch, per-account transaction pagination, and DEBIT/CREDIT sign-from-type; `rtf pluggy setup --client-id --client-secret --item-id` persists credentials; `sync --provider pluggy` wired; PLUGGY_SETUP.md documents the one-time Connect UI flow.**

## What Happened

Pluggy adapter implementing `BankSyncAdapter` alongside the existing SimpleFIN one.

**PluggyAdapter** (`src/infrastructure/sync_adapter/pluggy.rs`):
- Constructed with `HttpClient`, `client_id`, `client_secret`, `item_id`. Base URL defaults to `https://api.pluggy.ai` (overridable via `with_base_url` for tests; currently `#[cfg(test)]` only).
- **Lazy auth cache:** `RefCell<Option<String>>` for the apiKey. First method call runs `POST /auth` with `{"clientId":..., "clientSecret":...}` + `Content-Type: application/json` header. Caches the returned `apiKey` for the adapter's lifetime (one process = one token). Subsequent calls use `X-API-KEY: <apiKey>` + `Accept: application/json` via the new `HttpClient::get_with_headers`.
- **Fetch flow:**
  - `GET /accounts?itemId=<ITEM_ID>` — returns `results[]` with `id` and `currencyCode`. No pagination (accounts are few).
  - For each account: `GET /transactions?accountId=<ID>&from=YYYY-MM-DD&pageSize=500&page=N`, looping 1..=totalPages.
- **Transaction mapping:**
  - `id → external_id`.
  - `date` (ISO datetime like `2024-04-08T13:00:00Z`) → first 10 chars parsed as `%Y-%m-%d`.
  - `amount` accepted as JSON number OR string (some connectors differ); always taken as magnitude (`abs`).
  - Sign derived from `type`: `DEBIT → -magnitude`, `CREDIT → +magnitude`. Unknown `type` → `DomainError::Import`. Safer than trusting whatever sign the connector emits — several of Pluggy's connectors pre-sign, others don't.
  - `description` → description; `merchant.name` preferred over description for `payee`, falling back to description when absent.
- Currency from each account's `currencyCode` (parsed via `CurrencyCode::from_str`).

**HttpClient trait extension:** added `get_with_headers(url, headers)` with the default "not implemented" impl. Production `UreqHttpClient` implements it for Pluggy's multi-header needs (X-API-KEY + Accept).

**Tests (+11):** provider_name, happy-path-debit with sign inversion, credit-keeps-positive, already-negative-amount-still-respects-type (proves sign derivation from `type` is authoritative), auth cached across calls (2 accounts → 1 POST + 1 accounts GET + 2 transactions GETs = 4 calls), pagination walks all pages (3-page response), auth-failure propagates, auth-response-missing-apiKey errors, merchant-name-preferred-as-payee, unknown-type errors, HTTP error on accounts endpoint propagates.

**CLI `rtf pluggy setup`** (`src/cli/pluggy.rs`):
- `--client-id`, `--client-secret`, `--item-id` all required non-empty.
- Stores `{"client_id":..., "client_secret":..., "item_id":...}` JSON blob in `provider_credentials` via upsert. No network call at setup (auth happens lazily on first sync).
- +3 tests: persist round-trip, upsert-on-rerun, empty-field rejections.

**CLI `sync --provider pluggy`** (`src/cli/sync.rs`): parallels the simplefin path. Loads pluggy credentials from DB; clear error if missing telling the user to run `pluggy setup`. Parses the three-field JSON, constructs `PluggyAdapter` + `SyncService`, runs, prints report.

**PLUGGY_SETUP.md** (`.gsd/milestones/M001/slices/S04/PLUGGY_SETUP.md`): step-by-step for the one-time Connect UI flow to obtain `itemId` — register app → get clientId/secret → exchange for apiKey → create Connect Token → open Pluggy's hosted UI → link bank → copy itemId → run `rtf pluggy setup`. Includes the curl commands for the non-browser steps and notes on rotation.

**Totals:** 262 unit + 3 convert_demo + 1 import_demo = 266 passing, 1 ignored. Up from 252 after T02 (+14 new: 11 Pluggy adapter + 3 Pluggy CLI).

## Verification

`cargo test` → 262 unit + 4 integration = 266 passing, 1 ignored, 0 failed. `cargo build` → clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 310ms |

## Deviations

None. Plan called for +1 ignored live smoke; deferred to T04 where it joins the SimpleFIN live smoke in `tests/sync_demo.rs` so both sit together.

## Known Issues

PLUGGY_SETUP.md references `https://connect.pluggy.ai/?connect_token=<TOKEN>` as the widget launch URL \u2014 Pluggy's docs sometimes change this; user should consult the current reference if the URL 404s. Not a code issue."

## Files Created/Modified

- `src/infrastructure/sync_adapter/pluggy.rs`
- `src/infrastructure/sync_adapter/mod.rs`
- `src/infrastructure/http.rs`
- `src/cli/pluggy.rs`
- `src/cli/sync.rs`
- `src/cli/mod.rs`
- `src/main.rs`
- `.gsd/milestones/M001/slices/S04/PLUGGY_SETUP.md`
