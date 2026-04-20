//! Monarch Money adapter (S06 T02).
//!
//! Shells out to the `mmoney` CLI (unofficial Monarch client, installed via
//! `uv tool install mmoney` — see reference_monarch memory for the install
//! recipe). The CLI already handles auth (macOS keychain), pagination,
//! and JSON envelope shape; this adapter is a thin translator that parses
//! `mmoney --format json <subcommand>` output into rtf's domain.
//!
//! We don't implement `BankSyncAdapter` here — Monarch's surface (accounts,
//! category groups, categories, tags, transactions) is richer than the
//! trait's single `sync()` method, and the SyncService wiring (T03) needs
//! all of it. A future subset-impl of BankSyncAdapter can be added if the
//! unified multi-provider code path ever needs Monarch too.
//!
//! Tests inject canned JSON via `MmoneyRunner` to avoid spawning
//! subprocesses.

use std::env;
use std::process::Command;
use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;

/// Abstracts the subprocess call to `mmoney`. Real impl spawns
/// `mmoney --format json <args...>`; tests inject canned bytes.
pub trait MmoneyRunner {
    fn run(&self, args: &[&str]) -> Result<Vec<u8>, DomainError>;
}

/// Real runner that spawns the `mmoney` binary.
///
/// Resolves the binary path in this order:
/// 1. `MMONEY_BIN` env var (full path)
/// 2. `~/.local/bin/mmoney` (the default uv-tool install location)
/// 3. Fallback to `mmoney` (resolved via PATH)
pub struct SubprocessMmoneyRunner;

impl SubprocessMmoneyRunner {
    fn resolve_binary() -> String {
        if let Ok(p) = env::var("MMONEY_BIN") {
            return p;
        }
        if let Ok(home) = env::var("HOME") {
            let candidate = format!("{home}/.local/bin/mmoney");
            if std::path::Path::new(&candidate).exists() {
                return candidate;
            }
        }
        "mmoney".to_string()
    }
}

impl MmoneyRunner for SubprocessMmoneyRunner {
    fn run(&self, args: &[&str]) -> Result<Vec<u8>, DomainError> {
        let bin = Self::resolve_binary();
        let mut full_args: Vec<&str> = vec!["--format", "json"];
        full_args.extend_from_slice(args);
        let output = Command::new(&bin)
            .args(&full_args)
            .output()
            .map_err(|e| {
                DomainError::Import(format!(
                    "failed to spawn mmoney at {bin}: {e}. Is mmoney-cli installed? Try `uv tool install --python 3.12 mmoney`."
                ))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Not authenticated") || stderr.contains("authentication") {
                return Err(DomainError::Import(
                    "mmoney is not authenticated. Run `mmoney auth login` first.".into(),
                ));
            }
            return Err(DomainError::Import(format!(
                "mmoney {} failed (exit {:?}): {}",
                args.join(" "),
                output.status.code(),
                stderr.trim()
            )));
        }
        Ok(output.stdout)
    }
}

