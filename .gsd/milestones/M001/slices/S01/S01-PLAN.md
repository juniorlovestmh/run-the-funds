# S01: Domain Foundation + Storage

**Goal:** Establish the Rust project with DDD package structure, core domain entities (Account, Transaction, Category, Money, Person), SQLite storage with migrations, and working `rtf accounts create` + `rtf accounts list` CLI commands returning JSON. Full TDD coverage.
**Demo:** rtf accounts create persists an account to SQLite and rtf accounts list returns it as JSON. go test ./... passes with full domain model coverage for Account, Transaction, Category, Money, Person entities.

## Must-Haves

- `cargo test` passes with unit tests for all domain entities and value objects
- `cargo test` passes with integration tests for SQLite repository round-trips
- `rtf accounts create --name "Nubank Checking" --type checking --currency BRL --owner "Sky"` persists to SQLite
- `rtf accounts list --format json` returns JSON array of accounts
- Domain traits (Repository, Importer) define adapter boundaries for country-agnostic extensibility
- All money operations use rust_decimal, never f64

## Proof Level

- This slice proves: contract — domain model compiles, all entities round-trip through SQLite, CLI produces correct JSON output

## Integration Closure

- Upstream surfaces consumed: none (first slice, greenfield)
- New wiring introduced: Rust binary crate, SQLite DB creation, CLI entrypoint
- What remains: transaction import (S02), currency conversion (S03), categorization (S04), household queries (S05), goals/debt (S06), agent query layer (S07)

## Verification

- Inspection surfaces: `rtf accounts list --format json` shows all persisted accounts
- Failure visibility: CLI returns structured JSON errors with context (missing fields, duplicate names, DB errors)
- Redaction constraints: none (no secrets in this slice)

## Tasks

- [x] **T01: Initialize Rust project with DDD module structure and dependencies** `est:30m`
  Create the rtf Rust binary crate with Cargo.toml, DDD-aligned module hierarchy (domain/, application/, infrastructure/, cli/), and all core dependencies. Set up the module tree so subsequent tasks can add entities and implementations without restructuring. Include a minimal main.rs with clap CLI skeleton that prints help.
  - Files: `Cargo.toml`, `src/main.rs`, `src/lib.rs`, `src/domain/mod.rs`, `src/domain/account/mod.rs`, `src/domain/transaction/mod.rs`, `src/domain/category/mod.rs`, `src/domain/currency/mod.rs`, `src/domain/household/mod.rs`, `src/application/mod.rs`, `src/infrastructure/mod.rs`, `src/infrastructure/storage/mod.rs`, `src/infrastructure/importer/mod.rs`, `src/infrastructure/exchange/mod.rs`, `src/infrastructure/sync_adapter/mod.rs`, `src/cli/mod.rs`
  - Verify: cargo build && cargo test

- [x] **T02: Implement Money value object, CurrencyCode, and domain error types with TDD** `est:45m`
  Build the foundational value objects: Money (wraps rust_decimal::Decimal + CurrencyCode), CurrencyCode enum (USD, BRL — extensible via country adapters), and domain error types (DomainError enum). Money must support arithmetic (add/subtract same currency, reject mixed-currency arithmetic), comparison, Display, and serde serialization. All TDD — write failing tests first, then implement.

Also define the core repository trait (AccountRepository) as an interface the storage layer will implement.
  - Files: `src/domain/currency/mod.rs`, `src/domain/currency/money.rs`, `src/domain/currency/currency_code.rs`, `src/domain/error.rs`, `src/domain/account/repository.rs`
  - Verify: cargo test -- domain::currency && cargo test -- domain::error

- [x] **T03: Implement Account and Person entities with TDD** `est:45m`
  Build Account entity (id, name, account_type, currency, owner, institution, account_number_last4, opened_date, notes, created_at, updated_at) and AccountType enum (Checking, Savings, CreditCard, Loan). Build Person entity (id, name, relationship — e.g. Self, Spouse, Child). All fields use strong types. TDD — write tests for entity creation, validation (required fields, valid types), and serde round-trips.

Account uses Money for balance. AccountType determines behavior (credit cards have credit limits, loans have interest rates — fields optional based on type).
  - Files: `src/domain/account/mod.rs`, `src/domain/account/account.rs`, `src/domain/account/account_type.rs`, `src/domain/household/mod.rs`, `src/domain/household/person.rs`
  - Verify: cargo test -- domain::account && cargo test -- domain::household

- [x] **T04: Implement Transaction, Category, and CategoryGroup entities with TDD** `est:45m`
  Build Transaction entity (id, account_id, date, amount as Money, payee, description, category_id, beneficiary_id, transfer_pair_id, status, external_id, imported_at, created_at). TransactionStatus enum (Pending, Cleared, Reconciled). Category entity (id, group_id, name). CategoryGroup entity (id, name). All with serde support and TDD.

