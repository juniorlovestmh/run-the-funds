---
estimated_steps: 2
estimated_files: 8
skills_used: []
---

# T04: Implement Transaction, Category, and CategoryGroup entities with TDD

Build Transaction entity (id, account_id, date, amount as Money, payee, description, category_id, beneficiary_id, transfer_pair_id, status, external_id, imported_at, created_at). TransactionStatus enum (Pending, Cleared, Reconciled). Category entity (id, group_id, name). CategoryGroup entity (id, name). All with serde support and TDD.

Define TransactionRepository and CategoryRepository traits.

## Inputs

- `src/domain/currency/money.rs`
- `src/domain/account/account.rs`
- `src/domain/household/person.rs`

## Expected Output

- `src/domain/transaction/transaction.rs`
- `src/domain/transaction/status.rs`
- `src/domain/category/category.rs`
- `src/domain/category/category_group.rs`
- `src/domain/transaction/repository.rs`
- `src/domain/category/repository.rs`

## Verification

cargo test -- domain::transaction && cargo test -- domain::category
