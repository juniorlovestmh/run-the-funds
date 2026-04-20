---
id: T07
parent: S01
milestone: M001
key_files:
  - src/application/account_service.rs
  - src/cli/accounts.rs
  - src/cli/response.rs
  - src/cli/mod.rs
  - src/main.rs
key_decisions:
  - JSON response envelope: {status, data} for success, {status, message} for error
  - Database path configurable via --db flag, defaults to rtf.db
  - Errors go to stderr as JSON, exit code 1 for validation/parse failures
duration: 
verification_result: passed
completed_at: 2026-04-18T17:45:48.359Z
blocker_discovered: false
---

# T07: CLI wired end-to-end: accounts create + list with JSON output, AccountService, 131 total tests

**CLI wired end-to-end: accounts create + list with JSON output, AccountService, 131 total tests**

## What Happened

Built AccountService in the application layer orchestrating domain logic through repository traits. Wired clap CLI subcommands: `rtf accounts create` persists to SQLite and returns JSON, `rtf accounts list --format json` returns JSON array, `rtf accounts list` shows human-readable table. Added `--db` flag for database path (defaults to rtf.db). Structured JSON response format: {status: ok, data: ...} for success, {status: error, message: ...} for errors. Validation errors return JSON to stderr with exit code 1. AccountService tests (6): create persists, validation rejection, list empty, get not found, delete, delete nonexistent. End-to-end verified: created BRL + USD accounts, listed as JSON with correct fields.

## Verification

cargo test: 131 passed. E2E: rtf accounts create (BRL + USD), rtf accounts list --format json returns both, rtf accounts list shows table, validation error returns JSON error with exit 1

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test --manifest-path rtf/Cargo.toml` | 0 | pass | 1890ms |
| 2 | `rtf --db /tmp/test.db accounts create --name 'Nubank Checking' --type checking --currency BRL --owner Sky` | 0 | pass | 50ms |
| 3 | `rtf --db /tmp/test.db accounts list --format json` | 0 | pass | 50ms |
| 4 | `rtf --db /tmp/test.db accounts create --name '' --type checking --currency USD --owner Sky` | 1 | pass | 50ms |

## Deviations

None

## Known Issues

None.

## Files Created/Modified

- `src/application/account_service.rs`
- `src/cli/accounts.rs`
- `src/cli/response.rs`
- `src/cli/mod.rs`
- `src/main.rs`
