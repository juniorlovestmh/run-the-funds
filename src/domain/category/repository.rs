use super::{Category, CategoryGroup};
use crate::domain::error::DomainError;

pub trait CategoryRepository {
    fn save_group(&self, group: &CategoryGroup) -> Result<(), DomainError>;
    fn save_category(&self, category: &Category) -> Result<(), DomainError>;
    fn find_group_by_id(&self, id: &str) -> Result<Option<CategoryGroup>, DomainError>;
    fn find_category_by_id(&self, id: &str) -> Result<Option<Category>, DomainError>;
    fn find_group_by_external(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<CategoryGroup>, DomainError>;
    fn find_category_by_external(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Result<Option<Category>, DomainError>;
    fn find_categories_by_group(&self, group_id: &str) -> Result<Vec<Category>, DomainError>;
    fn find_all_groups(&self) -> Result<Vec<CategoryGroup>, DomainError>;
    fn find_all_categories(&self) -> Result<Vec<Category>, DomainError>;
}
