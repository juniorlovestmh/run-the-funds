use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use std::str::FromStr;

use crate::domain::error::DomainError;
use crate::domain::household::person::{Person, Relationship};

use super::database::Database;

pub struct SqlitePersonRepository<'a> {
    db: &'a Database,
}

impl<'a> SqlitePersonRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_person(row: &rusqlite::Row) -> rusqlite::Result<Person> {
        let id: String = row.get("id")?;
        let name: String = row.get("name")?;
        let rel_str: String = row.get("relationship")?;
        let created_at_str: String = row.get("created_at")?;

        let relationship = Relationship::from_str(&rel_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;

        Ok(Person {
            id,
            name,
            relationship,
            created_at,
        })
    }

    pub fn save(&self, person: &Person) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "INSERT OR REPLACE INTO persons (id, name, relationship, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    person.id,
                    person.name,
                    person.relationship.to_string(),
                    person.created_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save person: {e}")))?;
        Ok(())
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<Person>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM persons WHERE id = ?1")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([id], Self::row_to_person)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_id: {e}")))
    }

    pub fn find_all(&self) -> Result<Vec<Person>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM persons ORDER BY name")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map([], Self::row_to_person)
            .map_err(|e| DomainError::Storage(format!("find_all: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("collect: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn save_and_find_by_id() {
        let db = setup();
        let repo = SqlitePersonRepository::new(&db);
        let person = Person::new("p-001".into(), "Sky".into(), Relationship::Self_).unwrap();

        repo.save(&person).unwrap();
        let found = repo.find_by_id("p-001").unwrap().unwrap();

        assert_eq!(found.id, "p-001");
        assert_eq!(found.name, "Sky");
        assert_eq!(found.relationship, Relationship::Self_);
    }

    #[test]
    fn find_by_id_not_found() {
        let db = setup();
        let repo = SqlitePersonRepository::new(&db);
        assert!(repo.find_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn find_all() {
        let db = setup();
        let repo = SqlitePersonRepository::new(&db);

        repo.save(&Person::new("p-1".into(), "Sky".into(), Relationship::Self_).unwrap())
            .unwrap();
        repo.save(&Person::new("p-2".into(), "Wife".into(), Relationship::Spouse).unwrap())
            .unwrap();
        repo.save(&Person::new("p-3".into(), "Baby".into(), Relationship::Child).unwrap())
            .unwrap();

        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "Baby");
        assert_eq!(all[1].name, "Sky");
        assert_eq!(all[2].name, "Wife");
    }

    #[test]
    fn upsert_on_save() {
        let db = setup();
        let repo = SqlitePersonRepository::new(&db);

        let mut person = Person::new("p-001".into(), "Old".into(), Relationship::Self_).unwrap();
        repo.save(&person).unwrap();

        person.name = "New".into();
        repo.save(&person).unwrap();

        let found = repo.find_by_id("p-001").unwrap().unwrap();
        assert_eq!(found.name, "New");
        assert_eq!(repo.find_all().unwrap().len(), 1);
    }

    #[test]
    fn all_relationship_types_roundtrip() {
        let db = setup();
        let repo = SqlitePersonRepository::new(&db);

        for (id, rel) in [
            ("1", Relationship::Self_),
            ("2", Relationship::Spouse),
            ("3", Relationship::Child),
            ("4", Relationship::Other),
        ] {
            let p = Person::new(id.into(), format!("Person {id}"), rel.clone()).unwrap();
            repo.save(&p).unwrap();
            let found = repo.find_by_id(id).unwrap().unwrap();
            assert_eq!(found.relationship, rel);
        }
    }
}
