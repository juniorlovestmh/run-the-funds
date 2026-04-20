use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::domain::currency::{CurrencyCode, Money};
use crate::domain::error::DomainError;
use crate::domain::transaction::{Transaction, TransactionRepository, TransactionStatus};

use super::database::Database;

pub struct SqliteTransactionRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteTransactionRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_transaction(row: &rusqlite::Row) -> rusqlite::Result<Transaction> {
        let id: String = row.get("id")?;
        let account_id: String = row.get("account_id")?;
        let date_str: String = row.get("date")?;
        let amount_value_str: String = row.get("amount_value")?;
        let amount_currency_str: String = row.get("amount_currency")?;
        let payee: Option<String> = row.get("payee")?;
        let description: Option<String> = row.get("description")?;
        let category_id: Option<String> = row.get("category_id")?;
        let beneficiary_id: Option<String> = row.get("beneficiary_id")?;
        let transfer_pair_id: Option<String> = row.get("transfer_pair_id")?;
        let status_str: String = row.get("status")?;
        let external_id: Option<String> = row.get("external_id")?;
        let imported_at_str: Option<String> = row.get("imported_at")?;
        let created_at_str: String = row.get("created_at")?;
        let updated_at_str: String = row.get("updated_at")?;

        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let amount_value = Decimal::from_str(&amount_value_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let amount_currency = CurrencyCode::from_str(&amount_currency_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let status = TransactionStatus::from_str(&status_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::from(e))
        })?;

        let parse_dt = |s: &str| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::from(e),
                    )
                })
        };

        let imported_at = imported_at_str.as_deref().map(parse_dt).transpose()?;
        let created_at = parse_dt(&created_at_str)?;
        let updated_at = parse_dt(&updated_at_str)?;

        Ok(Transaction {
            id,
            account_id,
            date,
            amount: Money::new(amount_value, amount_currency),
            payee,
            description,
            category_id,
            beneficiary_id,
            transfer_pair_id,
            status,
            external_id,
            imported_at,
            created_at,
            updated_at,
        })
    }
}

