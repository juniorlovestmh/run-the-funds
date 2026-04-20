//! S06 T03 — Monarch → rtf sync orchestration.
//!
//! Three-phase import (taxonomy → accounts → transactions) with external_id
//! upsert lookups so re-running the sync is idempotent. Monarch is the owner
//! of the synced entities; locally-created categories/groups/tags/accounts
//! (those without external_id + external_provider) are untouched.
//!
//! The service takes a constructed `MonarchAdapter<R>` so tests can inject a
//! FakeMmoneyRunner and production can use the subprocess runner without any
//! change to this code.

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::domain::account::{Account, AccountRepository, AccountType};
use crate::domain::category::{Category, CategoryGroup, CategoryRepository};
use crate::domain::currency::Money;
use crate::domain::error::DomainError;
use crate::domain::tag::{Tag, TagRepository};
use crate::domain::transaction::{Transaction, TransactionRepository};
use crate::infrastructure::sync_adapter::monarch::{MmoneyRunner, MonarchAdapter};

pub const MONARCH_PROVIDER: &str = "monarch";

/// Default history window (days) for a first-time Monarch sync.
const DEFAULT_BACKFILL_DAYS: i64 = 730;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MonarchSyncReport {
    pub category_groups_imported: usize,
    pub categories_imported: usize,
    pub tags_imported: usize,
    pub accounts_imported: usize,
    pub transactions_imported: usize,
    pub duplicates_skipped: usize,
    pub transactions_updated: usize,
    pub window_start: NaiveDate,
    pub window_end: NaiveDate,
}

pub struct MonarchSyncService<T, A, C, Tg>
where
    T: TransactionRepository,
    A: AccountRepository,
    C: CategoryRepository,
    Tg: TagRepository,
{
    txn_repo: T,
    account_repo: A,
    category_repo: C,
    tag_repo: Tg,
}