Define TransactionRepository and CategoryRepository traits.
  - Files: `src/domain/transaction/mod.rs`, `src/domain/transaction/transaction.rs`, `src/domain/transaction/status.rs`, `src/domain/category/mod.rs`, `src/domain/category/category.rs`, `src/domain/category/category_group.rs`, `src/domain/transaction/repository.rs`, `src/domain/category/repository.rs`
  - Verify: cargo test -- domain::transaction && cargo test -- domain::category

- [x] **T05: Implement SQLite migrations and Account repository with TDD** `est:1h`
  Build the SQLite storage layer: connection management (Database struct wrapping rusqlite::Connection), migration system (embedded SQL files run on first connect), and SqliteAccountRepository implementing the AccountRepository trait.

Migration creates tables for: accounts, persons, categories, category_groups, transactions, exchange_rates. All tables use TEXT for IDs (UUIDs), TEXT for amounts (rust_decimal serialized), and appropriate indexes.

TDD: write integration tests that create an in-memory SQLite DB, run migrations, then test CRUD operations on accounts. Test round-trip: create account → retrieve by ID → list all → verify fields match including Money amounts.
  - Files: `src/infrastructure/storage/mod.rs`, `src/infrastructure/storage/database.rs`, `src/infrastructure/storage/migrations.rs`, `src/infrastructure/storage/account_repo.rs`, `migrations/001_initial.sql`
  - Verify: cargo test -- infrastructure::storage

- [x] **T06: Implement Transaction, Category, and Person SQLite repositories with TDD** `est:1h`
  Build SqliteTransactionRepository, SqliteCategoryRepository, and SqlitePersonRepository — each implementing their respective domain traits. TDD with in-memory SQLite.

Transaction repo tests: create transaction with Money amount → retrieve by ID → list by account → list by date range → verify all fields including status and optional beneficiary_id.
Category repo tests: create group → create category in group → list categories → list by group.
Person repo tests: create person → retrieve → list all.
  - Files: `src/infrastructure/storage/transaction_repo.rs`, `src/infrastructure/storage/category_repo.rs`, `src/infrastructure/storage/person_repo.rs`
  - Verify: cargo test -- infrastructure::storage

- [x] **T07: Wire CLI commands — accounts create + accounts list with JSON output** `est:1h`
  Build the application service layer (AccountService) that orchestrates domain logic through repository traits. Wire clap CLI subcommands:

- `rtf accounts create --name <name> --type <type> --currency <currency> --owner <owner>` — creates account via AccountService, prints JSON confirmation
- `rtf accounts list --format json` — lists all accounts as JSON array
- `rtf accounts list` — lists accounts in human-readable table format (default)

JSON output uses serde_json with consistent structure: `{"status": "ok", "data": ...}` for success, `{"status": "error", "message": ...}` for errors.

End-to-end test: build binary, run create command, run list command, verify JSON output contains created account with correct fields.
  - Files: `src/application/account_service.rs`, `src/application/mod.rs`, `src/cli/mod.rs`, `src/cli/accounts.rs`, `src/main.rs`
  - Verify: cargo test -- cli && cargo build && ./target/debug/rtf accounts create --name 'Test' --type checking --currency USD --owner 'Sky' && ./target/debug/rtf accounts list --format json

## Files Likely Touched

- Cargo.toml
- src/main.rs
- src/lib.rs
- src/domain/mod.rs
- src/domain/account/mod.rs
- src/domain/transaction/mod.rs
- src/domain/category/mod.rs
- src/domain/currency/mod.rs
- src/domain/household/mod.rs
- src/application/mod.rs
- src/infrastructure/mod.rs
- src/infrastructure/storage/mod.rs
- src/infrastructure/importer/mod.rs
- src/infrastructure/exchange/mod.rs
- src/infrastructure/sync_adapter/mod.rs
- src/cli/mod.rs
- src/domain/currency/money.rs
- src/domain/currency/currency_code.rs
- src/domain/error.rs
- src/domain/account/repository.rs
- src/domain/account/account.rs
- src/domain/account/account_type.rs
- src/domain/household/person.rs
- src/domain/transaction/transaction.rs
- src/domain/transaction/status.rs
- src/domain/category/category.rs
- src/domain/category/category_group.rs
- src/domain/transaction/repository.rs
- src/domain/category/repository.rs
- src/infrastructure/storage/database.rs
- src/infrastructure/storage/migrations.rs
- src/infrastructure/storage/account_repo.rs
- migrations/001_initial.sql
- src/infrastructure/storage/transaction_repo.rs
- src/infrastructure/storage/category_repo.rs
- src/infrastructure/storage/person_repo.rs
- src/application/account_service.rs
- src/cli/accounts.rs
