use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::CurrencyCode;
use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub amount: Decimal,
    pub currency: CurrencyCode,
}

impl Money {
    pub fn new(amount: Decimal, currency: CurrencyCode) -> Self {
        Self { amount, currency }
    }

    pub fn zero(currency: CurrencyCode) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    pub fn add(&self, other: &Money) -> Result<Money, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch {
                expected: self.currency.to_string(),
                got: other.currency.to_string(),
            });
        }
        Ok(Money::new(self.amount + other.amount, self.currency))
    }

    pub fn subtract(&self, other: &Money) -> Result<Money, DomainError> {
        if self.currency != other.currency {
            return Err(DomainError::CurrencyMismatch {
                expected: self.currency.to_string(),
                got: other.currency.to_string(),
            });
        }
        Ok(Money::new(self.amount - other.amount, self.currency))
    }

    pub fn is_negative(&self) -> bool {
        self.amount < Decimal::ZERO
    }

    pub fn is_zero(&self) -> bool {
        self.amount == Decimal::ZERO
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

impl Money {
    pub fn negate(&self) -> Money {
        Money::new(-self.amount, self.currency)
    }

    pub fn abs(&self) -> Money {
        Money::new(self.amount.abs(), self.currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn new_creates_money_with_correct_fields() {
        let m = Money::new(dec!(100.50), CurrencyCode::USD);
        assert_eq!(m.amount, dec!(100.50));
        assert_eq!(m.currency, CurrencyCode::USD);
    }

    #[test]
    fn zero_creates_zero_amount() {
        let m = Money::zero(CurrencyCode::BRL);
        assert_eq!(m.amount, Decimal::ZERO);
        assert_eq!(m.currency, CurrencyCode::BRL);
        assert!(m.is_zero());
    }

    #[test]
    fn add_same_currency() {
        let a = Money::new(dec!(100.00), CurrencyCode::USD);
        let b = Money::new(dec!(50.25), CurrencyCode::USD);
        let result = a.add(&b).unwrap();
        assert_eq!(result.amount, dec!(150.25));
        assert_eq!(result.currency, CurrencyCode::USD);
    }

    #[test]
    fn add_mixed_currency_fails() {
        let usd = Money::new(dec!(100.00), CurrencyCode::USD);
        let brl = Money::new(dec!(50.00), CurrencyCode::BRL);
        let err = usd.add(&brl).unwrap_err();
        assert!(matches!(err, DomainError::CurrencyMismatch { .. }));
    }

    #[test]
    fn subtract_same_currency() {
        let a = Money::new(dec!(100.00), CurrencyCode::BRL);
        let b = Money::new(dec!(30.50), CurrencyCode::BRL);
        let result = a.subtract(&b).unwrap();
        assert_eq!(result.amount, dec!(69.50));
    }

    #[test]
    fn subtract_mixed_currency_fails() {
        let usd = Money::new(dec!(100.00), CurrencyCode::USD);
        let brl = Money::new(dec!(50.00), CurrencyCode::BRL);
        assert!(usd.subtract(&brl).is_err());
    }

    #[test]
    fn subtract_resulting_in_negative() {
        let a = Money::new(dec!(10.00), CurrencyCode::USD);
        let b = Money::new(dec!(25.00), CurrencyCode::USD);
        let result = a.subtract(&b).unwrap();
        assert!(result.is_negative());
        assert_eq!(result.amount, dec!(-15.00));
    }

    #[test]
    fn is_negative() {
        assert!(Money::new(dec!(-1.00), CurrencyCode::USD).is_negative());
        assert!(!Money::new(dec!(0.00), CurrencyCode::USD).is_negative());
        assert!(!Money::new(dec!(1.00), CurrencyCode::USD).is_negative());
    }

    #[test]
    fn is_zero() {
        assert!(Money::new(dec!(0.00), CurrencyCode::USD).is_zero());
        assert!(Money::new(dec!(0), CurrencyCode::USD).is_zero());
        assert!(!Money::new(dec!(0.01), CurrencyCode::USD).is_zero());
    }

    #[test]
    fn negate() {
        let m = Money::new(dec!(42.00), CurrencyCode::BRL);
        let neg = m.negate();
        assert_eq!(neg.amount, dec!(-42.00));
        assert_eq!(neg.currency, CurrencyCode::BRL);

        let double_neg = neg.negate();
        assert_eq!(double_neg, m);
    }

    #[test]
    fn abs_of_negative() {
        let m = Money::new(dec!(-99.99), CurrencyCode::USD);
        let a = m.abs();
        assert_eq!(a.amount, dec!(99.99));
    }

    #[test]
    fn abs_of_positive() {
        let m = Money::new(dec!(50.00), CurrencyCode::USD);
        assert_eq!(m.abs().amount, dec!(50.00));
    }

    #[test]
    fn display_format() {
        let m = Money::new(dec!(1234.56), CurrencyCode::USD);
        assert_eq!(m.to_string(), "1234.56 USD");
    }

    #[test]
    fn display_format_brl() {
        let m = Money::new(dec!(999.00), CurrencyCode::BRL);
        assert_eq!(m.to_string(), "999.00 BRL");
    }

    #[test]
    fn equality() {
        let a = Money::new(dec!(10.00), CurrencyCode::USD);
        let b = Money::new(dec!(10.00), CurrencyCode::USD);
        assert_eq!(a, b);
    }

    #[test]
    fn inequality_different_amount() {
        let a = Money::new(dec!(10.00), CurrencyCode::USD);
        let b = Money::new(dec!(10.01), CurrencyCode::USD);
        assert_ne!(a, b);
    }

    #[test]
    fn inequality_different_currency() {
        let a = Money::new(dec!(10.00), CurrencyCode::USD);
        let b = Money::new(dec!(10.00), CurrencyCode::BRL);
        assert_ne!(a, b);
    }

    #[test]
    fn serde_roundtrip() {
        let m = Money::new(dec!(1234.56), CurrencyCode::BRL);
        let json = serde_json::to_string(&m).unwrap();
        let deserialized: Money = serde_json::from_str(&json).unwrap();
        assert_eq!(m, deserialized);
    }

    #[test]
    fn serde_json_structure() {
        let m = Money::new(dec!(100.50), CurrencyCode::USD);
        let json: serde_json::Value = serde_json::to_value(&m).unwrap();
        assert_eq!(json["amount"], "100.50");
        assert_eq!(json["currency"], "USD");
    }

    #[test]
    fn precise_decimal_arithmetic() {
        // Verify no floating-point drift: 0.1 + 0.2 == 0.3
        let a = Money::new(dec!(0.1), CurrencyCode::USD);
        let b = Money::new(dec!(0.2), CurrencyCode::USD);
        let result = a.add(&b).unwrap();
        assert_eq!(result.amount, dec!(0.3));
    }

    #[test]
    fn large_amounts() {
        let a = Money::new(dec!(999_999_999.99), CurrencyCode::USD);
        let b = Money::new(dec!(0.01), CurrencyCode::USD);
        let result = a.add(&b).unwrap();
        assert_eq!(result.amount, dec!(1_000_000_000.00));
    }
}
