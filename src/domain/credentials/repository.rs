use crate::domain::error::DomainError;

use super::ProviderCredentials;

pub trait ProviderCredentialsRepository {
    /// Upsert — if a row already exists for `credentials.provider`, replace it.
    fn save(&self, credentials: &ProviderCredentials) -> Result<(), DomainError>;
    fn find_by_provider(&self, provider: &str) -> Result<Option<ProviderCredentials>, DomainError>;
    fn delete(&self, provider: &str) -> Result<(), DomainError>;
}
