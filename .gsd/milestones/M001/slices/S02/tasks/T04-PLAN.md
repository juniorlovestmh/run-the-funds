---
estimated_steps: 13
estimated_files: 2
skills_used: []
---

# T04: End-to-end demo: import Chase + Nubank fixtures via the built binary; write S02-UAT.md

Lock the slice with an integration test that shells out to the compiled `rtf` binary and runs the full import → list → re-import path. Mirrors the S01-UAT structure for consistency.

Create `tests/import_demo.rs` (cargo integration test — separate crate, so it uses `env!("CARGO_BIN_EXE_rtf")` to find the binary):
1. Spawn a temp DB via `tempfile::NamedTempFile` — pass its path to every invocation via `--db`.
2. `accounts create --name Chase --type checking --currency USD --owner Sky` — parse stdout JSON, capture the returned account id.
3. `accounts create --name Nubank --type checking --currency BRL --owner Sky` — same.
4. `transactions import --format ofx --file tests/fixtures/chase-sample.qfx --account-id <chase_id>` — assert `status=="ok"`, `data.imported == <expected>`, `data.duplicates == 0`.
5. Re-run the same import — assert `data.imported == 0`, `data.duplicates == <expected>`.
6. Same two-step dance for Nubank fixture (gate with `if Path::new("tests/fixtures/nubank-sample.ofx").exists()` — skip if missing so CI doesn't fail before the user supplies it).
7. `transactions list --account-id <chase_id> --format json` — parse JSON array, assert first entry has non-empty `external_id`, an `amount.currency == "USD"`, and a parseable `date`.
8. Currency-mismatch path: `transactions import --format ofx --file tests/fixtures/chase-sample.qfx --account-id <nubank_id>` — assert `status=="error"`, message contains `currency mismatch`.
9. CSV rejection: `transactions import --format csv --file /dev/null --account-id <chase_id>` — exit 2, message contains `deferred`.

Use `std::process::Command::new(env!("CARGO_BIN_EXE_rtf"))` and `serde_json::from_slice` to parse each stdout.

Also write `.gsd/milestones/M001/slices/S02/S02-UAT.md` mirroring the shape of S01-UAT: list the exact CLI commands, the expected JSON shapes, and the roadmap Done criteria this proves.

## Inputs

- `target/debug/rtf (built binary)`
- `tests/fixtures/chase-sample.qfx`
- `tests/fixtures/nubank-sample.ofx (when available)`
- `.gsd/milestones/M001/slices/S01/S01-UAT.md (as template)`

## Expected Output

- `Passing integration test covering import → list → re-import → mismatch → csv-rejection`
- `S02-UAT.md with user-facing demo commands + expected output`
- `Roadmap S02 'Done' criteria proven end-to-end`

## Verification

cargo test --test import_demo