impl<T, A, C, Tg> MonarchSyncService<T, A, C, Tg>
where
    T: TransactionRepository,
    A: AccountRepository,
    C: CategoryRepository,
    Tg: TagRepository,
{
    pub fn new(txn_repo: T, account_repo: A, category_repo: C, tag_repo: Tg) -> Self {
        Self {
            txn_repo,
            account_repo,
            category_repo,
            tag_repo,
        }
    }

    pub fn run<R: MmoneyRunner>(
        &self,
        adapter: &MonarchAdapter<R>,
        since_override: Option<NaiveDate>,
    ) -> Result<MonarchSyncReport, DomainError> {
        let today = Utc::now().date_naive();
        let since = since_override
            .unwrap_or_else(|| today - chrono::Duration::days(DEFAULT_BACKFILL_DAYS));

        // ---- Phase 1: taxonomy -----------------------------------------------
        // Order matters: groups before categories (categories FK groups), either
        // order for tags. Build external→local id lookups as we go.

        let mut groups_imported = 0usize;
        let mut group_ext_to_local: HashMap<String, String> = HashMap::new();
        for g in adapter.fetch_category_groups()? {
            let local_id = match self
                .category_repo
                .find_group_by_external(MONARCH_PROVIDER, &g.external_id)?
            {
                Some(existing) => {
                    // Upsert: name or other fields may have changed.
                    let updated = CategoryGroup::from_external(
                        existing.id.clone(),
                        g.name.clone(),
                        MONARCH_PROVIDER.into(),
                        g.external_id.clone(),
                    )?;
                    self.category_repo.save_group(&updated)?;
                    existing.id
                }
                None => {
                    let local_id = Uuid::new_v4().to_string();
                    let new = CategoryGroup::from_external(
                        local_id.clone(),
                        g.name.clone(),
                        MONARCH_PROVIDER.into(),
                        g.external_id.clone(),
                    )?;
                    self.category_repo.save_group(&new)?;
                    groups_imported += 1;
                    local_id
                }
            };
            group_ext_to_local.insert(g.external_id, local_id);
        }

        let mut cats_imported = 0usize;
        let mut category_ext_to_local: HashMap<String, String> = HashMap::new();
        for c in adapter.fetch_categories()? {
            let local_group_id = match group_ext_to_local.get(&c.group_external_id) {
                Some(id) => id.clone(),
                None => {
                    // Orphan — Monarch returned a category whose group wasn't in
                    // the earlier groups call. Skip with no error; the rare edge
                    // shouldn't fail the whole sync.
                    continue;
                }
            };
            let local_id = match self
                .category_repo
                .find_category_by_external(MONARCH_PROVIDER, &c.external_id)?
            {
                Some(existing) => {
                    let updated = Category::from_external(
                        existing.id.clone(),
                        local_group_id,
                        c.name.clone(),
                        MONARCH_PROVIDER.into(),
                        c.external_id.clone(),
                    )?;
                    self.category_repo.save_category(&updated)?;
                    existing.id
                }
                None => {
                    let local_id = Uuid::new_v4().to_string();
                    let new = Category::from_external(
                        local_id.clone(),
                        local_group_id,
                        c.name.clone(),
                        MONARCH_PROVIDER.into(),
                        c.external_id.clone(),
                    )?;
                    self.category_repo.save_category(&new)?;
                    cats_imported += 1;
                    local_id
                }
            };
            category_ext_to_local.insert(c.external_id, local_id);
        }

        let mut tags_imported = 0usize;
        let mut tag_ext_to_local: HashMap<String, String> = HashMap::new();
        for t in adapter.fetch_tags()? {
            let local_id = match self
                .tag_repo
                .find_by_external(MONARCH_PROVIDER, &t.external_id)?
            {
                Some(existing) => {
                    let mut updated = Tag::from_external(
                        existing.id.clone(),
                        t.name.clone(),
                        MONARCH_PROVIDER.into(),
                        t.external_id.clone(),
                        t.color.clone(),
                        t.order,
                    )?;
                    updated.created_at = existing.created_at;
                    self.tag_repo.save(&updated)?;
                    existing.id
                }
                None => {
                    let local_id = Uuid::new_v4().to_string();
                    let new = Tag::from_external(
                        local_id.clone(),
                        t.name.clone(),
                        MONARCH_PROVIDER.into(),
                        t.external_id.clone(),
                        t.color.clone(),
                        t.order,
                    )?;
                    self.tag_repo.save(&new)?;
                    tags_imported += 1;
                    local_id
                }
            };
            tag_ext_to_local.insert(t.external_id, local_id);
        }

        // ---- Phase 2: accounts -----------------------------------------------

        let mut accounts_imported = 0usize;
        let mut account_ext_to_local: HashMap<String, String> = HashMap::new();
        for a in adapter.fetch_accounts()? {
            let account_type = monarch_to_account_type(&a.account_type, &a.subtype);
            let local_id = match self
                .account_repo
                .find_by_external_link(MONARCH_PROVIDER, &a.external_id)?
            {
                Some(mut existing) => {
                    // Update the mutable display fields; keep local-generated fields alone.
                    existing.name = a.display_name.clone();
                    existing.account_type = account_type;
                    existing.currency = a.currency;
                    existing.institution = a.institution_name.clone();
                    existing.balance = Money::new(a.current_balance, a.currency);
                    existing.updated_at = Utc::now();
                    self.account_repo.save(&existing)?;
                    existing.id
                }
                None => {
                    let local_id = Uuid::new_v4().to_string();
                    let mut new = Account::new(
                        local_id.clone(),
                        a.display_name.clone(),
                        account_type,
                        a.currency,
                        "Monarch".into(),
                    )?;
                    new.institution = a.institution_name.clone();
                    new.balance = Money::new(a.current_balance, a.currency);
                    new.link(MONARCH_PROVIDER.into(), a.external_id.clone())?;
                    self.account_repo.save(&new)?;
                    accounts_imported += 1;
                    local_id
                }
            };
            account_ext_to_local.insert(a.external_id, local_id);
        }

        // ---- Phase 3: transactions ------------------------------------------

        let sync_ts = Utc::now();
        let mut tx_imported = 0usize;
        let mut tx_updated = 0usize;
        let mut duplicates_skipped = 0usize;

        let monarch_txns = adapter.fetch_transactions(since, today)?;
        for mt in monarch_txns {
            let local_account_id = match account_ext_to_local.get(&mt.account_external_id) {
                Some(id) => id.clone(),
                None => {
                    // Transaction for an account we didn't import (e.g. hidden
                    // or outside our fetch window). Skip without error.
                    continue;
                }
            };

            // Determine the fintrack txn id: reuse existing on (account, external) match.
            let existing =
                self.txn_repo
                    .find_by_external_id(&local_account_id, &mt.external_id)?;
            let local_txn_id = existing
                .as_ref()
                .map(|e| e.id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let is_new = existing.is_none();

            let money = Money::new(mt.amount, mt.currency);
            let mut t = Transaction::new(
                local_txn_id.clone(),
                local_account_id.clone(),
                mt.date,
                money,
            )?;
            t.external_id = Some(mt.external_id.clone());
            t.payee = mt.merchant_name.clone();
            t.description = mt
                .notes
                .clone()
                .or_else(|| mt.plaid_name.clone());
            t.imported_at = Some(sync_ts);

            // Resolve Monarch's category external id to fintrack's local id.
            t.category_id = mt
                .category_external_id
                .as_ref()
                .and_then(|ext| category_ext_to_local.get(ext).cloned());

            // Detect whether this upsert changes anything semantic.
            let semantic_changed = match &existing {
                Some(e) => {
                    e.payee != t.payee
                        || e.description != t.description
                        || e.category_id != t.category_id
                        || e.amount != t.amount
                        || e.date != t.date
                }
                None => true,
            };

            self.txn_repo.save(&t)?;

            // Resolve tag ids and apply (replaces existing set).
            let local_tag_ids: Vec<String> = mt
                .tag_external_ids
                .iter()
                .filter_map(|ext| tag_ext_to_local.get(ext).cloned())
                .collect();
            self.tag_repo
                .set_tags_for_transaction(&local_txn_id, &local_tag_ids)?;

            if is_new {
                tx_imported += 1;
            } else if semantic_changed {
                tx_updated += 1;
            } else {
                duplicates_skipped += 1;
            }
        }

        // Advance last_sync_at on every imported account.
        for local_id in account_ext_to_local.values() {
            if let Some(mut acc) = self.account_repo.find_by_id(local_id)? {
                acc.mark_synced(sync_ts);
                self.account_repo.save(&acc)?;
            }
        }

        Ok(MonarchSyncReport {
            category_groups_imported: groups_imported,
            categories_imported: cats_imported,
            tags_imported,
            accounts_imported,
            transactions_imported: tx_imported,
            transactions_updated: tx_updated,
            duplicates_skipped,
            window_start: since,
            window_end: today,
        })
    }
}

/// Map Monarch's (type.name, subtype.name) pair to fintrack's AccountType.
/// Falls back to Other for anything unrecognized.
fn monarch_to_account_type(monarch_type: &str, subtype: &str) -> AccountType {
    // Prefer subtype when it cleanly parses; fall back to type.
    if let Ok(t) = AccountType::from_str(subtype) {
        return t;
    }
    match monarch_type.to_lowercase().as_str() {
        "depository" => AccountType::Checking,
        "credit" => AccountType::CreditCard,
        "loan" => AccountType::Loan,
        "brokerage" => AccountType::Brokerage,
        _ => AccountType::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::sync_adapter::monarch::MmoneyRunner;
    use crate::infrastructure::storage::{
        Database, SqliteAccountRepository, SqliteCategoryRepository, SqliteTagRepository,
        SqliteTransactionRepository,
    };
    use std::cell::RefCell;

    struct FakeRunner {
        responses: RefCell<Vec<(&'static str, String)>>,
    }

    impl FakeRunner {
        fn new(pairs: Vec<(&'static str, &str)>) -> Self {
            Self {
                responses: RefCell::new(
                    pairs.into_iter().map(|(k, v)| (k, v.to_string())).collect(),
                ),
            }
        }
    }

    impl MmoneyRunner for FakeRunner {
        fn run(&self, args: &[&str]) -> Result<Vec<u8>, DomainError> {
            let joined = args.join(" ");
            let mut r = self.responses.borrow_mut();
            if let Some(pos) = r.iter().position(|(k, _)| joined.starts_with(k)) {
                return Ok(r.remove(pos).1.into_bytes());
            }
            Err(DomainError::Import(format!(
                "fake runner has no response for: {joined}"
            )))
        }
    }

    const GROUPS: &str = r#"{
        "categoryGroups": [
            {"id": "g-income", "name": "Income", "type": "income", "order": 0},
            {"id": "g-food", "name": "Food & Dining", "type": "expense", "order": 5}
        ]
    }"#;

    const CATEGORIES: &str = r#"{
        "categories": [
            {"id": "c-pay", "name": "Paychecks", "group": {"id": "g-income"}, "isSystemCategory": true, "isDisabled": false},
            {"id": "c-groc", "name": "Groceries", "group": {"id": "g-food"}, "isSystemCategory": true, "isDisabled": false}
        ]
    }"#;

    const TAGS: &str = r##"{
        "householdTransactionTags": [
            {"id": "t-br", "name": "BR", "color": "#ff0000", "order": 0}
        ]
    }"##;

    const ACCOUNTS: &str = r#"{
        "accounts": [
            {
                "id": "a-chase",
                "displayName": "Chase Checking",
                "type": {"name": "depository"},
                "subtype": {"name": "checking"},
                "currentBalance": 1648.39,
                "isManual": false,
                "isHidden": false,
                "credential": {"institution": {"name": "Chase"}}
            }
        ]
    }"#;

    fn txns_json(rows: &[(&str, &str, f64, Option<&str>, Vec<&str>)]) -> String {
        let mut parts = Vec::new();
        for (id, date, amt, cat, tags) in rows {
            let cat_json = match cat {
                Some(c) => format!(r#","category":{{"id":"{c}"}}"#),
                None => String::new(),
            };
            let tag_rows: Vec<String> = tags.iter().map(|t| format!(r#"{{"id":"{t}"}}"#)).collect();
            parts.push(format!(
                r#"{{"id":"{id}","account":{{"id":"a-chase"}},"date":"{date}","amount":{amt},"merchant":{{"name":"Test Merchant"}}{cat_json},"tags":[{}]}}"#,
                tag_rows.join(",")
            ));
        }
        format!(
            r#"{{"allTransactions":{{"results":[{}]}}}}"#,
            parts.join(",")
        )
    }

    fn service(
        db: &Database,
    ) -> MonarchSyncService<
        SqliteTransactionRepository<'_>,
        SqliteAccountRepository<'_>,
        SqliteCategoryRepository<'_>,
        SqliteTagRepository<'_>,
    > {
        MonarchSyncService::new(
            SqliteTransactionRepository::new(db),
            SqliteAccountRepository::new(db),
            SqliteCategoryRepository::new(db),
            SqliteTagRepository::new(db),
        )
    }

    fn build_adapter(responses: Vec<(&'static str, &str)>) -> MonarchAdapter<FakeRunner> {
        MonarchAdapter::new(FakeRunner::new(responses))
    }

    #[test]
    fn imports_taxonomy_on_first_run() {
        let db = Database::in_memory().unwrap();
        let adapter = build_adapter(vec![
            ("categories groups", GROUPS),
            ("categories list", CATEGORIES),
            ("tags list", TAGS),
            ("accounts list", ACCOUNTS),
            (
                "transactions list",
                &txns_json(&[("tx-1", "2026-04-10", -12.34, Some("c-groc"), vec!["t-br"])]),
            ),
        ]);
        let svc = service(&db);

        let report = svc
            .run(
                &adapter,
                Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
            )
            .unwrap();
        assert_eq!(report.category_groups_imported, 2);
        assert_eq!(report.categories_imported, 2);
        assert_eq!(report.tags_imported, 1);
        assert_eq!(report.accounts_imported, 1);
        assert_eq!(report.transactions_imported, 1);
        assert_eq!(report.duplicates_skipped, 0);
    }

    #[test]
    fn rerun_is_idempotent() {
        let db = Database::in_memory().unwrap();

        let first_responses = vec![
            ("categories groups", GROUPS.to_string()),
            ("categories list", CATEGORIES.to_string()),
            ("tags list", TAGS.to_string()),
            ("accounts list", ACCOUNTS.to_string()),
            (
                "transactions list",
                txns_json(&[("tx-1", "2026-04-10", -12.34, Some("c-groc"), vec!["t-br"])]),
            ),
        ];
        let first_leaked: Vec<(&'static str, &str)> = first_responses
            .iter()
            .map(|(k, v)| (*k, Box::leak(v.clone().into_boxed_str()) as &'static str))
            .collect();
        let svc = service(&db);
        svc.run(
            &build_adapter(first_leaked),
            Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
        )
        .unwrap();

        // Second run — same data — should report zero new imports.
        let second_leaked: Vec<(&'static str, &str)> = first_responses
            .iter()
            .map(|(k, v)| (*k, Box::leak(v.clone().into_boxed_str()) as &'static str))
            .collect();
        let report2 = svc
            .run(
                &build_adapter(second_leaked),
                Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
            )
            .unwrap();
        assert_eq!(report2.category_groups_imported, 0);
        assert_eq!(report2.categories_imported, 0);
        assert_eq!(report2.tags_imported, 0);
        assert_eq!(report2.accounts_imported, 0);
        assert_eq!(report2.transactions_imported, 0);
        // Same-content upsert counts as a duplicate, not an update.
        assert_eq!(report2.duplicates_skipped, 1);
    }

    #[test]
    fn transaction_category_and_tag_resolve_to_local_ids() {
        let db = Database::in_memory().unwrap();
        let adapter = build_adapter(vec![
            ("categories groups", GROUPS),
            ("categories list", CATEGORIES),
            ("tags list", TAGS),
            ("accounts list", ACCOUNTS),
            (
                "transactions list",
                &txns_json(&[("tx-1", "2026-04-10", -12.34, Some("c-groc"), vec!["t-br"])]),
            ),
        ]);
        let svc = service(&db);
        svc.run(
            &adapter,
            Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
        )
        .unwrap();

        // Find the transaction and confirm its category is the local id that
        // maps from Monarch's "c-groc", and its tags include the local "BR".
        let repo = SqliteTransactionRepository::new(&db);
        let all = repo.find_by_account("").unwrap(); // "" returns nothing; use a broader fetch
        // Actually, the repo doesn't have a find_all; pull via raw SQL.
        let row: (String, Option<String>) = db
            .conn()
            .query_row(
                "SELECT id, category_id FROM transactions WHERE external_id = 'tx-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let _ = all;
        let (txn_id, category_id) = row;
        assert!(category_id.is_some(), "category should be resolved");

        let cat_repo = SqliteCategoryRepository::new(&db);
        let cat = cat_repo
            .find_category_by_id(&category_id.unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(cat.name, "Groceries");
        assert_eq!(cat.external_provider.as_deref(), Some("monarch"));
        assert_eq!(cat.external_id.as_deref(), Some("c-groc"));

        let tag_repo = SqliteTagRepository::new(&db);
        let tags = tag_repo.find_tags_for_transaction(&txn_id).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "BR");
    }

    #[test]
    fn transaction_without_category_leaves_category_null() {
        let db = Database::in_memory().unwrap();
        let adapter = build_adapter(vec![
            ("categories groups", GROUPS),
            ("categories list", CATEGORIES),
            ("tags list", TAGS),
            ("accounts list", ACCOUNTS),
            (
                "transactions list",
                &txns_json(&[("tx-1", "2026-04-10", -5.00, None, vec![])]),
            ),
        ]);
        let svc = service(&db);
        svc.run(
            &adapter,
            Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
        )
        .unwrap();
        let category_id: Option<String> = db
            .conn()
            .query_row(
                "SELECT category_id FROM transactions WHERE external_id = 'tx-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(category_id.is_none());
    }

    #[test]
    fn transaction_for_unknown_account_is_skipped() {
        let db = Database::in_memory().unwrap();
        let txn_json = r#"{"allTransactions":{"results":[
            {"id":"tx-ghost","account":{"id":"unknown-account"},"date":"2026-04-10","amount":-1.00,"merchant":{"name":"?"}}
        ]}}"#;
        let adapter = build_adapter(vec![
            ("categories groups", GROUPS),
            ("categories list", CATEGORIES),
            ("tags list", TAGS),
            ("accounts list", ACCOUNTS),
            ("transactions list", txn_json),
        ]);
        let svc = service(&db);
        let report = svc
            .run(
                &adapter,
                Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
            )
            .unwrap();
        assert_eq!(report.transactions_imported, 0);
    }

    #[test]
    fn account_upsert_updates_balance_and_name() {
        let db = Database::in_memory().unwrap();
        // First sync creates it.
        let first = build_adapter(vec![
            ("categories groups", GROUPS),
            ("categories list", CATEGORIES),
            ("tags list", TAGS),
            ("accounts list", ACCOUNTS),
            ("transactions list", &txns_json(&[])),
        ]);
        let svc = service(&db);
        svc.run(
            &first,
            Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
        )
        .unwrap();

        // Second sync: same external_id, different display name + balance.
        let updated_accounts = r#"{
            "accounts": [
                {
                    "id": "a-chase",
                    "displayName": "Chase Checking (renamed)",
                    "type": {"name": "depository"},
                    "subtype": {"name": "checking"},
                    "currentBalance": 9999.99,
                    "isManual": false,
                    "isHidden": false,
                    "credential": {"institution": {"name": "Chase"}}
                }
            ]
        }"#;
        let second = build_adapter(vec![
            ("categories groups", GROUPS),
            ("categories list", CATEGORIES),
            ("tags list", TAGS),
            ("accounts list", updated_accounts),
            ("transactions list", &txns_json(&[])),
        ]);
        svc.run(
            &second,
            Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()),
        )
        .unwrap();

        let repo = SqliteAccountRepository::new(&db);
        let acc = repo
            .find_by_external_link("monarch", "a-chase")
            .unwrap()
            .unwrap();
        assert_eq!(acc.name, "Chase Checking (renamed)");
        assert_eq!(
            acc.balance.amount,
            rust_decimal::Decimal::from_str("9999.99").unwrap()
        );
    }

    #[test]
    fn monarch_type_mapping() {
        assert_eq!(
            monarch_to_account_type("depository", "checking"),
            AccountType::Checking
        );
        assert_eq!(
            monarch_to_account_type("depository", "savings"),
            AccountType::Savings
        );
        assert_eq!(
            monarch_to_account_type("credit", "credit_card"),
            AccountType::CreditCard
        );
        assert_eq!(
            monarch_to_account_type("loan", "student"),
            AccountType::Loan
        );
        assert_eq!(
            monarch_to_account_type("brokerage", "brokerage"),
            AccountType::Brokerage
        );
        assert_eq!(
            monarch_to_account_type("brokerage", "cryptocurrency"),
            AccountType::Brokerage
        );
        assert_eq!(
            monarch_to_account_type("vehicle", "car"),
            AccountType::Other
        );
        assert_eq!(monarch_to_account_type("unknown_top", "wat"), AccountType::Other);
    }
}
