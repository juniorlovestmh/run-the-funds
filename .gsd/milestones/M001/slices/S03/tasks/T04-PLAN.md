---
estimated_steps: 12
estimated_files: 3
skills_used: []
---

# T04: End-to-end demo: live BCB fetch, convert, list with USD equivalents; write S03-UAT.md

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

## Inputs

- `target/debug/rtf (built binary)`
- `tests/fixtures/nubank-sample.ofx`
- `.gsd/milestones/M001/slices/S02/S02-UAT.md (as template)`
- `.gsd/milestones/M001/M001-ROADMAP.md`

## Expected Output

- `Integration test covering offline + live variants`
- `S03-UAT.md with full demo commands and expected shapes`
- `Roadmap updated to split S03 and renumber S04-S07 → S05-S08`

## Verification

cargo test --test convert_demo && cargo test
