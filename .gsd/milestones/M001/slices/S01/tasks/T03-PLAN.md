---
estimated_steps: 2
estimated_files: 5
skills_used: []
---

# T03: Implement Account and Person entities with TDD

Build Account entity (id, name, account_type, currency, owner, institution, account_number_last4, opened_date, notes, created_at, updated_at) and AccountType enum (Checking, Savings, CreditCard, Loan). Build Person entity (id, name, relationship — e.g. Self, Spouse, Child). All fields use strong types. TDD — write tests for entity creation, validation (required fields, valid types), and serde round-trips.

Account uses Money for balance. AccountType determines behavior (credit cards have credit limits, loans have interest rates — fields optional based on type).

## Inputs

- `src/domain/currency/money.rs`
- `src/domain/currency/currency_code.rs`
- `src/domain/error.rs`

## Expected Output

- `src/domain/account/account.rs`
- `src/domain/account/account_type.rs`
- `src/domain/household/person.rs`

## Verification

cargo test -- domain::account && cargo test -- domain::household
