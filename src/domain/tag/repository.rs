use super::Tag;
use crate::domain::error::DomainError;

pub trait TagRepository {
    fn save(&self, tag: &Tag) -> Result<(), DomainError>;
    fn find_by_id(&self, id: &str) -> Result<Option<Tag>, DomainError>;
    fn find_by_name(&self, name: &str) -> Result<Option<Tag>, DomainError>;
    fn find_by_external(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<Tag>, DomainError>;
    fn find_all(&self) -> Result<Vec<Tag>, DomainError>;

    /// Replace the full set of tags on a transaction (delete + re-insert).
    fn set_tags_for_transaction(
        &self,
        transaction_id: &str,
        tag_ids: &[String],
    ) -> Result<(), DomainError>;

    fn find_tags_for_transaction(&self, transaction_id: &str) -> Result<Vec<Tag>, DomainError>;
}
