---
id: T01
parent: S03
milestone: M001
key_files:
  - src/domain/exchange/mod.rs
  - src/domain/exchange/rate.rs
  - src/domain/exchange/repository.rs
  - src/domain/mod.rs
  - src/infrastructure/storage/exchange_rate_repo.rs
  - src/infrastructure/storage/mod.rs
  - src/infrastructure/exchange/mod.rs
  - src/infrastructure/exchange/bcb_ptax.rs
  - Cargo.toml
  - src/application/transaction_service.rs
key_decisions:
  - Midpoint of BCB compra/venda as the neutral rate — avoids directional bias.
  - BCB returns BRL-per-USD; provider handles the BRL→USD inversion internally, so the service layer never has to care about which direction is 'native'.
  - Small HttpClient trait seam (get(url)→String) for mockable tests — keeps ureq behind one abstraction point.
  - Empty BCB response array → NotFound (not Import). This signals 'no publication on this date' so the service layer can do its walk-back; distinct from 'actual HTTP failure'.
  - ureq sync over reqwest blocking — smaller dep tree, zero async infrastructure needed. Can revisit if bank-sync slices want async.
  - save() uses INSERT OR REPLACE for rates (unlike transactions) — rate for (from, to, date) can legitimately be refreshed if BCB republishes.
duration: 
verification_result: passed
completed_at: 2026-04-19T20:27:32.659Z
blocker_discovered: false
---

# T01: ExchangeRate domain + SQLite repo with pair-date + nearest-on-or-before lookup; BcbPtaxProvider with mockable HTTP seam and midpoint+invert calculation; 16 new tests + 1 ignored live-BCB test.

**ExchangeRate domain + SQLite repo with pair-date + nearest-on-or-before lookup; BcbPtaxProvider with mockable HTTP seam and midpoint+invert calculation; 16 new tests + 1 ignored live-BCB test.**

## What Happened

Three layers in one task — domain, storage, and the network adapter.

**Domain (`src/domain/exchange/`):**
- `ExchangeRate { id, from, to, rate, date, source, fetched_at }` with a `new()` constructor that stamps `fetched_at = Utc::now()`.
- `ExchangeRateRepository` trait: `save`, `find_by_pair_date` (exact match), `find_nearest_on_or_before` (walk-back fallback). The "nearest" method is named to make the semantics obvious — it's not "find the closest regardless of direction", it's specifically "latest rate with date ≤ queried".
- Registered `exchange` submodule in `src/domain/mod.rs`.

**Storage (`src/infrastructure/storage/exchange_rate_repo.rs`):**
- `SqliteExchangeRateRepository` implements the trait against the `exchange_rates` table that's existed since migration 001. Row mapping mirrors the transaction_repo pattern (string-parsed Decimal, ISO date, RFC3339 timestamp).
- `find_nearest_on_or_before` uses `WHERE date <= ?3 ORDER BY date DESC LIMIT 1`. Efficient given the unique pair+date index.
- `save` uses `INSERT OR REPLACE` — unlike transactions (which are append-only with UUID PKs), rates for a given (from, to, date) can legitimately be refreshed if BCB republishes.
- +6 tests: save/find round-trip, missing-row returns None, nearest picks latest prior, nearest returns None with no earlier row, upsert on duplicate pair+date, BRL→USD and USD→BRL are independent rows.

**Network adapter (`src/infrastructure/exchange/`):**
- `RateProvider` trait — `fetch(from, to, date) → Result<Decimal>` + `source_name()`. Same-currency must short-circuit without a network call.
- `HttpClient` trait — a thin 1-method abstraction (`get(url) → String`) so tests can inject a `FakeHttpClient` with canned responses. `UreqHttpClient` is the production impl (ureq 2.x, default tls feature for HTTPS). HTTP failures map to `DomainError::Import` with status code + body preview (first 200 chars).
- `BcbPtaxProvider<C: HttpClient>` builds the Olinda OData URL (`dataCotacao='MM-DD-YYYY'`, odd but documented), parses `value[0].cotacaoCompra` + `cotacaoVenda`, returns the midpoint `(compra + venda) / 2` at 6 decimal places. Empty `value` array → `DomainError::NotFound` (caller handles weekend/holiday walk-back).
- Rate direction logic: BCB publishes BRL-per-USD. For `USD→BRL`, return midpoint directly. For `BRL→USD`, return `1 / midpoint`. For same-currency, return `Decimal::ONE` without hitting HTTP. For any non-USD/BRL pair, reject up front (sets a clear boundary — future slices can add other providers).
- +8 tests using `FakeHttpClient` (url→canned body or canned error-string, with call-count tracking): short-circuit for same currency, USD→BRL midpoint, BRL→USD inverted midpoint, empty-array → NotFound, malformed JSON → Import, HTTP error propagation, source_name, no-HTTP-call on same currency.
- +1 `#[ignore]`d live test that hits the real BCB endpoint for a known past business day (2025-01-02) and asserts a rate in the 1–20 BRL/USD sanity band. Flip with `cargo test -- --ignored` for manual verification.

**Cargo:** added `ureq = "2"`. Default TLS feature gives HTTPS support. No tokio, no reqwest — sync is fine and keeps the dep tree small.

**Lint cleanup:** removed a leftover `AccountRepository as _` import in `transaction_service.rs` tests that was throwing an "unused import" warning under current rustc. Plain `AccountRepository` resolves trait methods equivalently and doesn't warn.

187 unit tests passing (was 171 after S02; +16 new in T01). Plus 1 ignored live test and the existing integration test.

## Verification

`cargo test` → 187 unit + 1 integration passed, 1 ignored (live BCB), 0 failed. `cargo build` → clean. Ureq pulls in rustls + ring + url (a dozen small crates); compile time +a few seconds, not blocking.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 120ms |

## Deviations

Added 6 test variants beyond the minimum plan (direction independence, nearest-on-or-before edge cases). Cheap and they pin important invariants. Removed an empty placeholder test (non-usd-brl rejection) that couldn't be exercised meaningfully until a third currency variant exists \u2014 cleaner than leaving a `let _ = ` stub. Fixed an unrelated `unused import` warning in transaction_service.rs tests that surfaced when the compiler reran the full test suite.

## Known Issues

None.

## Files Created/Modified

- `src/domain/exchange/mod.rs`
- `src/domain/exchange/rate.rs`
- `src/domain/exchange/repository.rs`
- `src/domain/mod.rs`
- `src/infrastructure/storage/exchange_rate_repo.rs`
- `src/infrastructure/storage/mod.rs`
- `src/infrastructure/exchange/mod.rs`
- `src/infrastructure/exchange/bcb_ptax.rs`
- `Cargo.toml`
- `src/application/transaction_service.rs`
