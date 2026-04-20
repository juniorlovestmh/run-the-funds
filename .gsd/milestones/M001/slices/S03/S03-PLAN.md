# S03: Multi-Currency + Bank Sync

**Goal:** Add USD↔BRL currency conversion backed by Banco Central do Brasil (BCB) PTAX rates. Rates fetch lazily from the live BCB API on first use and cache in the `exchange_rates` table so subsequent lookups are local. `rtf convert` returns a real rate for any date. `rtf transactions list --format json` enriches BRL transactions with their USD equivalent at the transaction date. Zero manual steps — if a rate isn't cached, the CLI fetches it automatically.
**Demo:** BRL transactions display with USD equivalent at the correct PTAX rate for each transaction date. SimpleFIN adapter pulls US bank transactions. Pluggy adapter pulls Brazilian bank transactions. rtf convert 1000 BRL --to USD --date 2026-03-15 returns accurate conversion.

## Must-Haves

- `rtf convert 1000 BRL --to USD --date 2026-04-07` returns `{status:"ok", data:{amount:"...", currency:"USD", rate:"...", source:"BCB PTAX", date:"2026-04-07"}}` with a real PTAX-backed rate (fetched on first call, cached on subsequent calls).
- `rtf convert 500 USD --to BRL --date 2026-04-07` — reverse direction works identically.
- Weekend/holiday fallback: `rtf convert 100 BRL --to USD --date 2026-04-05` (Sunday) succeeds by falling back to the most recent business-day rate and notes the fallback in the response (`rate_date: "2026-04-03"` when the queried date differs from the quote date).
- Same-currency conversion returns rate=1 and no BCB round-trip.
- `rtf transactions list --account-id <brl> --format json` returns rows with a new `amount_usd` field populated via PTAX rates for each transaction's date. Missing-rate rows surface `amount_usd: null` with a `rate_status: "unavailable"` hint (rather than erroring the whole listing).
- `cargo test` passes, including unit tests with a mocked rate provider and at least one live `#[ignore]`d integration test hitting the real BCB endpoint so we can flip it on for manual verification.
- No manual step required to populate rates before use.

## Proof Level

- This slice proves: contract — exchange-rate fetching, caching, and multi-currency display work end-to-end against the real BCB PTAX API. Downstream slices (S05 categorization, S06 household attribution, S07 agent-ready query layer) can assume any transaction can be expressed in USD at its transaction-date rate without additional work.

## Integration Closure

- Upstream surfaces consumed: `exchange_rates` table from migration 001 (S01); `Transaction`, `Money`, `CurrencyCode` domain types (S01); `TransactionService::list_by_account` (S02); CLI envelope shape (S01).
- New wiring introduced: `ExchangeRate` domain type + `ExchangeRateRepository` trait + SQLite impl; `RateProvider` trait with `BcbPtaxProvider` impl (HTTP); `CurrencyConverter` service; `rtf convert` CLI subcommand; a thin presentation struct that enriches `Transaction` with `amount_usd` for JSON output; `ureq` added to `Cargo.toml` as the HTTP client.
- What remains: **S04 (bank sync adapters — SimpleFIN + Pluggy)** as the next slice — roadmap renumbers so current S04-S07 shift to S05-S08. No schema changes blocked by this slice.

## Verification

- `rtf convert` response includes `rate`, `source: "BCB PTAX"`, `rate_date`, and (when different from the queried date) a `fallback_reason` explaining the weekend/holiday shift.
- `rtf transactions list --format json` per-row `amount_usd` + `rate_status` ("ok" | "unavailable" | "cached" | "fetched") lets an operator/agent see at a glance which rows are live vs stale vs fallback.
- HTTP failures from BCB surface as `DomainError::Import(String)` carrying the BCB error (status code + body) — never silently coerce to NaN or zero.
- First-run verbosity: when a lookup triggers a BCB fetch, stderr gets a one-line log (`fetching PTAX for 2026-04-07...`) so the user knows why the call took ~500ms. Subsequent cached runs are silent.

## Tasks

- [x] **T01: ExchangeRate domain + SQLite repo + BCB PTAX provider (lazy fetch with weekend fallback)** `est:2.5h`
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
  - Files: `src/domain/exchange/mod.rs`, `src/domain/exchange/rate.rs`, `src/domain/exchange/repository.rs`, `src/domain/mod.rs`, `src/infrastructure/storage/exchange_rate_repo.rs`, `src/infrastructure/storage/mod.rs`, `src/infrastructure/exchange/mod.rs`, `src/infrastructure/exchange/bcb_ptax.rs`, `Cargo.toml`
  - Verify: cargo test -- domain::exchange && cargo test -- infrastructure::storage::exchange_rate_repo && cargo test -- infrastructure::exchange

