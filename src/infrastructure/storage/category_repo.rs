use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

use crate::domain::category::{Category, CategoryGroup, CategoryRepository};
use crate::domain::error::DomainError;

use super::database::Database;

pub struct SqliteCategoryRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteCategoryRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_group(row: &rusqlite::Row) -> rusqlite::Result<CategoryGroup> {
        let id: String = row.get("id")?;
        let name: String = row.get("name")?;
        let external_id: Option<String> = row.get("external_id")?;
        let external_provider: Option<String> = row.get("external_provider")?;
        let created_at_str: String = row.get("created_at")?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;
        Ok(CategoryGroup {
            id,
            name,
            external_id,
            external_provider,
            created_at,
        })
    }

    fn row_to_category(row: &rusqlite::Row) -> rusqlite::Result<Category> {
        let id: String = row.get("id")?;
        let group_id: String = row.get("group_id")?;
        let name: String = row.get("name")?;
        let external_id: Option<String> = row.get("external_id")?;
        let external_provider: Option<String> = row.get("external_provider")?;
        let created_at_str: String = row.get("created_at")?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;
        Ok(Category {
            id,
            group_id,
            name,
            external_id,
            external_provider,
            created_at,
        })
    }
}

impl CategoryRepository for SqliteCategoryRepository<'_> {
    fn save_group(&self, group: &CategoryGroup) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "INSERT OR REPLACE INTO category_groups (id, name, external_id, external_provider, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    group.id,
                    group.name,
                    group.external_id,
                    group.external_provider,
                    group.created_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save group: {e}")))?;
        Ok(())
    }

    fn save_category(&self, category: &Category) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "INSERT OR REPLACE INTO categories (id, group_id, name, external_id, external_provider, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    category.id,
                    category.group_id,
                    category.name,
                    category.external_id,
                    category.external_provider,
                    category.created_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save category: {e}")))?;
        Ok(())
    }

    fn find_group_by_id(&self, id: &str) -> Result<Option<CategoryGroup>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM category_groups WHERE id = ?1")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([id], Self::row_to_group)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_group: {e}")))
    }

    fn find_category_by_id(&self, id: &str) -> Result<Option<Category>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM categories WHERE id = ?1")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([id], Self::row_to_category)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_category: {e}")))
    }

    fn find_group_by_external(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<CategoryGroup>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM category_groups WHERE external_provider = ?1 AND external_id = ?2",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([provider, external_id], Self::row_to_group)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_group_by_external: {e}")))
    }

    fn find_category_by_external(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<Category>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM categories WHERE external_provider = ?1 AND external_id = ?2",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([provider, external_id], Self::row_to_category)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_category_by_external: {e}")))
    }

    fn find_categories_by_group(&self, group_id: &str) -> Result<Vec<Category>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM categories WHERE group_id = ?1 ORDER BY name")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map([group_id], Self::row_to_category)
            .map_err(|e| DomainError::Storage(format!("query: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("collect: {e}")))
    }

    fn find_all_groups(&self) -> Result<Vec<CategoryGroup>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM category_groups ORDER BY name")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map([], Self::row_to_group)
            .map_err(|e| DomainError::Storage(format!("query: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("collect: {e}")))
    }

    fn find_all_categories(&self) -> Result<Vec<Category>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM categories ORDER BY name")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map([], Self::row_to_category)
            .map_err(|e| DomainError::Storage(format!("query: {e}")))?
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

    fn make_group(id: &str, name: &str) -> CategoryGroup {
        CategoryGroup::new(id.into(), name.into()).unwrap()
    }

    fn make_category(id: &str, group_id: &str, name: &str) -> Category {
        Category::new(id.into(), group_id.into(), name.into()).unwrap()
    }

    #[test]
    fn save_and_find_group() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);
        let grp = make_group("grp-food", "Food");

        repo.save_group(&grp).unwrap();
        let found = repo.find_group_by_id("grp-food").unwrap().unwrap();
        assert_eq!(found.name, "Food");
    }

    #[test]
    fn save_and_find_category() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);

        let grp = make_group("grp-food", "Food");
        repo.save_group(&grp).unwrap();

        let cat = make_category("cat-groceries", "grp-food", "Groceries");
        repo.save_category(&cat).unwrap();

        let found = repo.find_category_by_id("cat-groceries").unwrap().unwrap();
        assert_eq!(found.name, "Groceries");
        assert_eq!(found.group_id, "grp-food");
    }

    #[test]
    fn find_categories_by_group() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);

        repo.save_group(&make_group("grp-food", "Food")).unwrap();
        repo.save_group(&make_group("grp-transport", "Transport"))
            .unwrap();

        repo.save_category(&make_category("cat-1", "grp-food", "Groceries"))
            .unwrap();
        repo.save_category(&make_category("cat-2", "grp-food", "Restaurants"))
            .unwrap();
        repo.save_category(&make_category("cat-3", "grp-transport", "Gas"))
            .unwrap();

        let food_cats = repo.find_categories_by_group("grp-food").unwrap();
        assert_eq!(food_cats.len(), 2);
        assert_eq!(food_cats[0].name, "Groceries");
        assert_eq!(food_cats[1].name, "Restaurants");

        let transport_cats = repo.find_categories_by_group("grp-transport").unwrap();
        assert_eq!(transport_cats.len(), 1);
    }

    #[test]
    fn find_all_groups() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);

        repo.save_group(&make_group("grp-2", "Transport")).unwrap();
        repo.save_group(&make_group("grp-1", "Food")).unwrap();

        let groups = repo.find_all_groups().unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Food");
        assert_eq!(groups[1].name, "Transport");
    }

    #[test]
    fn find_all_categories() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);

        repo.save_group(&make_group("grp-food", "Food")).unwrap();
        repo.save_category(&make_category("cat-1", "grp-food", "Groceries"))
            .unwrap();
        repo.save_category(&make_category("cat-2", "grp-food", "Coffee"))
            .unwrap();

        let all = repo.find_all_categories().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "Coffee");
        assert_eq!(all[1].name, "Groceries");
    }

    #[test]
    fn category_requires_existing_group() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);

        let result =
            repo.save_category(&make_category("cat-1", "nonexistent-group", "Groceries"));
        assert!(result.is_err());
    }

    #[test]
    fn group_not_found() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);
        assert!(repo.find_group_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn category_not_found() {
        let db = setup();
        let repo = SqliteCategoryRepository::new(&db);
        assert!(repo
            .find_category_by_id("nonexistent")
            .unwrap()
            .is_none());
    }
}