impl TransactionRepository for SqliteTransactionRepository<'_> {
    fn save(&self, txn: &Transaction) -> Result<(), DomainError> {
        // ON CONFLICT(id) DO UPDATE preserves the partial unique index on
        // (account_id, external_id) while still allowing field updates on the
        // same PK (e.g. `categorize` setting category_id on an existing row).
        self.db
            .conn()
            .execute(
                "INSERT INTO transactions (
                    id, account_id, date, amount_value, amount_currency,
                    payee, description, category_id, beneficiary_id,
                    transfer_pair_id, status, external_id, imported_at,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                ON CONFLICT(id) DO UPDATE SET
                    account_id = excluded.account_id,
                    date = excluded.date,
                    amount_value = excluded.amount_value,
                    amount_currency = excluded.amount_currency,
                    payee = excluded.payee,
                    description = excluded.description,
                    category_id = excluded.category_id,
                    beneficiary_id = excluded.beneficiary_id,
                    transfer_pair_id = excluded.transfer_pair_id,
                    status = excluded.status,
                    external_id = excluded.external_id,
                    imported_at = excluded.imported_at,
                    updated_at = excluded.updated_at",
                rusqlite::params![
                    txn.id,
                    txn.account_id,
                    txn.date.format("%Y-%m-%d").to_string(),
                    txn.amount.amount.to_string(),
                    txn.amount.currency.to_string(),
                    txn.payee,
                    txn.description,
                    txn.category_id,
                    txn.beneficiary_id,
                    txn.transfer_pair_id,
                    txn.status.to_string(),
                    txn.external_id,
                    txn.imported_at.map(|dt| dt.to_rfc3339()),
                    txn.created_at.to_rfc3339(),
                    txn.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save transaction: {e}")))?;
        Ok(())
    }

    fn find_by_id(&self, id: &str) -> Result<Option<Transaction>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM transactions WHERE id = ?1")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([id], Self::row_to_transaction)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_id: {e}")))
    }

    fn find_by_account(&self, account_id: &str) -> Result<Vec<Transaction>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM transactions WHERE account_id = ?1 ORDER BY date DESC")
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map([account_id], Self::row_to_transaction)
            .map_err(|e| DomainError::Storage(format!("query: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("collect: {e}")))
    }

    fn find_by_date_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM transactions WHERE date >= ?1 AND date <= ?2 ORDER BY date DESC",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_map(
                [
                    start.format("%Y-%m-%d").to_string(),
                    end.format("%Y-%m-%d").to_string(),
                ],
                Self::row_to_transaction,
            )
            .map_err(|e| DomainError::Storage(format!("query: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("collect: {e}")))
    }

    fn find_by_external_id(
        &self,
        account_id: &str,
        external_id: &str,
    ) -> Result<Option<Transaction>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM transactions \
                 WHERE account_id = ?1 AND external_id = ?2 LIMIT 1",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row([account_id, external_id], Self::row_to_transaction)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_external_id: {e}")))
    }

    fn find_uncategorized(
        &self,
        account_id: Option<&str>,
    ) -> Result<Vec<Transaction>, DomainError> {
        let (sql, params): (&str, Vec<String>) = match account_id {
            Some(acc) => (
                "SELECT * FROM transactions WHERE category_id IS NULL AND account_id = ?1 ORDER BY date DESC",
                vec![acc.to_string()],
            ),
            None => (
                "SELECT * FROM transactions WHERE category_id IS NULL ORDER BY date DESC",
                Vec::new(),
            ),
        };
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| DomainError::Storage(format!("prepare find_uncategorized: {e}")))?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter()),
                Self::row_to_transaction,
            )
            .map_err(|e| DomainError::Storage(format!("find_uncategorized: {e}")))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_uncategorized collect: {e}")))
    }

    fn find_untagged(&self) -> Result<Vec<Transaction>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM transactions \
                 WHERE category_id IS NULL AND transfer_pair_id IS NULL \
                 ORDER BY date ASC, id ASC",
            )
            .map_err(|e| DomainError::Storage(format!("prepare find_untagged: {e}")))?
            .query_map([], Self::row_to_transaction)
            .map_err(|e| DomainError::Storage(format!("find_untagged: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_untagged collect: {e}")))
    }

    fn clear_categories(&self, account_id: Option<&str>) -> Result<usize, DomainError> {
        let conn = self.db.conn();
        let affected = match account_id {
            Some(acc) => conn
                .execute(
                    "UPDATE transactions SET category_id = NULL, updated_at = ?1 \
                     WHERE category_id IS NOT NULL AND account_id = ?2",
                    rusqlite::params![chrono::Utc::now().to_rfc3339(), acc],
                )
                .map_err(|e| DomainError::Storage(format!("clear_categories: {e}")))?,
            None => conn
                .execute(
                    "UPDATE transactions SET category_id = NULL, updated_at = ?1 \
                     WHERE category_id IS NOT NULL",
                    [chrono::Utc::now().to_rfc3339()],
                )
                .map_err(|e| DomainError::Storage(format!("clear_categories: {e}")))?,
        };
        Ok(affected)
    }

    fn delete(&self, id: &str) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute("DELETE FROM transactions WHERE id = ?1", [id])
            .map_err(|e| DomainError::Storage(format!("delete: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::AccountRepository as _;
    use crate::domain::account::{Account, AccountType};
    use crate::infrastructure::storage::SqliteAccountRepository;
    use rust_decimal_macros::dec;

    fn setup() -> Database {
        let db = Database::in_memory().unwrap();
        let acc_repo = SqliteAccountRepository::new(&db);
        acc_repo
            .save(
                &Account::new(
                    "acc-001".into(),
                    "Chase".into(),
                    AccountType::Checking,
                    CurrencyCode::USD,
                    "Sky".into(),
                )
                .unwrap(),
            )
            .unwrap();
        acc_repo
            .save(
                &Account::new(
                    "acc-002".into(),
                    "Nubank".into(),
                    AccountType::Checking,
                    CurrencyCode::BRL,
                    "Sky".into(),
                )
                .unwrap(),
            )
            .unwrap();
        db
    }

    fn make_txn(
        id: &str,
        account_id: &str,
        date: (i32, u32, u32),
        amount: Decimal,
        currency: CurrencyCode,
    ) -> Transaction {
        Transaction::new(
            id.into(),
            account_id.into(),
            NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            Money::new(amount, currency),
        )
        .unwrap()
    }

    #[test]
    fn save_and_find_by_id() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);
        let txn = make_txn(
            "txn-001",
            "acc-001",
            (2026, 3, 15),
            dec!(-45.99),
            CurrencyCode::USD,
        );

        repo.save(&txn).unwrap();
        let found = repo.find_by_id("txn-001").unwrap().unwrap();

        assert_eq!(found.id, "txn-001");
        assert_eq!(found.account_id, "acc-001");
        assert_eq!(found.date, NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());
        assert_eq!(found.amount.amount, dec!(-45.99));
        assert_eq!(found.amount.currency, CurrencyCode::USD);
        assert_eq!(found.status, TransactionStatus::Pending);
    }

    #[test]
    fn find_by_id_not_found() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);
        assert!(repo.find_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn find_by_account() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        repo.save(&make_txn(
            "t1",
            "acc-001",
            (2026, 3, 10),
            dec!(-10.00),
            CurrencyCode::USD,
        ))
        .unwrap();
        repo.save(&make_txn(
            "t2",
            "acc-001",
            (2026, 3, 15),
            dec!(-20.00),
            CurrencyCode::USD,
        ))
        .unwrap();
        repo.save(&make_txn(
            "t3",
            "acc-002",
            (2026, 3, 12),
            dec!(-30.00),
            CurrencyCode::BRL,
        ))
        .unwrap();

        let chase_txns = repo.find_by_account("acc-001").unwrap();
        assert_eq!(chase_txns.len(), 2);
        assert_eq!(
            chase_txns[0].date,
            NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
        );
        assert_eq!(
            chase_txns[1].date,
            NaiveDate::from_ymd_opt(2026, 3, 10).unwrap()
        );
    }

    #[test]
    fn find_by_date_range() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        repo.save(&make_txn(
            "t1",
            "acc-001",
            (2026, 3, 1),
            dec!(-10.00),
            CurrencyCode::USD,
        ))
        .unwrap();
        repo.save(&make_txn(
            "t2",
            "acc-001",
            (2026, 3, 15),
            dec!(-20.00),
            CurrencyCode::USD,
        ))
        .unwrap();
        repo.save(&make_txn(
            "t3",
            "acc-001",
            (2026, 3, 31),
            dec!(-30.00),
            CurrencyCode::USD,
        ))
        .unwrap();
        repo.save(&make_txn(
            "t4",
            "acc-001",
            (2026, 4, 5),
            dec!(-40.00),
            CurrencyCode::USD,
        ))
        .unwrap();

        let march = repo
            .find_by_date_range(
                NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            )
            .unwrap();
        assert_eq!(march.len(), 3);
    }

    #[test]
    fn delete_transaction() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        repo.save(&make_txn(
            "t1",
            "acc-001",
            (2026, 3, 15),
            dec!(-10.00),
            CurrencyCode::USD,
        ))
        .unwrap();
        assert!(repo.find_by_id("t1").unwrap().is_some());

        repo.delete("t1").unwrap();
        assert!(repo.find_by_id("t1").unwrap().is_none());
    }

    #[test]
    fn roundtrip_optional_fields() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        let mut txn = make_txn(
            "t1",
            "acc-001",
            (2026, 3, 15),
            dec!(-99.99),
            CurrencyCode::USD,
        );
        txn.payee = Some("Costco".into());
        txn.description = Some("Weekly groceries".into());
        txn.external_id = Some("ext-123".into());
        txn.status = TransactionStatus::Cleared;
        txn.imported_at = Some(Utc::now());

        repo.save(&txn).unwrap();
        let found = repo.find_by_id("t1").unwrap().unwrap();

        assert_eq!(found.payee.as_deref(), Some("Costco"));
        assert_eq!(found.description.as_deref(), Some("Weekly groceries"));
        assert_eq!(found.external_id.as_deref(), Some("ext-123"));
        assert_eq!(found.status, TransactionStatus::Cleared);
        assert!(found.imported_at.is_some());
    }

    #[test]
    fn brl_transaction_roundtrip() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        let txn = make_txn(
            "t-brl",
            "acc-002",
            (2026, 3, 10),
            dec!(-150.50),
            CurrencyCode::BRL,
        );
        repo.save(&txn).unwrap();

        let found = repo.find_by_id("t-brl").unwrap().unwrap();
        assert_eq!(found.amount.amount, dec!(-150.50));
        assert_eq!(found.amount.currency, CurrencyCode::BRL);
    }

    #[test]
    fn foreign_key_enforced() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        let txn = make_txn(
            "t1",
            "nonexistent-account",
            (2026, 3, 15),
            dec!(-10.00),
            CurrencyCode::USD,
        );
        assert!(repo.save(&txn).is_err());
    }

    #[test]
    fn find_by_external_id_returns_none_when_absent() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);
        assert!(
            repo.find_by_external_id("acc-001", "fitid-missing")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn find_by_external_id_roundtrips() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        let mut txn = make_txn(
            "t1",
            "acc-001",
            (2026, 3, 15),
            dec!(-45.99),
            CurrencyCode::USD,
        );
        txn.external_id = Some("FITID-123".into());
        repo.save(&txn).unwrap();

        let found = repo
            .find_by_external_id("acc-001", "FITID-123")
            .unwrap()
            .expect("should have found the transaction");
        assert_eq!(found.id, "t1");
        assert_eq!(found.external_id.as_deref(), Some("FITID-123"));
    }

    #[test]
    fn find_by_external_id_scoped_to_account() {
        // Two accounts can legitimately share a FITID (different banks may issue
        // the same id). The lookup must scope by account.
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        let mut t1 = make_txn(
            "t1",
            "acc-001",
            (2026, 3, 15),
            dec!(-10.00),
            CurrencyCode::USD,
        );
        t1.external_id = Some("shared-fitid".into());
        repo.save(&t1).unwrap();

        let mut t2 = make_txn(
            "t2",
            "acc-002",
            (2026, 3, 15),
            dec!(-20.00),
            CurrencyCode::BRL,
        );
        t2.external_id = Some("shared-fitid".into());
        repo.save(&t2).unwrap();

        assert_eq!(
            repo.find_by_external_id("acc-001", "shared-fitid")
                .unwrap()
                .unwrap()
                .id,
            "t1"
        );
        assert_eq!(
            repo.find_by_external_id("acc-002", "shared-fitid")
                .unwrap()
                .unwrap()
                .id,
            "t2"
        );
    }

    #[test]
    fn unique_index_rejects_duplicate_external_id_in_same_account() {
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        let mut t1 = make_txn(
            "t1",
            "acc-001",
            (2026, 3, 15),
            dec!(-10.00),
            CurrencyCode::USD,
        );
        t1.external_id = Some("dup".into());
        repo.save(&t1).unwrap();

        // Different PK but same (account_id, external_id) must be rejected by the
        // partial unique index from migration 002.
        let mut t2 = make_txn(
            "t2",
            "acc-001",
            (2026, 3, 16),
            dec!(-99.99),
            CurrencyCode::USD,
        );
        t2.external_id = Some("dup".into());
        let err = repo.save(&t2).unwrap_err();
        assert!(matches!(err, DomainError::Storage(_)));
    }

    #[test]
    fn null_external_id_not_constrained() {
        // Manually-entered transactions (no FITID) must be allowed to coexist
        // under the same account — the partial index skips NULL rows.
        let db = setup();
        let repo = SqliteTransactionRepository::new(&db);

        let t1 = make_txn(
            "t1",
            "acc-001",
            (2026, 3, 15),
            dec!(-10.00),
            CurrencyCode::USD,
        );
        let t2 = make_txn(
            "t2",
            "acc-001",
            (2026, 3, 16),
            dec!(-20.00),
            CurrencyCode::USD,
        );
        assert!(t1.external_id.is_none());
        assert!(t2.external_id.is_none());

        repo.save(&t1).unwrap();
        repo.save(&t2).unwrap();

        assert_eq!(repo.find_by_account("acc-001").unwrap().len(), 2);
    }
}
