use chrono::NaiveDate;

use super::Transaction;
use crate::domain::error::DomainError;

pub trait TransactionRepository {
    fn save(&self, transaction: &Transaction) -> Result<(), DomainError>;
    fn find_by_id(&self, id: &str) -> Result<Option<Transaction>, DomainError>;
    fn find_by_account(&self, account_id: &str) -> Result<Vec<Transaction>, DomainError>;
    fn find_by_date_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DomainError>;
    fn find_by_external_id(
        &self,
        account_id: &str,
        external_id: &str,
    ) -> Result<Option<Transaction>, DomainError>;
    /// Transactions with `category_id IS NULL`. Optionally scoped to one account.
    /// Used by `rtf categorize` to skip already-categorized rows.
    fn find_uncategorized(&self, account_id: Option<&str>)
    -> Result<Vec<Transaction>, DomainError>;
    /// Clear `category_id` on every row (optionally scoped). Returns rows affected.
    /// Used by `rtf categorize --reset` to re-run rules from scratch.
    fn clear_categories(&self, account_id: Option<&str>) -> Result<usize, DomainError>;
    /// Transactions with `category_id IS NULL AND transfer_pair_id IS NULL`.
    /// Used by transfer detection so already-categorized or already-paired
    /// rows are skipped.
    fn find_untagged(&self) -> Result<Vec<Transaction>, DomainError>;
    fn delete(&self, id: &str) -> Result<(), DomainError>;
}