// ===========================================================================
// Domain shapes returned by the adapter
// ===========================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonarchAccount {
    /// Monarch's account id (used as `external_id` in rtf).
    pub external_id: String,
    pub display_name: String,
    /// Monarch's top-level type: "depository", "credit", "brokerage", "loan", "vehicle", etc.
    pub account_type: String,
    /// Monarch's subtype: "checking", "credit_card", "brokerage", "student", etc.
    pub subtype: String,
    pub currency: CurrencyCode,
    pub current_balance: Decimal,
    pub is_manual: bool,
    pub is_hidden: bool,
    pub institution_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonarchCategoryGroup {
    pub external_id: String,
    pub name: String,
    /// "income", "expense", or "transfer".
    pub kind: String,
    pub order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonarchCategory {
    pub external_id: String,
    pub name: String,
    pub group_external_id: String,
    pub is_system: bool,
    pub is_disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonarchTag {
    pub external_id: String,
    pub name: String,
    pub color: Option<String>,
    pub order: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonarchTransaction {
    pub external_id: String,
    pub account_external_id: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub currency: CurrencyCode,
    pub merchant_name: Option<String>,
    pub plaid_name: Option<String>,
    pub notes: Option<String>,
    pub category_external_id: Option<String>,
    pub tag_external_ids: Vec<String>,
    pub is_pending: bool,
}

// ===========================================================================
// Adapter
// ===========================================================================

pub struct MonarchAdapter<R: MmoneyRunner> {
    runner: R,
}

impl<R: MmoneyRunner> MonarchAdapter<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

impl MonarchAdapter<SubprocessMmoneyRunner> {
    pub fn subprocess() -> Self {
        Self::new(SubprocessMmoneyRunner)
    }
}

// -- raw JSON shapes -------------------------------------------------------

#[derive(Deserialize)]
struct RawAccountsEnvelope {
    accounts: Vec<RawAccount>,
}

#[derive(Deserialize)]
struct RawAccount {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    r#type: RawAccountType,
    subtype: RawAccountSubtype,
    #[serde(rename = "displayBalance", default)]
    display_balance: serde_json::Value,
    #[serde(rename = "currentBalance", default)]
    current_balance: serde_json::Value,
    #[serde(rename = "isManual", default)]
    is_manual: bool,
    #[serde(rename = "isHidden", default)]
    is_hidden: bool,
    #[serde(default)]
    credential: Option<RawCredential>,
    #[serde(default)]
    institution: Option<RawInstitutionName>,
}

#[derive(Deserialize)]
struct RawAccountType {
    name: String,
}

#[derive(Deserialize)]
struct RawAccountSubtype {
    name: String,
}

#[derive(Deserialize)]
struct RawCredential {
    institution: Option<RawInstitutionName>,
}

#[derive(Deserialize)]
struct RawInstitutionName {
    name: String,
}

#[derive(Deserialize)]
struct RawCategoryGroupsEnvelope {
    #[serde(rename = "categoryGroups")]
    category_groups: Vec<RawCategoryGroup>,
}

#[derive(Deserialize)]
struct RawCategoryGroup {
    id: String,
    name: String,
    r#type: String,
    #[serde(default)]
    order: i64,
}

#[derive(Deserialize)]
struct RawCategoriesEnvelope {
    categories: Vec<RawCategory>,
}

#[derive(Deserialize)]
struct RawCategory {
    id: String,
    name: String,
    group: RawCategoryGroupRef,
    #[serde(rename = "isSystemCategory", default)]
    is_system: bool,
    #[serde(rename = "isDisabled", default)]
    is_disabled: bool,
}

#[derive(Deserialize)]
struct RawCategoryGroupRef {
    id: String,
}

#[derive(Deserialize)]
struct RawTagsEnvelope {
    #[serde(rename = "householdTransactionTags")]
    tags: Vec<RawTag>,
}

#[derive(Deserialize)]
struct RawTag {
    id: String,
    name: String,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    order: Option<i64>,
}

#[derive(Deserialize)]
struct RawTxnsEnvelope {
    #[serde(rename = "allTransactions")]
    all_transactions: RawTxnsResults,
}

#[derive(Deserialize)]
struct RawTxnsResults {
    results: Vec<RawTxn>,
}

#[derive(Deserialize)]
struct RawTxn {
    id: String,
    account: RawTxnAccount,
    date: String, // "YYYY-MM-DD"
    amount: serde_json::Value,
    #[serde(default)]
    merchant: Option<RawMerchant>,
    #[serde(rename = "plaidName", default)]
    plaid_name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    category: Option<RawTxnCategory>,
    #[serde(default)]
    tags: Vec<RawTxnTag>,
    #[serde(default)]
    pending: bool,
}

#[derive(Deserialize)]
struct RawTxnAccount {
    id: String,
}

#[derive(Deserialize)]
struct RawMerchant {
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawTxnCategory {
    id: String,
}

#[derive(Deserialize)]
struct RawTxnTag {
    id: String,
}

// -- parsing helpers -------------------------------------------------------

fn decimal_from_json(v: &serde_json::Value, field: &str) -> Result<Decimal, DomainError> {
    if let Some(n) = v.as_number() {
        return Decimal::from_str(&n.to_string())
            .map_err(|e| DomainError::Import(format!("{field}: bad decimal '{n}': {e}")));
    }
    if let Some(s) = v.as_str() {
        return Decimal::from_str(s)
            .map_err(|e| DomainError::Import(format!("{field}: bad decimal '{s}': {e}")));
    }
    if v.is_null() {
        return Ok(Decimal::ZERO);
    }
    Err(DomainError::Import(format!(
        "{field}: expected number or string, got {v}"
    )))
}

// Monarch accounts don't directly expose a currency field at the top level
// we've relied on; for now, default everything to USD. All observed accounts
// in the live DB are USD. T03 can extend this if multi-currency accounts
// land in Monarch.
fn account_currency_default() -> CurrencyCode {
    CurrencyCode::USD
}

// -- fetch methods ---------------------------------------------------------

impl<R: MmoneyRunner> MonarchAdapter<R> {
    pub fn fetch_accounts(&self) -> Result<Vec<MonarchAccount>, DomainError> {
        let bytes = self.runner.run(&["accounts", "list"])?;
        let env: RawAccountsEnvelope = serde_json::from_slice(&bytes)
            .map_err(|e| DomainError::Import(format!("accounts json: {e}")))?;
        env.accounts
            .into_iter()
            .map(|a| {
                let balance_value = if !a.current_balance.is_null() {
                    &a.current_balance
                } else {
                    &a.display_balance
                };
                let current_balance = decimal_from_json(balance_value, "account.balance")?;
                let institution_name = a
                    .credential
                    .and_then(|c| c.institution)
                    .map(|i| i.name)
                    .or(a.institution.map(|i| i.name));
                Ok(MonarchAccount {
                    external_id: a.id,
                    display_name: a.display_name,
                    account_type: a.r#type.name,
                    subtype: a.subtype.name,
                    currency: account_currency_default(),
                    current_balance,
                    is_manual: a.is_manual,
                    is_hidden: a.is_hidden,
                    institution_name,
                })
            })
            .collect()
    }

    pub fn fetch_category_groups(&self) -> Result<Vec<MonarchCategoryGroup>, DomainError> {
        let bytes = self.runner.run(&["categories", "groups"])?;
        let env: RawCategoryGroupsEnvelope = serde_json::from_slice(&bytes)
            .map_err(|e| DomainError::Import(format!("category groups json: {e}")))?;
        Ok(env
            .category_groups
            .into_iter()
            .map(|g| MonarchCategoryGroup {
                external_id: g.id,
                name: g.name,
                kind: g.r#type,
                order: g.order,
            })
            .collect())
    }

    pub fn fetch_categories(&self) -> Result<Vec<MonarchCategory>, DomainError> {
        let bytes = self.runner.run(&["categories", "list"])?;
        let env: RawCategoriesEnvelope = serde_json::from_slice(&bytes)
            .map_err(|e| DomainError::Import(format!("categories json: {e}")))?;
        Ok(env
            .categories
            .into_iter()
            .map(|c| MonarchCategory {
                external_id: c.id,
                name: c.name,
                group_external_id: c.group.id,
                is_system: c.is_system,
                is_disabled: c.is_disabled,
            })
            .collect())
    }

    pub fn fetch_tags(&self) -> Result<Vec<MonarchTag>, DomainError> {
        let bytes = self.runner.run(&["tags", "list"])?;
        let env: RawTagsEnvelope = serde_json::from_slice(&bytes)
            .map_err(|e| DomainError::Import(format!("tags json: {e}")))?;
        Ok(env
            .tags
            .into_iter()
            .map(|t| MonarchTag {
                external_id: t.id,
                name: t.name,
                color: t.color,
                order: t.order,
            })
            .collect())
    }

    /// Fetches transactions in the inclusive window [start, end].
    /// Pages internally in 500-row chunks until mmoney returns fewer than 500.
    pub fn fetch_transactions(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<MonarchTransaction>, DomainError> {
        const PAGE: i64 = 500;
        let start_str = start.format("%Y-%m-%d").to_string();
        let end_str = end.format("%Y-%m-%d").to_string();
        let mut offset: i64 = 0;
        let mut out = Vec::new();
        loop {
            let offset_str = offset.to_string();
            let limit_str = PAGE.to_string();
            let args: Vec<&str> = vec![
                "transactions",
                "list",
                "--start-date",
                &start_str,
                "--end-date",
                &end_str,
                "--limit",
                &limit_str,
                "--offset",
                &offset_str,
            ];
            let bytes = self.runner.run(&args)?;
            let env: RawTxnsEnvelope = serde_json::from_slice(&bytes)
                .map_err(|e| DomainError::Import(format!("transactions json: {e}")))?;
            let page_len = env.all_transactions.results.len();
            for t in env.all_transactions.results {
                let date = NaiveDate::parse_from_str(&t.date, "%Y-%m-%d")
                    .map_err(|e| DomainError::Import(format!("txn date '{}': {e}", t.date)))?;
                let amount = decimal_from_json(&t.amount, "txn.amount")?;
                out.push(MonarchTransaction {
                    external_id: t.id,
                    account_external_id: t.account.id,
                    date,
                    amount,
                    currency: CurrencyCode::USD,
                    merchant_name: t.merchant.and_then(|m| m.name),
                    plaid_name: t.plaid_name,
                    notes: t.notes,
                    category_external_id: t.category.map(|c| c.id),
                    tag_external_ids: t.tags.into_iter().map(|tg| tg.id).collect(),
                    is_pending: t.pending,
                });
            }
            if (page_len as i64) < PAGE {
                break;
            }
            offset += PAGE;
            // Safety: cap at a million transactions just to prevent infinite loops on API weirdness.
            if offset > 1_000_000 {
                return Err(DomainError::Import(
                    "transactions pagination exceeded 1M rows — aborting".into(),
                ));
            }
        }
        Ok(out)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Fake runner backed by a simple "args prefix -> canned bytes" lookup.
    /// The key is joined args, eg. "accounts list" or
    /// "transactions list --start-date 2026-03-01 --end-date 2026-03-31 --limit 500 --offset 0".
    struct FakeRunner {
        responses: RefCell<Vec<(&'static str, Vec<u8>)>>,
    }

    impl FakeRunner {
        fn new(pairs: Vec<(&'static str, &'static str)>) -> Self {
            Self {
                responses: RefCell::new(
                    pairs
                        .into_iter()
                        .map(|(k, v)| (k, v.as_bytes().to_vec()))
                        .collect(),
                ),
            }
        }
    }

    impl MmoneyRunner for FakeRunner {
        fn run(&self, args: &[&str]) -> Result<Vec<u8>, DomainError> {
            let joined = args.join(" ");
            let mut resps = self.responses.borrow_mut();
            if let Some(pos) = resps.iter().position(|(k, _)| joined.starts_with(k)) {
                return Ok(resps.remove(pos).1);
            }
            Err(DomainError::Import(format!(
                "fake runner: no canned response for args `{joined}`"
            )))
        }
    }

    const ACCOUNTS_JSON: &str = r#"{
        "accounts": [
            {
                "id": "acc-1",
                "displayName": "Chase Checking (...8332)",
                "type": {"name": "depository"},
                "subtype": {"name": "checking"},
                "currentBalance": 1648.39,
                "isManual": false,
                "isHidden": false,
                "credential": {"institution": {"name": "Chase"}}
            },
            {
                "id": "acc-2",
                "displayName": "BTC",
                "type": {"name": "brokerage"},
                "subtype": {"name": "cryptocurrency"},
                "currentBalance": 7719.78,
                "displayBalance": 7752.28,
                "isManual": true,
                "isHidden": false
            }
        ]
    }"#;

    const GROUPS_JSON: &str = r#"{
        "categoryGroups": [
            {"id": "g1", "name": "Income", "type": "income", "order": 0},
            {"id": "g2", "name": "Food & Dining", "type": "expense", "order": 5}
        ]
    }"#;

    const CATEGORIES_JSON: &str = r#"{
        "categories": [
            {"id": "c1", "name": "Paychecks", "group": {"id": "g1"}, "isSystemCategory": true, "isDisabled": false},
            {"id": "c2", "name": "Groceries", "group": {"id": "g2"}, "isSystemCategory": true, "isDisabled": false}
        ]
    }"#;

    const TAGS_JSON: &str = r##"{
        "householdTransactionTags": [
            {"id": "t1", "name": "BR", "color": "#ff0000", "order": 0},
            {"id": "t2", "name": "Subscription", "color": null, "order": 1}
        ]
    }"##;

    // Two pages so we can prove pagination: first page has 500 rows, second has 2.
    fn txns_page(n: usize, id_offset: usize) -> String {
        let mut rows = Vec::new();
        for i in 0..n {
            let id = id_offset + i;
            rows.push(format!(
                r#"{{
                    "id": "tx-{id}",
                    "account": {{"id": "acc-1"}},
                    "date": "2026-04-10",
                    "amount": -12.34,
                    "merchant": {{"name": "Casas Guanabara"}},
                    "plaidName": "CASAS GUANABARA COMES",
                    "category": {{"id": "c2"}},
                    "tags": [{{"id": "t1"}}]
                }}"#
            ));
        }
        format!(
            r#"{{"allTransactions": {{"results": [{}]}}}}"#,
            rows.join(",")
        )
    }

    #[test]
    fn fetch_accounts_parses_shape() {
        let runner = FakeRunner::new(vec![("accounts list", ACCOUNTS_JSON)]);
        let adapter = MonarchAdapter::new(runner);
        let accounts = adapter.fetch_accounts().unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].external_id, "acc-1");
        assert_eq!(accounts[0].display_name, "Chase Checking (...8332)");
        assert_eq!(accounts[0].account_type, "depository");
        assert_eq!(accounts[0].subtype, "checking");
        assert_eq!(
            accounts[0].current_balance,
            Decimal::from_str("1648.39").unwrap()
        );
        assert_eq!(accounts[0].institution_name.as_deref(), Some("Chase"));
        assert!(!accounts[0].is_manual);
        assert_eq!(accounts[1].external_id, "acc-2");
        assert_eq!(accounts[1].display_name, "BTC");
        assert!(accounts[1].is_manual);
        assert_eq!(
            accounts[1].current_balance,
            Decimal::from_str("7719.78").unwrap()
        );
    }

    #[test]
    fn fetch_category_groups_parses_shape() {
        let runner = FakeRunner::new(vec![("categories groups", GROUPS_JSON)]);
        let adapter = MonarchAdapter::new(runner);
        let groups = adapter.fetch_category_groups().unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Income");
        assert_eq!(groups[0].kind, "income");
        assert_eq!(groups[1].name, "Food & Dining");
        assert_eq!(groups[1].kind, "expense");
        assert_eq!(groups[1].order, 5);
    }

    #[test]
    fn fetch_categories_parses_shape_and_group_link() {
        let runner = FakeRunner::new(vec![("categories list", CATEGORIES_JSON)]);
        let adapter = MonarchAdapter::new(runner);
        let cats = adapter.fetch_categories().unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].name, "Paychecks");
        assert_eq!(cats[0].group_external_id, "g1");
        assert!(cats[0].is_system);
        assert_eq!(cats[1].name, "Groceries");
        assert_eq!(cats[1].group_external_id, "g2");
    }

    #[test]
    fn fetch_tags_parses_shape_with_null_color() {
        let runner = FakeRunner::new(vec![("tags list", TAGS_JSON)]);
        let adapter = MonarchAdapter::new(runner);
        let tags = adapter.fetch_tags().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "BR");
        assert_eq!(tags[0].color.as_deref(), Some("#ff0000"));
        assert_eq!(tags[0].order, Some(0));
        assert_eq!(tags[1].name, "Subscription");
        assert!(tags[1].color.is_none());
    }

    #[test]
    fn fetch_transactions_single_page() {
        let page = txns_page(3, 0);
        let runner = FakeRunner::new(vec![(
            "transactions list",
            Box::leak(page.into_boxed_str()),
        )]);
        let adapter = MonarchAdapter::new(runner);
        let txns = adapter
            .fetch_transactions(
                NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            )
            .unwrap();
        assert_eq!(txns.len(), 3);
        assert_eq!(txns[0].external_id, "tx-0");
        assert_eq!(txns[0].account_external_id, "acc-1");
        assert_eq!(txns[0].amount, Decimal::from_str("-12.34").unwrap());
        assert_eq!(txns[0].merchant_name.as_deref(), Some("Casas Guanabara"));
        assert_eq!(txns[0].category_external_id.as_deref(), Some("c2"));
        assert_eq!(txns[0].tag_external_ids, vec!["t1"]);
    }

    #[test]
    fn fetch_transactions_paginates_until_short_page() {
        let first = txns_page(500, 0);
        let second = txns_page(2, 500);
        // FakeRunner matches by args-prefix and consumes; "transactions list" matches both requests.
        let runner = FakeRunner::new(vec![
            ("transactions list", Box::leak(first.into_boxed_str())),
            ("transactions list", Box::leak(second.into_boxed_str())),
        ]);
        let adapter = MonarchAdapter::new(runner);
        let txns = adapter
            .fetch_transactions(
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
            )
            .unwrap();
        assert_eq!(txns.len(), 502);
        assert_eq!(txns[0].external_id, "tx-0");
        assert_eq!(txns[500].external_id, "tx-500");
        assert_eq!(txns[501].external_id, "tx-501");
    }

    #[test]
    fn fetch_transactions_stops_on_empty_page() {
        // Single page with fewer than 500 rows — pagination should not request a second page.
        let page = txns_page(7, 0);
        let runner = FakeRunner::new(vec![(
            "transactions list",
            Box::leak(page.into_boxed_str()),
        )]);
        let adapter = MonarchAdapter::new(runner);
        let txns = adapter
            .fetch_transactions(
                NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            )
            .unwrap();
        assert_eq!(txns.len(), 7);
        // If pagination had requested a second page, the runner would have errored
        // (no more canned responses).
    }

    #[test]
    fn fetch_transactions_handles_missing_optional_fields() {
        let json = r#"{"allTransactions": {"results": [
            {"id": "tx-1", "account": {"id": "acc-1"}, "date": "2026-04-10", "amount": 1.5}
        ]}}"#;
        let runner = FakeRunner::new(vec![("transactions list", json)]);
        let adapter = MonarchAdapter::new(runner);
        let txns = adapter
            .fetch_transactions(
                NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            )
            .unwrap();
        assert_eq!(txns.len(), 1);
        assert!(txns[0].merchant_name.is_none());
        assert!(txns[0].category_external_id.is_none());
        assert!(txns[0].tag_external_ids.is_empty());
        assert_eq!(txns[0].amount, Decimal::from_str("1.5").unwrap());
    }

    #[test]
    fn malformed_json_returns_import_error() {
        let runner = FakeRunner::new(vec![("accounts list", "{ bad json")]);
        let adapter = MonarchAdapter::new(runner);
        let err = adapter.fetch_accounts().unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("accounts json")),
            _ => panic!("expected Import error"),
        }
    }

    #[test]
    fn runner_missing_binary_surfaces_guidance() {
        struct BrokenRunner;
        impl MmoneyRunner for BrokenRunner {
            fn run(&self, _args: &[&str]) -> Result<Vec<u8>, DomainError> {
                Err(DomainError::Import(
                    "failed to spawn mmoney at /bin/mmoney: not found. Is mmoney-cli installed?"
                        .into(),
                ))
            }
        }
        let adapter = MonarchAdapter::new(BrokenRunner);
        let err = adapter.fetch_accounts().unwrap_err();
        match err {
            DomainError::Import(msg) => {
                assert!(msg.contains("mmoney-cli"));
            }
            _ => panic!("expected Import error with install guidance"),
        }
    }

    #[test]
    fn decimal_parser_handles_integer_string_and_null() {
        assert_eq!(
            decimal_from_json(&serde_json::Value::from(42), "x").unwrap(),
            Decimal::from(42)
        );
        assert_eq!(
            decimal_from_json(&serde_json::Value::from("3.14"), "x").unwrap(),
            Decimal::from_str("3.14").unwrap()
        );
        assert_eq!(
            decimal_from_json(&serde_json::Value::Null, "x").unwrap(),
            Decimal::ZERO
        );
    }

    #[test]
    fn adapter_subprocess_constructor_exists() {
        // Just a compile-time check that the subprocess constructor is wired.
        let _ = MonarchAdapter::subprocess();
    }
}
