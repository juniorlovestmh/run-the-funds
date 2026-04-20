//! Generate a realistic-looking synthetic demo DB for screen recordings / demos.
//!
//! Zero PII: all merchant names, account numbers, and personal identifiers
//! are invented. Data shape is inspired by a typical household running rtf
//! (multiple US accounts, one BRL account for multi-currency, subscriptions,
//! rideshare, groceries, payroll, student loans, internal transfers).
//!
//! Usage:
//!     cargo run --release --example seed_demo -- --db /tmp/rtf-demo.db
//!
//! Deterministic: same inputs produce the same DB every run (fixed RNG seed).
//! Safe to run repeatedly; output DB is wiped at start.

use std::env;
use std::str::FromStr;

use chrono::{Datelike, Duration, NaiveDate, Utc};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use uuid::Uuid;

use rtf::domain::account::{Account, AccountRepository, AccountType};
use rtf::domain::category::{Category, CategoryGroup, CategoryRepository};
use rtf::domain::currency::{CurrencyCode, Money};
use rtf::domain::rules::{MatchField, MatchKind, Rule, RuleRepository};
use rtf::domain::tag::{Tag, TagRepository};
use rtf::domain::transaction::{Transaction, TransactionRepository};
use rtf::infrastructure::storage::{
    Database, SqliteAccountRepository, SqliteCategoryRepository, SqliteRuleRepository,
    SqliteTagRepository, SqliteTransactionRepository,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = parse_db_arg();
    // Wipe any previous demo DB.
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(format!("{db_path}-shm"));
    let _ = std::fs::remove_file(format!("{db_path}-wal"));

    let db = Database::open(&db_path)?;

    let acc_repo = SqliteAccountRepository::new(&db);
    let cat_repo = SqliteCategoryRepository::new(&db);
    let tag_repo = SqliteTagRepository::new(&db);
    let rule_repo = SqliteRuleRepository::new(&db);
    let txn_repo = SqliteTransactionRepository::new(&db);

    // -- groups + categories ------------------------------------------------
    let groups = seed_groups(&cat_repo)?;
    let cats = seed_categories(&cat_repo, &groups)?;

    // -- tags ---------------------------------------------------------------
    let tags = seed_tags(&tag_repo)?;

    // -- accounts -----------------------------------------------------------
    let accounts = seed_accounts(&acc_repo)?;

    // -- rules --------------------------------------------------------------
    seed_rules(&rule_repo, &cats)?;

    // -- transactions + splits + transfer pairs -----------------------------
    let (n_txns, n_splits, n_pairs) =
        seed_transactions(&txn_repo, &tag_repo, &accounts, &cats, &tags)?;

    let _ = (accounts, groups, cats, tags);
    println!("Seeded demo DB at {db_path}");
    println!("  accounts:      10");
    println!("  groups:        11");
    println!("  categories:    23");
    println!("  tags:          6");
    println!("  transactions:  {n_txns}");
    println!("  splits:        {n_splits}");
    println!("  transfer prs:  {n_pairs}");

    Ok(())
}

fn parse_db_arg() -> String {
    let args: Vec<String> = env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--db" {
            return args
                .get(i + 1)
                .cloned()
                .unwrap_or_else(|| "/tmp/rtf-demo.db".into());
        }
    }
    "/tmp/rtf-demo.db".into()
}

// ---- groups -------------------------------------------------------------

struct SeedGroups {
    income: String,
    food: String,
    shopping: String,
    transport: String,
    bills: String,
    housing: String,
    health: String,
    education: String,
    travel: String,
    family: String,
    transfers: String,
}

