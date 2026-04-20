---
id: S04B
parent: M001
milestone: M001
provides:
  - ["TellerAdapter (Basic auth + cursor pagination)", "rtf teller setup --access-token [--cert --key]", "rtf sync --provider teller dispatch", "UreqHttpClient::with_mtls for any future mTLS provider", "UnifiedSyncReport.teller slot", "Improved HTTP error handler that includes response body on 4xx/5xx (cross-cutting benefit for all providers)"]
requires:
  - slice: S04
    provides: BankSyncAdapter trait, HttpClient trait, SyncService, ProviderCredentialsRepository, unified sync CLI handler, Account::link, persist_batch
affects:
  - ["Cargo.toml (rustls + rustls-pemfile + webpki-roots deps)", "src/infrastructure/http.rs (UreqHttpClient gained Agent field + with_mtls constructor)", "src/infrastructure/sync_adapter/teller.rs (new)", "src/cli/teller.rs (new, extended with --cert --key in T02)", "src/cli/sync.rs (teller branch + run_teller_inline + TellerCreds parser + build_teller_http)", "src/cli/mod.rs (Teller subcommand + TellerCommands with optional cert/key)", "src/cli/accounts.rs (provider validator accepts teller)", "src/main.rs (Teller dispatch)", "src/infrastructure/exchange/bcb_ptax.rs + src/cli/simplefin.rs (unit-struct \u2192 ::new() call-site updates)"]
key_files:
  - ["Cargo.toml", "src/infrastructure/http.rs", "src/infrastructure/sync_adapter/teller.rs", "src/infrastructure/sync_adapter/mod.rs", "src/cli/teller.rs", "src/cli/mod.rs", "src/cli/sync.rs", "src/main.rs", "src/infrastructure/exchange/bcb_ptax.rs", "src/cli/simplefin.rs", ".gsd/milestones/M001/slices/S04B/S04B-UAT.md", ".gsd/milestones/M001/slices/S04B/TELLER_SETUP.md"]
key_decisions:
  - ["Embedded cert + key PEM contents in credentials JSON, not file paths. Users can delete the Downloads files; DB is self-contained.", "rustls 0.23 with ring feature + install_default on first use. Explicit crypto provider is rustls 0.23's requirement.", "UreqHttpClient::with_mtls as a constructor variant instead of a separate TellerHttpClient type \u2014 one HTTP client abstraction, mTLS is just configuration.", "HTTP Basic with token-as-username instead of Bearer \u2014 matches Teller's documented auth scheme. Caught by first live sync; fixed before commit.", "Teller uses the same Option-wrapped slot in UnifiedSyncReport as SimpleFIN and Pluggy; error isolation works uniformly.", "No service-layer changes at all \u2014 the BankSyncAdapter trait from S04 handled the extension cleanly."]
patterns_established:
  - ["UreqHttpClient::with_mtls pattern \u2014 provider-specific HTTP config via constructor variant rather than separate client types. Reusable for any future mTLS provider.", "Cert+key embedded in credentials JSON \u2014 template for any provider whose auth involves file-based credentials.", "Live-test discovering a real auth bug (Bearer vs Basic) before commit \u2014 justifies running live tests even when unit tests pass."]
observability_surfaces:
  - ["HTTP body preserved on 4xx/5xx \u2014 actual error messages reach operators.", "Teller tier reported in setup message (mTLS vs Bearer-only).", "Same per-provider envelope as SimpleFIN/Pluggy in unified sync.", "Clear TLS error if mTLS config is incomplete or cert is invalid."]
drill_down_paths:
  []
duration: ""
verification_result: passed
completed_at: 2026-04-19T22:33:35.038Z
blocker_discovered: false
---

# S04B: Teller Adapter (third bank-sync provider)

**Added Teller as a third bank-sync provider with mTLS client-cert support; live-verified against real Chase account (524 transactions pulled in 2.8s). Rolls in the S04 HTTP-body-on-error fix that surfaces response bodies on 4xx/5xx.**

## What Happened

S04B landed in two tasks plus real-world validation.

**T01 — TellerAdapter + CLI + tests.** Following the SimpleFIN/Pluggy templates: adapter parses Teller's top-level JSON arrays (accounts + per-account transactions), cursor-paginates via `from_id`, maps to `RemoteTransaction` with Teller's pre-signed amounts (no inversion needed). `rtf teller setup --access-token <TOKEN>` stores credentials as JSON. `sync --provider teller` dispatches via a new branch; unified `rtf sync` gains a third `teller` Option slot on `UnifiedSyncReport`. `accounts link` provider validator extended. +11 unit tests + 3 integration tests.

