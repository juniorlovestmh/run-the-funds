---
estimated_steps: 2
estimated_files: 5
skills_used: []
---

# T02: Implement Money value object, CurrencyCode, and domain error types with TDD

Build the foundational value objects: Money (wraps rust_decimal::Decimal + CurrencyCode), CurrencyCode enum (USD, BRL — extensible via country adapters), and domain error types (DomainError enum). Money must support arithmetic (add/subtract same currency, reject mixed-currency arithmetic), comparison, Display, and serde serialization. All TDD — write failing tests first, then implement.

Also define the core repository trait (AccountRepository) as an interface the storage layer will implement.

## Inputs

- `src/domain/mod.rs`

## Expected Output

- `src/domain/currency/money.rs`
- `src/domain/currency/currency_code.rs`
- `src/domain/error.rs`
- `src/domain/account/repository.rs`

## Verification

cargo test -- domain::currency && cargo test -- domain::error
