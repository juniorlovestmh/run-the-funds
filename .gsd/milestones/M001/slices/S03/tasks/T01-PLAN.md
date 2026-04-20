---
estimated_steps: 19
estimated_files: 9
skills_used: []
---

# T01: ExchangeRate domain + SQLite repo + BCB PTAX provider (lazy fetch with weekend fallback)

Add the foundation for historical-rate lookup.

**Domain** (`src/domain/exchange/`):
- `ExchangeRate { id, from: CurrencyCode, to: CurrencyCode, rate: Decimal, date: NaiveDate, source: String, fetched_at: DateTime<Utc> }`.
- `ExchangeRateRepository` trait: `save(&ExchangeRate)`, `find_by_pair_date(from, to, date) -> Option<ExchangeRate>`, `find_nearest_on_or_before(from, to, date) -> Option<ExchangeRate>` (for weekend fallback).
- Rationale: keeping the domain free of HTTP lets the provider swap for tests.

**SQLite impl** (`src/infrastructure/storage/exchange_rate_repo.rs`): implements the trait against the existing `exchange_rates` table (migration 001 already defined schema with a unique index on `(from_currency, to_currency, date)`). TDD: save/find round-trip, `find_nearest_on_or_before` returns the latest row ≤ queried date.

**RateProvider trait** (`src/infrastructure/exchange/mod.rs`): `fn fetch(&self, from: CurrencyCode, to: CurrencyCode, date: NaiveDate) -> Result<Decimal, DomainError>`. Abstract so tests can swap for a canned impl.

**BCB PTAX provider** (`src/infrastructure/exchange/bcb_ptax.rs`):
- Endpoint: `https://olinda.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata/CotacaoDolarDia(dataCotacao=@dataCotacao)?@dataCotacao='MM-DD-YYYY'&$format=json`.
- HTTP via `ureq` (new dep — pick latest stable; use the default `tls` feature for HTTPS).
- Parse JSON, grab `value[0].cotacaoCompra` and `value[0].cotacaoVenda`, return the **midpoint** (`(compra + venda) / 2`) as the rate. Midpoint avoids bias against either direction.
- Empty `value` array → `DomainError::NotFound { entity: "ExchangeRate", id: format!("{}-{}-{}", from, to, date) }` so the caller (T02) can do its fallback walk.
- HTTP errors (non-2xx, timeouts) → `DomainError::Import(String)` with status + first 200 chars of body.
- Rate direction: BCB publishes BRL-per-USD. For `from=USD, to=BRL`, return the midpoint directly. For `from=BRL, to=USD`, return `1 / midpoint`. For `from=USD, to=USD` or `from=BRL, to=BRL`, short-circuit to 1 before the HTTP call.

Add `ureq` to `Cargo.toml` (workspace dep). Do not add `tokio`/`reqwest`— sync is fine and pulls less.

TDD:
- `exchange_rate_repo`: save/find round-trip; `find_nearest_on_or_before` returns the closest ≤ date; unique-index prevents duplicate (from, to, date).
- `bcb_ptax` unit tests: **mock the HTTP layer** — wrap the ureq call behind a small `HttpClient` trait or use `ureq`'s test utilities so we can inject canned responses. Cover: successful parse, empty `value`, HTTP 500, malformed JSON, same-currency short-circuit.
- `bcb_ptax` live test: `#[ignore]` by default, hits the real API for a known past date (e.g. 2025-01-02) and asserts a reasonable non-zero rate.

## Inputs

- `migrations/001_initial.sql (exchange_rates table already defined)`
- `src/domain/currency/currency_code.rs`
- `src/domain/error.rs`

## Expected Output

- `ExchangeRate domain type + repository trait + SQLite impl`
- `RateProvider trait + BcbPtaxProvider with midpoint calculation + same-currency short-circuit`
- `HTTP layer mockable via a small trait seam — unit tests never hit the network`
- `One #[ignore]d live-BCB test for manual verification`

## Verification

cargo test -- domain::exchange && cargo test -- infrastructure::storage::exchange_rate_repo && cargo test -- infrastructure::exchange
