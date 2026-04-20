use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::AccountType;
use crate::domain::currency::{CurrencyCode, Money};
use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency: CurrencyCode,
    pub owner: String,
    pub institution: Option<String>,
    pub account_number_last4: Option<String>,
    pub balance: Money,
    pub credit_limit: Option<Money>,
    pub interest_rate: Option<rust_decimal::Decimal>,
    pub notes: Option<String>,
    /// Bank-sync provider this account is linked to (e.g. "simplefin", "pluggy").
    /// `None` means the account is unlinked and must be populated via manual import.
    pub external_provider: Option<String>,
    /// Identifier the provider uses for this account on its side.
    pub external_account_id: Option<String>,
    /// Timestamp of the last successful sync from the provider, for incremental pulls.
    pub last_sync_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Account {
    pub fn new(
        id: String,
        name: String,
        account_type: AccountType,
        currency: CurrencyCode,
        owner: String,
    ) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::Validation("account name is required".into()));
        }
        if owner.trim().is_empty() {
            return Err(DomainError::Validation("account owner is required".into()));
        }

        let now = Utc::now();
        Ok(Self {
            id,
            name,
            account_type,
            currency,
            owner,
            institution: None,
            account_number_last4: None,
            balance: Money::zero(currency),
            credit_limit: None,
            interest_rate: None,
            notes: None,
            external_provider: None,
            external_account_id: None,
            last_sync_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn is_liability(&self) -> bool {
        matches!(
            self.account_type,
            AccountType::CreditCard | AccountType::Loan
        )
    }

    pub fn is_asset(&self) -> bool {
        !self.is_liability()
    }

    /// True when the account is linked to a bank-sync provider.
    pub fn is_linked(&self) -> bool {
        self.external_provider.is_some() && self.external_account_id.is_some()
    }

    /// Link this account to a remote provider. Both fields must be non-empty.
    pub fn link(&mut self, provider: String, external_account_id: String) -> Result<(), DomainError> {
        if provider.trim().is_empty() {
            return Err(DomainError::Validation("provider is required".into()));
        }
        if external_account_id.trim().is_empty() {
            return Err(DomainError::Validation(
                "external_account_id is required".into(),
            ));
        }
        self.external_provider = Some(provider);
        self.external_account_id = Some(external_account_id);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Record a successful sync. Sets `last_sync_at` to the given timestamp
    /// and bumps `updated_at`.
    pub fn mark_synced(&mut self, at: DateTime<Utc>) {
        self.last_sync_at = Some(at);
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_account() -> Account {
        Account::new(
            "acc-001".into(),
            "Nubank Checking".into(),
            AccountType::Checking,
            CurrencyCode::BRL,
            "Sky".into(),
        )
        .unwrap()
    }

    #[test]
    fn new_creates_account_with_zero_balance() {
        let acc = make_account();
        assert_eq!(acc.id, "acc-001");
        assert_eq!(acc.name, "Nubank Checking");
        assert_eq!(acc.account_type, AccountType::Checking);
        assert_eq!(acc.currency, CurrencyCode::BRL);
        assert_eq!(acc.owner, "Sky");
        assert!(acc.balance.is_zero());
        assert_eq!(acc.balance.currency, CurrencyCode::BRL);
    }

    #[test]
    fn new_sets_timestamps() {
        let acc = make_account();
        assert!(acc.created_at <= Utc::now());
        assert_eq!(acc.created_at, acc.updated_at);
    }

    #[test]
    fn new_optional_fields_are_none() {
        let acc = make_account();
        assert!(acc.institution.is_none());
        assert!(acc.account_number_last4.is_none());
        assert!(acc.credit_limit.is_none());
        assert!(acc.interest_rate.is_none());
        assert!(acc.notes.is_none());
    }

    #[test]
    fn new_rejects_empty_name() {
        let result = Account::new(
            "id".into(),
            "".into(),
            AccountType::Checking,
            CurrencyCode::USD,
            "Owner".into(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name is required"));
    }

    #[test]
    fn new_rejects_whitespace_only_name() {
        let result = Account::new(
            "id".into(),
            "   ".into(),
            AccountType::Checking,
            CurrencyCode::USD,
            "Owner".into(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn new_rejects_empty_owner() {
        let result = Account::new(
            "id".into(),
            "My Account".into(),
            AccountType::Checking,
            CurrencyCode::USD,
            "".into(),
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("owner is required")
        );
    }

    #[test]
    fn checking_is_asset() {
        let acc = make_account();
        assert!(acc.is_asset());
        assert!(!acc.is_liability());
    }

    #[test]
    fn credit_card_is_liability() {
        let acc = Account::new(
            "cc-001".into(),
            "Discover".into(),
            AccountType::CreditCard,
            CurrencyCode::USD,
            "Sky".into(),
        )
        .unwrap();
        assert!(acc.is_liability());
        assert!(!acc.is_asset());
    }

    #[test]
    fn loan_is_liability() {
        let acc = Account::new(
            "loan-001".into(),
            "Aidvantage".into(),
            AccountType::Loan,
            CurrencyCode::USD,
            "Sky".into(),
        )
        .unwrap();
        assert!(acc.is_liability());
    }

    #[test]
    fn savings_is_asset() {
        let acc = Account::new(
            "sav-001".into(),
            "Chase Savings".into(),
            AccountType::Savings,
            CurrencyCode::USD,
            "Sky".into(),
        )
        .unwrap();
        assert!(acc.is_asset());
    }

    #[test]
    fn credit_card_with_limit() {
        let mut acc = Account::new(
            "cc-001".into(),
            "Capital One".into(),
            AccountType::CreditCard,
            CurrencyCode::USD,
            "Sky".into(),
        )
        .unwrap();
        acc.credit_limit = Some(Money::new(dec!(5000.00), CurrencyCode::USD));
        assert_eq!(acc.credit_limit.unwrap().amount, dec!(5000.00));
    }

    #[test]
    fn loan_with_interest_rate() {
        let mut acc = Account::new(
            "loan-001".into(),
            "Student Loan".into(),
            AccountType::Loan,
            CurrencyCode::USD,
            "Sky".into(),
        )
        .unwrap();
        acc.interest_rate = Some(dec!(5.5));
        assert_eq!(acc.interest_rate.unwrap(), dec!(5.5));
    }

    #[test]
    fn serde_roundtrip() {
        let acc = make_account();
        let json = serde_json::to_string(&acc).unwrap();
        let deserialized: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, acc.id);
        assert_eq!(deserialized.name, acc.name);
        assert_eq!(deserialized.account_type, acc.account_type);
        assert_eq!(deserialized.currency, acc.currency);
        assert_eq!(deserialized.owner, acc.owner);
        assert_eq!(deserialized.balance, acc.balance);
    }

    #[test]
    fn serde_json_includes_all_fields() {
        let mut acc = make_account();
        acc.institution = Some("Nubank".into());
        acc.notes = Some("Primary checking".into());
        let json: serde_json::Value = serde_json::to_value(&acc).unwrap();
        assert_eq!(json["institution"], "Nubank");
        assert_eq!(json["notes"], "Primary checking");
        assert_eq!(json["account_type"], "checking");
    }

    #[test]
    fn new_is_unlinked() {
        let acc = make_account();
        assert!(!acc.is_linked());
        assert!(acc.external_provider.is_none());
        assert!(acc.external_account_id.is_none());
        assert!(acc.last_sync_at.is_none());
    }

    #[test]
    fn link_sets_both_fields_and_bumps_updated_at() {
        let mut acc = make_account();
        let before = acc.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        acc.link("simplefin".into(), "ext-123".into()).unwrap();
        assert!(acc.is_linked());
        assert_eq!(acc.external_provider.as_deref(), Some("simplefin"));
        assert_eq!(acc.external_account_id.as_deref(), Some("ext-123"));
        assert!(acc.updated_at > before);
    }

    #[test]
    fn link_rejects_empty_provider() {
        let mut acc = make_account();
        let err = acc.link("".into(), "ext-123".into()).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn link_rejects_empty_external_id() {
        let mut acc = make_account();
        let err = acc.link("simplefin".into(), "".into()).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn mark_synced_updates_last_sync_at() {
        let mut acc = make_account();
        let ts = Utc::now();
        acc.mark_synced(ts);
        assert_eq!(acc.last_sync_at, Some(ts));
    }

    #[test]
    fn link_roundtrips_through_serde() {
        let mut acc = make_account();
        acc.link("pluggy".into(), "pluggy-item-xyz".into()).unwrap();
        let json = serde_json::to_string(&acc).unwrap();
        let decoded: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.external_provider.as_deref(), Some("pluggy"));
        assert_eq!(decoded.external_account_id.as_deref(), Some("pluggy-item-xyz"));
    }
}
