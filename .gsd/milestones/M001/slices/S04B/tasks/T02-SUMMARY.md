---
id: T02
parent: S04B
milestone: M001
key_files:
  - Cargo.toml
  - src/infrastructure/http.rs
  - src/infrastructure/sync_adapter/teller.rs
  - src/cli/teller.rs
  - src/cli/mod.rs
  - src/cli/sync.rs
  - src/main.rs
  - src/infrastructure/exchange/bcb_ptax.rs
  - src/cli/simplefin.rs
  - .gsd/milestones/M001/slices/S04B/S04B-UAT.md
key_decisions:
  - Embed cert+key PEM contents in credentials JSON (not file paths) — self-contained DB, source files deletable, consistent with access_token storage posture.
  - rustls ring feature + explicit install_default call — rustls 0.23 requires either aws-lc-rs or ring; ring is smaller and battle-tested.
  - Install_default return value ignored — second call returns Err if already installed; idempotent behavior is what we want.
  - Teller uses HTTP Basic with empty password, not Bearer — matches their docs. Initial Bearer impl was wrong and failed on first live sync.
  - Added mTLS support to UreqHttpClient (shared), not a Teller-specific HTTP client — keeps one HTTP abstraction; mTLS cert is just another configuration option. Other providers still use UreqHttpClient::new() unchanged.
duration: 
verification_result: passed
completed_at: 2026-04-19T22:32:09.509Z
blocker_discovered: false
---

# T02: Live-verified Teller sync against real Chase account via mTLS: 524 transactions pulled in ~2.8s, dedup + incremental behavior confirmed. Folds in the S04 HTTP-body-on-error fix and the new mTLS client-auth support.

**Live-verified Teller sync against real Chase account via mTLS: 524 transactions pulled in ~2.8s, dedup + incremental behavior confirmed. Folds in the S04 HTTP-body-on-error fix and the new mTLS client-auth support.**

## What Happened

T02 landed three things: (1) mTLS support for Teller's Developer tier, (2) Teller auth-header fix (Basic, not Bearer), (3) live verification against the user's real Chase account.

**mTLS support in UreqHttpClient.**
- `UreqHttpClient::with_mtls(cert_pem, key_pem)` parses PEM strings via `rustls-pemfile`, builds a rustls `ClientConfig` with `with_client_auth_cert(certs, key)`, wraps webpki-roots for server verification, passes the Arc<ClientConfig> into `ureq::AgentBuilder::tls_config`.
- rustls 0.23 requires an explicit `CryptoProvider` — `rustls::crypto::ring::default_provider().install_default()` runs on first call (idempotent; the `Err` on already-installed is ignored).
- New deps: `rustls = { version = "0.23", features = ["ring"] }`, `rustls-pemfile = "2"`, `webpki-roots = "0.26"`. Matched ureq 2.12's internal rustls version so the `Arc<ClientConfig>` type is compatible.
- `UreqHttpClient::new()` path unchanged (default ureq agent); every other provider's code gets no-op impact.
- `UreqHttpClient` grew an `agent: ureq::Agent` field; all four trait methods now call `self.agent.get(...)` / `self.agent.post(...)`. Side-effect: `UreqHttpClient;` as a unit-struct constructor stopped working at 7 call sites across `sync.rs`, `simplefin.rs`, and `bcb_ptax.rs` — replaced with `UreqHttpClient::new()`.

**Teller credentials extension.**
- `fintrack teller setup` grew two optional flags: `--cert <path>` and `--key <path>`. Provided together: PEM contents read at setup and embedded in the credentials JSON as `cert_pem` + `key_pem` (self-contained DB; source files can be deleted). Provided neither: stays Bearer-only for Sandbox. Partial args (one without the other): rejected with a clear validation error.
- `cli/sync.rs::run_teller` + `run_teller_inline` load credentials, call `build_teller_http` which picks `UreqHttpClient::with_mtls(...)` when cert+key are present or `UreqHttpClient::new()` otherwise. Adapter never cares which.

**Teller auth-header fix.**
- T01 shipped with `Authorization: Bearer <token>`. Teller actually uses HTTP Basic with the token as username and empty password (per their docs `-u "token_xxx:"`). First live sync returned 401; fixed by encoding `Basic <base64("token:")>` via `get_with_headers`. Updated the matching unit test to assert Basic-with-empty-password instead of Bearer.

