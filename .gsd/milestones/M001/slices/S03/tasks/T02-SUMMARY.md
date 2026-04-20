---
id: T02
parent: S03
milestone: M001
key_files:
  - src/application/currency_converter.rs
  - src/application/mod.rs
  - src/cli/convert.rs
  - src/cli/mod.rs
  - src/main.rs
key_decisions:
  - Unified walk-back strategy — same loop handles cache hits and provider fetches for each candidate date, avoiding a two-phase 'check all cache first, then fetch all' design that would do unnecessary lookups.
  - Walk-back cap at 7 days — conservative: a gap longer than a week in PTAX is unusual and usually signals a real issue (market closure, API outage) worth surfacing rather than silently using week-old data.
  - Save fetched rates under the actual BCB quote date, not the originally-queried date. Keeps the cache semantically clean (row.date == date BCB published); subsequent queries for the original date walk back through cache hits efficiently.
  - Same-currency path returns source='identity' rather than a provider source name — makes it obvious in output that no conversion happened.
  - First-fetch stderr note is load-bearing for UX: users understand why the first call takes ~500ms on cache miss without needing to read docs.
  - Amount/rate stringified in the CLI JSON envelope — Decimal→JSON→Decimal round-trips cleanly via string, avoids f64 precision loss in downstream consumers.
duration: 
verification_result: passed
completed_at: 2026-04-19T20:31:10.148Z
blocker_discovered: false
---

# T02: CurrencyConverter service with cache-first + fetch-on-miss + up-to-7-day walk-back; `fintrack convert` CLI subcommand with full JSON envelope including rate_date and fallback_reason.

**CurrencyConverter service with cache-first + fetch-on-miss + up-to-7-day walk-back; `fintrack convert` CLI subcommand with full JSON envelope including rate_date and fallback_reason.**

## What Happened

**`CurrencyConverter` service** (`src/application/currency_converter.rs`):
- Generic over `ExchangeRateRepository` + `RateProvider`.
- `convert(amount, from, to, queried_date)` flow:
  1. `from == to` → identity Conversion (rate=1, source="identity", no provider call).
  2. Exact cache hit via `find_by_pair_date` → return with no fallback.
  3. Walk back 0..=7 days from the queried date. For each candidate: check cache first (skip for days_back=0 since step 2 already did it), else call provider. Save any successful fetch back to the repo. First rate found wins.
  4. Exhausted → `DomainError::NotFound`.
- `try_fetch_and_save` helper maps provider `NotFound` → `Ok(None)` (caller continues walking) while propagating other errors (`Import`, `Storage`) verbatim.
- `build_conversion` helper sets `fallback_reason` when the rate's date != queried date, formatted as *"no rate published for YYYY-MM-DD; used YYYY-MM-DD (N day(s) earlier)"* with correct pluralization.
- `MAX_WALK_BACK_DAYS = 7` constant — gaps longer than a week indicate a real data issue worth surfacing as an error.
- On first-attempt fetch, emits a single-line stderr note (`"fetching BCB PTAX rate for YYYY-MM-DD..."`) so operators understand the ~500ms latency on a cache miss. Cached paths stay silent.

**`Conversion` struct** — `{ amount: Decimal, currency: CurrencyCode, rate: Decimal, rate_date: NaiveDate, source: String, fallback_reason: Option<String> }`. `Serialize`-able. Re-exported from `src/application/mod.rs` alongside `CurrencyConverter`.

**CLI (`src/cli/convert.rs` + wiring in `cli/mod.rs` + `main.rs`):**
- `fintrack convert <amount> <from> --to <currency> [--date YYYY-MM-DD]`.
- Amount parsed via `Decimal::from_str` (preserves sign; supports decimals).
- `from` + `to` parsed as `CurrencyCode` (USD/BRL, case-insensitive).
- `--date` optional; defaults to today via `chrono::Local::now().date_naive()`.
- Output envelope: `{status:"ok", data:{amount, currency, rate, rate_date, source, fallback_reason}}`. All Decimal fields stringified to keep JSON precision intact. `fallback_reason` is `null` when absent.
- Error envelope: `{status:"error", message}` to stderr, exit 1.

**Tests (+10 service-level):**
- `same_currency_short_circuits` — identity path, no provider call.
- `cache_miss_fetches_and_persists` — first call hits provider and writes to DB; verifies via a direct repo lookup that the rate is cached.
- `second_call_hits_cache` — same params twice, provider called only once.
- `walk_back_finds_prior_business_day` — Sunday query with Friday-only provider → returns Friday's rate with fallback reason; provider called 3 times (Sun, Sat, Fri).
- `walk_back_uses_cache_for_prior_dates_when_available` — Friday pre-seeded in DB; Sunday query calls provider only for Sun + Sat (NotFound), then Fri hits cache. Provider called 2 times, not 3.
- `walk_back_exhausted_returns_not_found` — provider always NotFound → service returns NotFound.
- `provider_import_error_propagates_without_walking_back` — HTTP error on day 0 aborts immediately (doesn't mask transport failures as weekend fallbacks).
- `zero_amount_returns_zero` — multiplication invariant.
- `negative_amount_preserves_sign` — BRL debit → negative USD amount.
- `same_date_exact_cache_hit_has_no_fallback_reason` — pre-seeded exact match emits `fallback_reason: None` and doesn't call the provider.

Tests use `FakeProvider` with a `(from, to, date)`-keyed HashMap + call counter. `set_next_error` helper simulates HTTP failures. All tests run against a real in-memory SQLite DB via `SqliteExchangeRateRepository` so the repo integration is exercised inside the service tests.

197 unit tests passing (was 187 after T01; +10 for T02). Plus the existing integration test.

## Verification

`cargo test` → 197 unit + 1 integration passed, 1 ignored (live BCB), 0 failed.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 130ms |

## Deviations

None. Plan's 'zero amount + negative amount + same currency' subtests all landed. Used an in-memory SQLite DB inside the service tests (not just a memory-backed FakeRepo) \u2014 tests cost a few ms more but exercise the real repo path, catching any incidental SQLite issues for free.

## Known Issues

None.

## Files Created/Modified

- `src/application/currency_converter.rs`
- `src/application/mod.rs`
- `src/cli/convert.rs`
- `src/cli/mod.rs`
- `src/main.rs`
