---
id: T04
parent: S02
milestone: M001
key_files:
  - tests/import_demo.rs
  - .gsd/milestones/M001/slices/S02/S02-UAT.md
key_decisions:
  - One consolidated E2E test rather than many small ones — shares the account-creation setup cost and keeps the happy + unhappy paths tightly related in the same test body.
  - Panic-on-parse helpers (`parse_stdout`, `parse_stderr`) include stderr in their panic messages so a CLI failure points directly at the cause instead of a vague JSON-decode error.
  - Tested the `qfx` alias explicitly on the re-import step to cover two concerns in one call (alias routing + dedup).
duration: 
verification_result: passed
completed_at: 2026-04-19T19:19:27.387Z
blocker_discovered: false
---

# T04: End-to-end integration test drives the compiled binary through the full import → list → re-import → mismatch → csv-rejection path; S02-UAT.md documents the user-facing demo.

**End-to-end integration test drives the compiled binary through the full import → list → re-import → mismatch → csv-rejection path; S02-UAT.md documents the user-facing demo.**

## What Happened

**`tests/import_demo.rs`** — a single `end_to_end_import_demo` integration test that shells out to the binary via `env!("CARGO_BIN_EXE_fintrack")` and a temp SQLite DB (`TempDir`), exercising:

1. `accounts create` for Chase (USD) and Nubank (BRL), capturing each account id from the JSON envelope.
2. `transactions import --format ofx --file chase-sample.qfx --account-id <chase>` — asserts `imported == 523`, `duplicates == 0`, `format == "ofx"`.
3. Re-import Chase via `--format qfx` (alias) — asserts `imported == 0`, `duplicates == 523`, `format == "qfx"` (proves the alias wiring AND dedup against migration 002's partial unique index).
4. `transactions import --format ofx --file nubank-sample.ofx --account-id <nubank>` — asserts `imported == 13`, `duplicates == 0`.
5. Re-import Nubank — asserts `imported == 0`, `duplicates == 13`.
6. `transactions list --account-id <chase> --format json` — parses the JSON, asserts 523 rows, and spot-checks the first row (FITID `202604160`, date `2026-04-16`, currency `USD`). Same pattern for Nubank, asserting every row is BRL.
7. **Currency mismatch path:** Chase QFX into the Nubank account — asserts exit non-zero, stderr JSON carries `status:"error"` with `"currency mismatch"` in the message.
8. **CSV rejection:** `--format csv --file /dev/null` — asserts exit 2, stderr message contains `"deferred"`.

Helpers: `bin()` resolves the compiled binary path; `run(args)` executes and returns `Output`; `parse_stdout` / `parse_stderr` validate JSON and give readable panic messages when parsing fails; `create_account` is a DRY wrapper that returns the newly-created account's id.

**`.gsd/milestones/M001/slices/S02/S02-UAT.md`** — mirrors the S01-UAT structure. Eight scenarios with the exact CLI commands, expected JSON shapes, and results (all PASS). Includes a "Roadmap Done criteria coverage" table that explicitly maps each Done bullet to the proving test — including the revised scope note that Chase CSV was replaced by Chase QFX (same adapter, real FITIDs).

**Test totals:** 171 unit tests + 1 integration test = **172 passing, 0 failed**.

## Verification

`cargo test --test import_demo` → 1 passed, 0 failed (0.73s). `cargo test` (full suite) → 171 unit + 1 integration = 172 passed, 0 failed. Binary exercises all S02-UAT steps end-to-end.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test --test import_demo` | 0 | pass | 730ms |
| 2 | `cargo test` | 0 | pass | 880ms |

## Deviations

None from the planned scope.

## Known Issues

None.

## Files Created/Modified

- `tests/import_demo.rs`
- `.gsd/milestones/M001/slices/S02/S02-UAT.md`
