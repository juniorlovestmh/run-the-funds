# S02: Transaction Import Pipeline — UAT

**Milestone:** M001
**Written:** 2026-04-19T19:22:36.682Z

# S02: Transaction Import Pipeline — UAT

**Milestone:** M001
**Written:** 2026-04-19

Scope: `rtf transactions import` parses real-shape Nubank (OFX 1.x SGML with XML-closed leaves) and Chase QFX (OFX 1.x SGML with unclosed leaves) files end-to-end, persists the resulting transactions with FITID-backed dedup, enforces currency match against the destination account, and surfaces a clear "csv deferred" error.

Automated coverage: `tests/import_demo.rs` exercises every step below against the built binary and a temp DB.

## Test: Create USD + BRL accounts

```bash
rtf --db /tmp/s02.db accounts create --name "Chase" --type checking --currency USD --owner "Sky"
rtf --db /tmp/s02.db accounts create --name "Nubank" --type checking --currency BRL --owner "Sky"
```

**Expected:** Two `{status:"ok", data:{...}}` envelopes; capture `data.id` for subsequent commands.
**Result:** PASS

## Test: Import Chase QFX (OFX 1.x SGML, unclosed leaves)

```bash
rtf --db /tmp/s02.db transactions import --format ofx --file tests/fixtures/chase-sample.qfx --account-id <CHASE_ID>
```

**Expected:** `{status:"ok", data:{imported:523, duplicates:0, format:"ofx", ...}}`.
**Result:** PASS — 523 STMTTRN records parsed from the 4 240-line QFX.

## Test: Re-import Chase QFX via the `qfx` alias

```bash
rtf --db /tmp/s02.db transactions import --format qfx --file tests/fixtures/chase-sample.qfx --account-id <CHASE_ID>
```

**Expected:** `{status:"ok", data:{imported:0, duplicates:523, format:"qfx", ...}}`.
**Result:** PASS — dedup via the partial unique index on `(account_id, external_id)`.

## Test: Import Nubank OFX (OFX 1.x header, XML-closed leaves, UTF-8 BRL)

```bash
rtf --db /tmp/s02.db transactions import --format ofx --file tests/fixtures/nubank-sample.ofx --account-id <NUBANK_ID>
```

**Expected:** `{status:"ok", data:{imported:13, duplicates:0, ...}}`.
**Result:** PASS — 13 transactions, all BRL, Portuguese accents preserved (e.g. `Transferência ... Câmbio`).

## Test: Re-import Nubank OFX

```bash
rtf --db /tmp/s02.db transactions import --format ofx --file tests/fixtures/nubank-sample.ofx --account-id <NUBANK_ID>
```

**Expected:** `{status:"ok", data:{imported:0, duplicates:13, ...}}`.
**Result:** PASS

## Test: List transactions by account as JSON

```bash
rtf --db /tmp/s02.db transactions list --account-id <CHASE_ID> --format json
```

**Expected:** JSON array of 523 transactions, sorted `date DESC`. First row: `external_id == "202604160"`, `date == "2026-04-16"`, `amount.amount == "-200.00"`, `amount.currency == "USD"`, payee contains "Wise".
**Result:** PASS

## Test: Currency mismatch rejected atomically

```bash
rtf --db /tmp/s02.db transactions import --format ofx --file tests/fixtures/chase-sample.qfx --account-id <NUBANK_ID>
```

**Expected:** Exit 1, stderr `{status:"error", message:"currency mismatch: expected BRL, got USD"}`. Zero rows persisted to the Nubank account.
**Result:** PASS

## Test: CSV format rejected with explicit "deferred" message

```bash
rtf --db /tmp/s02.db transactions import --format csv --file /dev/null --account-id <CHASE_ID>
```

**Expected:** Exit 2, stderr message contains `"deferred to a later slice"`.
**Result:** PASS

## Test: Full test suite

```bash
cargo test
```

**Result:** PASS — 171 unit tests + 1 integration test.

## Roadmap "Done" criteria coverage

| Criterion | Proven by |
|-----------|-----------|
| Import a real Nubank OFX file | "Import Nubank OFX" test above |
| Import a real Chase CSV file | **Revised scope:** replaced by Chase QFX — same OFX adapter, real FITIDs for tight dedup. CSV deferred to a later slice (see S02-PLAN). |
| `rtf transactions list --format json` shows normalized transactions from both banks with correct amounts, dates, and payee names | "List transactions by account as JSON" test above |

