use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::domain::connections::{ProviderConnection, ProviderConnectionRepository};
use crate::domain::error::DomainError;

use super::database::Database;

pub struct SqliteProviderConnectionRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteProviderConnectionRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_connection(row: &rusqlite::Row) -> rusqlite::Result<ProviderConnection> {
        let id: String = row.get("id")?;
        let provider: String = row.get("provider")?;
        let external_id: String = row.get("external_id")?;
        let data: String = row.get("data")?;
        let institution_name: Option<String> = row.get("institution_name")?;
        let created_at_str: String = row.get("created_at")?;
        let updated_at_str: String = row.get("updated_at")?;

        let parse_dt = |s: &str, col: usize| -> rusqlite::Result<DateTime<Utc>> {
            // provider_connections stores timestamps in RFC3339 when written by
            // the repo, but the data-migration in migration 006 uses
            // SQLite's `datetime('now')` which produces `YYYY-MM-DD HH:MM:SS`
            // (no timezone). Accept either by trying RFC3339 first then
            // falling back to a naive parse assumed to be UTC.
            if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                return Ok(dt.with_timezone(&Utc));
            }
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc())
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        col,
                        rusqlite::types::Type::Text,
                        Box::from(e),
                    )
                })
        };

        Ok(ProviderConnection {
            id,
            provider,
            external_id,
            data,
            institution_name,
            created_at: parse_dt(&created_at_str, 5)?,
            updated_at: parse_dt(&updated_at_str, 6)?,
        })
    }
}

