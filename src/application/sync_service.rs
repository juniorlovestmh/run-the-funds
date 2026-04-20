//! Bank-sync orchestration: fetch via a `BankSyncAdapter`, filter to
//! locally-linked accounts, persist with FITID dedup, stamp `last_sync_at`.
//!
//! The service is generic over `AccountRepository` + `TransactionRepository`
//! so both production (SQLite) and tests (fake in-memory) use the same flow.
//! Credential loading is the CLI's responsibility — adapters arrive already
//! constructed.

use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::domain::account::AccountRepository;
use crate::domain::currency::Money;
use crate::domain::error::DomainError;
use crate::domain::transaction::{Transaction, TransactionRepository};
use crate::infrastructure::sync_adapter::BankSyncAdapter;

/// Number of days of history to pull on an account's very first sync when
/// no `--since` was specified and `last_sync_at` is still `None`.
const FIRST_SYNC_BACKFILL_DAYS: i64 = 730;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProviderSyncReport {
    pub provider: String,
    pub imported: usize,
    pub duplicates: usize,
    pub accounts_synced: usize,
    pub window_start: Option<NaiveDate>,
    pub window_end: Option<NaiveDate>,
}

pub struct SyncService<T: TransactionRepository, A: AccountRepository> {
    txn_repo: T,
    account_repo: A,
}

impl<T: TransactionRepository, A: AccountRepository> SyncService<T, A> {
    pub fn new(txn_repo: T, account_repo: A) -> Self {
        Self {
            txn_repo,
            account_repo,
        }
    }

