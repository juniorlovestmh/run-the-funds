use crate::domain::error::DomainError;

use super::database::Database;

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial",
        include_str!("../../../migrations/001_initial.sql"),
    ),
    (
        "002_transaction_external_id_unique",
        include_str!("../../../migrations/002_transaction_external_id_unique.sql"),
    ),
    (
        "003_account_external_link",
        include_str!("../../../migrations/003_account_external_link.sql"),
    ),
    (
        "004_provider_credentials",
        include_str!("../../../migrations/004_provider_credentials.sql"),
    ),
    (
        "005_provider_connections",
        include_str!("../../../migrations/005_provider_connections.sql"),
    ),
    (
        "006_migrate_connections",
        include_str!("../../../migrations/006_migrate_connections.sql"),
    ),
    (
        "007_rules",
        include_str!("../../../migrations/007_rules.sql"),
    ),
    (
        "008_transaction_splits",
        include_str!("../../../migrations/008_transaction_splits.sql"),
    ),
    (
        "009_tags",
        include_str!("../../../migrations/009_tags.sql"),
    ),
];

pub fn run(db: &mut Database) -> Result<(), DomainError> {
    db.conn()
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            )",
        )
        .map_err(|e| DomainError::Storage(format!("create schema_version: {e}")))?;

    let current_version: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .map_err(|e| DomainError::Storage(format!("read schema_version: {e}")))?;

    for (i, (_name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version <= current_version {
            continue;
        }

        db.conn()
            .execute_batch(sql)
            .map_err(|e| DomainError::Storage(format!("migration {version}: {e}")))?;

        db.conn()
            .execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
                [version],
            )
            .map_err(|e| DomainError::Storage(format!("record migration {version}: {e}")))?;
    }

    Ok(())
}
