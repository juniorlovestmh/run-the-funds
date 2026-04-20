---
id: S03
parent: M001
milestone: M001
provides:
  - ["ExchangeRate domain + ExchangeRateRepository trait + SQLite impl + partial unique-pair-date index (from migration 001)", "RateProvider trait + BcbPtaxProvider + HttpClient trait seam", "CurrencyConverter service with cache+fetch+walk-back", "ImportReport / Conversion / TransactionView presentation types", "CLI: rtf convert <amount> <from> --to <cur> [--date YYYY-MM-DD]", "transactions list --format json now carries per-row amount_usd + rate_status metadata", "ureq HTTP client added to workspace deps", "Integration test infrastructure for offline-deterministic CLI tests (seed rates via rtf::* imports)"]
requires:
  - slice: S01
    provides: exchange_rates table (migration 001), CurrencyCode + Money + DomainError + CLI envelope
  - slice: S02
    provides: Transaction model with account_id+date+amount.currency, list_by_account service method, import pipeline to seed BRL transactions for the demo
affects:
  - ["src/domain/exchange/ (new)", "src/infrastructure/storage/exchange_rate_repo.rs (new)", "src/infrastructure/exchange/ (filled in)", "src/application/currency_converter.rs (new)", "src/application/mod.rs (re-export)", "src/cli/convert.rs (new)", "src/cli/mod.rs (Convert subcommand + allow_hyphen_values)", "src/cli/transactions.rs (TransactionView enrichment in JSON output)", "src/main.rs (wire Convert handler)", "Cargo.toml (ureq added)", "tests/import_demo.rs (pre-seed rates, assert amount_usd)"]
key_files:
  - ["src/domain/exchange/rate.rs", "src/domain/exchange/repository.rs", "src/domain/exchange/mod.rs", "src/infrastructure/storage/exchange_rate_repo.rs", "src/infrastructure/exchange/bcb_ptax.rs", "src/infrastructure/exchange/mod.rs", "src/application/currency_converter.rs", "src/application/mod.rs", "src/cli/convert.rs", "src/cli/transactions.rs", "src/cli/mod.rs", "src/main.rs", "Cargo.toml", "tests/convert_demo.rs", "tests/import_demo.rs", ".gsd/milestones/M001/slices/S03/S03-UAT.md"]
key_decisions:
  - ["Midpoint of BCB compra/venda as the neutral rate \u2014 avoids directional bias.", "Provider-side handles BRL\u2192USD inversion, so the service layer never cares about directionality.", "Walk back up to 7 days on weekend/holiday; longer gaps surface NotFound rather than silently using stale data.", "Two-step cache check inside walk-back (exact hit before provider call) avoids unnecessary network round-trips when nearby dates are already cached.", "Small HttpClient trait seam in the BCB provider \u2014 tests never touch the network.", "Rate status vocabulary (ok/fallback/unavailable/same_currency) \u2014 self-documenting for agents reading the JSON.", "Error-handling split in list enrichment: NotFound degrades one row, other errors abort listing. Avoids both noisy 'all unavailable' on BCB outages AND silent-failure where one bad date masks others.", "Integration tests pre-seed rates via rtf::* library imports rather than calling `rtf convert` for seeding (avoids chicken-and-egg with BCB).", "allow_hyphen_values on the convert amount arg \u2014 scoped fix for clap parsing negative numbers as flags.", "Mid-slice split: moved bank-sync out of S03 into a new S04 after confirming user has SimpleFIN+Pluggy credentials and that each is S02-sized."]
patterns_established:
  - ["HttpClient trait seam for testable HTTP integrations \u2014 template for S04's SimpleFIN and Pluggy adapters.", "Library-level integration test seeding via rtf::* imports to avoid network coupling in default test runs.", "#[ignore]d live smoke tests paired with offline fixtures \u2014 default CI stays fast+deterministic; manual runs verify the real integration.", "Cache-first, fetch-on-miss, walk-back-on-gap flow \u2014 reusable template for any time-series data source (S06 might need similar for goal-progress history?).", "Presentation-layer TransactionView instead of enriching the domain Transaction \u2014 keeps the domain pure, enrichment at the output boundary."]
observability_surfaces:
  - ["`rtf convert` JSON envelope with rate + rate_date + source + fallback_reason \u2014 operators can tell exact-hit from walk-back at a glance.", "First-fetch stderr note (`fetching BCB PTAX rate for YYYY-MM-DD...`) explains the ~500ms latency on cache miss; subsequent cached runs are silent.", "Per-row rate_status in transactions list \u2014 ok / fallback / unavailable / same_currency.", "HTTP failures from BCB carry status code + body preview (first 200 chars) in DomainError::Import.", "NotFound from provider distinct from Import errors \u2014 no ambiguity between 'weekend' and 'BCB down'."]
drill_down_paths:
  []
duration: ""
verification_result: passed
completed_at: 2026-04-19T20:41:27.329Z
blocker_discovered: false
---

# S03: Multi-Currency (USD↔BRL via BCB PTAX)

**USD↔BRL conversion backed by BCB PTAX with lazy fetch + cache + weekend walk-back; rtf convert CLI; transactions list JSON enriched with amount_usd + rate metadata; all offline-testable via seeded rates.**

## What Happened

S03 delivered multi-currency support across four tasks (plus a mid-slice scope split that moved bank-sync to a new S04).