    /// Run one provider's sync cycle. Returns `imported:0, duplicates:0, accounts_synced:0`
    /// (not an error) when no local accounts are linked to this provider —
    /// useful for a "run all providers" loop that shouldn't fail loudly on
    /// a not-yet-linked provider.
    pub fn sync_provider<B: BankSyncAdapter>(
        &self,
        adapter: &B,
        since_override: Option<NaiveDate>,
    ) -> Result<ProviderSyncReport, DomainError> {
        let provider_name = adapter.provider_name();
        let linked = self.account_repo.find_by_provider(provider_name)?;

        if linked.is_empty() {
            return Ok(ProviderSyncReport {
                provider: provider_name.into(),
                imported: 0,
                duplicates: 0,
                accounts_synced: 0,
                window_start: None,
                window_end: None,
            });
        }

        let today = Utc::now().date_naive();
        let default_since = today - chrono::Duration::days(FIRST_SYNC_BACKFILL_DAYS);

        // Per-account effective since: --since wins; else account.last_sync_at;
        // else default (2 years back). Keyed by external_account_id so we can
        // look up quickly when matching remote results.
        let mut per_account_since: HashMap<String, NaiveDate> = HashMap::new();
        for acc in &linked {
            if let Some(ext) = &acc.external_account_id {
                let since = since_override
                    .or_else(|| acc.last_sync_at.map(|t| t.date_naive()))
                    .unwrap_or(default_since);
                per_account_since.insert(ext.clone(), since);
            }
        }

        let earliest_since = per_account_since
            .values()
            .min()
            .copied()
            .unwrap_or(default_since);

        let remote_results = adapter.sync(Some(earliest_since))?;

        let sync_ts = Utc::now();
        let mut total_imported = 0;
        let mut total_duplicates = 0;
        let mut accounts_synced = 0;

        for (external_id, remote_txns) in remote_results {
            let mut account = match self
                .account_repo
                .find_by_external_link(provider_name, &external_id)?
            {
                Some(a) => a,
                None => continue, // Not linked locally — skip without error.
            };

            let effective_since = per_account_since
                .get(&external_id)
                .copied()
                .unwrap_or(default_since);

            // Filter to the account's own since window + convert to domain txns.
            let mut txns: Vec<Transaction> = Vec::with_capacity(remote_txns.len());
            for r in remote_txns {
                if r.date < effective_since {
                    continue;
                }
                let money = Money::new(r.amount, r.currency);
                let mut t = Transaction::new(
                    Uuid::new_v4().to_string(),
                    account.id.clone(),
                    r.date,
                    money,
                )?;
                t.external_id = Some(r.external_id);
                t.payee = r.payee;
                t.description = r.description;
                t.imported_at = Some(sync_ts);
                txns.push(t);
            }

            // Currency validation (two-pass) — abort this account on mismatch.
            for t in &txns {
                if t.amount.currency != account.currency {
                    return Err(DomainError::CurrencyMismatch {
                        expected: account.currency.to_string(),
                        got: t.amount.currency.to_string(),
                    });
                }
            }

            // Dedup + persist.
            for t in txns {
                if let Some(ext) = t.external_id.as_deref() {
                    if self
                        .txn_repo
                        .find_by_external_id(&account.id, ext)?
                        .is_some()
                    {
                        total_duplicates += 1;
                        continue;
                    }
                }
                self.txn_repo.save(&t)?;
                total_imported += 1;
            }

            account.mark_synced(sync_ts);
            self.account_repo.save(&account)?;
            accounts_synced += 1;
        }

        Ok(ProviderSyncReport {
            provider: provider_name.into(),
            imported: total_imported,
            duplicates: total_duplicates,
            accounts_synced,
            window_start: Some(earliest_since),
            window_end: Some(today),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::{Account, AccountType};
    use crate::domain::currency::CurrencyCode;
    use crate::infrastructure::sync_adapter::RemoteTransaction;
    use crate::infrastructure::storage::{
        Database, SqliteAccountRepository, SqliteTransactionRepository,
    };
    use rust_decimal_macros::dec;
    use std::cell::RefCell;

    struct FakeAdapter {
        results: Vec<(String, Vec<RemoteTransaction>)>,
        last_since: RefCell<Option<NaiveDate>>,
        calls: RefCell<usize>,
    }

    impl FakeAdapter {
        fn new(results: Vec<(String, Vec<RemoteTransaction>)>) -> Self {
            Self {
                results,
                last_since: RefCell::new(None),
                calls: RefCell::new(0),
            }
        }
    }

    impl BankSyncAdapter for FakeAdapter {
        fn provider_name(&self) -> &'static str {
            "simplefin"
        }
        fn sync(
            &self,
            since: Option<NaiveDate>,
        ) -> Result<Vec<(String, Vec<RemoteTransaction>)>, DomainError> {
            *self.last_since.borrow_mut() = since;
            *self.calls.borrow_mut() += 1;
            Ok(self.results.clone())
        }
    }

    fn setup_db() -> Database {
        Database::in_memory().unwrap()
    }

    fn make_linked_account(
        db: &Database,
        local_id: &str,
        name: &str,
        currency: CurrencyCode,
        provider: &str,
        ext_id: &str,
    ) -> Account {
        let repo = SqliteAccountRepository::new(db);
        let mut acc = Account::new(
            local_id.into(),
            name.into(),
            AccountType::Checking,
            currency,
            "Sky".into(),
        )
        .unwrap();
        acc.link(provider.into(), ext_id.into()).unwrap();
        repo.save(&acc).unwrap();
        acc
    }

    fn service(db: &Database) -> SyncService<SqliteTransactionRepository<'_>, SqliteAccountRepository<'_>> {
        SyncService::new(
            SqliteTransactionRepository::new(db),
            SqliteAccountRepository::new(db),
        )
    }

    fn rtx(ext_id: &str, date: (i32, u32, u32), amount: rust_decimal::Decimal, currency: CurrencyCode) -> RemoteTransaction {
        RemoteTransaction {
            external_id: ext_id.into(),
            date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            amount,
            currency,
            payee: None,
            description: None,
        }
    }

    #[test]
    fn sync_no_linked_accounts_returns_empty_report() {
        let db = setup_db();
        let svc = service(&db);
        let adapter = FakeAdapter::new(vec![]);
        let report = svc.sync_provider(&adapter, None).unwrap();
        assert_eq!(report.provider, "simplefin");
        assert_eq!(report.accounts_synced, 0);
        assert_eq!(report.imported, 0);
        assert_eq!(*adapter.calls.borrow(), 0, "no adapter call when nothing linked");
    }

    #[test]
    fn sync_persists_and_updates_last_sync_at() {
        let db = setup_db();
        make_linked_account(&db, "acc-1", "Chase", CurrencyCode::USD, "simplefin", "ext-1");
        let svc = service(&db);

        let adapter = FakeAdapter::new(vec![(
            "ext-1".into(),
            vec![
                rtx("r1", (2026, 4, 1), dec!(-10.00), CurrencyCode::USD),
                rtx("r2", (2026, 4, 2), dec!(100.00), CurrencyCode::USD),
            ],
        )]);
        let report = svc.sync_provider(&adapter, None).unwrap();
        assert_eq!(report.imported, 2);
        assert_eq!(report.duplicates, 0);
        assert_eq!(report.accounts_synced, 1);

        let repo = SqliteAccountRepository::new(&db);
        let acc = repo.find_by_id("acc-1").unwrap().unwrap();
        assert!(acc.last_sync_at.is_some());
    }

    #[test]
    fn sync_default_since_is_two_years_back() {
        let db = setup_db();
        make_linked_account(&db, "acc-1", "Chase", CurrencyCode::USD, "simplefin", "ext-1");
        let svc = service(&db);
        let adapter = FakeAdapter::new(vec![]);
        svc.sync_provider(&adapter, None).unwrap();
        let since = adapter.last_since.borrow().unwrap();
        let today = Utc::now().date_naive();
        let diff = (today - since).num_days();
        assert!((725..=735).contains(&diff), "expected ~730 days, got {diff}");
    }

    #[test]
    fn sync_since_override_wins_over_last_sync_at() {
        let db = setup_db();
        let mut acc = make_linked_account(&db, "acc-1", "Chase", CurrencyCode::USD, "simplefin", "ext-1");
        // Simulate a prior sync: last_sync_at = 2026-04-10.
        acc.mark_synced(
            NaiveDate::from_ymd_opt(2026, 4, 10)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc(),
        );
        SqliteAccountRepository::new(&db).save(&acc).unwrap();

        let svc = service(&db);
        let adapter = FakeAdapter::new(vec![]);
        let override_date = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        svc.sync_provider(&adapter, Some(override_date)).unwrap();
        assert_eq!(*adapter.last_since.borrow(), Some(override_date));
    }

    #[test]
    fn sync_only_persists_linked_external_accounts() {
        // Adapter returns 3 external accounts; only 2 are locally linked.
        let db = setup_db();
        make_linked_account(&db, "acc-1", "Chase", CurrencyCode::USD, "simplefin", "ext-1");
        make_linked_account(&db, "acc-2", "Cap1", CurrencyCode::USD, "simplefin", "ext-2");
        // ext-3 exists at the provider but isn't linked locally.
        let svc = service(&db);

        let adapter = FakeAdapter::new(vec![
            ("ext-1".into(), vec![rtx("r1", (2026, 4, 1), dec!(-10.00), CurrencyCode::USD)]),
            ("ext-2".into(), vec![rtx("r2", (2026, 4, 2), dec!(-20.00), CurrencyCode::USD)]),
            ("ext-3".into(), vec![rtx("r3", (2026, 4, 3), dec!(-30.00), CurrencyCode::USD)]),
        ]);
        let report = svc.sync_provider(&adapter, None).unwrap();
        assert_eq!(report.accounts_synced, 2);
        assert_eq!(report.imported, 2);
    }

    #[test]
    fn sync_dedups_on_rerun() {
        // Use an explicit since on both runs so the post-first-sync
        // last_sync_at advancement doesn't filter out the rows before dedup
        // can see them. (In production, the adapter honors `since` and
        // simply wouldn't return old rows on a second incremental call —
        // but with a fake that returns the same static list, we need to
        // override `since` to let dedup run.)
        let db = setup_db();
        make_linked_account(&db, "acc-1", "Chase", CurrencyCode::USD, "simplefin", "ext-1");
        let svc = service(&db);
        let adapter = FakeAdapter::new(vec![(
            "ext-1".into(),
            vec![
                rtx("r1", (2026, 4, 1), dec!(-10.00), CurrencyCode::USD),
                rtx("r2", (2026, 4, 2), dec!(-20.00), CurrencyCode::USD),
            ],
        )]);
        let override_date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();

        let first = svc.sync_provider(&adapter, Some(override_date)).unwrap();
        assert_eq!(first.imported, 2);
        assert_eq!(first.duplicates, 0);

        let second = svc.sync_provider(&adapter, Some(override_date)).unwrap();
        assert_eq!(second.imported, 0);
        assert_eq!(second.duplicates, 2);
    }

    #[test]
    fn sync_currency_mismatch_surfaces_as_error() {
        // Linked BRL account but adapter returns USD transactions.
        let db = setup_db();
        make_linked_account(&db, "acc-1", "Nubank", CurrencyCode::BRL, "simplefin", "ext-1");
        let svc = service(&db);
        let adapter = FakeAdapter::new(vec![(
            "ext-1".into(),
            vec![rtx("r1", (2026, 4, 1), dec!(-10.00), CurrencyCode::USD)],
        )]);
        let err = svc.sync_provider(&adapter, None).unwrap_err();
        assert!(matches!(err, DomainError::CurrencyMismatch { .. }));
    }

    #[test]
    fn sync_filters_transactions_before_effective_since() {
        let db = setup_db();
        make_linked_account(&db, "acc-1", "Chase", CurrencyCode::USD, "simplefin", "ext-1");
        let svc = service(&db);

        let override_date = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let adapter = FakeAdapter::new(vec![(
            "ext-1".into(),
            vec![
                rtx("old", (2026, 4, 1), dec!(-5.00), CurrencyCode::USD), // before cutoff
                rtx("new", (2026, 4, 15), dec!(-10.00), CurrencyCode::USD),
            ],
        )]);
        let report = svc.sync_provider(&adapter, Some(override_date)).unwrap();
        assert_eq!(report.imported, 1);
    }
}
