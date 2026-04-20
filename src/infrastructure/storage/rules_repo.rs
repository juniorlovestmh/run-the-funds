use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::domain::error::DomainError;
use crate::domain::rules::{MatchField, MatchKind, Rule, RuleRepository};

use super::database::Database;

pub struct SqliteRuleRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteRuleRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_rule(row: &rusqlite::Row) -> rusqlite::Result<Rule> {
        let id: String = row.get("id")?;
        let name: String = row.get("name")?;
        let match_field_str: String = row.get("match_field")?;
        let match_kind_str: String = row.get("match_kind")?;
        let match_pattern: String = row.get("match_pattern")?;
        let category_id: String = row.get("category_id")?;
        let priority: i64 = row.get("priority")?;
        let created_at_str: String = row.get("created_at")?;
        let updated_at_str: String = row.get("updated_at")?;

        let match_field = MatchField::from_str(&match_field_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let match_kind = MatchKind::from_str(&match_kind_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::from(e))
        })?;

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

        Ok(Rule {
            id,
            name,
            match_field,
            match_kind,
            match_pattern,
            category_id,
            priority,
            created_at: parse_dt(&created_at_str, 7)?,
            updated_at: parse_dt(&updated_at_str, 8)?,
        })
    }
}

impl RuleRepository for SqliteRuleRepository<'_> {
    fn save(&self, r: &Rule) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "INSERT INTO rules (
                    id, name, match_field, match_kind, match_pattern,
                    category_id, priority, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    match_field = excluded.match_field,
                    match_kind = excluded.match_kind,
                    match_pattern = excluded.match_pattern,
                    category_id = excluded.category_id,
                    priority = excluded.priority,
                    updated_at = excluded.updated_at",
                rusqlite::params![
                    r.id,
                    r.name,
                    r.match_field.to_string(),
                    r.match_kind.to_string(),
                    r.match_pattern,
                    r.category_id,
                    r.priority,
                    r.created_at.to_rfc3339(),
                    r.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save rule: {e}")))?;
        Ok(())
    }

    fn find_by_id(&self, id: &str) -> Result<Option<Rule>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM rules WHERE id = ?1 LIMIT 1")
            .map_err(|e| DomainError::Storage(format!("prepare find_by_id: {e}")))?
            .query_row([id], Self::row_to_rule)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_id: {e}")))
    }

    fn find_all(&self) -> Result<Vec<Rule>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM rules ORDER BY priority DESC, created_at ASC")
            .map_err(|e| DomainError::Storage(format!("prepare find_all: {e}")))?
            .query_map([], Self::row_to_rule)
            .map_err(|e| DomainError::Storage(format!("find_all: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_all collect: {e}")))
    }

    fn delete(&self, id: &str) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute("DELETE FROM rules WHERE id = ?1", [id])
            .map_err(|e| DomainError::Storage(format!("delete rule: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_category(db: &Database, id: &str, name: &str) {
        // Rules reference categories — need a real category + group for FK.
        let conn = db.conn();
        conn.execute(
            "INSERT INTO category_groups (id, name, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["grp-1", "Group1", chrono::Utc::now().to_rfc3339()],
        )
        .ok(); // Ignore duplicate insert errors if already seeded.
        conn.execute(
            "INSERT INTO categories (id, group_id, name, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, "grp-1", name, chrono::Utc::now().to_rfc3339()],
        )
        .unwrap();
    }

    fn mk_rule(id: &str, name: &str, priority: i64) -> Rule {
        Rule::new(
            id.into(),
            name.into(),
            MatchField::Payee,
            MatchKind::Substring,
            "pattern".into(),
            "cat-1".into(),
            priority,
        )
        .unwrap()
    }

    #[test]
    fn save_and_find_by_id() {
        let db = Database::in_memory().unwrap();
        seed_category(&db, "cat-1", "Food");
        let repo = SqliteRuleRepository::new(&db);
        let rule = mk_rule("r1", "Student Loan", 100);
        repo.save(&rule).unwrap();

        let found = repo.find_by_id("r1").unwrap().unwrap();
        assert_eq!(found.id, "r1");
        assert_eq!(found.name, "Student Loan");
        assert_eq!(found.match_field, MatchField::Payee);
        assert_eq!(found.match_kind, MatchKind::Substring);
        assert_eq!(found.priority, 100);
    }

    #[test]
    fn find_by_id_returns_none() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteRuleRepository::new(&db);
        assert!(repo.find_by_id("missing").unwrap().is_none());
    }

    #[test]
    fn find_all_orders_by_priority_desc_then_created_asc() {
        let db = Database::in_memory().unwrap();
        seed_category(&db, "cat-1", "Food");
        let repo = SqliteRuleRepository::new(&db);

        // Insert in shuffled priority order.
        repo.save(&mk_rule("r1", "Low", 10)).unwrap();
        repo.save(&mk_rule("r2", "High", 200)).unwrap();
        repo.save(&mk_rule("r3", "Mid", 100)).unwrap();

        let all = repo.find_all().unwrap();
        let names: Vec<&str> = all.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["High", "Mid", "Low"]);
    }

    #[test]
    fn save_upserts_on_same_id() {
        let db = Database::in_memory().unwrap();
        seed_category(&db, "cat-1", "Food");
        let repo = SqliteRuleRepository::new(&db);
        let rule = mk_rule("r1", "Original", 100);
        repo.save(&rule).unwrap();

        let updated = Rule::new(
            "r1".into(),
            "Renamed".into(),
            MatchField::Description,
            MatchKind::Regex,
            r"^x".into(),
            "cat-1".into(),
            200,
        )
        .unwrap();
        repo.save(&updated).unwrap();

        let found = repo.find_by_id("r1").unwrap().unwrap();
        assert_eq!(found.name, "Renamed");
        assert_eq!(found.priority, 200);
        assert_eq!(found.match_kind, MatchKind::Regex);
    }

    #[test]
    fn delete_removes_row() {
        let db = Database::in_memory().unwrap();
        seed_category(&db, "cat-1", "Food");
        let repo = SqliteRuleRepository::new(&db);
        let rule = mk_rule("r1", "X", 100);
        repo.save(&rule).unwrap();
        repo.delete("r1").unwrap();
        assert!(repo.find_by_id("r1").unwrap().is_none());
    }

    #[test]
    fn fk_rejects_unknown_category() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteRuleRepository::new(&db);
        // No seeded category → FK violation on save.
        let rule = mk_rule("r1", "X", 100);
        assert!(repo.save(&rule).is_err());
    }
}
