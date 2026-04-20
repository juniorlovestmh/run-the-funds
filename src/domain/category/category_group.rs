use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryGroup {
    pub id: String,
    pub name: String,
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
            created_at: Utc::now(),
        })
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
    }

    #[test]
    fn new_rejects_empty_name() {
        assert!(CategoryGroup::new("grp-001".into(), "".into()).is_err());
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
