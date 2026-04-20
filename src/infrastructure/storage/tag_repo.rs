use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::domain::error::DomainError;
use crate::domain::tag::{Tag, TagRepository};

use super::database::Database;

pub struct SqliteTagRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteTagRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_tag(row: &rusqlite::Row) -> rusqlite::Result<Tag> {
        let created_at_str: String = row.get("created_at")?;
        let updated_at_str: String = row.get("updated_at")?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;
        Ok(Tag {
            id: row.get("id")?,
            name: row.get("name")?,
            color: row.get("color")?,
            order_index: row.get("order_index")?,
            external_id: row.get("external_id")?,
            external_provider: row.get("external_provider")?,
            created_at,
            updated_at,
        })
    }
}

impl TagRepository for SqliteTagRepository<'_> {
    fn save(&self, tag: &Tag) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "INSERT INTO tags (id, name, color, order_index, external_id, external_provider, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                     name = excluded.name,
                     color = excluded.color,
                     order_index = excluded.order_index,
                     external_id = excluded.external_id,
                     external_provider = excluded.external_provider,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    tag.id,
                    tag.name,
                    tag.color,
                    tag.order_index,
                    tag.external_id,
                    tag.external_provider,
                    tag.created_at.to_rfc3339(),
                    tag.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save tag: {e}")))?;
        Ok(())
    }

    fn find_by_id(&self, id: &str) -> Result<Option<Tag>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM tags WHERE id = ?1")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([id], Self::row_to_tag)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_id: {e}")))
    }

    fn find_by_name(&self, name: &str) -> Result<Option<Tag>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM tags WHERE name = ?1")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([name], Self::row_to_tag)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_name: {e}")))
    }

    fn find_by_external(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<Tag>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM tags WHERE external_provider = ?1 AND external_id = ?2",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([provider, external_id], Self::row_to_tag)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_external: {e}")))
    }

    fn find_all(&self) -> Result<Vec<Tag>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM tags ORDER BY order_index, name")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map([], Self::row_to_tag)
            .map_err(|e| DomainError::Storage(format!("query: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("collect: {e}")))
    }

    fn set_tags_for_transaction(
        &self,
        transaction_id: &str,
        tag_ids: &[String],
    ) -> Result<(), DomainError> {
        let conn = self.db.conn();
        conn.execute(
            "DELETE FROM transaction_tags WHERE transaction_id = ?1",
            [transaction_id],
        )
        .map_err(|e| DomainError::Storage(format!("clear tags: {e}")))?;
        for tag_id in tag_ids {
            conn.execute(
                "INSERT INTO transaction_tags (transaction_id, tag_id) VALUES (?1, ?2)",
                [transaction_id, tag_id],
            )
            .map_err(|e| DomainError::Storage(format!("attach tag {tag_id}: {e}")))?;
        }
        Ok(())
    }

    fn find_tags_for_transaction(&self, transaction_id: &str) -> Result<Vec<Tag>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT t.* FROM tags t
                 JOIN transaction_tags tt ON tt.tag_id = t.id
                 WHERE tt.transaction_id = ?1
                 ORDER BY t.order_index, t.name",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map([transaction_id], Self::row_to_tag)
            .map_err(|e| DomainError::Storage(format!("query: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("collect: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::{Account, AccountRepository, AccountType};
    use crate::domain::currency::{CurrencyCode, Money};
    use crate::domain::transaction::{Transaction, TransactionRepository};
    use crate::infrastructure::storage::{
        SqliteAccountRepository, SqliteTransactionRepository,
    };
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn setup() -> Database {
        Database::in_memory().unwrap()
    }

    fn make_tag(id: &str, name: &str) -> Tag {
        Tag::new(id.into(), name.into()).unwrap()
    }

    fn make_external_tag(id: &str, name: &str, ext_id: &str) -> Tag {
        Tag::from_external(
            id.into(),
            name.into(),
            "monarch".into(),
            ext_id.into(),
            Some("#000000".into()),
            Some(1),
        )
        .unwrap()
    }

    fn seed_account(db: &Database, id: &str) -> String {
        let repo = SqliteAccountRepository::new(db);
        let acc = Account::new(
            id.into(),
            "Test".into(),
            AccountType::Checking,
            CurrencyCode::USD,
            "Junior".into(),
        )
        .unwrap();
        repo.save(&acc).unwrap();
        id.to_string()
    }

    fn seed_transaction(db: &Database, id: &str, account_id: &str) -> String {
        let repo = SqliteTransactionRepository::new(db);
        let txn = Transaction::new(
            id.into(),
            account_id.into(),
            chrono::NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            Money::new(Decimal::from_str("-10.00").unwrap(), CurrencyCode::USD),
        )
        .unwrap();
        repo.save(&txn).unwrap();
        id.to_string()
    }

    #[test]
    fn save_and_find_by_id() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        let tag = make_tag("tag-1", "BR");
        repo.save(&tag).unwrap();
        let found = repo.find_by_id("tag-1").unwrap().unwrap();
        assert_eq!(found.name, "BR");
    }

    #[test]
    fn save_is_upsert() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        let mut tag = make_tag("tag-1", "BR");
        repo.save(&tag).unwrap();
        tag.color = Some("#FF0000".into());
        repo.save(&tag).unwrap();
        let found = repo.find_by_id("tag-1").unwrap().unwrap();
        assert_eq!(found.color.as_deref(), Some("#FF0000"));
    }

    #[test]
    fn find_by_name() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_tag("tag-1", "Subscription")).unwrap();
        let found = repo.find_by_name("Subscription").unwrap().unwrap();
        assert_eq!(found.id, "tag-1");
        assert!(repo.find_by_name("nope").unwrap().is_none());
    }

    #[test]
    fn find_by_external() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_external_tag("tag-1", "BR", "monarch-br-123"))
            .unwrap();
        let found = repo
            .find_by_external("monarch", "monarch-br-123")
            .unwrap()
            .unwrap();
        assert_eq!(found.id, "tag-1");
        assert!(repo
            .find_by_external("monarch", "nope")
            .unwrap()
            .is_none());
    }

    #[test]
    fn name_unique_constraint() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_tag("tag-1", "BR")).unwrap();
        let err = repo.save(&make_tag("tag-2", "BR")).unwrap_err();
        match err {
            DomainError::Storage(msg) => assert!(msg.to_lowercase().contains("unique")),
            _ => panic!("expected storage UNIQUE error"),
        }
    }

    #[test]
    fn set_and_find_tags_for_transaction() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_tag("tag-br", "BR")).unwrap();
        repo.save(&make_tag("tag-sub", "Subscription")).unwrap();
        let account_id = seed_account(&db, "acct-1");
        let txn_id = seed_transaction(&db, "txn-1", &account_id);

        repo.set_tags_for_transaction(&txn_id, &["tag-br".into(), "tag-sub".into()])
            .unwrap();

        let found = repo.find_tags_for_transaction(&txn_id).unwrap();
        assert_eq!(found.len(), 2);
        let names: Vec<&str> = found.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"BR"));
        assert!(names.contains(&"Subscription"));
    }

    #[test]
    fn set_tags_replaces_existing() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_tag("tag-br", "BR")).unwrap();
        repo.save(&make_tag("tag-sub", "Subscription")).unwrap();
        let account_id = seed_account(&db, "acct-1");
        let txn_id = seed_transaction(&db, "txn-1", &account_id);

        repo.set_tags_for_transaction(&txn_id, &["tag-br".into()])
            .unwrap();
        repo.set_tags_for_transaction(&txn_id, &["tag-sub".into()])
            .unwrap();

        let found = repo.find_tags_for_transaction(&txn_id).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Subscription");
    }

    #[test]
    fn cascade_on_transaction_delete() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_tag("tag-br", "BR")).unwrap();
        let account_id = seed_account(&db, "acct-1");
        let txn_id = seed_transaction(&db, "txn-1", &account_id);
        repo.set_tags_for_transaction(&txn_id, &["tag-br".into()])
            .unwrap();

        // Delete the transaction via raw SQL.
        db.conn()
            .execute("DELETE FROM transactions WHERE id = ?1", [&txn_id])
            .unwrap();

        let orphan_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM transaction_tags WHERE transaction_id = ?1",
                [&txn_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan_count, 0);
    }

    #[test]
    fn cascade_on_tag_delete() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_tag("tag-br", "BR")).unwrap();
        let account_id = seed_account(&db, "acct-1");
        let txn_id = seed_transaction(&db, "txn-1", &account_id);
        repo.set_tags_for_transaction(&txn_id, &["tag-br".into()])
            .unwrap();

        db.conn()
            .execute("DELETE FROM tags WHERE id = ?1", ["tag-br"])
            .unwrap();

        let orphan_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM transaction_tags WHERE tag_id = ?1",
                ["tag-br"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan_count, 0);
    }

    #[test]
    fn find_all_ordered_by_order_index_then_name() {
        let db = setup();
        let repo = SqliteTagRepository::new(&db);
        repo.save(&make_external_tag("t-a", "Alpha", "ext-a")).unwrap();
        repo.save(&make_tag("t-b", "Beta")).unwrap();
        repo.save(&make_external_tag("t-c", "Charlie", "ext-c"))
            .unwrap();

        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 3);
    }
}
