use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::domain::credentials::{ProviderCredentials, ProviderCredentialsRepository};
use crate::domain::error::DomainError;

use super::database::Database;

pub struct SqliteProviderCredentialsRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteProviderCredentialsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_creds(row: &rusqlite::Row) -> rusqlite::Result<ProviderCredentials> {
        let id: String = row.get("id")?;
        let provider: String = row.get("provider")?;
        let data: String = row.get("data")?;
        let created_at_str: String = row.get("created_at")?;
        let updated_at_str: String = row.get("updated_at")?;

        let parse_dt = |s: &str, col: usize| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        col,
                        rusqlite::types::Type::Text,
                        Box::from(e),
                    )
                })
        };
        Ok(ProviderCredentials {
            id,
            provider,
            data,
            created_at: parse_dt(&created_at_str, 3)?,
            updated_at: parse_dt(&updated_at_str, 4)?,
        })
    }
}

impl ProviderCredentialsRepository for SqliteProviderCredentialsRepository<'_> {
    fn save(&self, credentials: &ProviderCredentials) -> Result<(), DomainError> {
        // Upsert keyed on `provider` — the UNIQUE constraint from migration 004.
        // Use ON CONFLICT so `updated_at` can advance on repeat setup calls while
        // `created_at` stays pinned to the first setup.
        self.db
            .conn()
            .execute(
                "INSERT INTO provider_credentials (id, provider, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(provider) DO UPDATE SET
                     data = excluded.data,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    credentials.id,
                    credentials.provider,
                    credentials.data,
                    credentials.created_at.to_rfc3339(),
                    credentials.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save provider_credentials: {e}")))?;
        Ok(())
    }

    fn find_by_provider(&self, provider: &str) -> Result<Option<ProviderCredentials>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM provider_credentials WHERE provider = ?1 LIMIT 1")
            .map_err(|e| DomainError::Storage(format!("prepare find_by_provider: {e}")))?
            .query_row([provider], Self::row_to_creds)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_provider: {e}")))
    }

    fn delete(&self, provider: &str) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "DELETE FROM provider_credentials WHERE provider = ?1",
                [provider],
            )
            .map_err(|e| DomainError::Storage(format!("delete provider_credentials: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds(provider: &str, data: &str) -> ProviderCredentials {
        ProviderCredentials::new(format!("c-{provider}"), provider.into(), data.into()).unwrap()
    }

    #[test]
    fn save_and_find_by_provider() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderCredentialsRepository::new(&db);
        let c = creds("simplefin", r#"{"access_url":"https://u:p@host/p"}"#);
        repo.save(&c).unwrap();

        let found = repo.find_by_provider("simplefin").unwrap().unwrap();
        assert_eq!(found.provider, "simplefin");
        assert_eq!(found.data, r#"{"access_url":"https://u:p@host/p"}"#);
    }

    #[test]
    fn find_by_provider_returns_none_when_absent() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderCredentialsRepository::new(&db);
        assert!(repo.find_by_provider("simplefin").unwrap().is_none());
    }

    #[test]
    fn save_upserts_by_provider() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderCredentialsRepository::new(&db);
        repo.save(&creds("simplefin", r#"{"access_url":"old"}"#))
            .unwrap();

        // Second save with different id + data but same provider: UPSERT
        // should overwrite data and leave only one row.
        let mut updated = creds("simplefin", r#"{"access_url":"new"}"#);
        updated.id = "c-different".into();
        repo.save(&updated).unwrap();

        let found = repo.find_by_provider("simplefin").unwrap().unwrap();
        assert_eq!(found.data, r#"{"access_url":"new"}"#);
    }

    #[test]
    fn different_providers_coexist() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderCredentialsRepository::new(&db);
        repo.save(&creds("simplefin", r#"{"access_url":"x"}"#))
            .unwrap();
        repo.save(&creds(
            "pluggy",
            r#"{"client_id":"a","client_secret":"b","item_id":"c"}"#,
        ))
        .unwrap();
        assert!(repo.find_by_provider("simplefin").unwrap().is_some());
        assert!(repo.find_by_provider("pluggy").unwrap().is_some());
    }

    #[test]
    fn delete_removes_row() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderCredentialsRepository::new(&db);
        repo.save(&creds("simplefin", r#"{"access_url":"x"}"#))
            .unwrap();
        repo.delete("simplefin").unwrap();
        assert!(repo.find_by_provider("simplefin").unwrap().is_none());
    }

    #[test]
    fn delete_missing_provider_is_noop() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderCredentialsRepository::new(&db);
        repo.delete("nope").unwrap(); // no error
    }
}
