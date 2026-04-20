//! Application service for transactions.
//!
//! Owns the transaction + account repositories and orchestrates import flow:
//! looks up the destination account, validates adapter-reported currency
//! against the account's currency, and persists with FITID-based dedup via
//! `find_by_external_id`. Validation and persistence are separated into
//! two passes so a mid-file mismatch aborts the whole import with zero
//! rows persisted.

use std::path::Path;

use chrono::NaiveDate;
use serde::Serialize;

use crate::domain::account::AccountRepository;
use crate::domain::error::DomainError;
use crate::domain::transaction::{Transaction, TransactionRepository};
use crate::infrastructure::importer::{ImportError, Importer};

pub struct TransactionService<T: TransactionRepository, A: AccountRepository> {
    txn_repo: T,
    account_repo: A,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    pub imported: usize,
    pub duplicates: usize,
}

impl<T: TransactionRepository, A: AccountRepository> TransactionService<T, A> {
    pub fn new(txn_repo: T, account_repo: A) -> Self {
        Self {
            txn_repo,
            account_repo,
        }
    }

    pub fn save_transaction(&self, txn: &Transaction) -> Result<(), DomainError> {
        self.txn_repo.save(txn)
    }

    pub fn list_by_account(&self, account_id: &str) -> Result<Vec<Transaction>, DomainError> {
        self.txn_repo.find_by_account(account_id)
    }

    pub fn list_by_date_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<Transaction>, DomainError> {
        self.txn_repo.find_by_date_range(start, end)
    }

