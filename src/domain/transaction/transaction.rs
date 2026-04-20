use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::TransactionStatus;
use crate::domain::currency::Money;
use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub date: NaiveDate,
    pub amount: Money,
    pub payee: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub beneficiary_id: Option<String>,
    pub transfer_pair_id: Option<String>,
    pub status: TransactionStatus,
    pub external_id: Option<String>,
    pub imported_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Transaction {
    pub fn new(
        id: String,
        account_id: String,
        date: NaiveDate,
        amount: Money,
    ) -> Result<Self, DomainError> {
        if account_id.trim().is_empty() {
            return Err(DomainError::Validation(
                "transaction account_id is required".into(),
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id,
            account_id,
            date,
            amount,
            payee: None,
            description: None,
            category_id: None,
            beneficiary_id: None,
            transfer_pair_id: None,
            status: TransactionStatus::Pending,
            external_id: None,
            imported_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn is_transfer(&self) -> bool {
        self.transfer_pair_id.is_some()
    }

    pub fn is_categorized(&self) -> bool {
        self.category_id.is_some()
    }

    pub fn is_income(&self) -> bool {
        !self.amount.is_negative() && !self.amount.is_zero()
    }

    pub fn is_expense(&self) -> bool {
        self.amount.is_negative()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::currency::CurrencyCode;
    use rust_decimal_macros::dec;

    fn make_transaction() -> Transaction {
        Transaction::new(
            "txn-001".into(),
            "acc-001".into(),
            NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
            Money::new(dec!(-45.99), CurrencyCode::USD),
        )
        .unwrap()
    }

    #[test]
    fn new_creates_transaction() {
        let txn = make_transaction();
        assert_eq!(txn.id, "txn-001");
        assert_eq!(txn.account_id, "acc-001");
        assert_eq!(txn.date, NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());
        assert_eq!(txn.amount.amount, dec!(-45.99));
        assert_eq!(txn.amount.currency, CurrencyCode::USD);
        assert_eq!(txn.status, TransactionStatus::Pending);
    }

    #[test]
    fn new_defaults() {
        let txn = make_transaction();
        assert!(txn.payee.is_none());
        assert!(txn.description.is_none());
        assert!(txn.category_id.is_none());
        assert!(txn.beneficiary_id.is_none());
        assert!(txn.transfer_pair_id.is_none());
        assert!(txn.external_id.is_none());
        assert!(txn.imported_at.is_none());
    }

    #[test]
    fn new_rejects_empty_account_id() {
        let result = Transaction::new(
            "txn-001".into(),
            "".into(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            Money::new(dec!(10.00), CurrencyCode::USD),
        );
        assert!(result.is_err());
    }

    #[test]
    fn is_expense() {
        let txn = make_transaction();
        assert!(txn.is_expense());
        assert!(!txn.is_income());
    }

    #[test]
    fn is_income() {
        let txn = Transaction::new(
            "txn-002".into(),
            "acc-001".into(),
            NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            Money::new(dec!(3500.00), CurrencyCode::USD),
        )
        .unwrap();
        assert!(txn.is_income());
        assert!(!txn.is_expense());
    }

    #[test]
    fn zero_amount_is_neither_income_nor_expense() {
        let txn = Transaction::new(
            "txn-003".into(),
            "acc-001".into(),
            NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            Money::zero(CurrencyCode::USD),
        )
        .unwrap();
        assert!(!txn.is_income());
        assert!(!txn.is_expense());
    }

    #[test]
    fn is_transfer() {
        let mut txn = make_transaction();
        assert!(!txn.is_transfer());
        txn.transfer_pair_id = Some("txn-pair-001".into());
        assert!(txn.is_transfer());
    }

    #[test]
    fn is_categorized() {
        let mut txn = make_transaction();
        assert!(!txn.is_categorized());
        txn.category_id = Some("cat-food".into());
        assert!(txn.is_categorized());
    }

    #[test]
    fn serde_roundtrip() {
        let mut txn = make_transaction();
        txn.payee = Some("Costco".into());
        txn.category_id = Some("cat-groceries".into());
        txn.beneficiary_id = Some("p-001".into());

        let json = serde_json::to_string(&txn).unwrap();
        let deserialized: Transaction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, txn.id);
        assert_eq!(deserialized.payee, txn.payee);
        assert_eq!(deserialized.amount, txn.amount);
        assert_eq!(deserialized.date, txn.date);
        assert_eq!(deserialized.status, txn.status);
    }

    #[test]
    fn brl_transaction() {
        let txn = Transaction::new(
            "txn-brl".into(),
            "acc-nubank".into(),
            NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(),
            Money::new(dec!(-150.00), CurrencyCode::BRL),
        )
        .unwrap();
        assert_eq!(txn.amount.currency, CurrencyCode::BRL);
        assert!(txn.is_expense());
    }
}