fn seed_groups(repo: &SqliteCategoryRepository) -> Result<SeedGroups, Box<dyn std::error::Error>> {
    let groups = [
        ("income",    "Income"),
        ("food",      "Food & Dining"),
        ("shopping",  "Shopping"),
        ("transport", "Auto & Transport"),
        ("bills",     "Bills & Utilities"),
        ("housing",   "Housing"),
        ("health",    "Health & Wellness"),
        ("education", "Education"),
        ("travel",    "Travel & Lifestyle"),
        ("family",    "Family & Kids"),
        ("transfers", "Transfers"),
    ];
    let mut ids = std::collections::HashMap::new();
    for (key, name) in groups {
        let g = CategoryGroup::new(Uuid::new_v4().to_string(), name.into())?;
        repo.save_group(&g)?;
        ids.insert(key, g.id);
    }
    Ok(SeedGroups {
        income:    ids.remove("income").unwrap(),
        food:      ids.remove("food").unwrap(),
        shopping:  ids.remove("shopping").unwrap(),
        transport: ids.remove("transport").unwrap(),
        bills:     ids.remove("bills").unwrap(),
        housing:   ids.remove("housing").unwrap(),
        health:    ids.remove("health").unwrap(),
        education: ids.remove("education").unwrap(),
        travel:    ids.remove("travel").unwrap(),
        family:    ids.remove("family").unwrap(),
        transfers: ids.remove("transfers").unwrap(),
    })
}

// ---- categories ---------------------------------------------------------

#[derive(Clone)]
struct SeedCats {
    paychecks: String,
    interest: String,
    groceries: String,
    restaurants: String,
    coffee: String,
    clothing: String,
    household: String,
    gas: String,
    rideshare: String,
    phone: String,
    internet: String,
    subscriptions: String,
    rent: String,
    medical: String,
    pharmacy: String,
    fitness: String,
    student_loans: String,
    travel: String,
    entertainment: String,
    kids: String,
    transfer: String,
    cc_payment: String,
    buy: String,
}

fn seed_categories(
    repo: &SqliteCategoryRepository,
    g: &SeedGroups,
) -> Result<SeedCats, Box<dyn std::error::Error>> {
    let mut save = |name: &str, group_id: &str| -> Result<String, Box<dyn std::error::Error>> {
        let c = Category::new(Uuid::new_v4().to_string(), group_id.into(), name.into())?;
        repo.save_category(&c)?;
        Ok(c.id)
    };
    Ok(SeedCats {
        paychecks:     save("Paychecks",            &g.income)?,
        interest:      save("Interest",             &g.income)?,
        groceries:     save("Groceries",            &g.food)?,
        restaurants:   save("Restaurants & Bars",   &g.food)?,
        coffee:        save("Coffee Shops",         &g.food)?,
        clothing:      save("Clothing",             &g.shopping)?,
        household:     save("Household",            &g.shopping)?,
        gas:           save("Gas",                  &g.transport)?,
        rideshare:     save("Rideshare",            &g.transport)?,
        phone:         save("Phone",                &g.bills)?,
        internet:      save("Internet",             &g.bills)?,
        subscriptions: save("Subscriptions",        &g.bills)?,
        rent:          save("Rent",                 &g.housing)?,
        medical:       save("Medical",              &g.health)?,
        pharmacy:      save("Pharmacy",             &g.health)?,
        fitness:       save("Fitness",              &g.health)?,
        student_loans: save("Student Loans",        &g.education)?,
        travel:        save("Travel & Vacation",    &g.travel)?,
        entertainment: save("Entertainment",        &g.travel)?,
        kids:          save("Kids",                 &g.family)?,
        transfer:      save("Transfer",             &g.transfers)?,
        cc_payment:    save("Credit Card Payment",  &g.transfers)?,
        buy:           save("Buy",                  &g.transfers)?,
    })
}

// ---- tags ---------------------------------------------------------------

struct SeedTags {
    joint: String,
    spouse: String,
    kids: String,
    subscription: String,
    cross_border: String,
    work_reimburse: String,
}

fn seed_tags(repo: &SqliteTagRepository) -> Result<SeedTags, Box<dyn std::error::Error>> {
    let mut mk = |name: &str| -> Result<String, Box<dyn std::error::Error>> {
        let t = Tag::new(Uuid::new_v4().to_string(), name.into())?;
        repo.save(&t)?;
        Ok(t.id)
    };
    Ok(SeedTags {
        joint:         mk("Joint")?,
        spouse:        mk("For Spouse")?,
        kids:          mk("For Kids")?,
        subscription:  mk("Subscription")?,
        cross_border:  mk("Cross-Border")?,
        work_reimburse: mk("Work Reimbursable")?,
    })
}

// ---- accounts -----------------------------------------------------------

