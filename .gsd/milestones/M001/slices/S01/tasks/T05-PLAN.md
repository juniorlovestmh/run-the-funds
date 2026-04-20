---
estimated_steps: 3
estimated_files: 5
skills_used: []
---

# T05: Implement SQLite migrations and Account repository with TDD

Build the SQLite storage layer: connection management (Database struct wrapping rusqlite::Connection), migration system (embedded SQL files run on first connect), and SqliteAccountRepository implementing the AccountRepository trait.

Migration creates tables for: accounts, persons, categories, category_groups, transactions, exchange_rates. All tables use TEXT for IDs (UUIDs), TEXT for amounts (rust_decimal serialized), and appropriate indexes.

TDD: write integration tests that create an in-memory SQLite DB, run migrations, then test CRUD operations on accounts. Test round-trip: create account → retrieve by ID → list all → verify fields match including Money amounts.

## Inputs

- `src/domain/account/account.rs`
- `src/domain/account/repository.rs`
- `src/domain/currency/money.rs`
- `src/domain/household/person.rs`

## Expected Output

- `src/infrastructure/storage/database.rs`
- `src/infrastructure/storage/migrations.rs`
- `src/infrastructure/storage/account_repo.rs`
- `migrations/001_initial.sql`

## Verification

cargo test -- infrastructure::storage
