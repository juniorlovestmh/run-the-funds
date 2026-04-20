---
estimated_steps: 24
estimated_files: 5
skills_used: []
---

# T02: CurrencyConverter service + `rtf convert` CLI (lazy fetch + weekend fallback)

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

## Inputs

- `src/domain/exchange/ (T01)`
- `src/infrastructure/exchange/ (T01)`

## Expected Output

- ``CurrencyConverter` service with cache-first, fetch-on-miss, weekend-walkback logic`
- ``Conversion` struct (Serialize)`
- ``rtf convert` CLI handler with consistent JSON envelope`
- `Tests covering cache-hit, cache-miss, walk-back, exhausted, zero, negative, same-currency`

## Verification

cargo test -- application::currency_converter && cargo test -- cli::convert
