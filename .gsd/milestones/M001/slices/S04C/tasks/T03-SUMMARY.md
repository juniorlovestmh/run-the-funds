---
id: T03
parent: S04C
milestone: M001
key_files:
  - src/cli/teller.rs
  - src/cli/teller_connect.html
  - src/cli/mod.rs
  - src/cli/sync.rs
  - src/main.rs
  - tests/sync_demo.rs
key_decisions:
  - Drop --access-token from teller setup entirely. Setup is now per-app only; per-bank credentials come through Connect. Forces the right workflow and eliminates a 'why would I paste a token when there's a connect command' ambiguity.
  - Environment defaulted from cert presence (cert → development, no cert → sandbox). Explicit --environment overrides. Covers the 95% case without an extra flag for most users.
  - Sync aggregation happens in the CLI layer, not in SyncService. Keeps SyncService's single-adapter contract intact; CLI knows about provider_connections and iterates.
  - Zero-enrollment case returns a zero report instead of an error. Lets a fresh post-setup state flow through unified sync without appearing to fail.
  - HTTP client built once per sync call and reused across enrollments. mTLS handshake cost is paid once.
duration: 
verification_result: passed
completed_at: 2026-04-19T23:44:25.448Z
blocker_discovered: false
---

# T03: `rtf teller connect` browser flow + multi-enrollment sync iteration. Live-verified: user linked Capital One via widget, sync pulled 984 new transactions across 5 Capital One accounts plus 524 dedup'd Chase transactions in one call (accounts_synced: 6, 2-year window).

**`rtf teller connect` browser flow + multi-enrollment sync iteration. Live-verified: user linked Capital One via widget, sync pulled 984 new transactions across 5 Capital One accounts plus 524 dedup'd Chase transactions in one call (accounts_synced: 6, 2-year window).**

## What Happened