struct SeedAccounts {
    checking:   String,
    savings:    String,
    rewards:    String,
    everyday:   String,
    brokerage:  String,
    roth:       String,
    crypto:     String,
    fx_wallet:  String,
    loan:       String,
    brl:        String,
}

fn seed_accounts(
    repo: &SqliteAccountRepository,
) -> Result<SeedAccounts, Box<dyn std::error::Error>> {
    let mut mk = |name: &str, t: AccountType, c: CurrencyCode, inst: Option<&str>, balance: &str|
        -> Result<String, Box<dyn std::error::Error>> {
        let mut a = Account::new(
            Uuid::new_v4().to_string(),
            name.into(),
            t,
            c,
            "Household".into(),
        )?;
        a.institution = inst.map(|s| s.to_string());
        a.balance = Money::new(Decimal::from_str(balance)?, c);
        repo.save(&a)?;
        Ok(a.id)
    };
    Ok(SeedAccounts {
        checking:  mk("Primary Checking",    AccountType::Checking,   CurrencyCode::USD, Some("Atlas Bank"),       "4312.88")?,
        savings:   mk("High-Yield Savings",  AccountType::Savings,    CurrencyCode::USD, Some("Atlas Bank"),       "18204.50")?,
        rewards:   mk("Rewards Card",        AccountType::CreditCard, CurrencyCode::USD, Some("Northline Cards"),  "-1842.19")?,
        everyday:  mk("Everyday Card",       AccountType::CreditCard, CurrencyCode::USD, Some("Northline Cards"),  "-230.44")?,
        brokerage: mk("Taxable Brokerage",   AccountType::Brokerage,  CurrencyCode::USD, Some("Meridian Invest"),  "7410.22")?,
        roth:      mk("Roth IRA",            AccountType::Brokerage,  CurrencyCode::USD, Some("Meridian Invest"),  "912.60")?,
        crypto:    mk("Crypto Wallet",       AccountType::Brokerage,  CurrencyCode::USD, Some("Demo Exchange"),    "2581.93")?,
        fx_wallet: mk("Cross-Border Wallet", AccountType::Checking,   CurrencyCode::USD, Some("Demo FX"),          "120.00")?,
        loan:      mk("Student Loans",       AccountType::Loan,       CurrencyCode::USD, Some("Demo Loan Svcs"),   "-5983.11")?,
        brl:       mk("BRL Checking",        AccountType::Checking,   CurrencyCode::BRL, Some("Demo BR Bank"),     "1230.55")?,
    })
}

// ---- rules --------------------------------------------------------------

fn seed_rules(
    repo: &SqliteRuleRepository,
    c: &SeedCats,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mk = |name: &str, pattern: &str, category_id: &str, priority: i64|
        -> Result<(), Box<dyn std::error::Error>> {
        let r = Rule::new(
            Uuid::new_v4().to_string(),
            name.into(),
            MatchField::Payee,
            MatchKind::Substring,
            pattern.into(),
            category_id.into(),
            priority,
        )?;
        repo.save(&r)?;
        Ok(())
    };
    mk("Rideshare A",       "Uber Demo",         &c.rideshare,     100)?;
    mk("Rideshare B",       "MetroTaxi",         &c.rideshare,     100)?;
    mk("Groceries Chain 1", "Green Market",      &c.groceries,     100)?;
    mk("Groceries Chain 2", "Fresh Pantry",      &c.groceries,     100)?;
    mk("Groceries BR",      "Mercado Demo",      &c.groceries,     100)?;
    mk("Cafe",              "Corner Cafe",       &c.coffee,        100)?;
    mk("Restaurant",        "Downtown Bistro",   &c.restaurants,   100)?;
    mk("Subscription Svc",  "StreamFlix",        &c.subscriptions, 100)?;
    mk("AI Assistant",      "CodeGen Pro",       &c.subscriptions, 100)?;
    mk("Phone Bill",        "Demo Phone Co",     &c.phone,         100)?;
    mk("Internet Bill",     "Demo Net Co",       &c.internet,      100)?;
    mk("Pharmacy",          "Demo Pharmacy",     &c.pharmacy,      100)?;
    mk("Gym",               "Demo Gym",          &c.fitness,       100)?;
    mk("Payroll",           "Demo Payroll",      &c.paychecks,     100)?;
    mk("Interest",          "Monthly Interest",  &c.interest,      100)?;
    mk("Student Loan",      "Demo Loan Svcs",    &c.student_loans, 100)?;
    mk("Card Payment",      "Northline Payment", &c.cc_payment,    100)?;
    mk("Transfer Out",      "Demo FX Send",      &c.transfer,      100)?;
    mk("Rent",              "Demo Landlord",     &c.rent,          100)?;
    mk("Kids School",       "Demo School",       &c.kids,          100)?;
    Ok(())
}

