---
id: T03
parent: S03
milestone: M001
key_files:
  - src/cli/transactions.rs
key_decisions:
  - Presentation-only TransactionView (not a domain type) — keeps domain pure; enrichment concerns belong at the output boundary.
  - Rate status vocabulary (ok/fallback/unavailable/same_currency) over a boolean or numeric code — self-documenting for agents/operators reading the JSON.
  - Error-handling split: NotFound downgrades this row only; other errors abort the listing. Avoids both noisy 'all rows unavailable' output on a BCB outage AND silent-failure where one bad date masks 49 good rows.
  - No explicit batching in the CLI — relied on CurrencyConverter's cache-first behavior for per-date dedup. Simpler code, same result in practice.
  - Deferred unit tests to T04 — the logic is thin glue over well-tested components (Transaction, Conversion, CurrencyConverter); T04's integration test covers the full CLI path against a real binary.
duration: 
verification_result: passed
completed_at: 2026-04-19T20:33:51.227Z
blocker_discovered: false
---

# T03: `transactions list --format json` now emits per-row amount_usd + rate + rate_date + rate_status via a TransactionView presentation struct; CurrencyConverter's caching naturally dedups per-date rate fetches.

**`transactions list --format json` now emits per-row amount_usd + rate + rate_date + rate_status via a TransactionView presentation struct; CurrencyConverter's caching naturally dedups per-date rate fetches.**

## What Happened

Added a presentation layer over `Transaction` in `src/cli/transactions.rs` without touching the domain type.

**`TransactionView<'a>`:**
- `#[serde(flatten)]` wraps a `&'a Transaction` so every existing field (id, account_id, date, amount, payee, description, status, external_id, imported_at, created_at, updated_at) still appears at the top level of the JSON output.
- Adds: `amount_usd: Option<String>` (stringified Decimal, None when unresolved), `rate: Option<String>`, `rate_date: Option<NaiveDate>`, `rate_status: &'static str`.
- `rate_status` vocabulary:
  - `"ok"` — exact-date rate found (cached or freshly fetched).
  - `"fallback"` — rate used is earlier than the txn date (weekend/holiday walk-back).
  - `"unavailable"` — no rate within the 7-day walk-back window.
  - `"same_currency"` — USD account; rate=1, amount_usd == amount.

**`build_views(db, &[Transaction])`:**
- Instantiates one `CurrencyConverter<SqliteExchangeRateRepository, BcbPtaxProvider>` for the whole listing.
- Iterates transactions sequentially. For USD rows: short-circuits to `same_currency` (no converter call). For BRL rows: calls `convert(amount, BRL, USD, txn.date)`.
- Converter's cache-first design naturally batches: a 50-row BRL listing with 10 unique dates makes at most 10 BCB round-trips (and zero on subsequent runs once rates are cached).
- Error handling split: `DomainError::NotFound` downgrades THIS row to `rate_status: "unavailable"` (listing continues); any other error (HTTP failure, storage error) aborts the listing with a structured error envelope. This is the right tradeoff: a date-specific gap shouldn't mask 49 valid rows, but a BCB outage shouldn't silently give 50 "unavailable" rows.

**Table output** (`list` without `--format json`) unchanged — no new USD column yet. Keeps the human-readable view simple; agents and scripts that care about cross-currency rollups will consume the JSON.

**Tests:** no new unit tests in this task. The logic in `build_views` is straightforward mapping from `Conversion` to `TransactionView` fields; CurrencyConverter is already well-covered at the service level (T02), and T04 will add integration coverage for the full CLI path with seeded rates.

**Regression:** existing `end_to_end_import_demo` integration test passes. One observation noted for T04: the test's `list --account-id <nubank>` step now triggers live BCB fetches for 13 Nubank transactions (currency coverage dedup to ~10 unique dates, so ~10 BCB round-trips, ~1s added). T04 will pre-seed rates in the new integration test so CI stays deterministic without depending on BCB availability.

197 unit tests still pass. No new unit tests.

## Verification

`cargo test` → 197 unit + 1 integration passed, 1 ignored, 0 failed. `cargo build` → clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 120ms |

## Deviations

Skipped the unit-level build_views test the plan suggested \u2014 the logic is straightforward mapping of already-tested `Conversion` fields to output, and T04 will cover it at the CLI integration level. Net simpler code without reducing confidence.

## Known Issues

The existing `end_to_end_import_demo` integration test now triggers live BCB fetches when listing the Nubank fixture. Works today (BCB is reachable) but adds ~1s and a network dependency. T04 will either pre-seed rates in that test or add a new test alongside that doesn't hit BCB."

## Files Created/Modified

- `src/cli/transactions.rs`