impl ProviderConnectionRepository for SqliteProviderConnectionRepository<'_> {
    fn save(&self, c: &ProviderConnection) -> Result<(), DomainError> {
        // Upsert on (provider, external_id) so re-running connect for the same
        // bank replaces rather than duplicates. `created_at` stays pinned to
        // the first insert; `updated_at` advances.
        self.db
            .conn()
            .execute(
                "INSERT INTO provider_connections (id, provider, external_id, data, institution_name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(provider, external_id) DO UPDATE SET
                     data = excluded.data,
                     institution_name = excluded.institution_name,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    c.id,
                    c.provider,
                    c.external_id,
                    c.data,
                    c.institution_name,
                    c.created_at.to_rfc3339(),
                    c.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save provider_connection: {e}")))?;
        Ok(())
    }

    fn find_by_provider(&self, provider: &str) -> Result<Vec<ProviderConnection>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM provider_connections WHERE provider = ?1 ORDER BY created_at",
            )
            .map_err(|e| DomainError::Storage(format!("prepare find_by_provider: {e}")))?
            .query_map([provider], Self::row_to_connection)
            .map_err(|e| DomainError::Storage(format!("find_by_provider: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_by_provider collect: {e}")))
    }

    fn find_by_external_id(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<ProviderConnection>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM provider_connections \
                 WHERE provider = ?1 AND external_id = ?2 LIMIT 1",
            )
            .map_err(|e| DomainError::Storage(format!("prepare find_by_external_id: {e}")))?
            .query_row([provider, external_id], Self::row_to_connection)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_external_id: {e}")))
    }

    fn find_by_id(&self, id: &str) -> Result<Option<ProviderConnection>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM provider_connections WHERE id = ?1 LIMIT 1")
            .map_err(|e| DomainError::Storage(format!("prepare find_by_id: {e}")))?
            .query_row([id], Self::row_to_connection)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_id: {e}")))
    }

    fn find_all(&self) -> Result<Vec<ProviderConnection>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM provider_connections ORDER BY provider, created_at")
            .map_err(|e| DomainError::Storage(format!("prepare find_all: {e}")))?
            .query_map([], Self::row_to_connection)
            .map_err(|e| DomainError::Storage(format!("find_all: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_all collect: {e}")))
    }

    fn delete(&self, id: &str) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute("DELETE FROM provider_connections WHERE id = ?1", [id])
            .map_err(|e| DomainError::Storage(format!("delete provider_connection: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::credentials::{ProviderCredentials, ProviderCredentialsRepository};
    use crate::infrastructure::storage::SqliteProviderCredentialsRepository;

    fn conn(provider: &str, ext: &str, data: &str, inst: Option<&str>) -> ProviderConnection {
        ProviderConnection::new(
            format!("c-{provider}-{ext}"),
            provider.into(),
            ext.into(),
            data.into(),
            inst.map(|s| s.to_string()),
        )
        .unwrap()
    }

    #[test]
    fn save_and_find_by_provider() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        repo.save(&conn("teller", "enr_1", r#"{"access_token":"t1"}"#, Some("Chase")))
            .unwrap();
        repo.save(&conn("teller", "enr_2", r#"{"access_token":"t2"}"#, Some("Capital One")))
            .unwrap();
        repo.save(&conn("pluggy", "item_x", "{}", Some("Nubank"))).unwrap();

        let teller = repo.find_by_provider("teller").unwrap();
        assert_eq!(teller.len(), 2);
        assert!(teller.iter().any(|c| c.external_id == "enr_1"));
        assert!(teller.iter().any(|c| c.external_id == "enr_2"));

        let pluggy = repo.find_by_provider("pluggy").unwrap();
        assert_eq!(pluggy.len(), 1);
        assert_eq!(pluggy[0].institution_name.as_deref(), Some("Nubank"));
    }

    #[test]
    fn save_upserts_on_duplicate_provider_external_id() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        repo.save(&conn("teller", "enr_1", r#"{"access_token":"old"}"#, Some("Chase")))
            .unwrap();
        repo.save(&conn("teller", "enr_1", r#"{"access_token":"new"}"#, Some("Chase Rotated")))
            .unwrap();

        let all = repo.find_by_provider("teller").unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].data.contains("new"));
        assert_eq!(all[0].institution_name.as_deref(), Some("Chase Rotated"));
    }

    #[test]
    fn find_by_external_id_returns_matching() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        repo.save(&conn("teller", "enr_a", r#"{"access_token":"x"}"#, None)).unwrap();
        repo.save(&conn("pluggy", "enr_a", "{}", None)).unwrap();

        let a = repo.find_by_external_id("teller", "enr_a").unwrap().unwrap();
        assert_eq!(a.provider, "teller");
        let b = repo.find_by_external_id("pluggy", "enr_a").unwrap().unwrap();
        assert_eq!(b.provider, "pluggy");
        assert!(
            repo.find_by_external_id("teller", "missing")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn find_all_orders_by_provider_then_created() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        repo.save(&conn("teller", "enr_1", "{}", None)).unwrap();
        repo.save(&conn("pluggy", "item_a", "{}", None)).unwrap();
        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 2);
        // Pluggy rows come before Teller (alphabetical provider order).
        assert_eq!(all[0].provider, "pluggy");
        assert_eq!(all[1].provider, "teller");
    }

    #[test]
    fn delete_removes_row() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        let c = conn("teller", "enr_1", "{}", None);
        repo.save(&c).unwrap();
        repo.delete(&c.id).unwrap();
        assert!(repo.find_by_provider("teller").unwrap().is_empty());
    }

    #[test]
    fn migration_006_splits_existing_teller_access_token_into_connection() {
        // Simulate the pre-S04C state: a Teller provider_credentials row with
        // access_token + cert_pem + key_pem, and no provider_connections rows.
        // But the migrations run on Database::in_memory() already, so we
        // can't inject pre-migration state easily. Instead: simulate the
        // POST-migration shape by inserting an old-style row AFTER migrations
        // ran, then re-running migration 006 manually via execute_batch, and
        // asserting the split happened. But rerunning 006 on a DB already at
        // v6 is a no-op because `schema_version` is version-gated in our
        // migration runner. So just verify the SQL is idempotent enough that
        // a fresh run on seed data produces the expected rows.
        //
        // Pragmatic approach: insert a seed row, run the migration SQL
        // directly (bypassing the version gate), assert the split.
        let db = Database::in_memory().unwrap();

        // Seed a pre-migration-style Teller credentials row.
        let creds_repo = SqliteProviderCredentialsRepository::new(&db);
        // Delete anything already there so we can stage.
        creds_repo.delete("teller").unwrap();
        let seeded = ProviderCredentials::new(
            "seed".into(),
            "teller".into(),
            r#"{"access_token":"legacy-token","cert_pem":"C","key_pem":"K"}"#.into(),
        )
        .unwrap();
        creds_repo.save(&seeded).unwrap();

        // Re-run migration 006 SQL directly.
        db.conn()
            .execute_batch(include_str!("../../../migrations/006_migrate_connections.sql"))
            .unwrap();

        // Assert the access_token moved into provider_connections.
        let connections = SqliteProviderConnectionRepository::new(&db);
        let teller_conns = connections.find_by_provider("teller").unwrap();
        assert_eq!(teller_conns.len(), 1);
        assert_eq!(teller_conns[0].external_id, "legacy-token");
        assert!(teller_conns[0].data.contains("legacy-token"));

        // And access_token is gone from provider_credentials.
        let remaining = creds_repo.find_by_provider("teller").unwrap().unwrap();
        assert!(!remaining.data.contains("access_token"));
        assert!(remaining.data.contains("cert_pem"));
        assert!(remaining.data.contains("key_pem"));
    }
}
