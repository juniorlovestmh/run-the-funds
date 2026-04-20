//! Rule-based transaction categorization + transfer detection (S05 T02/T04).
//!
//! Walks uncategorized transactions in priority-sorted rule order; the first
//! rule that matches each transaction wins. Supports dry-run (preview without
//! writes) and reset (clear existing categorizations first).
//!
//! Transfer detection: greedy pairing of same-amount transactions across
//! different accounts within a ±3-day window, same currency.

use std::collections::HashMap;

use chrono::Duration;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::domain::rules::{Rule, RuleRepository};
use crate::domain::transaction::{Transaction, TransactionRepository};

/// Window for debit ↔ credit pairing (inclusive, absolute value).
const TRANSFER_DAY_TOLERANCE: i64 = 3;

pub struct CategorizationService<R: RuleRepository, T: TransactionRepository> {
    rules: R,
    txns: T,
}

#[derive(Debug, Clone, Copy)]
pub struct CategorizeOptions<'a> {
    pub dry_run: bool,
    pub reset: bool,
    pub account_id: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CategorizeReport {
    pub categorized: usize,
    pub skipped: usize,
    pub reset: usize,
    pub dry_run: bool,
    pub rules_fired: Vec<RuleFireCount>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuleFireCount {
    pub rule_id: String,
    pub rule_name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TransferPairingReport {
    pub pairs_created: usize,
    /// Same-amount groups we considered but couldn't fully pair (odd counts,
    /// all-same-account, beyond the date tolerance). Useful signal for
    /// operators diagnosing missed transfers.
    pub candidates_skipped: usize,
    pub dry_run: bool,
}

impl<R: RuleRepository, T: TransactionRepository> CategorizationService<R, T> {
    pub fn new(rules: R, txns: T) -> Self {
        Self { rules, txns }
    }

    pub fn categorize(&self, opts: CategorizeOptions) -> Result<CategorizeReport, DomainError> {
        // Step 1: optional reset.
        let reset_count = if opts.reset {
            self.txns.clear_categories(opts.account_id)?
        } else {
            0
        };

        // Step 2: load rules (already ordered by priority DESC).
        let rules = self.rules.find_all()?;

        // Step 3: walk uncategorized transactions.
        let transactions = self.txns.find_uncategorized(opts.account_id)?;

        let mut categorized = 0usize;
        let mut skipped = 0usize;
        // Track per-rule fire counts keyed by rule id. Preserves insertion
        // order so the report shows rules in "first-fired" order.
        let mut fire_counts: Vec<(String, String, usize)> = Vec::new();

        for mut txn in transactions {
            let matched = first_matching_rule(&rules, &txn);
            match matched {
                Some(rule) => {
                    txn.category_id = Some(rule.category_id.clone());
                    if !opts.dry_run {
                        self.txns.save(&txn)?;
                    }
                    bump_fire_count(&mut fire_counts, rule);
                    categorized += 1;
                }
                None => {
                    skipped += 1;
                }
            }
        }

        let rules_fired = fire_counts
            .into_iter()
            .map(|(rule_id, rule_name, count)| RuleFireCount {
                rule_id,
                rule_name,
                count,
            })
            .collect();

        Ok(CategorizeReport {
            categorized,
            skipped,
            reset: reset_count,
            dry_run: opts.dry_run,
            rules_fired,
        })
    }

    /// Detect transfers: pair up debit/credit transactions that look like
    /// internal money movements (e.g. Wise USD→BRL, Chase→Capital One). Sets
    /// `transfer_pair_id` on both sides. Idempotent — already-paired rows
    /// are excluded via `find_untagged`.
    pub fn detect_transfers(
        &self,
        dry_run: bool,
    ) -> Result<TransferPairingReport, DomainError> {
        let untagged = self.txns.find_untagged()?;

        // Group by (currency, |amount|). Same-currency, same-magnitude pairs
        // are transfer candidates.
        let mut groups: HashMap<(CurrencyCode, Decimal), Vec<Transaction>> = HashMap::new();
        for t in untagged {
            let key = (t.amount.currency, t.amount.amount.abs());
            groups.entry(key).or_default().push(t);
        }

        let mut pairs_created = 0usize;
        let mut candidates_skipped = 0usize;

        for ((_currency, _magnitude), mut group) in groups {
            if group.len() < 2 {
                // Singleton — not a candidate at all.
                continue;
            }

            // Sort chronologically so pairing walks in date order.
            group.sort_by_key(|t| (t.date, t.id.clone()));

            // Split into debits (negative) and credits (positive/zero).
            let (mut debits, mut credits): (Vec<_>, Vec<_>) =
                group.into_iter().partition(|t| t.amount.amount.is_sign_negative());

            // Greedy pairing: for each debit, find the earliest credit from a
            // different account within the day-tolerance window.
            let mut local_pairs = 0usize;
            let mut debit_consumed = vec![false; debits.len()];
            let mut credit_consumed = vec![false; credits.len()];

            for (di, debit) in debits.iter().enumerate() {
                if debit_consumed[di] {
                    continue;
                }
                let candidate_idx = credits.iter().enumerate().find_map(|(ci, credit)| {
                    if credit_consumed[ci] {
                        return None;
                    }
                    if credit.account_id == debit.account_id {
                        return None;
                    }
                    let date_diff = (debit.date - credit.date).num_days().abs();
                    if date_diff <= TRANSFER_DAY_TOLERANCE {
                        Some(ci)
                    } else {
                        None
                    }
                });
                if let Some(ci) = candidate_idx {
                    debit_consumed[di] = true;
                    credit_consumed[ci] = true;
                    local_pairs += 1;

                    if !dry_run {
                        let pair_id = Uuid::new_v4().to_string();
                        let mut a = debits[di].clone();
                        let mut b = credits[ci].clone();
                        a.transfer_pair_id = Some(pair_id.clone());
                        b.transfer_pair_id = Some(pair_id);
                        self.txns.save(&a)?;
                        self.txns.save(&b)?;
                    }
                }
            }

            pairs_created += local_pairs;

            // Whatever's left in this group couldn't be paired.
            let leftover_debits = debit_consumed.iter().filter(|c| !**c).count();
            let leftover_credits = credit_consumed.iter().filter(|c| !**c).count();
            candidates_skipped += leftover_debits + leftover_credits;
        }

        Ok(TransferPairingReport {
            pairs_created,
            candidates_skipped,
            dry_run,
        })
    }
}

/// Find the first (highest-priority) rule whose pattern matches this transaction.
/// `rules` must already be sorted by priority DESC.
fn first_matching_rule<'a>(
    rules: &'a [Rule],
    txn: &crate::domain::transaction::Transaction,
) -> Option<&'a Rule> {
    rules.iter().find(|r| {
        r.matches(
            txn.payee.as_deref(),
            txn.description.as_deref(),
            &txn.amount.amount,
        )
    })
}

