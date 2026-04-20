use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub group_id: String,
    pub name: String,
    pub external_id: Option<String>,
    pub external_provider: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Category {
    pub fn new(id: String, group_id: String, name: String) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::Validation("category name is required".into()));
        }
        if group_id.trim().is_empty() {
            return Err(DomainError::Validation(
                "category group_id is required".into(),
            ));
        }
        Ok(Self {
            id,
            group_id,
            name,
            external_id: None,
            external_provider: None,
            created_at: Utc::now(),
        })
    }

    pub fn from_external(
        id: String,
        group_id: String,
        name: String,
        external_provider: String,
        external_id: String,
    ) -> Result<Self, DomainError> {
        let mut c = Self::new(id, group_id, name)?;
        c.external_id = Some(external_id);
        c.external_provider = Some(external_provider);
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_category() {
        let cat = Category::new("cat-001".into(), "grp-food".into(), "Groceries".into()).unwrap();
        assert_eq!(cat.id, "cat-001");
        assert_eq!(cat.group_id, "grp-food");
        assert_eq!(cat.name, "Groceries");
        assert!(cat.external_id.is_none());
    }

    #[test]
    fn new_rejects_empty_name() {
        assert!(Category::new("cat-001".into(), "grp-food".into(), "".into()).is_err());
    }

    #[test]
    fn new_rejects_empty_group_id() {
        assert!(Category::new("cat-001".into(), "".into(), "Groceries".into()).is_err());
    }

    #[test]
    fn from_external_captures_provenance() {
        let c = Category::from_external(
            "cat-001".into(),
            "grp-food".into(),
            "Groceries".into(),
            "monarch".into(),
            "monarch-c-123".into(),
        )
        .unwrap();
        assert_eq!(c.external_provider.as_deref(), Some("monarch"));
        assert_eq!(c.external_id.as_deref(), Some("monarch-c-123"));
    }

    #[test]
    fn serde_roundtrip() {
        let cat = Category::new("cat-001".into(), "grp-food".into(), "Groceries".into()).unwrap();
        let json = serde_json::to_string(&cat).unwrap();
        let deserialized: Category = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, cat.id);
        assert_eq!(deserialized.name, cat.name);
        assert_eq!(deserialized.group_id, cat.group_id);
    }
}
