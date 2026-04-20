//! Per-bank connection record for bank-sync providers (S04C).
//!
//! One row per linked bank. Lives alongside `ProviderCredentials`, which
//! holds per-app (not per-bank) data like Teller's cert/key/app_id or
//! Pluggy's client_id/client_secret. For Teller the `external_id` is the
//! enrollment id and `data` carries `{"access_token":"..."}`. For Pluggy
//! the `external_id` is the `itemId` and `data` is typically `{}`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderConnection {
    pub id: String,
    pub provider: String,
    pub external_id: String,
    /// Provider-specific JSON blob. Adapters parse their own shape.
    pub data: String,
    pub institution_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProviderConnection {
    pub fn new(
        id: String,
        provider: String,
        external_id: String,
        data: String,
        institution_name: Option<String>,
    ) -> Result<Self, DomainError> {
        if provider.trim().is_empty() {
            return Err(DomainError::Validation("provider is required".into()));
        }
        if external_id.trim().is_empty() {
            return Err(DomainError::Validation("external_id is required".into()));
        }
        let now = Utc::now();
        Ok(Self {
            id,
            provider,
            external_id,
            data,
            institution_name,
            created_at: now,
            updated_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stamps_timestamps() {
        let before = Utc::now();
        let c = ProviderConnection::new(
            "c1".into(),
            "teller".into(),
            "enr_xxx".into(),
            r#"{"access_token":"t"}"#.into(),
            Some("Chase".into()),
        )
        .unwrap();
        let after = Utc::now();
        assert!(c.created_at >= before && c.created_at <= after);
        assert_eq!(c.created_at, c.updated_at);
    }

    #[test]
    fn new_rejects_empty_provider_and_external_id() {
        assert!(ProviderConnection::new(
            "c1".into(),
            "".into(),
            "x".into(),
            "{}".into(),
            None
        )
        .is_err());
        assert!(ProviderConnection::new(
            "c1".into(),
            "teller".into(),
            "".into(),
            "{}".into(),
            None
        )
        .is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let c = ProviderConnection::new(
            "c1".into(),
            "pluggy".into(),
            "item-123".into(),
            "{}".into(),
            Some("Nubank".into()),
        )
        .unwrap();
        let json = serde_json::to_string(&c).unwrap();
        let decoded: ProviderConnection = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, c);
    }
}
