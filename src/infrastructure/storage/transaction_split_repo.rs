use std::str::FromStr;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::currency::{CurrencyCode, Money};
use crate::domain::error::DomainError;
use crate::domain::transaction::{TransactionSplit, TransactionSplitRepository};

use super::database::Database;

pub struct SqliteTransactionSplitRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteTransactionSplitRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_split(row: &rusqlite::Row) -> rusqlite::Result<TransactionSplit> {
        let id: String = row.get("id")?;
        let transaction_id: String = row.get("transaction_id")?;
        let category_id: String = row.get("category_id")?;
        let amount_value: String = row.get("amount_value")?;
        let amount_currency: String = row.get("amount_currency")?;
        let notes: Option<String> = row.get("notes")?;
        let created_at_str: String = row.get("created_at")?;

        let amount = Decimal::from_str(&amount_value).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let currency = CurrencyCode::from_str(&amount_currency).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;

        Ok(TransactionSplit {
            id,
            transaction_id,
            category_id,
            amount: Money::new(amount, currency),
            notes,
            created_at,
        })
    }
}

impl TransactionSplitRepository for SqliteTransactionSplitRepository<'_> {
    fn save(&self, s: &TransactionSplit) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "INSERT INTO transaction_splits (
                    id, transaction_id, category_id,
                    amount_value, amount_currency, notes, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    s.id,
                    s.transaction_id,
                    s.category_id,
                    s.amount.amount.to_string(),
                    s.amount.currency.to_string(),
                    s.notes,
                    s.created_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save transaction_split: {e}")))?;
        Ok(())
    }

    fn find_by_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<Vec<TransactionSplit>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM transaction_splits WHERE transaction_id = ?1 ORDER BY created_at ASC")
            .map_err(|e| DomainError::Storage(format!("prepare find_by_transaction: {e}")))?
            .query_map([transaction_id], Self::row_to_split)
            .map_err(|e| DomainError::Storage(format!("find_by_transaction: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_by_transaction collect: {e}")))
    }

    fn delete_by_transaction(&self, transaction_id: &str) -> Result<usize, DomainError> {
        let affected = self
            .db
            .conn()
            .execute(
                "DELETE FROM transaction_splits WHERE transaction_id = ?1",
                [transaction_id],
            )
            .map_err(|e| DomainError::Storage(format!("delete_by_transaction: {e}")))?;
        Ok(affected)
    }

    fn delete(&self, id: &str) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute("DELETE FROM transaction_splits WHERE id = ?1", [id])
            .map_err(|e| DomainError::Storage(format!("delete split: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::{Account, AccountRepository as _, AccountType};
    use crate::domain::transaction::{Transaction, TransactionRepository as _};
    use crate::infrastructure::storage::{SqliteAccountRepository, SqliteTransactionRepository};
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn setup() -> (Database, String, String) {
        let db = Database::in_memory().unwrap();
        SqliteAccountRepository::new(&db)
            .save(
                &Account::new(
                    "acc-1".into(),
                    "Chase".into(),
                    AccountType::Checking,
                    CurrencyCode::USD,
                    "Sky".into(),
                )
                .unwrap(),
            )
            .unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO category_groups (id, name, created_at) VALUES ('g1', 'Grp', ?1)",
                [&now],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO categories (id, group_id, name, created_at) VALUES \
                 ('cat-food', 'g1', 'Food', ?1), ('cat-house', 'g1', 'House', ?1)",
                [&now],
            )
            .unwrap();

        let mut t = Transaction::new(
            "txn-1".into(),
            "acc-1".into(),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(dec!(-150), CurrencyCode::USD),
        )
        .unwrap();
        t.external_id = Some("ext-1".into());
        SqliteTransactionRepository::new(&db).save(&t).unwrap();

        (db, "cat-food".into(), "cat-house".into())
    }

    fn mk_split(txn: &str, cat: &str, amount: Decimal) -> TransactionSplit {
        TransactionSplit::new(
            uuid::Uuid::new_v4().to_string(),
            txn.into(),
            cat.into(),
            Money::new(amount, CurrencyCode::USD),
            None,
        )
        .unwrap()
    }

    #[test]
    fn save_and_find_by_transaction() {
        let (db, food, house) = setup();
        let repo = SqliteTransactionSplitRepository::new(&db);
        repo.save(&mk_split("txn-1", &food, dec!(-100))).unwrap();
        repo.save(&mk_split("txn-1", &house, dec!(-50))).unwrap();
        let found = repo.find_by_transaction("txn-1").unwrap();
        assert_eq!(found.len(), 2);
        let cats: Vec<&str> = found.iter().map(|s| s.category_id.as_str()).collect();
        assert!(cats.contains(&food.as_str()));
        assert!(cats.contains(&house.as_str()));
    }

    #[test]
    fn delete_by_transaction_returns_count() {
        let (db, food, house) = setup();
        let repo = SqliteTransactionSplitRepository::new(&db);
        repo.save(&mk_split("txn-1", &food, dec!(-100))).unwrap();
        repo.save(&mk_split("txn-1", &house, dec!(-50))).unwrap();
        assert_eq!(repo.delete_by_transaction("txn-1").unwrap(), 2);
        assert!(repo.find_by_transaction("txn-1").unwrap().is_empty());
    }

    #[test]
    fn cascade_delete_from_parent_transaction() {
        // Deleting the parent transaction removes its splits via FK ON DELETE CASCADE.
        let (db, food, _) = setup();
        let split_repo = SqliteTransactionSplitRepository::new(&db);
        split_repo.save(&mk_split("txn-1", &food, dec!(-150))).unwrap();

        SqliteTransactionRepository::new(&db).delete("txn-1").unwrap();

        assert!(split_repo.find_by_transaction("txn-1").unwrap().is_empty());
    }

    #[test]
    fn fk_rejects_unknown_category() {
        let (db, _, _) = setup();
        let repo = SqliteTransactionSplitRepository::new(&db);
        let s = mk_split("txn-1", "cat-ghost", dec!(-50));
        assert!(repo.save(&s).is_err());
    }

    #[test]
    fn fk_rejects_unknown_transaction() {
        let (db, food, _) = setup();
        let repo = SqliteTransactionSplitRepository::new(&db);
        let s = mk_split("txn-ghost", &food, dec!(-50));
        assert!(repo.save(&s).is_err());
    }
}
