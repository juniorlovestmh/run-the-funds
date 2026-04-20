//! Transaction splits (S05 T03).
//!
//! One transaction → N category allocations. Each split row carries its own
//! amount + category + optional notes. The parent transaction's
//! `category_id` is cleared when splits exist (the splits carry the
//! categorization). Splits must sum to the parent's absolute amount.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::currency::Money;
use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionSplit {
    pub id: String,
    pub transaction_id: String,
    pub category_id: String,
    pub amount: Money,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl TransactionSplit {
    pub fn new(
        id: String,
        transaction_id: String,
        category_id: String,
        amount: Money,
        notes: Option<String>,
    ) -> Result<Self, DomainError> {
        if transaction_id.trim().is_empty() {
            return Err(DomainError::Validation("split transaction_id is required".into()));
        }
        if category_id.trim().is_empty() {
            return Err(DomainError::Validation("split category_id is required".into()));
        }
        if amount.is_zero() {
            return Err(DomainError::Validation("split amount must be non-zero".into()));
        }
        Ok(Self {
            id,
            transaction_id,
            category_id,
            amount,
            notes,
            created_at: Utc::now(),
        })
    }
}

pub trait TransactionSplitRepository {
    fn save(&self, split: &TransactionSplit) -> Result<(), DomainError>;
    fn find_by_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<Vec<TransactionSplit>, DomainError>;
    fn delete_by_transaction(&self, transaction_id: &str) -> Result<usize, DomainError>;
    fn delete(&self, id: &str) -> Result<(), DomainError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::currency::CurrencyCode;
    use rust_decimal_macros::dec;

    #[test]
    fn new_validates_non_empty_fields() {
        let ok = TransactionSplit::new(
            "s1".into(),
            "txn-1".into(),
            "cat-1".into(),
            Money::new(dec!(-50), CurrencyCode::USD),
            None,
        );
        assert!(ok.is_ok());

        assert!(TransactionSplit::new(
            "s1".into(), "".into(), "cat-1".into(),
            Money::new(dec!(-50), CurrencyCode::USD), None
        ).is_err());
        assert!(TransactionSplit::new(
            "s1".into(), "txn-1".into(), "".into(),
            Money::new(dec!(-50), CurrencyCode::USD), None
        ).is_err());
    }

    #[test]
    fn new_rejects_zero_amount() {
        assert!(TransactionSplit::new(
            "s1".into(), "txn-1".into(), "cat-1".into(),
            Money::new(dec!(0), CurrencyCode::USD), None
        ).is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let split = TransactionSplit::new(
            "s1".into(), "txn-1".into(), "cat-1".into(),
            Money::new(dec!(-99.99), CurrencyCode::USD),
            Some("groceries".into()),
        ).unwrap();
        let json = serde_json::to_string(&split).unwrap();
        let decoded: TransactionSplit = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, split);
    }
}
