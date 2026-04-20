---
estimated_steps: 4
estimated_files: 3
skills_used: []
---

# T06: Implement Transaction, Category, and Person SQLite repositories with TDD

Build SqliteTransactionRepository, SqliteCategoryRepository, and SqlitePersonRepository — each implementing their respective domain traits. TDD with in-memory SQLite.

Transaction repo tests: create transaction with Money amount → retrieve by ID → list by account → list by date range → verify all fields including status and optional beneficiary_id.
Category repo tests: create group → create category in group → list categories → list by group.
Person repo tests: create person → retrieve → list all.

## Inputs

- `src/infrastructure/storage/database.rs`
- `src/infrastructure/storage/migrations.rs`
- `src/domain/transaction/repository.rs`
- `src/domain/category/repository.rs`

## Expected Output

- `src/infrastructure/storage/transaction_repo.rs`
- `src/infrastructure/storage/category_repo.rs`
- `src/infrastructure/storage/person_repo.rs`

## Verification

cargo test -- infrastructure::storage
