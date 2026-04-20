//! Spending rollup (S05 T05).
//!
//! Computes category / group / account totals from persisted transactions
//! within an optional date window. Splits expand into their per-line rows
//! (so a $150 Costco → groceries + household comes out as two category
//! contributions, not one parent-level line). Transfers are excluded by
//! default (`transfer_pair_id IS NOT NULL` → not spending).
//!
//! **MVP note:** single-currency reporting for now. If the account mix
//! spans currencies (USD + BRL), we return per-currency sub-totals rather
//! than converting — S03's CurrencyConverter is available for a follow-up
//! that adds cross-currency normalization.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::domain::category::CategoryRepository;
use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::domain::transaction::{
    Transaction, TransactionRepository, TransactionSplit, TransactionSplitRepository,
};

#[derive(Debug, Clone, Copy)]
pub struct SpendingOptions<'a> {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub exclude_transfers: bool,
    /// Scope to one account; None = every account.
    pub account_id: Option<&'a str>,
}

impl Default for SpendingOptions<'_> {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            exclude_transfers: true,
            account_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SpendingReport {
    /// Per-currency totals of everything the report considered (sum of all
    /// line amounts, signed). Lets a consumer see the final burn/inflow.
    pub totals_by_currency: Vec<CurrencyTotal>,
    /// Per-(currency, category) breakdown, sorted by `amount` ascending
    /// (most negative first — biggest expense categories lead).
    pub by_category: Vec<CategoryLine>,
    pub by_group: Vec<GroupLine>,
    pub by_account: Vec<AccountLine>,
    pub uncategorized: Vec<CurrencyTotal>,
    pub transfers_excluded: usize,
    pub transaction_count: usize,
    pub split_count: usize,
    pub window_start: Option<NaiveDate>,
    pub window_end: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CategoryLine {
    pub category_id: String,
    pub category_name: String,
    pub group_name: Option<String>,
    pub currency: CurrencyCode,
    pub amount: String, // stringified Decimal for precision
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GroupLine {
    pub group_id: String,
    pub group_name: String,
    pub currency: CurrencyCode,
    pub amount: String,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AccountLine {
    pub account_id: String,
    pub currency: CurrencyCode,
    pub amount: String,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CurrencyTotal {
    pub currency: CurrencyCode,
    pub amount: String,
    pub line_count: usize,
}

pub struct SpendingService<T, S, C>
where
    T: TransactionRepository,
    S: TransactionSplitRepository,
    C: CategoryRepository,
{
    txns: T,
    splits: S,
    categories: C,
}

impl<T, S, C> SpendingService<T, S, C>
where
    T: TransactionRepository,
    S: TransactionSplitRepository,
    C: CategoryRepository,
{
    pub fn new(txns: T, splits: S, categories: C) -> Self {
        Self {
            txns,
            splits,
            categories,
        }
    }

    pub fn compute(&self, opts: SpendingOptions) -> Result<SpendingReport, DomainError> {
        let (from, to) = date_window(opts.from, opts.to);
        let raw_txns = self.txns.find_by_date_range(from, to)?;

        // Filter to the requested account if scoped.
        let txns: Vec<Transaction> = raw_txns
            .into_iter()
            .filter(|t| match opts.account_id {
                Some(a) => t.account_id == a,
                None => true,
            })
            .collect();

        // Pre-load category + group metadata for name lookups.
        let all_categories = self.categories.find_all_categories()?;
        let all_groups = self.categories.find_all_groups()?;
        let cat_index: HashMap<String, (String, String)> = all_categories
            .iter()
            .map(|c| (c.id.clone(), (c.name.clone(), c.group_id.clone())))
            .collect();
        let group_index: HashMap<String, String> = all_groups
            .iter()
            .map(|g| (g.id.clone(), g.name.clone()))
            .collect();

        // Aggregation buckets, keyed on tuples so same category across
        // currencies stays separate.
        let mut cat_buckets: HashMap<(String, CurrencyCode), (Decimal, usize)> = HashMap::new();
        let mut group_buckets: HashMap<(String, CurrencyCode), (Decimal, usize)> = HashMap::new();
        let mut acc_buckets: HashMap<(String, CurrencyCode), (Decimal, usize)> = HashMap::new();
        let mut uncat_buckets: HashMap<CurrencyCode, (Decimal, usize)> = HashMap::new();
        let mut total_buckets: HashMap<CurrencyCode, (Decimal, usize)> = HashMap::new();

        let mut transfers_excluded = 0usize;
        let mut split_count = 0usize;
        let mut transaction_count = 0usize;

        for txn in &txns {
            // Skip transfers unless explicitly included.
            if opts.exclude_transfers && txn.transfer_pair_id.is_some() {
                transfers_excluded += 1;
                continue;
            }

            transaction_count += 1;

            // If the transaction has splits, use those as the lines;
            // otherwise the transaction itself is one line.
            let splits = self.splits.find_by_transaction(&txn.id)?;
            let lines: Vec<Line> = if !splits.is_empty() {
                split_count += splits.len();
                splits
                    .into_iter()
                    .map(|s| line_from_split(&s, &txn.account_id))
                    .collect()
            } else {
                vec![line_from_txn(txn)]
            };

            for line in lines {
                let total_key = line.currency;
                let entry = total_buckets.entry(total_key).or_insert((Decimal::ZERO, 0));
                entry.0 += line.amount;
                entry.1 += 1;

                match line.category_id {
                    Some(cat_id) => {
                        let entry = cat_buckets
                            .entry((cat_id.clone(), line.currency))
                            .or_insert((Decimal::ZERO, 0));
                        entry.0 += line.amount;
                        entry.1 += 1;

                        if let Some((_, group_id)) = cat_index.get(&cat_id) {
                            let entry = group_buckets
                                .entry((group_id.clone(), line.currency))
                                .or_insert((Decimal::ZERO, 0));
                            entry.0 += line.amount;
                            entry.1 += 1;
                        }
                    }
                    None => {
                        let entry = uncat_buckets
                            .entry(line.currency)
                            .or_insert((Decimal::ZERO, 0));
                        entry.0 += line.amount;
                        entry.1 += 1;
                    }
                }

                let entry = acc_buckets
                    .entry((line.account_id, line.currency))
                    .or_insert((Decimal::ZERO, 0));
                entry.0 += line.amount;
                entry.1 += 1;
            }
        }

        // Sort outputs: categories by amount ASC (largest expense first),
        // same for groups and accounts.
        let mut by_category: Vec<CategoryLine> = cat_buckets
            .into_iter()
            .map(|((cat_id, currency), (amount, count))| {
                let (cat_name, group_id) = cat_index
                    .get(&cat_id)
                    .cloned()
                    .unwrap_or_else(|| (cat_id.clone(), "".into()));
                let group_name = group_index.get(&group_id).cloned();
                CategoryLine {
                    category_id: cat_id,
                    category_name: cat_name,
                    group_name,
                    currency,
                    amount: amount.to_string(),
                    line_count: count,
                }
            })
            .collect();
        by_category.sort_by(|a, b| {
            Decimal::from_str_exact(&a.amount)
                .unwrap_or(Decimal::ZERO)
                .cmp(&Decimal::from_str_exact(&b.amount).unwrap_or(Decimal::ZERO))
        });

        let mut by_group: Vec<GroupLine> = group_buckets
            .into_iter()
            .map(|((gid, currency), (amount, count))| GroupLine {
                group_id: gid.clone(),
                group_name: group_index.get(&gid).cloned().unwrap_or(gid),
                currency,
                amount: amount.to_string(),
                line_count: count,
            })
            .collect();
        by_group.sort_by(|a, b| {
            Decimal::from_str_exact(&a.amount)
                .unwrap_or(Decimal::ZERO)
                .cmp(&Decimal::from_str_exact(&b.amount).unwrap_or(Decimal::ZERO))
        });

        let mut by_account: Vec<AccountLine> = acc_buckets
            .into_iter()
            .map(|((aid, currency), (amount, count))| AccountLine {
                account_id: aid,
                currency,
                amount: amount.to_string(),
                line_count: count,
            })
            .collect();
        by_account.sort_by(|a, b| {
            Decimal::from_str_exact(&a.amount)
                .unwrap_or(Decimal::ZERO)
                .cmp(&Decimal::from_str_exact(&b.amount).unwrap_or(Decimal::ZERO))
        });

        let totals_by_currency = to_currency_totals(&total_buckets);
        let uncategorized = to_currency_totals(&uncat_buckets);

        Ok(SpendingReport {
            totals_by_currency,
            by_category,
            by_group,
            by_account,
            uncategorized,
            transfers_excluded,
            transaction_count,
            split_count,
            window_start: opts.from,
            window_end: opts.to,
        })
    }
}

struct Line {
    account_id: String,
    category_id: Option<String>,
    currency: CurrencyCode,
    amount: Decimal,
}

fn line_from_txn(txn: &Transaction) -> Line {
    Line {
        account_id: txn.account_id.clone(),
        category_id: txn.category_id.clone(),
        currency: txn.amount.currency,
        amount: txn.amount.amount,
    }
}

fn line_from_split(split: &TransactionSplit, account_id: &str) -> Line {
    Line {
        account_id: account_id.to_string(),
        category_id: Some(split.category_id.clone()),
        currency: split.amount.currency,
        amount: split.amount.amount,
    }
}

fn date_window(from: Option<NaiveDate>, to: Option<NaiveDate>) -> (NaiveDate, NaiveDate) {
    // Default window covers basically everything in the DB when dates are
    // unspecified. SQLite stores dates as ISO strings, so wide bounds are fine.
    let start = from.unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let end = to.unwrap_or_else(|| NaiveDate::from_ymd_opt(2999, 12, 31).unwrap());
    (start, end)
}

fn to_currency_totals(buckets: &HashMap<CurrencyCode, (Decimal, usize)>) -> Vec<CurrencyTotal> {
    buckets
        .iter()
        .map(|(c, (amount, count))| CurrencyTotal {
            currency: *c,
            amount: amount.to_string(),
            line_count: *count,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::{Account, AccountRepository as _, AccountType};
    use crate::domain::currency::Money;
    use crate::domain::transaction::Transaction;
    use crate::infrastructure::storage::{
        Database, SqliteAccountRepository, SqliteCategoryRepository, SqliteTransactionRepository,
        SqliteTransactionSplitRepository,
    };
    use rust_decimal_macros::dec;

    fn setup() -> (Database, String, String, String, String) {
        let db = Database::in_memory().unwrap();
        SqliteAccountRepository::new(&db)
            .save(
                &Account::new(
                    "chase".into(),
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
                "INSERT INTO category_groups (id, name, created_at) VALUES \
                 ('g-essentials', 'Essentials', ?1), ('g-lifestyle', 'Lifestyle', ?1)",
                [&now],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO categories (id, group_id, name, created_at) VALUES \
                 ('cat-food', 'g-essentials', 'Food', ?1), \
                 ('cat-rent', 'g-essentials', 'Rent', ?1), \
                 ('cat-fun', 'g-lifestyle', 'Fun', ?1)",
                [&now],
            )
            .unwrap();
        (
            db,
            "chase".into(),
            "cat-food".into(),
            "cat-rent".into(),
            "cat-fun".into(),
        )
    }

    fn seed_txn(
        db: &Database,
        id: &str,
        account: &str,
        category: Option<&str>,
        amount: Decimal,
        day: u32,
    ) {
        let mut t = Transaction::new(
            id.into(),
            account.into(),
            NaiveDate::from_ymd_opt(2026, 4, day).unwrap(),
            Money::new(amount, CurrencyCode::USD),
        )
        .unwrap();
        t.external_id = Some(format!("ext-{id}"));
        t.category_id = category.map(|s| s.to_string());
        SqliteTransactionRepository::new(db).save(&t).unwrap();
    }

    fn service(
        db: &Database,
    ) -> SpendingService<
        SqliteTransactionRepository<'_>,
        SqliteTransactionSplitRepository<'_>,
        SqliteCategoryRepository<'_>,
    > {
        SpendingService::new(
            SqliteTransactionRepository::new(db),
            SqliteTransactionSplitRepository::new(db),
            SqliteCategoryRepository::new(db),
        )
    }

    #[test]
    fn empty_db_returns_zero_report() {
        let (db, _, _, _, _) = setup();
        let report = service(&db).compute(SpendingOptions::default()).unwrap();
        assert_eq!(report.transaction_count, 0);
        assert!(report.by_category.is_empty());
    }

    #[test]
    fn categorized_txns_roll_up() {
        let (db, chase, food, rent, _) = setup();
        seed_txn(&db, "t1", &chase, Some(&food), dec!(-50), 10);
        seed_txn(&db, "t2", &chase, Some(&food), dec!(-30), 11);
        seed_txn(&db, "t3", &chase, Some(&rent), dec!(-1500), 1);

        let report = service(&db).compute(SpendingOptions::default()).unwrap();
        assert_eq!(report.transaction_count, 3);
        assert_eq!(report.by_category.len(), 2);
        // Rent comes first (more negative).
        assert_eq!(report.by_category[0].category_id, rent);
        assert_eq!(report.by_category[0].amount, "-1500");
        assert_eq!(report.by_category[1].category_id, food);
        assert_eq!(report.by_category[1].amount, "-80");
    }

    #[test]
    fn split_transaction_counts_by_split_lines() {
        let (db, chase, food, rent, _) = setup();
        // Parent $150 transaction, no category on parent.
        seed_txn(&db, "costco", &chase, None, dec!(-150), 10);
        // Split: $100 food + $50 rent.
        let split_repo = SqliteTransactionSplitRepository::new(&db);
        split_repo
            .save(
                &TransactionSplit::new(
                    "s1".into(),
                    "costco".into(),
                    food.clone(),
                    Money::new(dec!(-100), CurrencyCode::USD),
                    None,
                )
                .unwrap(),
            )
            .unwrap();
        split_repo
            .save(
                &TransactionSplit::new(
                    "s2".into(),
                    "costco".into(),
                    rent.clone(),
                    Money::new(dec!(-50), CurrencyCode::USD),
                    None,
                )
                .unwrap(),
            )
            .unwrap();

        let report = service(&db).compute(SpendingOptions::default()).unwrap();
        assert_eq!(report.transaction_count, 1);
        assert_eq!(report.split_count, 2);
        assert_eq!(report.by_category.len(), 2);
        let food_line = report
            .by_category
            .iter()
            .find(|c| c.category_id == food)
            .unwrap();
        assert_eq!(food_line.amount, "-100");
        let rent_line = report
            .by_category
            .iter()
            .find(|c| c.category_id == rent)
            .unwrap();
        assert_eq!(rent_line.amount, "-50");
    }

    #[test]
    fn transfers_excluded_by_default() {
        let (db, chase, food, _, _) = setup();
        seed_txn(&db, "real", &chase, Some(&food), dec!(-50), 10);

        let mut transfer = Transaction::new(
            "transfer".into(),
            chase.clone(),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(dec!(-500), CurrencyCode::USD),
        )
        .unwrap();
        transfer.external_id = Some("ext-transfer".into());
        transfer.transfer_pair_id = Some("pair-xyz".into());
        SqliteTransactionRepository::new(&db)
            .save(&transfer)
            .unwrap();

        let report = service(&db).compute(SpendingOptions::default()).unwrap();
        assert_eq!(report.transaction_count, 1);
        assert_eq!(report.transfers_excluded, 1);
    }

    #[test]
    fn transfers_included_when_opt_in() {
        let (db, chase, food, _, _) = setup();
        seed_txn(&db, "real", &chase, Some(&food), dec!(-50), 10);

        let mut transfer = Transaction::new(
            "transfer".into(),
            chase.clone(),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(dec!(-500), CurrencyCode::USD),
        )
        .unwrap();
        transfer.external_id = Some("ext-transfer".into());
        transfer.transfer_pair_id = Some("pair-xyz".into());
        SqliteTransactionRepository::new(&db)
            .save(&transfer)
            .unwrap();

        let report = service(&db)
            .compute(SpendingOptions {
                exclude_transfers: false,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(report.transaction_count, 2);
        assert_eq!(report.transfers_excluded, 0);
    }

    #[test]
    fn uncategorized_bucket_populated() {
        let (db, chase, _, _, _) = setup();
        seed_txn(&db, "t1", &chase, None, dec!(-75), 10);

        let report = service(&db).compute(SpendingOptions::default()).unwrap();
        assert_eq!(report.uncategorized.len(), 1);
        assert_eq!(report.uncategorized[0].amount, "-75");
    }

    #[test]
    fn date_window_filters() {
        let (db, chase, food, _, _) = setup();
        seed_txn(&db, "t1", &chase, Some(&food), dec!(-10), 1);
        seed_txn(&db, "t2", &chase, Some(&food), dec!(-20), 15);
        seed_txn(&db, "t3", &chase, Some(&food), dec!(-30), 28);

        let report = service(&db)
            .compute(SpendingOptions {
                from: Some(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()),
                to: Some(NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(report.transaction_count, 1);
        assert_eq!(report.by_category[0].amount, "-20");
    }

    #[test]
    fn account_scope_filters() {
        let (db, chase, food, _, _) = setup();
        SqliteAccountRepository::new(&db)
            .save(
                &Account::new(
                    "cap1".into(),
                    "Cap1".into(),
                    AccountType::CreditCard,
                    CurrencyCode::USD,
                    "Sky".into(),
                )
                .unwrap(),
            )
            .unwrap();
        seed_txn(&db, "t1", &chase, Some(&food), dec!(-10), 10);
        seed_txn(&db, "t2", "cap1", Some(&food), dec!(-50), 10);

        let report = service(&db)
            .compute(SpendingOptions {
                account_id: Some(&chase),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(report.transaction_count, 1);
        assert_eq!(report.by_category[0].amount, "-10");
    }

    #[test]
    fn group_rollup_sums_across_categories() {
        let (db, chase, food, rent, fun) = setup();
        seed_txn(&db, "t1", &chase, Some(&food), dec!(-50), 10);
        seed_txn(&db, "t2", &chase, Some(&rent), dec!(-1500), 1);
        seed_txn(&db, "t3", &chase, Some(&fun), dec!(-200), 5);

        let report = service(&db).compute(SpendingOptions::default()).unwrap();
        assert_eq!(report.by_group.len(), 2);
        // Essentials group: food + rent = -1550.
        let essentials = report
            .by_group
            .iter()
            .find(|g| g.group_id == "g-essentials")
            .unwrap();
        assert_eq!(essentials.amount, "-1550");
        // Lifestyle group: fun = -200.
        let lifestyle = report
            .by_group
            .iter()
            .find(|g| g.group_id == "g-lifestyle")
            .unwrap();
        assert_eq!(lifestyle.amount, "-200");
    }
}
