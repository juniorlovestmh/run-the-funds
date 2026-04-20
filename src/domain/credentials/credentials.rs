//! Stored per-provider credentials for bank-sync adapters (S04).
//!
//! `data` is an opaque JSON string whose shape is provider-specific:
//! - SimpleFIN: `{"access_url": "https://user:pass@host/path"}`
//! - Pluggy:    `{"client_id": "...", "client_secret": "...", "item_id": "..."}`
//!
//! The domain layer doesn't parse `data` — each adapter defines its own parser.
//! Keeping the shape opaque here lets new providers plug in without touching
//! the domain or repository code.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCredentials {
    pub id: String,
    pub provider: String,
    pub data: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ProviderCredentials {
    pub fn new(id: String, provider: String, data: String) -> Result<Self, DomainError> {
        if provider.trim().is_empty() {
            return Err(DomainError::Validation("provider is required".into()));
        }
        if data.trim().is_empty() {
            return Err(DomainError::Validation("credentials data is required".into()));
        }
        let now = Utc::now();
        Ok(Self {
            id,
            provider,
            data,
            created_at: now,
            updated_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_stamped_record() {
        let before = Utc::now();
        let c = ProviderCredentials::new(
            "c1".into(),
            "simplefin".into(),
            r#"{"access_url":"https://u:p@host/p"}"#.into(),
        )
        .unwrap();
        let after = Utc::now();
        assert_eq!(c.provider, "simplefin");
        assert!(c.created_at >= before && c.created_at <= after);
        assert_eq!(c.created_at, c.updated_at);
    }

    #[test]
    fn new_rejects_empty_provider() {
        let err =
            ProviderCredentials::new("c1".into(), "".into(), "{}".into()).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn new_rejects_empty_data() {
        let err =
            ProviderCredentials::new("c1".into(), "simplefin".into(), "".into()).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn serde_roundtrip() {
        let c = ProviderCredentials::new(
            "c1".into(),
            "pluggy".into(),
            r#"{"client_id":"x","client_secret":"y","item_id":"z"}"#.into(),
        )
        .unwrap();
        let json = serde_json::to_string(&c).unwrap();
        let decoded: ProviderCredentials = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, c);
    }
}
