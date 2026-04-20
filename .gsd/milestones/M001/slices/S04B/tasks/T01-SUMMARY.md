---
id: T01
parent: S04B
milestone: M001
key_files:
  - src/infrastructure/sync_adapter/teller.rs
  - src/infrastructure/sync_adapter/mod.rs
  - src/cli/teller.rs
  - src/cli/sync.rs
  - src/cli/mod.rs
  - src/cli/accounts.rs
  - src/main.rs
  - tests/sync_demo.rs
  - .gsd/milestones/M001/slices/S04B/TELLER_SETUP.md
key_decisions:
  - Cursor pagination via from_id = last transaction id from previous page. Stop when server returns fewer than count. Matches Teller's documented behavior and avoids needing to parse a pagination envelope.
  - Silently skip unsupported currencies (EUR, etc.) at the adapter boundary. The SyncService only needs accounts the user linked locally; remote-only unsupported currencies are a non-event.
  - Bearer-only auth shipped; mTLS deferred. Users on sandbox tier get value immediately; users on dev/prod tier get a clear error + follow-up path.
  - No network call at setup time — auth happens lazily on first sync. Matches Pluggy's pattern; avoids a setup-time failure when the endpoint is briefly unavailable.
  - TellerAdapter reuses the existing HttpClient::get_with_headers method; no new trait methods needed.
duration: 
verification_result: passed
completed_at: 2026-04-19T22:15:02.874Z
blocker_discovered: false
---

# T01: TellerAdapter (Bearer auth + cursor pagination), `fintrack teller setup --access-token`, `sync --provider teller`, unified sync gets a third `teller` slot, accounts link extended to teller, +14 tests (11 adapter + 3 integration).

**TellerAdapter (Bearer auth + cursor pagination), `fintrack teller setup --access-token`, `sync --provider teller`, unified sync gets a third `teller` slot, accounts link extended to teller, +14 tests (11 adapter + 3 integration).**

## What Happened

Follows the SimpleFIN/Pluggy template nearly line-for-line — the S04 adapter abstraction carried its weight.

**TellerAdapter** (`src/infrastructure/sync_adapter/teller.rs`) — Bearer token in `Authorization` header, GET `/accounts` → raw JSON array at top level (no wrapper, unlike SimpleFIN), then per-account `/accounts/{id}/transactions?from_date=&count=500[&from_id=cursor]` with cursor-based pagination that stops when the server returns fewer rows than requested. Sign convention: Teller pre-signs amounts (debit = negative, credit = positive) — no inversion. Silently skips accounts in unsupported currencies (e.g. EUR) so the domain only sees USD/BRL. Date arrives as ISO `YYYY-MM-DD`, amount as signed decimal string, description + `details.counterparty.name` for payee with fallback. 11 unit tests: happy path with mixed debit/credit signs, Bearer header present, cursor pagination (full page → short page), unsupported currency skipped, missing-amount error, malformed-date error, 401 propagates, payee fallback to description, default since triggers one /accounts call, empty-transactions short-circuits.

**`fintrack teller setup --access-token <TOKEN>`** (`src/cli/teller.rs`) — non-empty validation, persists `{"access_token":"..."}` in `provider_credentials` via upsert. No network at setup time (lazy auth).

**CLI glue:**
- New top-level `Teller { command: TellerCommands }` + `TellerCommands::Setup { access_token }`.
- `sync --provider teller` dispatches to `run_teller` + `run_teller_inline` for unified.
- `UnifiedSyncReport` gained a `teller: Option<ProviderSyncReport>` slot; `run_unified` adds the third try-and-collect step. Error isolation semantics unchanged.
- `accounts link` provider validator extended to accept "teller".

**mTLS caveat documented** in `TELLER_SETUP.md`: Teller's Development/Production tiers require mutual TLS; this slice ships Bearer-only. A user on an mTLS-required tier will get a clear TLS rejection surfaced via the improved HTTP error handler. mTLS support is a ~40-line follow-up (ureq's `AgentBuilder::tls_config` + cert/key paths in credentials JSON).

**Integration tests (+3 in tests/sync_demo.rs):** missing-creds error message, setup-stores-credentials roundtrip, accounts-link-accepts-teller. Plus one `#[ignore]`d live smoke `live_teller_sync_smoke` with `TELLER_ACCESS_TOKEN` env.

**TELLER_SETUP.md** — step-by-step guide: sign up at teller.io → create Application → launch Teller Connect → copy enrollment access token → `fintrack teller setup` → `accounts link` → `sync`. Notes on mTLS, rotation, and the single-enrollment-per-config limitation.

**Totals:** 276 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = **291 passing**, 5 ignored (live smokes + live BCB). Up from 274 after S04 — +14 new tests (+11 Teller adapter + +3 Teller CLI integration).

## Verification

`cargo test` → 291 passing, 5 ignored, 0 failed across 4 test artifacts. `cargo build` → clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 330ms |

## Deviations

None. Plan's ~12 unit tests landed as 11 (consolidated one of the planned cases). mTLS is deferred as planned; the tier caveat is prominently documented in TELLER_SETUP.md.

## Known Issues

None from T01 scope. mTLS support is a deferred follow-up documented in TELLER_SETUP.md."

## Files Created/Modified

- `src/infrastructure/sync_adapter/teller.rs`
- `src/infrastructure/sync_adapter/mod.rs`
- `src/cli/teller.rs`
- `src/cli/sync.rs`
- `src/cli/mod.rs`
- `src/cli/accounts.rs`
- `src/main.rs`
- `tests/sync_demo.rs`
- `.gsd/milestones/M001/slices/S04B/TELLER_SETUP.md`