**T02 — mTLS + auth fix + live verification.** User's Teller tier required mutual TLS; we added `UreqHttpClient::with_mtls(cert_pem, key_pem)` that parses PEMs via `rustls-pemfile`, builds a rustls `ClientConfig` with client auth, installs the ring CryptoProvider (rustls 0.23 needs it explicitly), and wires the Arc into `ureq::AgentBuilder::tls_config`. Extended `teller setup` with `--cert` + `--key` flags; PEM contents embed in credentials JSON so source files can be deleted. Required a small struct refactor — `UreqHttpClient` gained an `agent: ureq::Agent` field, breaking the previous unit-struct `UreqHttpClient;` construction at 7 call sites (all updated to `UreqHttpClient::new()`). Also caught a Teller auth bug on first live-sync: initial T01 used `Authorization: Bearer` but Teller wants HTTP Basic with the token as username + empty password. One-line fix + test update.

**Live verification.** User provided real Teller cert + key + enrollment access token. Flow:
1. `rtf teller setup --access-token token_xxx --cert ... --key ...` → OK.
2. `rtf sync --provider teller` → **524 real Chase transactions** pulled in 2.8 seconds, 2-year backfill window (2024-04-19 → 2026-04-19), account linked and `last_sync_at` stamped.
3. Immediate re-sync → `imported:0, duplicates:0` (incremental: `last_sync_at` = today, Teller returns nothing).
4. Forced `--since 2024-04-19` → `imported:0, duplicates:524` (FITID dedup via migration 002 rejected every one).
5. `rtf transactions list --format json` → all 524 rows present with correct fields; first row 2026-04-18 Bull & Bowtie -$10; every row carries S03's `rate_status: "same_currency"` enrichment (USD short-circuit working alongside Teller sync).

**Rollup fixes riding along:**
- **HTTP body on 4xx/5xx** (discovered during S04 live-test): `UreqHttpClient::collapse` now reads and preview-truncates the response body when ureq returns `Error::Status(code, response)`. Before: `"status code 402"`. After: `"HTTP 402: {\"errors\":[\"Payment required.\"]}"`. Changed the failure mode from "guess what went wrong" to "read the actual error".

**Cargo deps added:** rustls 0.23 (with ring feature), rustls-pemfile 2, webpki-roots 0.26. ~15 extra compile-time crates, one-time cost.

**Architecturally clean:** no service-layer or storage changes; new provider slot in UnifiedSyncReport; new CLI branch; one HTTP client config variant (`with_mtls`). The S04 abstraction carried its weight — Teller took ~400 lines end-to-end.

**293 tests passing**, 5 ignored (live smokes + live BCB), 0 failed. +17 from S04 (+11 Teller adapter, +3 Teller CLI integration, +2 Teller setup with cert, +1 ring provider init implicit via fixing one test).

## Verification

Live: `rtf sync --provider teller` → 524 real transactions from Chase via mTLS in 2.8s. Re-run: 0/0 (incremental). Forced re-pull: 0 imported, 524 duplicates (dedup). `cargo test`: 293 passing, 5 ignored, 0 failed across 4 test artifacts. `cargo build --release`: clean.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

"Plan had Bearer auth; actual Teller auth is HTTP Basic with empty password. Caught on first live sync, fixed with a one-line change. Plan deferred mTLS to a follow-up; actual user tier required it, so mTLS landed inside T02 instead. Net: S04B shipped with full mTLS support and a working live flow rather than a Sandbox-only demo."

## Known Limitations

["`rtf teller connect` browser-based flow is NOT implemented. User must obtain enrollment access token externally. Biggest remaining UX gap; file as follow-up.", "One Teller enrollment per config (same as Pluggy).", "Credentials (incl. cert+key PEM) plaintext in local DB. Unchanged posture from S04 SECURITY.md.", "ring crypto provider installed once per process \u2014 works for CLI usage; would need care in a long-lived daemon."]

## Follow-ups

["S04C (or similar): `rtf teller connect` with local callback server + browser launch. Replaces manual token-pasting with a fully automated Connect flow. Same pattern would apply to Pluggy's Connect UI.", "Multi-enrollment support for Teller + Pluggy \u2014 needs a separate credentials-per-enrollment table rather than one-row-per-provider.", "Keychain integration to avoid plaintext-in-DB posture.", "S05 categorization can now consume transactions from three live providers."]

## Files Created/Modified

None.