**Live verification (user's real Chase account).**
1. `fintrack teller setup --access-token token_2czxt73go... --cert ~/Downloads/teller/certificate.pem --key ~/Downloads/teller/private_key.pem` → message "Teller configured with mTLS (Development/Production tier)".
2. `curl --cert ... --key ... -u "token_...:" https://api.teller.io/accounts` → one Chase account (USD, acc_pr9ubcf1l6jujrg5m8000, "Day Spending", ending 8332 — same real account as S02's QFX fixture, now via Teller instead of manual download).
3. `fintrack accounts create --name "Chase Day Spending"` → new local account.
4. `fintrack accounts link --provider teller --external-id acc_pr9ubcf1l6jujrg5m8000`.
5. `fintrack sync --provider teller` → **524 transactions imported in ~2.8s**, 2-year window (2024-04-19 → 2026-04-19).
6. `fintrack sync --provider teller` (immediate re-run) → `imported:0, duplicates:0` (last_sync_at=today, Teller returns nothing — proper incremental).
7. `fintrack sync --provider teller --since 2024-04-19` (force full re-pull) → `imported:0, duplicates:524` (dedup via migration 002's partial unique index).
8. `fintrack transactions list --format json` → all 524 rows present; first row 2026-04-18 Bull & Bowtie -$10 with correct external_id, payee, description, signed amount; all 524 carry S03's `rate_status: "same_currency"`.

**S04B-UAT.md** (`.gsd/milestones/M001/slices/S04B/S04B-UAT.md`) mirrors the S04 shape with 9 scenarios + Roadmap coverage table. Documents the rollup fixes (HTTP body, mTLS, Teller auth) in a dedicated section. Flags the biggest remaining UX gap: `fintrack teller connect` — a browser-launching OAuth-style callback flow that would eliminate the "user pastes a token" step. Filed as a future slice.

**Test totals:** 278 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = **293 passing**, 5 ignored, 0 failed.

**Ready to commit** — single rollup commit covering both T01 (adapter) and T02 (live-test + mTLS + fixes) per the end-of-slice cadence rule. The S04 leftover HTTP-body fix rides along too.

## Verification

Live: `fintrack sync --provider teller` → 524 transactions imported from real Chase account via mTLS in 2.8s. Re-run: 0/0 (incremental). Forced re-pull: 0 imported, 524 duplicates (dedup). 524 rows round-trip cleanly through `fintrack transactions list --format json`. `cargo test`: 293 passing, 5 ignored, 0 failed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `fintrack teller setup --cert ... --key ... --access-token ...` | 0 | pass | 50ms |
| 2 | `fintrack sync --provider teller` | 0 | pass | 2800ms |
| 3 | `fintrack sync --provider teller (re-run)` | 0 | pass | 500ms |
| 4 | `fintrack sync --provider teller --since 2024-04-19` | 0 | pass | 2900ms |
| 5 | `cargo test` | 0 | pass | 330ms |

## Deviations

Plan assumed Teller Bearer auth would work for live-test. It didn't \u2014 Teller uses HTTP Basic. One-line fix + test update. Plan assumed mTLS would be deferred as a follow-up; instead we implemented it mid-T02 because the user's actual tier required it. Net: mTLS support + auth fix landed in the same slice commit."

## Known Issues

"Biggest remaining UX gap: user has to obtain the Teller enrollment access token externally (via their own app or curl against Teller's connect_token endpoint) before running `teller setup`. A proper `fintrack teller connect` command that spawns a local callback server + opens a browser to Teller Connect + captures the token automatically is the right fix. Filed as a follow-up slice \u2014 not blocking day-one sync functionality, but will significantly improve first-time setup."

## Files Created/Modified

- `Cargo.toml`
- `src/infrastructure/http.rs`
- `src/infrastructure/sync_adapter/teller.rs`
- `src/cli/teller.rs`
- `src/cli/mod.rs`
- `src/cli/sync.rs`
- `src/main.rs`
- `src/infrastructure/exchange/bcb_ptax.rs`
- `src/cli/simplefin.rs`
- `.gsd/milestones/M001/slices/S04B/S04B-UAT.md`