**`teller setup` signature change.** Flags are now `--app-id <APP_ID> [--cert <path>] [--key <path>] [--environment sandbox|development|production]` — dropped `--access-token` since per-bank credentials now live in `provider_connections` and are set via the Connect flow, not at setup time. Environment defaults: `development` when cert+key are provided, `sandbox` otherwise; explicit `--environment` wins. Cert+key remain optional (Sandbox tier doesn't need them) but partial args are rejected.

**`rtf teller connect`** (new command):
1. Loads app_id + cert + key + environment from `provider_credentials[provider='teller']`. Clear error if app_id is missing (e.g., pre-S04C state where the user only saved cert+key).
2. Renders `teller_connect.html` template (embedded via `include_str!`) with `{{APP_ID}}` and `{{ENVIRONMENT}}` substituted by `render_template` from T02.
3. `ConnectServer::bind()` → opens default browser via `launch_browser()` to `http://127.0.0.1:<port>/`. Prints the URL to stderr so the user can paste it manually if the launch fails.
4. Blocks for up to 5 minutes on `run_until_callback`. Maps widget callback: Success → parse + persist; Failure → `Import` error with the widget's payload; Exit → `Import` error "cancelled".
5. `parse_enrollment` extracts `accessToken`, `enrollment.id`, `enrollment.institution.name`. Missing accessToken or enrollment.id → clear error.
6. `persist_enrollment` UPSERTs a `provider_connections` row: `(provider='teller', external_id=enrollment.id, data={"access_token":"..."}, institution_name=...)`. Same `external_id` re-run refreshes the token atomically.

**`sync --provider teller` iterates enrollments.** `run_teller_inline` (and by extension `run_teller`) now:
1. Load app credentials from `provider_credentials`.
2. Load every row from `provider_connections[provider='teller']`.
3. If zero enrollments, return a zero-count report (not an error) so unified sync still shows the Teller slot as present.
4. Build one mTLS `UreqHttpClient` from cert+key.
5. For each enrollment: extract access_token, build TellerAdapter, call `SyncService::sync_provider`, aggregate into a single `ProviderSyncReport`. Aggregation: sum `imported` + `duplicates` + `accounts_synced` across calls; take min/max of `window_start`/`window_end`.

**HTML template** (`src/cli/teller_connect.html`): embeds `cdn.teller.io/connect/connect.js`, initializes `TellerConnect.setup({ applicationId, environment, products, onSuccess/onFailure/onExit })`. Each callback POSTs to the corresponding `/success`, `/failure`, `/exit` path on the local server. Minimal inline CSS; status text updates as the flow progresses.

**Tests (+10 unit, ~3 integration-like):** `store_app_creds` happy paths (sandbox no cert, dev with cert, partial cert rejected, empty app_id rejected, bogus environment rejected), `parse_enrollment` happy + error paths + missing-institution, `parse_app_id_and_env` roundtrip, `persist_enrollment` UPSERT + multi-enrollment.

**Live verification:**
1. `teller setup --app-id app_pr9uabthpvbhp573su000 --cert ~/Downloads/teller/certificate.pem --key ~/Downloads/teller/private_key.pem` — OK, development tier saved.
2. `teller connect` → browser opened automatically, user linked Capital One, widget callback fired, new row in `provider_connections` with `enr_pra0p5o89vbhp573su000` and institution "CapitalOne".
3. Discovered 5 Capital One accounts via direct `GET /accounts` (Venture, Quicksilver, Spark Cash Select, 360 Checking, 360 Savings); created + linked all 5 as rtf accounts.
4. `sync --provider teller --since 2024-04-19` → **984 imported, 524 duplicates, 6 accounts_synced** in one call across both enrollments (Chase Legacy + Capital One). Per-account breakdown: Capital One Venture 932, Chase 524, Spark Cash Select 37, 360 Savings 14, 360 Checking 1, Quicksilver 0.

**Sandbox regressions:** `teller_setup_stores_credentials` integration test renamed to `teller_setup_stores_app_credentials` and updated to assert the new shape (`app_id` + `environment`; no `access_token`). `live_teller_connect_smoke` added as `#[ignore]`d placeholder.

**Totals:** 301 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = **316 passing**, 5 ignored (live smokes + live BCB), 0 failed.

## Verification

`cargo test` → 316 passing, 5 ignored, 0 failed. Live: `rtf teller connect` opened the real Teller widget, user linked Capital One end-to-end, widget callback captured and stored. `rtf sync --provider teller --since 2024-04-19` → 984 imported, 524 duplicates, 6 accounts synced across 2 enrollments in one call. Per-account transaction counts verified via SQL.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 1180ms |
| 2 | `rtf teller setup --app-id ... --cert ... --key ...` | 0 | pass | 50ms |
| 3 | `rtf teller connect (live widget)` | 0 | pass | 60000ms |
| 4 | `rtf sync --provider teller --since 2024-04-19` | 0 | pass | 5000ms |

## Deviations

"Plan said --cert/--key required for dev tier at setup. Shipped as optional for backward-compat with Sandbox. Plan also deferred per-enrollment sync aggregation to an extension of SyncService; shipped as CLI-layer aggregation to avoid tangling the service's one-adapter-per-call contract."

## Known Issues

"Legacy enrollment row (from S04C T01's migration 006) still sits in provider_connections using the old access_token as its synthetic external_id. Harmless but noisy in `connections list`. User can delete it after a fresh `teller connect` for that same bank creates a new row with the real enrollment.id. Safe to postpone.\nWise and Novo aren't supported by Teller; user will address those separately (QFX for Wise via existing S02 importer, CSV adapter needed for Novo). Not in S04C scope."

## Files Created/Modified

- `src/cli/teller.rs`
- `src/cli/teller_connect.html`
- `src/cli/mod.rs`
- `src/cli/sync.rs`
- `src/main.rs`
- `tests/sync_demo.rs`