fn bump_fire_count(counts: &mut Vec<(String, String, usize)>, rule: &Rule) {
    if let Some(entry) = counts.iter_mut().find(|(id, _, _)| id == &rule.id) {
        entry.2 += 1;
    } else {
        counts.push((rule.id.clone(), rule.name.clone(), 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::{Account, AccountRepository as _, AccountType};
    use crate::domain::currency::{CurrencyCode, Money};
    use crate::domain::rules::{MatchField, MatchKind, Rule};
    use crate::domain::transaction::Transaction;
    use crate::infrastructure::storage::{
        Database, SqliteAccountRepository, SqliteRuleRepository, SqliteTransactionRepository,
    };
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn setup() -> (Database, String, String, String) {
        let db = Database::in_memory().unwrap();
        // One account, two categories.
        let acc_repo = SqliteAccountRepository::new(&db);
        let acc = Account::new(
            "acc-1".into(),
            "Chase".into(),
            AccountType::Checking,
            CurrencyCode::USD,
            "Sky".into(),
        )
        .unwrap();
        acc_repo.save(&acc).unwrap();

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
                 ('cat-food', 'g1', 'Food', ?1), ('cat-debt', 'g1', 'Debt', ?1)",
                [&now],
            )
            .unwrap();

        (db, "acc-1".into(), "cat-food".into(), "cat-debt".into())
    }

    fn seed_txn(
        db: &Database,
        id: &str,
        account: &str,
        payee: Option<&str>,
        amount: Decimal,
    ) -> Transaction {
        let repo = SqliteTransactionRepository::new(db);
        let mut t = Transaction::new(
            id.into(),
            account.into(),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(amount, CurrencyCode::USD),
        )
        .unwrap();
        t.payee = payee.map(|s| s.to_string());
        t.external_id = Some(format!("ext-{id}"));
        repo.save(&t).unwrap();
        t
    }

    fn seed_rule(
        db: &Database,
        name: &str,
        field: MatchField,
        pattern: &str,
        category_id: &str,
        priority: i64,
    ) -> Rule {
        let repo = SqliteRuleRepository::new(db);
        let r = Rule::new(
            Uuid::new_v4().to_string(),
            name.into(),
            field,
            MatchKind::Substring,
            pattern.into(),
            category_id.into(),
            priority,
        )
        .unwrap();
        repo.save(&r).unwrap();
        r
    }

    fn service(
        db: &Database,
    ) -> CategorizationService<SqliteRuleRepository<'_>, SqliteTransactionRepository<'_>> {
        CategorizationService::new(
            SqliteRuleRepository::new(db),
            SqliteTransactionRepository::new(db),
        )
    }

    fn default_opts() -> CategorizeOptions<'static> {
        CategorizeOptions {
            dry_run: false,
            reset: false,
            account_id: None,
        }
    }

    #[test]
    fn no_rules_skips_everything() {
        let (db, acc, _food, _debt) = setup();
        seed_txn(&db, "t1", &acc, Some("Whole Foods"), dec!(-45));
        seed_txn(&db, "t2", &acc, Some("Wise"), dec!(-100));

        let svc = service(&db);
        let report = svc.categorize(default_opts()).unwrap();
        assert_eq!(report.categorized, 0);
        assert_eq!(report.skipped, 2);
        assert!(report.rules_fired.is_empty());
    }

    #[test]
    fn matching_rule_categorizes_transaction() {
        let (db, acc, food, _debt) = setup();
        seed_txn(&db, "t1", &acc, Some("Whole Foods Market"), dec!(-45));
        seed_rule(&db, "Groceries", MatchField::Payee, "Whole Foods", &food, 100);

        let svc = service(&db);
        let report = svc.categorize(default_opts()).unwrap();
        assert_eq!(report.categorized, 1);
        assert_eq!(report.skipped, 0);
        assert_eq!(report.rules_fired.len(), 1);
        assert_eq!(report.rules_fired[0].count, 1);

        // Verify the write happened via the repo.
        let t = SqliteTransactionRepository::new(&db)
            .find_by_id("t1")
            .unwrap()
            .unwrap();
        assert_eq!(t.category_id.as_deref(), Some(food.as_str()));
    }

    #[test]
    fn higher_priority_rule_wins_when_multiple_match() {
        let (db, acc, food, debt) = setup();
        seed_txn(&db, "t1", &acc, Some("Capital One Student Loan"), dec!(-149));
        // Both rules match; "Debt" has higher priority.
        seed_rule(&db, "Generic Capital One", MatchField::Payee, "Capital One", &food, 50);
        seed_rule(&db, "Student Loan", MatchField::Payee, "Student Loan", &debt, 200);

        let svc = service(&db);
        let report = svc.categorize(default_opts()).unwrap();
        assert_eq!(report.categorized, 1);
        let t = SqliteTransactionRepository::new(&db)
            .find_by_id("t1")
            .unwrap()
            .unwrap();
        assert_eq!(t.category_id.as_deref(), Some(debt.as_str()));
        assert_eq!(report.rules_fired[0].rule_name, "Student Loan");
    }

    #[test]
    fn dry_run_counts_but_does_not_write() {
        let (db, acc, food, _debt) = setup();
        seed_txn(&db, "t1", &acc, Some("Whole Foods"), dec!(-45));
        seed_rule(&db, "Groceries", MatchField::Payee, "Whole Foods", &food, 100);

        let svc = service(&db);
        let report = svc
            .categorize(CategorizeOptions {
                dry_run: true,
                reset: false,
                account_id: None,
            })
            .unwrap();
        assert_eq!(report.categorized, 1);
        assert!(report.dry_run);

        let t = SqliteTransactionRepository::new(&db)
            .find_by_id("t1")
            .unwrap()
            .unwrap();
        // DB unchanged: category_id still None.
        assert!(t.category_id.is_none());
    }

    #[test]
    fn reset_clears_existing_then_reruns() {
        let (db, acc, food, _debt) = setup();
        let mut t1 = seed_txn(&db, "t1", &acc, Some("Whole Foods"), dec!(-45));
        // Pre-categorize t1 manually.
        t1.category_id = Some(food.clone());
        SqliteTransactionRepository::new(&db).save(&t1).unwrap();

        // Rule that would assign t1 to Debt (different category).
        seed_rule(&db, "Groceries", MatchField::Payee, "Whole Foods", &food, 100);

        let svc = service(&db);

        // Without reset, t1 is skipped (already categorized).
        let report = svc.categorize(default_opts()).unwrap();
        assert_eq!(report.categorized, 0);
        assert_eq!(report.skipped, 0, "already-categorized rows aren't returned by find_uncategorized");

        // With reset, t1 is cleared then re-categorized.
        let report = svc
            .categorize(CategorizeOptions {
                dry_run: false,
                reset: true,
                account_id: None,
            })
            .unwrap();
        assert_eq!(report.reset, 1);
        assert_eq!(report.categorized, 1);
    }

    #[test]
    fn amount_based_rule() {
        let (db, acc, _food, debt) = setup();
        seed_txn(&db, "t_big", &acc, Some("Big Withdrawal"), dec!(-5000));
        seed_txn(&db, "t_small", &acc, Some("Coffee"), dec!(-4.50));
        seed_rule(&db, "Large outflows", MatchField::Amount, "<=-1000", &debt, 100);

        let svc = service(&db);
        let report = svc.categorize(default_opts()).unwrap();
        assert_eq!(report.categorized, 1);
        assert_eq!(report.skipped, 1);

        let big = SqliteTransactionRepository::new(&db)
            .find_by_id("t_big")
            .unwrap()
            .unwrap();
        assert_eq!(big.category_id.as_deref(), Some(debt.as_str()));
    }

    #[test]
    fn account_id_scope_limits_work() {
        let (db, acc1, food, _debt) = setup();
        // Second account.
        let acc_repo = SqliteAccountRepository::new(&db);
        acc_repo
            .save(
                &Account::new(
                    "acc-2".into(),
                    "Capital One".into(),
                    AccountType::Checking,
                    CurrencyCode::USD,
                    "Sky".into(),
                )
                .unwrap(),
            )
            .unwrap();

        seed_txn(&db, "t1", &acc1, Some("Target"), dec!(-20));
        seed_txn(&db, "t2", "acc-2", Some("Target"), dec!(-30));
        seed_rule(&db, "Target", MatchField::Payee, "Target", &food, 100);

        let svc = service(&db);
        let report = svc
            .categorize(CategorizeOptions {
                dry_run: false,
                reset: false,
                account_id: Some(&acc1),
            })
            .unwrap();
        assert_eq!(report.categorized, 1); // only acc-1's transaction

        let t1 = SqliteTransactionRepository::new(&db)
            .find_by_id("t1")
            .unwrap()
            .unwrap();
        let t2 = SqliteTransactionRepository::new(&db)
            .find_by_id("t2")
            .unwrap()
            .unwrap();
        assert!(t1.category_id.is_some());
        assert!(t2.category_id.is_none());
    }

    #[test]
    fn rule_fire_counts_aggregate_across_transactions() {
        let (db, acc, food, _debt) = setup();
        seed_txn(&db, "t1", &acc, Some("Whole Foods"), dec!(-45));
        seed_txn(&db, "t2", &acc, Some("Whole Foods Market"), dec!(-99));
        seed_txn(&db, "t3", &acc, Some("Whole Foods #123"), dec!(-12));
        seed_rule(&db, "Groceries", MatchField::Payee, "Whole Foods", &food, 100);

        let svc = service(&db);
        let report = svc.categorize(default_opts()).unwrap();
        assert_eq!(report.categorized, 3);
        assert_eq!(report.rules_fired.len(), 1);
        assert_eq!(report.rules_fired[0].count, 3);
    }

    #[test]
    fn regex_rule_matches() {
        let (db, acc, food, _debt) = setup();
        seed_txn(&db, "t1", &acc, Some("WISE INC"), dec!(-100));
        seed_txn(&db, "t2", &acc, Some("wise"), dec!(-50));
        seed_txn(&db, "t3", &acc, Some("Citibank"), dec!(-200));

        let repo = SqliteRuleRepository::new(&db);
        let r = Rule::new(
            Uuid::new_v4().to_string(),
            "Wise".into(),
            MatchField::Payee,
            MatchKind::Regex,
            r"wise( inc)?".into(),
            food.clone(),
            100,
        )
        .unwrap();
        repo.save(&r).unwrap();

        let svc = service(&db);
        let report = svc.categorize(default_opts()).unwrap();
        assert_eq!(report.categorized, 2);
        assert_eq!(report.skipped, 1);
    }

    // ---- transfer detection tests ------------------------------------------

    fn setup_transfer_scenario() -> (Database, String, String) {
        let db = Database::in_memory().unwrap();
        let acc_repo = SqliteAccountRepository::new(&db);
        acc_repo
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
        acc_repo
            .save(
                &Account::new(
                    "cap1".into(),
                    "Cap One".into(),
                    AccountType::CreditCard,
                    CurrencyCode::USD,
                    "Sky".into(),
                )
                .unwrap(),
            )
            .unwrap();
        (db, "chase".into(), "cap1".into())
    }

    fn seed_with_date(
        db: &Database,
        id: &str,
        account: &str,
        amount: Decimal,
        y: i32,
        m: u32,
        d: u32,
    ) {
        let mut t = Transaction::new(
            id.into(),
            account.into(),
            NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            Money::new(amount, CurrencyCode::USD),
        )
        .unwrap();
        t.external_id = Some(format!("ext-{id}"));
        SqliteTransactionRepository::new(db).save(&t).unwrap();
    }

    #[test]
    fn simple_pair_detected() {
        let (db, chase, cap1) = setup_transfer_scenario();
        seed_with_date(&db, "t_debit", &chase, dec!(-500), 2026, 4, 10);
        seed_with_date(&db, "t_credit", &cap1, dec!(500), 2026, 4, 10);

        let svc = service(&db);
        let report = svc.detect_transfers(false).unwrap();
        assert_eq!(report.pairs_created, 1);
        assert_eq!(report.candidates_skipped, 0);

        let t = SqliteTransactionRepository::new(&db)
            .find_by_id("t_debit")
            .unwrap()
            .unwrap();
        let u = SqliteTransactionRepository::new(&db)
            .find_by_id("t_credit")
            .unwrap()
            .unwrap();
        assert!(t.transfer_pair_id.is_some());
        assert_eq!(t.transfer_pair_id, u.transfer_pair_id);
    }

    #[test]
    fn date_tolerance_plus_3_days_pairs() {
        let (db, chase, cap1) = setup_transfer_scenario();
        seed_with_date(&db, "debit", &chase, dec!(-250), 2026, 4, 10);
        seed_with_date(&db, "credit", &cap1, dec!(250), 2026, 4, 13); // 3 days later

        let svc = service(&db);
        let report = svc.detect_transfers(false).unwrap();
        assert_eq!(report.pairs_created, 1);
    }

    #[test]
    fn date_tolerance_plus_4_days_does_not_pair() {
        let (db, chase, cap1) = setup_transfer_scenario();
        seed_with_date(&db, "debit", &chase, dec!(-250), 2026, 4, 10);
        seed_with_date(&db, "credit", &cap1, dec!(250), 2026, 4, 14); // 4 days later

        let svc = service(&db);
        let report = svc.detect_transfers(false).unwrap();
        assert_eq!(report.pairs_created, 0);
        assert_eq!(report.candidates_skipped, 2);
    }

    #[test]
    fn same_account_not_paired() {
        let (db, chase, _cap1) = setup_transfer_scenario();
        seed_with_date(&db, "a", &chase, dec!(-500), 2026, 4, 10);
        seed_with_date(&db, "b", &chase, dec!(500), 2026, 4, 10);

        let svc = service(&db);
        let report = svc.detect_transfers(false).unwrap();
        assert_eq!(report.pairs_created, 0);
        assert_eq!(report.candidates_skipped, 2);
    }

    #[test]
    fn odd_count_pairs_only_what_fits() {
        let (db, chase, cap1) = setup_transfer_scenario();
        seed_with_date(&db, "d1", &chase, dec!(-100), 2026, 4, 10);
        seed_with_date(&db, "d2", &chase, dec!(-100), 2026, 4, 11);
        seed_with_date(&db, "c1", &cap1, dec!(100), 2026, 4, 10);

        let svc = service(&db);
        let report = svc.detect_transfers(false).unwrap();
        assert_eq!(report.pairs_created, 1);
        assert_eq!(report.candidates_skipped, 1); // leftover debit
    }

    #[test]
    fn idempotent_on_rerun() {
        let (db, chase, cap1) = setup_transfer_scenario();
        seed_with_date(&db, "d", &chase, dec!(-100), 2026, 4, 10);
        seed_with_date(&db, "c", &cap1, dec!(100), 2026, 4, 10);

        let svc = service(&db);
        assert_eq!(svc.detect_transfers(false).unwrap().pairs_created, 1);
        // Re-run: find_untagged skips already-paired rows.
        assert_eq!(svc.detect_transfers(false).unwrap().pairs_created, 0);
    }

    #[test]
    fn dry_run_detects_but_does_not_write() {
        let (db, chase, cap1) = setup_transfer_scenario();
        seed_with_date(&db, "d", &chase, dec!(-100), 2026, 4, 10);
        seed_with_date(&db, "c", &cap1, dec!(100), 2026, 4, 10);

        let svc = service(&db);
        let report = svc.detect_transfers(true).unwrap();
        assert_eq!(report.pairs_created, 1);
        assert!(report.dry_run);

        let t = SqliteTransactionRepository::new(&db)
            .find_by_id("d")
            .unwrap()
            .unwrap();
        assert!(t.transfer_pair_id.is_none());
    }

    #[test]
    fn different_currencies_dont_pair() {
        // Seed one BRL and one USD same-abs-amount; shouldn't pair across
        // currency boundaries (real behavior: a BRL 100 and USD 100 aren't
        // the same money — cross-currency transfers go through Wise and land
        // as separate-amount rows anyway).
        let (db, chase, _cap1) = setup_transfer_scenario();
        let brl_repo = SqliteAccountRepository::new(&db);
        brl_repo
            .save(
                &Account::new(
                    "nubank".into(),
                    "Nubank".into(),
                    AccountType::Checking,
                    CurrencyCode::BRL,
                    "Sky".into(),
                )
                .unwrap(),
            )
            .unwrap();

        let mut usd = Transaction::new(
            "usd".into(),
            chase,
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(dec!(-100), CurrencyCode::USD),
        )
        .unwrap();
        usd.external_id = Some("ext-usd".into());
        SqliteTransactionRepository::new(&db).save(&usd).unwrap();

        let mut brl = Transaction::new(
            "brl".into(),
            "nubank".into(),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(dec!(100), CurrencyCode::BRL),
        )
        .unwrap();
        brl.external_id = Some("ext-brl".into());
        SqliteTransactionRepository::new(&db).save(&brl).unwrap();

        let svc = service(&db);
        let report = svc.detect_transfers(false).unwrap();
        assert_eq!(report.pairs_created, 0);
    }

    #[test]
    fn already_categorized_rows_skipped() {
        // Once a transaction has a category_id, it's excluded from transfer
        // pairing — find_untagged filters it out.
        let (db, chase, cap1) = setup_transfer_scenario();
        let mut t = Transaction::new(
            "d".into(),
            chase,
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
            Money::new(dec!(-100), CurrencyCode::USD),
        )
        .unwrap();
        t.external_id = Some("ext-d".into());
        t.category_id = Some("some-cat".into()); // already categorized
        // Direct insert bypassing FK (need a category row) — seed one.
        let now = chrono::Utc::now().to_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO category_groups (id, name, created_at) VALUES ('g1', 'Grp', ?1)",
                [&now],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO categories (id, group_id, name, created_at) VALUES ('some-cat', 'g1', 'Food', ?1)",
                [&now],
            )
            .unwrap();
        SqliteTransactionRepository::new(&db).save(&t).unwrap();

        seed_with_date(&db, "c", &cap1, dec!(100), 2026, 4, 10);

        let svc = service(&db);
        let report = svc.detect_transfers(false).unwrap();
        assert_eq!(report.pairs_created, 0);
    }
}
