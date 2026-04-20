---
id: T02
parent: S01
milestone: M001
key_files:
  - src/domain/currency/money.rs
  - src/domain/currency/currency_code.rs
  - src/domain/error.rs
key_decisions:
  - Added rust_decimal_macros as dev-dependency for dec!() macro in tests
  - Added negate() and abs() methods to Money — needed for transaction sign handling
duration: 
verification_result: passed
completed_at: 2026-04-18T17:37:01.913Z
blocker_discovered: false
---

# T02: Money value object, CurrencyCode, and DomainError fully tested — 37 tests passing

**Money value object, CurrencyCode, and DomainError fully tested — 37 tests passing**

## What Happened

Added comprehensive TDD test suites for Money (21 tests), CurrencyCode (10 tests), and DomainError (6 tests). Tests cover: arithmetic with same/mixed currencies, precise decimal math (0.1+0.2=0.3), large amounts, negation, abs, zero detection, negative detection, Display formatting, serde JSON roundtrip/structure, FromStr parsing (upper/lower/mixed case + error cases), HashMap key usage, and error message formatting. Added negate() and abs() methods to Money. Confirmed rust_decimal prevents floating-point drift. All error types verified as Send+Sync.

## Verification

cargo test -- domain::currency domain::error: 37 passed, 0 failed

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test -- domain::currency domain::error` | 0 | pass | 2620ms |

## Deviations

None

## Known Issues

None.

## Files Created/Modified

- `src/domain/currency/money.rs`
- `src/domain/currency/currency_code.rs`
- `src/domain/error.rs`
