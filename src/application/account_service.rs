use uuid::Uuid;

use crate::domain::account::{Account, AccountRepository, AccountType};
use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;

pub struct AccountService<R: AccountRepository> {
    repo: R,
}

impl<R: AccountRepository> AccountService<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn create_account(
        &self,
        name: String,
        account_type: AccountType,
        currency: CurrencyCode,
        owner: String,
        institution: Option<String>,
    ) -> Result<Account, DomainError> {
        let id = Uuid::new_v4().to_string();
        let mut account = Account::new(id, name, account_type, currency, owner)?;
        account.institution = institution;
        self.repo.save(&account)?;
        Ok(account)
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>, DomainError> {
        self.repo.find_all()
    }

    pub fn get_account(&self, id: &str) -> Result<Account, DomainError> {
        self.repo
            .find_by_id(id)?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Account".into(),
                id: id.into(),
            })
    }

    pub fn delete_account(&self, id: &str) -> Result<(), DomainError> {
        self.get_account(id)?;
        self.repo.delete(id)
    }

    /// Link an account to a bank-sync provider. When `force` is false, an
    /// already-linked account is rejected to guard against accidental
    /// reconfiguration — the caller can opt in with `force = true`.
    pub fn link_account(
        &self,
        id: &str,
        provider: String,
        external_account_id: String,
        force: bool,
    ) -> Result<Account, DomainError> {
        let mut account = self.get_account(id)?;
        if account.is_linked() && !force {
            return Err(DomainError::Validation(format!(
                "account {id} is already linked to {} (external id {}); pass --force to override",
                account.external_provider.as_deref().unwrap_or("?"),
                account.external_account_id.as_deref().unwrap_or("?"),
            )));
        }
        account.link(provider, external_account_id)?;
        self.repo.save(&account)?;
        Ok(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::storage::{Database, SqliteAccountRepository};

    fn setup() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn create_account_persists() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let svc = AccountService::new(repo);

        let acc = svc
            .create_account(
                "Nubank Checking".into(),
                AccountType::Checking,
                CurrencyCode::BRL,
                "Sky".into(),
                Some("Nubank".into()),
            )
            .unwrap();

        assert_eq!(acc.name, "Nubank Checking");
        assert!(!acc.id.is_empty());

        let repo2 = SqliteAccountRepository::new(&db);
        let svc2 = AccountService::new(repo2);
        let listed = svc2.list_accounts().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Nubank Checking");
    }

    #[test]
    fn create_account_validates_name() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let svc = AccountService::new(repo);

        let result = svc.create_account(
            "".into(),
            AccountType::Checking,
            CurrencyCode::USD,
            "Sky".into(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn list_accounts_empty() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let svc = AccountService::new(repo);
        assert!(svc.list_accounts().unwrap().is_empty());
    }

    #[test]
    fn get_account_not_found() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let svc = AccountService::new(repo);

        let result = svc.get_account("nonexistent");
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[test]
    fn delete_account() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let svc = AccountService::new(repo);

        let acc = svc
            .create_account(
                "Test".into(),
                AccountType::Checking,
                CurrencyCode::USD,
                "Sky".into(),
                None,
            )
            .unwrap();

        let repo2 = SqliteAccountRepository::new(&db);
        let svc2 = AccountService::new(repo2);
        svc2.delete_account(&acc.id).unwrap();
        assert!(svc2.list_accounts().unwrap().is_empty());
    }

    #[test]
    fn delete_nonexistent_fails() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let svc = AccountService::new(repo);
        assert!(svc.delete_account("nonexistent").is_err());
    }

    fn create_unlinked(
        db: &crate::infrastructure::storage::Database,
        name: &str,
    ) -> crate::domain::account::Account {
        let svc = AccountService::new(SqliteAccountRepository::new(db));
        svc.create_account(
            name.into(),
            AccountType::Checking,
            CurrencyCode::USD,
            "Sky".into(),
            None,
        )
        .unwrap()
    }

    #[test]
    fn link_account_happy_path() {
        let db = setup();
        let acc = create_unlinked(&db, "Chase");
        let svc = AccountService::new(SqliteAccountRepository::new(&db));
        let linked = svc
            .link_account(&acc.id, "simplefin".into(), "ext-999".into(), false)
            .unwrap();
        assert_eq!(linked.external_provider.as_deref(), Some("simplefin"));
        assert_eq!(linked.external_account_id.as_deref(), Some("ext-999"));
    }

    #[test]
    fn link_account_rejects_second_link_without_force() {
        let db = setup();
        let acc = create_unlinked(&db, "Chase");
        let svc = AccountService::new(SqliteAccountRepository::new(&db));
        svc.link_account(&acc.id, "simplefin".into(), "ext-1".into(), false)
            .unwrap();

        let err = svc
            .link_account(&acc.id, "simplefin".into(), "ext-2".into(), false)
            .unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn link_account_force_overrides_existing_link() {
        let db = setup();
        let acc = create_unlinked(&db, "Chase");
        let svc = AccountService::new(SqliteAccountRepository::new(&db));
        svc.link_account(&acc.id, "simplefin".into(), "ext-1".into(), false)
            .unwrap();

        let relinked = svc
            .link_account(&acc.id, "simplefin".into(), "ext-2".into(), true)
            .unwrap();
        assert_eq!(relinked.external_account_id.as_deref(), Some("ext-2"));
    }

    #[test]
    fn link_account_unknown_id_errors() {
        let db = setup();
        let svc = AccountService::new(SqliteAccountRepository::new(&db));
        let err = svc
            .link_account("no-such-id", "simplefin".into(), "ext-1".into(), false)
            .unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }
}
