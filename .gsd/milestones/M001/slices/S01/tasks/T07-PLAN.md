---
estimated_steps: 6
estimated_files: 5
skills_used: []
---

# T07: Wire CLI commands — accounts create + accounts list with JSON output

Build the application service layer (AccountService) that orchestrates domain logic through repository traits. Wire clap CLI subcommands:

- `rtf accounts create --name <name> --type <type> --currency <currency> --owner <owner>` — creates account via AccountService, prints JSON confirmation
- `rtf accounts list --format json` — lists all accounts as JSON array
- `rtf accounts list` — lists accounts in human-readable table format (default)

JSON output uses serde_json with consistent structure: `{"status": "ok", "data": ...}` for success, `{"status": "error", "message": ...}` for errors.

End-to-end test: build binary, run create command, run list command, verify JSON output contains created account with correct fields.

## Inputs

- `src/domain/account/account.rs`
- `src/domain/account/repository.rs`
- `src/infrastructure/storage/account_repo.rs`
- `src/infrastructure/storage/database.rs`

## Expected Output

- `src/application/account_service.rs`
- `src/cli/accounts.rs`
- `src/main.rs`

## Verification

cargo test -- cli && cargo build && ./target/debug/rtf accounts create --name 'Test' --type checking --currency USD --owner 'Sky' && ./target/debug/rtf accounts list --format json
