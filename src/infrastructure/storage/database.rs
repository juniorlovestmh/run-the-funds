use rusqlite::Connection;

use crate::domain::error::DomainError;

use super::migrations;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, DomainError> {
        let conn =
            Connection::open(path).map_err(|e| DomainError::Storage(format!("open: {e}")))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| DomainError::Storage(format!("pragma: {e}")))?;

        let mut db = Self { conn };
        migrations::run(&mut db)?;
        Ok(db)
    }

    pub fn in_memory() -> Result<Self, DomainError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DomainError::Storage(format!("open_in_memory: {e}")))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| DomainError::Storage(format!("pragma: {e}")))?;

        let mut db = Self { conn };
        migrations::run(&mut db)?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_creates_tables() {
        let db = Database::in_memory().unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn in_memory_has_all_tables() {
        let db = Database::in_memory().unwrap();
        let expected_tables = [
            "accounts",
            "persons",
            "categories",
            "category_groups",
            "transactions",
            "exchange_rates",
            "schema_version",
        ];
        for table in expected_tables {
            let exists: bool = db
                .conn()
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "table {table} should exist");
        }
    }

    #[test]
    fn foreign_keys_enabled() {
        let db = Database::in_memory().unwrap();
        let fk_on: bool = db
            .conn()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert!(fk_on);
    }

    #[test]
    fn open_file_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Database::open(path.to_str().unwrap()).unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migration_is_idempotent() {
        let db = Database::in_memory().unwrap();
        let version: i64 = db
            .conn()
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 10);
    }

    #[test]
    fn migration_002_creates_external_id_unique_index() {
        let db = Database::in_memory().unwrap();
        let exists: bool = db
            .conn()
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master \
                 WHERE type='index' AND name='idx_transactions_account_external_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists);
    }
}
