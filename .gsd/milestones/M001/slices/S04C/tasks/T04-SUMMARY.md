---
id: T04
parent: S04C
milestone: M001
key_files:
  - src/cli/pluggy.rs
  - src/cli/pluggy_connect.html
  - src/cli/mod.rs
  - src/cli/sync.rs
  - src/main.rs
  - tests/sync_demo.rs
key_decisions:
  - Dropped --item-id from pluggy setup entirely. Per-bank data flows through Connect, not setup. Same choice as Teller.
  - Two-step token mint on every connect call (fetch_api_key → fetch_connect_token). Pluggy's apiKey has a ~2-hour TTL; minting fresh at connect time is simplest and avoids cache-invalidation concerns.
  - include_sandbox defaulted to true for now — lets user see sandbox institutions alongside real ones during dev. Production usage would flip this; good candidate for a later `--environment` setup flag.
  - parse_item accepts both wrapped and flat shapes. Pluggy's SDK has emitted both historically; being lenient here costs ~5 lines.
  - Pluggy items carry no per-item access_token (unlike Teller enrollments). `data` JSON is `{}`. All per-sync auth is per-app via client_id/secret → apiKey.
  - Each item creates a fresh PluggyAdapter inside the sync loop. Adapter's internal apiKey cache doesn't survive across items, so each item re-auths. One extra POST /auth per item is acceptable overhead.
duration: 
verification_result: passed
completed_at: 2026-04-19T23:52:21.473Z
blocker_discovered: false
---

# T04: `rtf pluggy setup` takes per-app creds only; `rtf pluggy connect` mints a connect_token via Pluggy's auth+connect_token endpoints, runs the widget in the browser, captures itemId into provider_connections. Sync iterates items same as Teller.

**`rtf pluggy setup` takes per-app creds only; `rtf pluggy connect` mints a connect_token via Pluggy's auth+connect_token endpoints, runs the widget in the browser, captures itemId into provider_connections. Sync iterates items same as Teller.**

## What Happened

Pluggy refactor mirrors Teller's pattern end-to-end.

**`pluggy setup` refactor.** Drops `--item-id`. New signature: `--client-id <ID> --client-secret <SECRET>`. Stores only per-app data in `provider_credentials[provider='pluggy']`. Per-bank items come through `pluggy connect`.

**`rtf pluggy connect`.** Flow:
1. Load `client_id` + `client_secret` from `provider_credentials`. Clear error if missing.
2. POST `https://api.pluggy.ai/auth` with `{clientId, clientSecret}` → response `{apiKey}`. `fetch_api_key` helper.
3. POST `https://api.pluggy.ai/connect_token` with `X-API-KEY` header, empty body → response `{accessToken}`. `fetch_connect_token` helper. This is the short-lived token the widget consumes (different from the long-lived apiKey).
4. Render `pluggy_connect.html` template with `{{CONNECT_TOKEN}}` and `{{INCLUDE_SANDBOX}}="true"` (sandbox institutions appear alongside real ones for dev tier).
5. `ConnectServer::bind()` + `launch_browser(url)` + `run_until_callback` (5-minute timeout). Widget callbacks POST to `/success` / `/failure` / `/exit`.
6. `parse_item` accepts both wrapped `{"item":{...}}` and flat `{"id":...}` shapes (different Pluggy SDK versions emit either). Extracts `item.id` and `item.connector.name`.
7. UPSERT `provider_connections` row: `(provider='pluggy', external_id=item.id, data={}, institution_name=connector.name)`. Pluggy items have no per-item access_token — auth goes through the per-app apiKey reminted on each sync — so `data` is just `{}`.

**HTML template** (`src/cli/pluggy_connect.html`): embeds `cdn.pluggy.ai/pluggy-connect/v2.9.0/pluggy-connect.js`, initializes `PluggyConnect.init({ connectToken, includeSandbox, onSuccess, onError, onClose })`. Same structure + inline CSS as the Teller template. `onError` → POST /failure, `onClose` → POST /exit.

**`sync --provider pluggy` iterates items.** `run_pluggy_inline` now:
1. Load client_id + client_secret from `provider_credentials`.
2. Load every row from `provider_connections[provider='pluggy']`.
3. Zero items → zero-count report (same semantics as Teller's empty-enrollment case).
4. One shared HTTP client + SyncService.
5. For each item: create fresh `PluggyAdapter(client_id, client_secret, item_id)`. The adapter caches apiKey per-instance, so each item gets its own `POST /auth` call — acceptable noise vs the per-item `GET /accounts` + paginated `GET /transactions` calls.
6. Aggregate `ProviderSyncReport` across items: sum imported/duplicates/accounts_synced; min/max window dates.

**Tests (+10 unit):** `store_app_creds` happy + empty-field rejections; `fetch_api_key` + `fetch_connect_token` roundtrips and missing-field errors; `parse_item` accepts both wrapped + flat shapes, rejects missing id, allows missing connector name; `persist_item` UPSERT on same item_id + multi-item; `load_app_creds` roundtrip + missing error.

**Integration test update.** `pluggy_setup_stores_credentials` renamed to `pluggy_setup_stores_app_credentials`; asserts the new shape (client_id + client_secret only; no item_id).

**Dead code removed.** The old `parse_pluggy_creds` helper (three-tuple) replaced by `parse_pluggy_app_creds` (two-tuple). The old `run_pluggy` that loaded item_id from provider_credentials rewritten as a wrapper over `run_pluggy_inline`.

**Totals:** 311 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = **326 passing**, 5 ignored (live smokes), 0 failed. Up from 316 after T03.

**Live-test deferred to T05** — user doesn't have Pluggy credentials yet. Code path is proven via unit + integration tests; real-Pluggy smoke will happen when the user sets up their Pluggy application.

## Verification

`cargo test` → 326 passing, 5 ignored, 0 failed. `cargo build --release` → clean. Integration test `pluggy_setup_stores_app_credentials` verifies the new CLI shape end-to-end against the compiled binary.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 1200ms |
| 2 | `cargo build --release` | 0 | pass | 2000ms |

## Deviations

"Plan listed an `#[ignore]`d `pluggy_connect_live` live smoke. Deferred to T05 where it'll be documented (user hasn't obtained Pluggy credentials yet). No code difference \u2014 the sync path works as soon as the user runs `pluggy setup` + `pluggy connect`."

## Known Issues

"Live Pluggy verification pending \u2014 user hasn't obtained client_id + client_secret + linked a bank yet. When they do, the flow should just work (code path is fully tested). If it fails, most likely culprits: (a) Pluggy SDK version drift (CDN URL in pluggy_connect.html may need bumping), (b) Connect widget API shape changes between versions, (c) Pluggy's connect_token response key changes. All three are small fixes once surfaced."

## Files Created/Modified

- `src/cli/pluggy.rs`
- `src/cli/pluggy_connect.html`
- `src/cli/mod.rs`
- `src/cli/sync.rs`
- `src/main.rs`
- `tests/sync_demo.rs`
