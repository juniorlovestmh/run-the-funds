use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryGroup {
    pub id: String,
    pub name: String,
    pub external_id: Option<String>,
    pub external_provider: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl CategoryGroup {
    pub fn new(id: String, name: String) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::Validation(
                "category group name is required".into(),
            ));
        }
        Ok(Self {
            id,
            name,
            external_id: None,
            external_provider: None,
            created_at: Utc::now(),
        })
    }

    pub fn from_external(
        id: String,
        name: String,
        external_provider: String,
        external_id: String,
    ) -> Result<Self, DomainError> {
        let mut g = Self::new(id, name)?;
        g.external_id = Some(external_id);
        g.external_provider = Some(external_provider);
        Ok(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_group() {
        let grp = CategoryGroup::new("grp-001".into(), "Food".into()).unwrap();
        assert_eq!(grp.id, "grp-001");
        assert_eq!(grp.name, "Food");
        assert!(grp.external_id.is_none());
    }

    #[test]
    fn new_rejects_empty_name() {
        assert!(CategoryGroup::new("grp-001".into(), "".into()).is_err());
    }

    #[test]
    fn from_external_captures_provenance() {
        let g = CategoryGroup::from_external(
            "grp-001".into(),
            "Food".into(),
            "monarch".into(),
            "monarch-g-42".into(),
        )
        .unwrap();
        assert_eq!(g.external_provider.as_deref(), Some("monarch"));
        assert_eq!(g.external_id.as_deref(), Some("monarch-g-42"));
    }

    #[test]
    fn serde_roundtrip() {
        let grp = CategoryGroup::new("grp-001".into(), "Food".into()).unwrap();
        let json = serde_json::to_string(&grp).unwrap();
        let deserialized: CategoryGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, grp.id);
        assert_eq!(deserialized.name, grp.name);
    }
}
