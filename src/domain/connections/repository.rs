use crate::domain::error::DomainError;

use super::ProviderConnection;

pub trait ProviderConnectionRepository {
    /// Upsert keyed on `(provider, external_id)` — re-running connect for the
    /// same bank replaces rather than duplicates.
    fn save(&self, connection: &ProviderConnection) -> Result<(), DomainError>;

    fn find_by_provider(&self, provider: &str) -> Result<Vec<ProviderConnection>, DomainError>;

    fn find_by_external_id(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<ProviderConnection>, DomainError>;

    fn find_by_id(&self, id: &str) -> Result<Option<ProviderConnection>, DomainError>;

    fn find_all(&self) -> Result<Vec<ProviderConnection>, DomainError>;

    fn delete(&self, id: &str) -> Result<(), DomainError>;
}
