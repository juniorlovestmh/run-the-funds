use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub group_id: String,
    pub name: String,
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
            created_at: Utc::now(),
        })
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
    fn serde_roundtrip() {
        let cat = Category::new("cat-001".into(), "grp-food".into(), "Groceries".into()).unwrap();
        let json = serde_json::to_string(&cat).unwrap();
        let deserialized: Category = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, cat.id);
        assert_eq!(deserialized.name, cat.name);
        assert_eq!(deserialized.group_id, cat.group_id);
    }
}
