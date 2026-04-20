use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub order_index: Option<i64>,
    pub external_id: Option<String>,
    pub external_provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Tag {
    pub fn new(id: String, name: String) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::Validation("tag name is required".into()));
        }
        let now = Utc::now();
        Ok(Self {
            id,
            name,
            color: None,
            order_index: None,
            external_id: None,
            external_provider: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn from_external(
        id: String,
        name: String,
        external_provider: String,
        external_id: String,
        color: Option<String>,
        order_index: Option<i64>,
    ) -> Result<Self, DomainError> {
        let mut t = Self::new(id, name)?;
        t.external_id = Some(external_id);
        t.external_provider = Some(external_provider);
        t.color = color;
        t.order_index = order_index;
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_tag() {
        let t = Tag::new("tag-1".into(), "BR".into()).unwrap();
        assert_eq!(t.name, "BR");
        assert!(t.color.is_none());
        assert!(t.external_id.is_none());
    }

    #[test]
    fn new_rejects_empty_name() {
        assert!(Tag::new("tag-1".into(), "  ".into()).is_err());
    }

    #[test]
    fn from_external_captures_provenance() {
        let t = Tag::from_external(
            "tag-1".into(),
            "BR".into(),
            "monarch".into(),
            "monarch-tag-123".into(),
            Some("#FFAA00".into()),
            Some(7),
        )
        .unwrap();
        assert_eq!(t.external_provider.as_deref(), Some("monarch"));
        assert_eq!(t.external_id.as_deref(), Some("monarch-tag-123"));
        assert_eq!(t.color.as_deref(), Some("#FFAA00"));
        assert_eq!(t.order_index, Some(7));
    }

    #[test]
    fn serde_roundtrip() {
        let t = Tag::new("tag-1".into(), "Subscription".into()).unwrap();
        let json = serde_json::to_string(&t).unwrap();
        let back: Tag = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "Subscription");
    }
}
