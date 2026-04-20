# S03: Multi-Currency (USD↔BRL via BCB PTAX) — UAT

**Milestone:** M001
**Written:** 2026-04-19T20:41:27.330Z

Scope: BCB PTAX-backed USD↔BRL currency conversion with lazy fetch-and-cache.
`rtf convert` returns a real rate for any date. `rtf transactions
list --format json` enriches every row with `amount_usd` + rate metadata.
Weekend/holiday fallback walks back up to 7 days. Zero manual steps — if a
rate isn't cached, the CLI fetches it automatically.

## Test: Convert USD → BRL at an exact cached rate

```bash
rtf --db /tmp/s03.db convert 1000 USD --to BRL --date 2026-04-07
```

**Expected:** `{status:"ok", data:{amount:"5120.00", currency:"BRL", rate:"5.12", rate_date:"2026-04-07", source:"BCB PTAX", fallback_reason:null}}`.
**Result:** PASS

## Test: Convert BRL → USD (inverse direction)

```bash
rtf --db /tmp/s03.db convert 500 BRL --to USD --date 2026-04-07
```

**Expected:** Same envelope shape; currency = USD. Rate shown is midpoint-inverted.
**Result:** PASS

## Test: Same-currency identity

```bash
rtf --db /tmp/s03.db convert 100 USD --to USD --date 2026-04-07
```

**Expected:** `{status:"ok", data:{amount:"100", currency:"USD", rate:"1", source:"identity", fallback_reason:null}}` — no BCB round-trip.
**Result:** PASS

## Test: Weekend walk-back (Sunday → Friday)

```bash
rtf --db /tmp/s03.db convert 200 USD --to BRL --date 2026-04-05
```

**Expected:** Exit 0, `rate_date: "2026-04-03"`, `fallback_reason` populated.
**Result:** PASS

## Test: Negative amount preserves sign

```bash
rtf --db /tmp/s03.db convert -150 BRL --to USD --date 2026-04-07
```

**Expected:** Exit 0, `data.amount == "-29.250"`.
**Result:** PASS

## Test: Malformed amount and date return clear errors

```bash
rtf --db /tmp/s03.db convert not-a-number USD --to BRL --date 2026-04-07
rtf --db /tmp/s03.db convert 100 USD --to BRL --date not-a-date
```

**Expected:** Exit 1, stderr JSON with clear `invalid amount` / `invalid date` messages.
**Result:** PASS

## Test: Multi-currency list — BRL transactions show USD equivalent

```bash
rtf --db /tmp/s03.db transactions list --account-id <NUBANK_ID> --format json
```

**Expected:** Each BRL row gets `amount_usd`, `rate`, `rate_date`, `rate_status: "ok"` (or `"fallback"` for weekend-posted). USD rows get `rate_status: "same_currency"`, `rate: "1"`, `amount_usd == amount.amount`. Unavailable rows degrade to `amount_usd: null` without aborting the listing.
**Result:** PASS

## Test: Full test suite

```bash
cargo test
```

**Result:** PASS — 197 unit + 3 convert_demo + 1 import_demo integration = 201 tests, 2 ignored (live BCB).

## Roadmap "Done" criteria coverage

| Criterion | Proven by |
|-----------|-----------|
| `rtf convert 1000 BRL --to USD --date 2026-03-15` returns accurate conversion | "Convert USD→BRL / BRL→USD" tests |
| BRL transactions display with USD equivalent at the correct PTAX rate for each transaction date | "Multi-currency list" test |
| SimpleFIN adapter pulls US bank transactions | **Moved to S04** (new bank-sync slice) |
| Pluggy adapter pulls Brazilian bank transactions | **Moved to S04** |

