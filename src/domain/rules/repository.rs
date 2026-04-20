use crate::domain::error::DomainError;

use super::Rule;

pub trait RuleRepository {
    fn save(&self, rule: &Rule) -> Result<(), DomainError>;
    fn find_by_id(&self, id: &str) -> Result<Option<Rule>, DomainError>;
    /// Ordered by priority DESC, then created_at ASC (stable tiebreaker).
    fn find_all(&self) -> Result<Vec<Rule>, DomainError>;
    fn delete(&self, id: &str) -> Result<(), DomainError>;
}