**T01 — ExchangeRate foundation + BCB PTAX provider.** New `ExchangeRate` domain type, `ExchangeRateRepository` trait with `save` / `find_by_pair_date` / `find_nearest_on_or_before`, `SqliteExchangeRateRepository` against the existing migration-001 `exchange_rates` table. `RateProvider` trait + `BcbPtaxProvider<C: HttpClient>` generic over an HTTP client seam. Production impl uses `ureq = "2"` with default TLS for HTTPS; tests inject a `FakeHttpClient` with canned (url → body or error) mappings. BCB publishes BRL-per-USD; provider computes the midpoint of `cotacaoCompra`/`cotacaoVenda`, inverts for BRL→USD. Empty `value` array → `NotFound` (signals "no publication on this date" so the caller can walk back). +16 unit tests + 1 ignored live smoke.

**T02 — CurrencyConverter + `rtf convert` CLI.** Service generic over both repo and provider. Flow: same-currency short-circuit → exact cache → walk back 0..=7 days checking cache then provider for each candidate. Maps provider `NotFound` to "continue walking"; persists every successful fetch; stamps `fallback_reason` when the rate's date differs from queried. `Conversion` struct (Serialize) carries `{amount, currency, rate, rate_date, source, fallback_reason}`. CLI subcommand: `rtf convert <amount> <from> --to <currency> [--date YYYY-MM-DD]`. First-attempt fetch emits a single stderr note so operators understand the ~500ms round-trip; cached paths are silent. +10 service tests + `allow_hyphen_values = true` on `amount` so negative amounts parse.

**T03 — Multi-currency display.** `transactions list --format json` now wraps every `Transaction` in a presentation-layer `TransactionView` that flattens the original fields and adds `amount_usd`, `rate`, `rate_date`, `rate_status`. USD accounts short-circuit to `rate_status: "same_currency"`. BRL rows go through the converter; successful lookups get `"ok"` or `"fallback"`, exhausted-walk-back rows get `"unavailable"` with null amount_usd (listing continues). Hard errors (HTTP/storage) abort the whole listing. Table output unchanged for human consumption.

**T04 — E2E demo + UAT + roadmap restructure.** `tests/convert_demo.rs` (3 offline tests + 1 ignored live-BCB smoke) shells to the compiled binary with rates seeded directly into SQLite via `rtf::*` library imports. Covers: exact-cache USD↔BRL, same-currency identity, weekend walk-back (Sun→Fri with fallback_reason), negative amount, malformed amount + malformed date error paths, and the full "import Nubank fixture → list --format json → every row has `amount_usd`" integration. Also updated `tests/import_demo.rs` to seed rates so its list step no longer hits live BCB (was silently flaky after T03). `.gsd/milestones/M001/slices/S03/S03-UAT.md` mirrors S02-UAT with 9 scenarios + Roadmap coverage table. `.gsd/milestones/M001/M001-ROADMAP.md` is about to be restructured in a follow-up `gsd_reassess_roadmap` call — split S03 (PTAX) from new S04 (Bank Sync: SimpleFIN + Pluggy), renumber old S04-S07 → S05-S08.

**Plan revision mid-slice.** Original S03 scoped "Multi-Currency + Bank Sync" together. After confirming with the user that SimpleFIN + Pluggy are required (not optional — user has credentials for both) AND that each adapter is S02-sized with its own HTTP/auth/pagination concerns, split S03 in half. PTAX stays here; bank sync becomes a dedicated S04. Rationale: smaller slices with clearer Done criteria, easier to reason about the HTTP layer once rather than juggling three adapters in one slice.

## Verification

`cargo test` → 197 unit + 3 convert_demo integration + 1 import_demo integration = **201 passed**, 2 ignored (live BCB smokes), 0 failed. `cargo build` → clean. Convert + list flows exercised end-to-end against the compiled binary with fully offline (seeded-rate) test fixtures — CI never touches BCB in the default test run.

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

"Split S03 mid-slice: original 5-item scope (PTAX + convert + display + SimpleFIN + Pluggy) became 4 tasks focused on PTAX-only. Bank-sync moved to a new S04 (old S04-S07 renumbered to S05-S08). Dropped the 'overwrite placeholder currency' concept from the plan in favor of strict adapter-currency matching. Added an unrelated clap bug fix (allow_hyphen_values for negative convert amounts) discovered by the new test suite. Pre-seeded rates in the existing end_to_end_import_demo test to make it deterministic (prior to T04 it was silently hitting live BCB post-T03)."

## Known Limitations

["Only USD\u2194BRL pairs are supported. Adding EUR or GBP would need enum variants and possibly a non-BCB provider.", "Walk-back is capped at 7 days; a longer PTAX gap (major market disruption) would surface NotFound rather than silent fallback.", "`amount_usd` in transactions list is always USD \u2014 no way to choose the reporting currency yet. S07 (agent query layer) will likely want a `--reporting-currency` flag.", "First-fetch stderr log is always on \u2014 no --quiet flag yet. Would be annoying in long batch operations; S04 sync may want to suppress it."]

## Follow-ups

["S04 bank-sync adapters should reuse the HttpClient trait seam for testability.", "Consider surfacing cached-vs-fetched in the convert CLI output (currently both are indistinguishable in fallback_reason=null cases) \u2014 low priority.", "Add a `rtf rates backfill --from <date> --to <date>` command for bulk pre-fetching if users want to warm the cache before going offline.", "Table output for `transactions list` could grow a USD column with a flag \u2014 not urgent; agent consumers use --format json."]

## Files Created/Modified

None.