// ---- transactions -------------------------------------------------------

fn seed_transactions(
    txn_repo: &SqliteTransactionRepository,
    tag_repo: &SqliteTagRepository,
    a: &SeedAccounts,
    c: &SeedCats,
    t: &SeedTags,
) -> Result<(usize, usize, usize), Box<dyn std::error::Error>> {
    let mut rng = StdRng::seed_from_u64(0xDEAD_BEEF_CAFE_F00Du64); // fixed seed for reproducibility
    let today = Utc::now().date_naive();
    let start = today - Duration::days(730);

    let mut n_txns = 0usize;
    let n_splits = 0usize;
    let mut n_pairs = 0usize;
    let now_ts = Utc::now();

    // Small helper closure to make + save a transaction.
    let mut mk = |date: NaiveDate,
                  account_id: &str,
                  amount: Decimal,
                  currency: CurrencyCode,
                  payee: &str,
                  description: Option<&str>,
                  category_id: Option<&str>,
                  tag_ids: Vec<String>,
                  transfer_pair_id: Option<String>|
     -> Result<String, Box<dyn std::error::Error>> {
        let mut tx = Transaction::new(
            Uuid::new_v4().to_string(),
            account_id.into(),
            date,
            Money::new(amount, currency),
        )?;
        tx.payee = Some(payee.into());
        tx.description = description.map(|s| s.to_string());
        tx.category_id = category_id.map(|s| s.to_string());
        tx.transfer_pair_id = transfer_pair_id;
        tx.imported_at = Some(now_ts);
        txn_repo.save(&tx)?;
        if !tag_ids.is_empty() {
            tag_repo.set_tags_for_transaction(&tx.id, &tag_ids)?;
        }
        n_txns += 1;
        Ok(tx.id)
    };

    // --- Monthly payroll (bi-weekly, Fridays) ---
    let mut d = start;
    while d <= today {
        let w = d.weekday().num_days_from_monday();
        if w == 4 {
            let is_payday_week = ((d - start).num_days() / 7) % 2 == 0;
            if is_payday_week {
                let gross = 3800.00 + (rng.random::<f64>() * 200.0 - 100.0);
                mk(
                    d,
                    &a.checking,
                    Decimal::from_f64(round2(gross)).unwrap(),
                    CurrencyCode::USD,
                    "Demo Payroll Inc",
                    Some("Direct deposit"),
                    Some(&c.paychecks),
                    vec![],
                    None,
                )?;
            }
        }
        d += Duration::days(1);
    }

    // --- Monthly interest on savings (1st of each month) ---
    let mut d = start;
    while d <= today {
        if d.day() == 1 {
            let interest = 50.0 + rng.random::<f64>() * 15.0;
            mk(
                d,
                &a.savings,
                Decimal::from_f64(round2(interest)).unwrap(),
                CurrencyCode::USD,
                "Monthly Interest",
                Some("Savings interest payment"),
                Some(&c.interest),
                vec![],
                None,
            )?;
        }
        d += Duration::days(1);
    }

    // --- Monthly bills (phone, internet, subscriptions, rent, student loan, gym) ---
    let recurring: Vec<(u32, &str, f64, &str, Vec<&str>)> = vec![
        (3,  "Demo Phone Co",     62.00, &c.phone,         vec![&t.joint]),
        (7,  "Demo Net Co",       89.99, &c.internet,      vec![&t.joint]),
        (10, "StreamFlix",        15.99, &c.subscriptions, vec![&t.subscription, &t.joint]),
        (10, "CodeGen Pro",       20.00, &c.subscriptions, vec![&t.subscription]),
        (12, "MusicBox",           9.99, &c.subscriptions, vec![&t.subscription]),
        (15, "Demo Gym",          79.00, &c.fitness,       vec![&t.joint]),
        (1,  "Demo Landlord",   1850.00, &c.rent,          vec![&t.joint]),
        (20, "Demo Loan Svcs",    93.26, &c.student_loans, vec![]),
        (5,  "Demo School",      260.00, &c.kids,          vec![&t.kids]),
    ];
    for (day_of_month, payee, amt, cat, tag_refs) in recurring {
        let mut month_start = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap();
        while month_start <= today {
            if let Some(d) = NaiveDate::from_ymd_opt(month_start.year(), month_start.month(), day_of_month) {
                if d >= start && d <= today {
                    mk(
                        d,
                        &a.checking,
                        Decimal::from_f64(-amt).unwrap(),
                        CurrencyCode::USD,
                        payee,
                        Some("Recurring monthly"),
                        Some(cat),
                        tag_refs.iter().map(|s| s.to_string()).collect(),
                        None,
                    )?;
                }
            }
            month_start = add_one_month(month_start);
        }
    }

    // --- Variable daily spending on the Rewards Card ---
    let merchants: Vec<(&str, f64, f64, &str, Vec<&str>)> = vec![
        ("Uber Demo",       6.0,  18.0, &c.rideshare,     vec![]),
        ("MetroTaxi",       8.0,  22.0, &c.rideshare,     vec![]),
        ("Corner Cafe",     3.5,   9.0, &c.coffee,        vec![]),
        ("Downtown Bistro", 18.0, 72.0, &c.restaurants,   vec![]),
        ("Pizza Time",      14.0, 38.0, &c.restaurants,   vec![]),
        ("Green Market",    22.0, 120.0, &c.groceries,    vec![&t.joint]),
        ("Fresh Pantry",    18.0,  95.0, &c.groceries,    vec![&t.joint]),
        ("Mercado Demo",    14.0,  85.0, &c.groceries,    vec![&t.joint, &t.cross_border]),
        ("Demo Pharmacy",   12.0,  65.0, &c.pharmacy,     vec![]),
        ("Style Outlet",    25.0, 180.0, &c.clothing,     vec![]),
        ("HomeGoods Co",    28.0, 210.0, &c.household,    vec![&t.joint]),
        ("StationFill",     28.0,  55.0, &c.gas,          vec![]),
        ("CineRoom",         9.0,  26.0, &c.entertainment, vec![]),
    ];
    let mut d = start;
    while d <= today {
        // 1-4 card transactions per day on average
        let n = rng.random_range(0..5);
        for _ in 0..n {
            let m = &merchants[rng.random_range(0..merchants.len())];
            let amt = m.1 + rng.random::<f64>() * (m.2 - m.1);
            mk(
                d,
                &a.rewards,
                Decimal::from_f64(-round2(amt)).unwrap(),
                CurrencyCode::USD,
                m.0,
                None,
                Some(m.3),
                m.4.iter().map(|s| s.to_string()).collect(),
                None,
            )?;
        }
        d += Duration::days(1);
    }

    // --- Everyday Card: lower volume ---
    let low_merchants: Vec<(&str, f64, f64, &str)> = vec![
        ("SubShop",         8.0, 20.0, &c.restaurants),
        ("Bagel Point",     6.0, 14.0, &c.coffee),
        ("Kids Store",     22.0, 80.0, &c.kids),
        ("Office Plus",    18.0, 65.0, &c.household),
    ];
    let mut d = start;
    while d <= today {
        if rng.random::<f64>() < 0.35 {
            let m = &low_merchants[rng.random_range(0..low_merchants.len())];
            let amt = m.1 + rng.random::<f64>() * (m.2 - m.1);
            let tag_ids: Vec<String> = if m.0 == "Kids Store" {
                vec![t.kids.clone()]
            } else {
                vec![]
            };
            mk(
                d,
                &a.everyday,
                Decimal::from_f64(-round2(amt)).unwrap(),
                CurrencyCode::USD,
                m.0,
                None,
                Some(m.3),
                tag_ids,
                None,
            )?;
        }
        d += Duration::days(1);
    }

    // --- BRL merchant activity on the BRL account (some uncategorized to show off "needs review") ---
    let brl_merchants: Vec<(&str, f64, f64, Option<&str>)> = vec![
        ("Mercado Demo",       25.0, 180.0, Some(&c.groceries)),
        ("Padaria Demo",        8.0,  35.0, None),
        ("Farmácia Demo",      15.0,  80.0, Some(&c.pharmacy)),
        ("Posto Demo Gas",     80.0, 250.0, Some(&c.gas)),
        ("Restaurante Demo",   30.0, 120.0, Some(&c.restaurants)),
    ];
    let mut d = start;
    while d <= today {
        if rng.random::<f64>() < 0.25 {
            let m = &brl_merchants[rng.random_range(0..brl_merchants.len())];
            let amt = m.1 + rng.random::<f64>() * (m.2 - m.1);
            mk(
                d,
                &a.brl,
                Decimal::from_f64(-round2(amt * 5.1)).unwrap(), // BRL is ~5.1x USD
                CurrencyCode::BRL,
                m.0,
                None,
                m.3,
                vec![t.cross_border.clone()],
                None,
            )?;
        }
        d += Duration::days(1);
    }

    // --- Transfer pairs: monthly CC payment (Checking → Rewards Card) ---
    let mut month_start = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap();
    while month_start <= today {
        if let Some(d) = NaiveDate::from_ymd_opt(month_start.year(), month_start.month(), 25) {
            if d >= start && d <= today {
                let pair = Uuid::new_v4().to_string();
                let amt = Decimal::from_f64(800.0 + rng.random::<f64>() * 400.0).unwrap().round_dp(2);
                mk(
                    d,
                    &a.checking,
                    -amt,
                    CurrencyCode::USD,
                    "Northline Payment",
                    Some("Credit card autopay"),
                    Some(&c.cc_payment),
                    vec![],
                    Some(pair.clone()),
                )?;
                mk(
                    d,
                    &a.rewards,
                    amt,
                    CurrencyCode::USD,
                    "Northline Payment",
                    Some("Credit card autopay"),
                    Some(&c.cc_payment),
                    vec![],
                    Some(pair),
                )?;
                n_pairs += 1;
            }
        }
        month_start = add_one_month(month_start);
    }

    // --- Transfer pairs: quarterly brokerage contribution (Checking → Brokerage) ---
    let mut d = start;
    let mut q = 0;
    while d <= today {
        if d.day() == 15 && (d.month() == 1 || d.month() == 4 || d.month() == 7 || d.month() == 10) && q >= 0 {
            let pair = Uuid::new_v4().to_string();
            mk(
                d,
                &a.checking,
                Decimal::from_str("-1000.00")?,
                CurrencyCode::USD,
                "Quarterly Contribution",
                Some("Transfer to brokerage"),
                Some(&c.transfer),
                vec![],
                Some(pair.clone()),
            )?;
            mk(
                d,
                &a.brokerage,
                Decimal::from_str("1000.00")?,
                CurrencyCode::USD,
                "Quarterly Contribution",
                Some("Transfer from checking"),
                Some(&c.transfer),
                vec![],
                Some(pair),
            )?;
            n_pairs += 1;
            q += 1;
        }
        d += Duration::days(1);
    }

    // --- Transfer: cross-border via FX wallet (monthly) ---
    let mut month_start = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap();
    while month_start <= today {
        if let Some(d) = NaiveDate::from_ymd_opt(month_start.year(), month_start.month(), 18) {
            if d >= start && d <= today {
                mk(
                    d,
                    &a.checking,
                    Decimal::from_str("-500.00")?,
                    CurrencyCode::USD,
                    "Demo FX Send",
                    Some("Cross-border transfer"),
                    Some(&c.transfer),
                    vec![t.cross_border.clone()],
                    None,
                )?;
            }
        }
        month_start = add_one_month(month_start);
    }

    Ok((n_txns, n_splits, n_pairs))
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn add_one_month(d: NaiveDate) -> NaiveDate {
    let (y, m) = if d.month() == 12 {
        (d.year() + 1, 1)
    } else {
        (d.year(), d.month() + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1).unwrap()
}