- [x] **T02: CurrencyConverter service + `rtf convert` CLI (lazy fetch + weekend fallback)** `est:2h`
  Glue the repository and provider into an orchestrated converter and expose it through the CLI.

**`CurrencyConverter` service** (`src/application/currency_converter.rs`):
- Generic over `ExchangeRateRepository` + `RateProvider`.
- Method `convert(amount: Decimal, from: CurrencyCode, to: CurrencyCode, date: NaiveDate) -> Result<Conversion, DomainError>` where `Conversion { amount: Decimal, currency: CurrencyCode, rate: Decimal, rate_date: NaiveDate, source: String, fallback_reason: Option<String> }`.
- Flow:
  1. `from == to` → return `{ amount, currency: to, rate: 1, rate_date: date, source: "identity", fallback_reason: None }` immediately.
  2. Check `repo.find_by_pair_date(from, to, date)` — cache hit → use it.
  3. Cache miss → `provider.fetch(from, to, date)`. On success, persist via `repo.save` and use the rate.
  4. `NotFound` from provider (weekend/holiday) → **walk back up to 7 days** calling `provider.fetch` until a rate is found. Persist the found rate under BOTH the original queried date AND the actual quote date so subsequent identical queries hit the cache directly. `fallback_reason` set to `"no PTAX published for YYYY-MM-DD; used YYYY-MM-DD (N days earlier)"`.
  5. Walk-back exhausted (no rate in 7 days) → `DomainError::NotFound`.
- Log a single-line stderr note when a fetch happens (`eprintln!("fetching PTAX for {date}...")`). Cached paths stay silent.

**CLI** (`src/cli/convert.rs`, wired in `src/cli/mod.rs` + `src/main.rs`):
- `rtf convert <amount> <from> --to <currency> --date <YYYY-MM-DD>`.
- amount: positional `Decimal` (parse via `Decimal::from_str`).
- from: positional `CurrencyCode`.
- `--to <CurrencyCode>` (required).
- `--date <NaiveDate>` optional; default: today.
- Output (success): `{status:"ok", data:{amount: "195.5500", currency:"USD", rate:"5.1150", rate_date:"2026-04-07", source:"BCB PTAX", fallback_reason: null}}`.
- Output (error): standard `{status:"error", message:"..."}` envelope.

**Tests:**
- Service-level with a `FakeRateProvider` (returns canned rates keyed by date): happy path (USD→BRL, BRL→USD, same currency), cache-miss-then-fetch (provider called once, persist, next call skips provider), weekend fallback (date N → NotFound; date N-2 → rate; result has `fallback_reason` set and persists under both dates), walk-back exhausted → `NotFound`.
- Zero amount returns zero result (no provider call needed beyond rate lookup).
- Negative amount preserves sign (`-500 BRL → USD` returns a negative USD amount).
- CLI-level: one smoke test via the binary with a mocked rate in the DB (pre-seeded) so the real CLI output is verified without network.
  - Files: `src/application/currency_converter.rs`, `src/application/mod.rs`, `src/cli/convert.rs`, `src/cli/mod.rs`, `src/main.rs`
  - Verify: cargo test -- application::currency_converter && cargo test -- cli::convert

- [x] **T03: Multi-currency display: enrich `transactions list --format json` with amount_usd** `est:1.5h`
  Make BRL transactions self-explain in USD terms without the user running `convert` per row.

**Presentation struct** (`src/cli/transactions.rs`): local `TransactionView` that wraps `Transaction` and adds:
- `amount_usd: Option<String>` — stringified Decimal, None when the conversion can't be resolved.
- `rate: Option<String>` — the rate used, stringified Decimal.
- `rate_date: Option<NaiveDate>`.
- `rate_status: String` — one of `"ok"` (cached hit), `"fetched"` (hit network during this call), `"fallback"` (weekend walk-back), `"unavailable"` (exhausted 7-day walk), `"same_currency"` (USD account; rate=1).
- Derives `Serialize`; all `Transaction` fields flattened via `#[serde(flatten)]`.

