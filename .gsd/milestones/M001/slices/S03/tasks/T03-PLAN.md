---
estimated_steps: 18
estimated_files: 2
skills_used: []
---

# T03: Multi-currency display: enrich `transactions list --format json` with amount_usd

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

## Inputs

- `src/application/currency_converter.rs (T02)`
- `src/cli/transactions.rs (current handler)`

## Expected Output

- ``TransactionView` presentation struct`
- `Batched per-date rate lookup in `handle_list``
- `JSON output carries amount_usd + rate_status per row`
- `Test proves the shape against both a mocked converter and a pre-seeded DB`

## Verification

cargo test -- cli::transactions && cargo test --test import_demo