    pub fn get_transaction(&self, id: &str) -> Result<Transaction, DomainError> {
        self.txn_repo
            .find_by_id(id)?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Transaction".into(),
                id: id.into(),
            })
    }

    pub fn delete_transaction(&self, id: &str) -> Result<(), DomainError> {
        self.get_transaction(id)?;
        self.txn_repo.delete(id)
    }

    /// Parse `source` with `importer` and persist the resulting transactions
    /// into `account_id`. Thin wrapper over `persist_batch` that adds the
    /// file-reading step from an `Importer`.
    pub fn import_from<I: Importer>(
        &self,
        importer: &I,
        source: &Path,
        account_id: &str,
    ) -> Result<ImportReport, DomainError> {
        let parsed = importer.import(source, account_id).map_err(|e| match e {
            ImportError::Domain(d) => d,
            other => DomainError::Import(other.to_string()),
        })?;
        self.persist_batch(account_id, parsed)
    }

    /// Validate currency and persist a batch of transactions with per-account
    /// external_id dedup. Two-pass design: pass 1 validates every row's
    /// currency against the account (mismatch aborts with zero rows persisted);
    /// pass 2 persists, counting duplicates for rows whose external_id is
    /// already stored.
    ///
    /// This is the hot path for both file-based import (`import_from`) and
    /// network-based bank sync (`SyncService::sync_provider` in S04).
    pub fn persist_batch(
        &self,
        account_id: &str,
        transactions: Vec<Transaction>,
    ) -> Result<ImportReport, DomainError> {
        let account = self
            .account_repo
            .find_by_id(account_id)?
            .ok_or_else(|| DomainError::NotFound {
                entity: "Account".into(),
                id: account_id.into(),
            })?;

        for txn in &transactions {
            if txn.amount.currency != account.currency {
                return Err(DomainError::CurrencyMismatch {
                    expected: account.currency.to_string(),
                    got: txn.amount.currency.to_string(),
                });
            }
        }

        let mut imported = 0;
        let mut duplicates = 0;
        for txn in transactions {
            if let Some(ext) = txn.external_id.as_deref() {
                if self
                    .txn_repo
                    .find_by_external_id(account_id, ext)?
                    .is_some()
                {
                    duplicates += 1;
                    continue;
                }
            }
            self.txn_repo.save(&txn)?;
            imported += 1;
        }

        Ok(ImportReport {
            imported,
            duplicates,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::{Account, AccountRepository, AccountType};
    use crate::domain::currency::{CurrencyCode, Money};
    use crate::infrastructure::storage::{
        Database, SqliteAccountRepository, SqliteTransactionRepository,
    };
    use chrono::Utc;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::path::{Path, PathBuf};

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

    fn service(db: &Database) -> TransactionService<SqliteTransactionRepository<'_>, SqliteAccountRepository<'_>> {
        TransactionService::new(
            SqliteTransactionRepository::new(db),
            SqliteAccountRepository::new(db),
        )
    }

    #[test]
    fn list_by_account_empty() {
        let db = setup();
        assert!(service(&db).list_by_account("acc-001").unwrap().is_empty());
    }

    #[test]
    fn save_and_list() {
        let db = setup();
        let svc = service(&db);
        let txn = Transaction::new(
            "txn-001".into(),
            "acc-001".into(),
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            Money::new(dec!(-12.34), CurrencyCode::USD),
        )
        .unwrap();
        svc.save_transaction(&txn).unwrap();
        let listed = svc.list_by_account("acc-001").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "txn-001");
    }

    #[test]
    fn get_transaction_not_found() {
        let db = setup();
        let result = service(&db).get_transaction("nonexistent");
        assert!(matches!(result, Err(DomainError::NotFound { .. })));
    }

    #[test]
    fn delete_transaction() {
        let db = setup();
        let svc = service(&db);
        let txn = Transaction::new(
            "txn-001".into(),
            "acc-001".into(),
            NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            Money::new(dec!(-1.00), CurrencyCode::USD),
        )
        .unwrap();
        svc.save_transaction(&txn).unwrap();
        svc.delete_transaction("txn-001").unwrap();
        assert!(svc.list_by_account("acc-001").unwrap().is_empty());
    }

    // ---- import_from tests --------------------------------------------------

    /// Canned importer that returns pre-built transactions and never touches
    /// the filesystem. Keeps service tests free of fixture I/O coupling.
    struct FakeImporter {
        output: Vec<Transaction>,
    }

    impl Importer for FakeImporter {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn import(&self, _: &Path, _: &str) -> Result<Vec<Transaction>, ImportError> {
            Ok(self.output.clone())
        }
    }

    fn make_txn(id: &str, account: &str, fitid: &str, amount: Decimal, currency: CurrencyCode) -> Transaction {
        let mut t = Transaction::new(
            id.into(),
            account.into(),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(amount, currency),
        )
        .unwrap();
        t.external_id = Some(fitid.into());
        t.imported_at = Some(Utc::now());
        t
    }

    fn stub_path() -> PathBuf {
        PathBuf::from("/dev/null")
    }

    #[test]
    fn import_from_unknown_account_errors() {
        let db = setup();
        let svc = service(&db);
        let imp = FakeImporter { output: vec![] };
        let err = svc
            .import_from(&imp, &stub_path(), "no-such-account")
            .unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[test]
    fn import_from_currency_mismatch_aborts() {
        // Adapter produces a USD txn but the target account is BRL —
        // we expect a CurrencyMismatch and zero rows persisted.
        let db = setup();
        let svc = service(&db);
        let imp = FakeImporter {
            output: vec![make_txn(
                "a",
                "acc-002",
                "fit-1",
                dec!(-10.00),
                CurrencyCode::USD,
            )],
        };
        let err = svc.import_from(&imp, &stub_path(), "acc-002").unwrap_err();
        match err {
            DomainError::CurrencyMismatch { expected, got } => {
                assert_eq!(expected, "BRL");
                assert_eq!(got, "USD");
            }
            other => panic!("expected CurrencyMismatch, got {other:?}"),
        }
        assert!(svc.list_by_account("acc-002").unwrap().is_empty());
    }

    #[test]
    fn import_from_atomic_on_mismatch() {
        // Mismatch on the 2nd of 3 rows must leave ALL of them unpersisted.
        let db = setup();
        let svc = service(&db);
        let imp = FakeImporter {
            output: vec![
                make_txn("a", "acc-001", "f1", dec!(-10.00), CurrencyCode::USD),
                make_txn("b", "acc-001", "f2", dec!(-20.00), CurrencyCode::BRL), // bad
                make_txn("c", "acc-001", "f3", dec!(-30.00), CurrencyCode::USD),
            ],
        };
        let err = svc.import_from(&imp, &stub_path(), "acc-001").unwrap_err();
        assert!(matches!(err, DomainError::CurrencyMismatch { .. }));
        assert!(svc.list_by_account("acc-001").unwrap().is_empty());
    }

    #[test]
    fn import_from_persists_and_reports_counts() {
        let db = setup();
        let svc = service(&db);
        let imp = FakeImporter {
            output: vec![
                make_txn("a", "acc-001", "f1", dec!(-10.00), CurrencyCode::USD),
                make_txn("b", "acc-001", "f2", dec!(-20.00), CurrencyCode::USD),
                make_txn("c", "acc-001", "f3", dec!(-30.00), CurrencyCode::USD),
            ],
        };
        let report = svc.import_from(&imp, &stub_path(), "acc-001").unwrap();
        assert_eq!(report.imported, 3);
        assert_eq!(report.duplicates, 0);
        assert_eq!(svc.list_by_account("acc-001").unwrap().len(), 3);
    }

    #[test]
    fn import_from_dedup_counts_existing_fitids() {
        let db = setup();
        let svc = service(&db);

        // First import: seed two rows.
        svc.import_from(
            &FakeImporter {
                output: vec![
                    make_txn("a", "acc-001", "f1", dec!(-10.00), CurrencyCode::USD),
                    make_txn("b", "acc-001", "f2", dec!(-20.00), CurrencyCode::USD),
                ],
            },
            &stub_path(),
            "acc-001",
        )
        .unwrap();

        // Second import: f1/f2 are duplicates, f3 is new.
        let report = svc
            .import_from(
                &FakeImporter {
                    output: vec![
                        // Fresh UUIDs on the adapter-side txns — FITIDs are what dedup on.
                        make_txn("a2", "acc-001", "f1", dec!(-10.00), CurrencyCode::USD),
                        make_txn("b2", "acc-001", "f2", dec!(-20.00), CurrencyCode::USD),
                        make_txn("c", "acc-001", "f3", dec!(-30.00), CurrencyCode::USD),
                    ],
                },
                &stub_path(),
                "acc-001",
            )
            .unwrap();

        assert_eq!(report.imported, 1);
        assert_eq!(report.duplicates, 2);
        assert_eq!(svc.list_by_account("acc-001").unwrap().len(), 3);
    }

    #[test]
    fn import_from_adapter_error_maps_to_domain_import() {
        struct FailingImporter;
        impl Importer for FailingImporter {
            fn name(&self) -> &'static str {
                "fail"
            }
            fn import(&self, _: &Path, _: &str) -> Result<Vec<Transaction>, ImportError> {
                Err(ImportError::Parse {
                    format: "ofx",
                    detail: "boom".into(),
                })
            }
        }
        let db = setup();
        let svc = service(&db);
        let err = svc
            .import_from(&FailingImporter, &stub_path(), "acc-001")
            .unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }
}