**Flow in `handle_list`:**
1. List transactions as today.
2. Collect unique `(currency, date)` pairs, dedup.
3. For each pair, call `CurrencyConverter` once — batching minimizes BCB round-trips when a BRL account has many transactions on the same date.
4. Map the resulting rates back onto each transaction; build a `Vec<TransactionView>`.
5. Serialize and print.

When `rate_status == "unavailable"`, still emit the row — just with `amount_usd: null`. Don't fail the whole listing.

**Table output:** untouched. Keep the human-readable `list` without `--format json` simple (no USD column yet — can come later).

**Tests:**
- Unit-level: given a fake converter returning a canned `Conversion`, `build_views` produces the expected JSON shape. Cover: BRL txn with rate → populated, BRL txn with unavailable → null + status, USD txn → status=`same_currency`, mixed list of both.
- Integration-level: extend `tests/import_demo.rs` (or add `tests/convert_demo.rs`) that pre-seeds an exchange rate in the DB, imports the Nubank fixture, then runs `transactions list --format json` and asserts `amount_usd` is populated on all 13 rows with `rate_status == "cached"`. This avoids live-BCB dependency in CI.
  - Files: `src/cli/transactions.rs`, `tests/import_demo.rs`
  - Verify: cargo test -- cli::transactions && cargo test --test import_demo

- [x] **T04: End-to-end demo: live BCB fetch, convert, list with USD equivalents; write S03-UAT.md** `est:1.5h`
  Lock the slice with a live-BCB integration test and a UAT document that mirrors S01/S02.

**`tests/convert_demo.rs`** (or extend `tests/import_demo.rs`): a new integration test that exercises the full flow against the compiled binary and a temp DB. Two variants:

1. **Offline demo** (always-on): pre-seed the `exchange_rates` table with a known USD↔BRL rate for the Nubank fixture's date range, then run `rtf convert` + `rtf transactions list --format json` and assert expected shapes. No network.
2. **Live demo** (`#[ignore]`d by default): same flow but WITHOUT pre-seeding, so the first `convert` call hits real BCB. Asserts the returned rate is non-zero and within a sane range (e.g., 0 < rate < 100) — a loose sanity check that survives real-world rate drift. Running this test manually proves the live path when the user wants to verify.

Both variants cover:
- `rtf accounts create --currency BRL` for Nubank.
- `rtf transactions import` the existing Nubank fixture.
- `rtf convert 1000 BRL --to USD --date <fixture-date>` — assert non-zero USD amount + rate.
- `rtf convert 100 BRL --to USD --date <weekend>` — assert `fallback_reason` is populated and the rate_date differs.
- `rtf transactions list --account-id <nubank> --format json` — assert every row has a populated `amount_usd` (or `rate_status: unavailable` with null).

**`S03-UAT.md`**: same shape as S02-UAT.md. Eight-ish scenarios with CLI commands, expected JSON shapes, Roadmap Done-criteria coverage table. Mark the USD-equivalent-display criterion as PROVEN; note that bank-sync (SimpleFIN + Pluggy) moves to S04.

**Roadmap update:** edit `.gsd/milestones/M001/M001-ROADMAP.md` to split the old S03 — current slice stays as S03 (PTAX + convert), new S04 gets "Bank Sync Adapters (SimpleFIN + Pluggy)", existing S04-S07 shift to S05-S08 with their `Depends` columns updated.
  - Files: `tests/convert_demo.rs`, `.gsd/milestones/M001/slices/S03/S03-UAT.md`, `.gsd/milestones/M001/M001-ROADMAP.md`
  - Verify: cargo test --test convert_demo && cargo test

## Files Likely Touched

- src/domain/exchange/mod.rs
- src/domain/exchange/rate.rs
- src/domain/exchange/repository.rs
- src/domain/mod.rs
- src/infrastructure/storage/exchange_rate_repo.rs
- src/infrastructure/storage/mod.rs
- src/infrastructure/exchange/mod.rs
- src/infrastructure/exchange/bcb_ptax.rs
- Cargo.toml
- src/application/currency_converter.rs
- src/application/mod.rs
- src/cli/convert.rs
- src/cli/mod.rs
- src/main.rs
- src/cli/transactions.rs
- tests/import_demo.rs
- tests/convert_demo.rs
- .gsd/milestones/M001/slices/S03/S03-UAT.md
- .gsd/milestones/M001/M001-ROADMAP.md
