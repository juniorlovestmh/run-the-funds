use crate::domain::error::DomainError;

use super::Account;

pub trait AccountRepository {
    fn save(&self, account: &Account) -> Result<(), DomainError>;
    fn find_by_id(&self, id: &str) -> Result<Option<Account>, DomainError>;
    fn find_all(&self) -> Result<Vec<Account>, DomainError>;
    /// Look up an account by the remote provider + external id it was linked to.
    fn find_by_external_link(
        &self,
        provider: &str,
        external_account_id: &str,
    ) -> Result<Option<Account>, DomainError>;
    /// Enumerate every account linked to a given provider (for sync orchestration).
    fn find_by_provider(&self, provider: &str) -> Result<Vec<Account>, DomainError>;
    fn delete(&self, id: &str) -> Result<(), DomainError>;
}
