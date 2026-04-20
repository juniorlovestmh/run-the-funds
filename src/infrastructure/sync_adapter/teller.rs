//! Teller bank-sync adapter for US banks.
//!
//! Auth: HTTP Basic with the enrollment access token as username, empty
//! password. (Teller's docs show `-u "token_xxx:"` in curl examples — that's
//! Basic auth with an empty password, NOT Bearer.)
//! API shape:
//!   1. `GET /accounts` — Bearer-authed, returns a RAW JSON array of accounts
//!      at the top level (unlike SimpleFIN's `{accounts: [...]}` wrapper).
//!   2. `GET /accounts/{id}/transactions?from_date=YYYY-MM-DD&count=500[&from_id=<cursor>]`
//!      — raw array; cursor pagination via `from_id` = last transaction's id
//!      from the previous page. Stop when returned count < requested count.
//!
//! Sign convention: Teller pre-signs amounts (debit = negative, credit =
//! positive). No inversion needed. Amounts come as signed string decimals.
//!
//! **Tier note:** Teller's Development and Production tiers require mutual
//! TLS (client certificate). Sandbox does not. This adapter uses Bearer-only;
//! a token on a tier requiring mTLS will error on first HTTP call with a
//! clear TLS-rejection body (the improved UreqHttpClient surfaces this).

use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::infrastructure::http::HttpClient;

use super::{BankSyncAdapter, RemoteTransaction};

const DEFAULT_BASE_URL: &str = "https://api.teller.io";
const PAGE_SIZE: usize = 500;

pub struct TellerAdapter<'a, C: HttpClient> {
    client: &'a C,
    access_token: String,
    base_url: String,
}

impl<'a, C: HttpClient> TellerAdapter<'a, C> {
    pub fn new(client: &'a C, access_token: String) -> Self {
        Self {
            client,
            access_token,
            base_url: DEFAULT_BASE_URL.into(),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, base: String) -> Self {
        self.base_url = base;
        self
    }

    fn authed_get(&self, url: &str) -> Result<String, DomainError> {
        // HTTP Basic auth: username = access_token, password = empty.
        // Matches Teller's docs (`-u "token_xxx:"` in curl examples).
        let encoded = B64.encode(format!("{}:", self.access_token));
        self.client.get_with_headers(
            url,
            &[
                ("Authorization", &format!("Basic {encoded}")),
                ("Accept", "application/json"),
            ],
        )
    }

    fn fetch_accounts(&self) -> Result<Vec<TellerAccount>, DomainError> {
        let url = format!("{}/accounts", self.base_url);
        let body = self.authed_get(&url)?;
        let parsed: Value = serde_json::from_str(&body)
            .map_err(|e| DomainError::Import(format!("Teller accounts JSON parse: {e}")))?;
        let arr = parsed
            .as_array()
            .ok_or_else(|| DomainError::Import("Teller /accounts expected a JSON array".into()))?;

        let mut out = Vec::new();
        for row in arr {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| DomainError::Import("Teller account missing 'id'".into()))?
                .to_string();
            let currency_str = row.get("currency").and_then(Value::as_str).ok_or_else(|| {
                DomainError::Import(format!("Teller account {id} missing 'currency'"))
            })?;
            // Silently skip accounts in currencies the domain doesn't know (e.g. EUR).
            // The caller (SyncService) only needs the accounts the user has
            // locally linked anyway; remote-only currencies are a non-event.
            let currency = match CurrencyCode::from_str(currency_str) {
                Ok(c) => c,
                Err(_) => continue,
            };
            out.push(TellerAccount { id, currency });
        }
        Ok(out)
    }

    fn fetch_transactions_for_account(
        &self,
        account_id: &str,
        currency: CurrencyCode,
        since: NaiveDate,
    ) -> Result<Vec<RemoteTransaction>, DomainError> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/accounts/{}/transactions?from_date={}&count={}",
                self.base_url,
                account_id,
                since.format("%Y-%m-%d"),
                PAGE_SIZE,
            );
            if let Some(c) = cursor.as_deref() {
                url.push_str("&from_id=");
                url.push_str(c);
            }

            let body = self.authed_get(&url)?;
            let parsed: Value = serde_json::from_str(&body)
                .map_err(|e| DomainError::Import(format!("Teller transactions JSON parse: {e}")))?;
            let rows = parsed.as_array().ok_or_else(|| {
                DomainError::Import("Teller /transactions expected a JSON array".into())
            })?;

            let page_len = rows.len();
            for row in rows {
                all.push(Self::parse_transaction(row, currency)?);
            }

            // End of pagination: server returned fewer than we asked for
            // (i.e. the last page) OR zero rows.
            if page_len < PAGE_SIZE || page_len == 0 {
                break;
            }
            // Next cursor = id of the last transaction on this page.
            cursor = all.last().map(|t| t.external_id.clone());
        }

        Ok(all)
    }

    fn parse_transaction(
        row: &Value,
        currency: CurrencyCode,
    ) -> Result<RemoteTransaction, DomainError> {
        let external_id = row
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| DomainError::Import("Teller transaction missing 'id'".into()))?
            .to_string();
        let date_str = row.get("date").and_then(Value::as_str).ok_or_else(|| {
            DomainError::Import(format!("Teller transaction {external_id} missing 'date'"))
        })?;
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| {
            DomainError::Import(format!(
                "Teller transaction {external_id} malformed date {date_str}: {e}"
            ))
        })?;
        let amount_str = row.get("amount").and_then(Value::as_str).ok_or_else(|| {
            DomainError::Import(format!("Teller transaction {external_id} missing 'amount'"))
        })?;
        let amount = Decimal::from_str(amount_str).map_err(|e| {
            DomainError::Import(format!(
                "Teller transaction {external_id} amount parse {amount_str}: {e}"
            ))
        })?;

        let description = row
            .get("description")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let payee = row
            .get("details")
            .and_then(|d| d.get("counterparty"))
            .and_then(|c| c.get("name"))
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .or_else(|| description.clone());

        Ok(RemoteTransaction {
            external_id,
            date,
            amount,
            currency,
            payee,
            description,
        })
    }
}

struct TellerAccount {
    id: String,
    currency: CurrencyCode,
}

impl<C: HttpClient> BankSyncAdapter for TellerAdapter<'_, C> {
    fn provider_name(&self) -> &'static str {
        "teller"
    }

    fn sync(
        &self,
        since: Option<NaiveDate>,
    ) -> Result<Vec<(String, Vec<RemoteTransaction>)>, DomainError> {
        let effective_since =
            since.unwrap_or_else(|| Utc::now().date_naive() - chrono::Duration::days(730));
        let accounts = self.fetch_accounts()?;
        let mut out = Vec::with_capacity(accounts.len());
        for acc in accounts {
            let txns =
                self.fetch_transactions_for_account(&acc.id, acc.currency, effective_since)?;
            out.push((acc.id, txns));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::cell::RefCell;
    use std::collections::HashMap;

    enum Canned {
        Body(String),
        Error(String),
    }

    struct FakeHttpClient {
        responses: RefCell<HashMap<String, Vec<Canned>>>,
        calls: RefCell<Vec<(String, Vec<(String, String)>)>>,
    }

    impl FakeHttpClient {
        fn new() -> Self {
            Self {
                responses: RefCell::new(HashMap::new()),
                calls: RefCell::new(Vec::new()),
            }
        }
        fn get_returns(self, url: &str, body: &str) -> Self {
            self.responses
                .borrow_mut()
                .entry(url.to_string())
                .or_default()
                .push(Canned::Body(body.to_string()));
            self
        }
        fn get_errors(self, url: &str, msg: &str) -> Self {
            self.responses
                .borrow_mut()
                .entry(url.to_string())
                .or_default()
                .push(Canned::Error(msg.to_string()));
            self
        }
        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }
        fn last_headers(&self) -> Vec<(String, String)> {
            self.calls.borrow().last().cloned().unwrap().1
        }
    }

    impl HttpClient for FakeHttpClient {
        fn get(&self, _: &str) -> Result<String, DomainError> {
            unreachable!("teller uses get_with_headers")
        }
        fn get_with_headers(
            &self,
            url: &str,
            headers: &[(&str, &str)],
        ) -> Result<String, DomainError> {
            let captured: Vec<(String, String)> = headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            self.calls.borrow_mut().push((url.to_string(), captured));
            let mut map = self.responses.borrow_mut();
            let entry = map.get_mut(url).and_then(|v| {
                if v.is_empty() {
                    None
                } else {
                    Some(v.remove(0))
                }
            });
            match entry {
                Some(Canned::Body(s)) => Ok(s),
                Some(Canned::Error(e)) => Err(DomainError::Import(e)),
                None => Err(DomainError::Import(format!("no canned GET for {url}"))),
            }
        }
    }

    const BASE: &str = "http://localhost:9998";

    fn adapter<'a>(client: &'a FakeHttpClient) -> TellerAdapter<'a, FakeHttpClient> {
        TellerAdapter::new(client, "test-token".into()).with_base_url(BASE.into())
    }

    #[test]
    fn provider_name_is_teller() {
        let client = FakeHttpClient::new();
        assert_eq!(adapter(&client).provider_name(), "teller");
    }

    #[test]
    fn happy_path_parses_signed_amounts() {
        let accounts = r#"[{"id":"acc_1","currency":"USD","name":"Chase Checking"}]"#;
        let txns = r#"[
            {"id":"txn_1","date":"2026-04-10","amount":"-45.99","description":"Whole Foods","details":{"counterparty":{"name":"Whole Foods Market"}}},
            {"id":"txn_2","date":"2026-04-11","amount":"1516.41","description":"Payroll","details":{}}
        ]"#;
        let client = FakeHttpClient::new()
            .get_returns(&format!("{BASE}/accounts"), accounts)
            .get_returns(
                &format!("{BASE}/accounts/acc_1/transactions?from_date=2026-04-01&count=500"),
                txns,
            );
        let a = adapter(&client);
        let result = a
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();

        assert_eq!(result.len(), 1);
        let (ext_id, ts) = &result[0];
        assert_eq!(ext_id, "acc_1");
        assert_eq!(ts.len(), 2);

        assert_eq!(ts[0].external_id, "txn_1");
        assert_eq!(ts[0].amount, dec!(-45.99));
        assert_eq!(ts[0].currency, CurrencyCode::USD);
        assert_eq!(ts[0].date, NaiveDate::from_ymd_opt(2026, 4, 10).unwrap());
        assert_eq!(ts[0].payee.as_deref(), Some("Whole Foods Market"));

        assert_eq!(ts[1].amount, dec!(1516.41));
        assert!(!ts[1].amount.is_sign_negative());
    }

    #[test]
    fn authorization_header_basic_token_colon_empty() {
        let accounts = r#"[]"#;
        let client = FakeHttpClient::new().get_returns(&format!("{BASE}/accounts"), accounts);
        adapter(&client)
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();
        let headers = client.last_headers();
        // Teller uses HTTP Basic with token as username + empty password.
        // base64("test-token:") = "dGVzdC10b2tlbjo="
        let expected = format!("Basic {}", B64.encode("test-token:"));
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v == &expected),
            "expected Basic auth with token:empty, got {headers:?}"
        );
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "Accept" && v == "application/json")
        );
    }

    #[test]
    fn cursor_pagination_walks_until_short_page() {
        // Build a full first page + short second page.
        let accounts = r#"[{"id":"acc_1","currency":"USD","name":"A"}]"#;
        let first_page: Vec<String> = (0..PAGE_SIZE)
            .map(|i| {
                format!(r#"{{"id":"t{i}","date":"2026-04-10","amount":"-1.00","description":"x"}}"#)
            })
            .collect();
        let first_page_body = format!("[{}]", first_page.join(","));
        let second_page_body =
            r#"[{"id":"t_last","date":"2026-04-11","amount":"-2.00","description":"y"}]"#;

        let cursor_id = format!("t{}", PAGE_SIZE - 1);
        let client = FakeHttpClient::new()
            .get_returns(&format!("{BASE}/accounts"), accounts)
            .get_returns(
                &format!("{BASE}/accounts/acc_1/transactions?from_date=2026-04-01&count=500"),
                &first_page_body,
            )
            .get_returns(
                &format!(
                    "{BASE}/accounts/acc_1/transactions?from_date=2026-04-01&count=500&from_id={cursor_id}"
                ),
                second_page_body,
            );

        let a = adapter(&client);
        let result = a
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();
        let (_, ts) = &result[0];
        assert_eq!(ts.len(), PAGE_SIZE + 1);
        assert_eq!(ts.last().unwrap().external_id, "t_last");
        // Total HTTP calls: 1 /accounts + 2 /transactions = 3.
        assert_eq!(client.call_count(), 3);
    }

    #[test]
    fn unsupported_currency_account_skipped() {
        // Teller returns a EUR account alongside USD; adapter drops EUR silently.
        let accounts = r#"[
            {"id":"acc_eur","currency":"EUR","name":"Paris account"},
            {"id":"acc_usd","currency":"USD","name":"US account"}
        ]"#;
        let txns_usd = r#"[{"id":"t1","date":"2026-04-10","amount":"-10.00","description":"x"}]"#;
        let client = FakeHttpClient::new()
            .get_returns(&format!("{BASE}/accounts"), accounts)
            .get_returns(
                &format!("{BASE}/accounts/acc_usd/transactions?from_date=2026-04-01&count=500"),
                txns_usd,
            );
        let result = adapter(&client)
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "acc_usd");
    }

    #[test]
    fn missing_transaction_amount_errors() {
        let accounts = r#"[{"id":"acc_1","currency":"USD","name":"A"}]"#;
        let txns = r#"[{"id":"bad","date":"2026-04-10","description":"no amount"}]"#;
        let client = FakeHttpClient::new()
            .get_returns(&format!("{BASE}/accounts"), accounts)
            .get_returns(
                &format!("{BASE}/accounts/acc_1/transactions?from_date=2026-04-01&count=500"),
                txns,
            );
        let err = adapter(&client)
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn malformed_date_errors() {
        let accounts = r#"[{"id":"acc_1","currency":"USD","name":"A"}]"#;
        let txns = r#"[{"id":"x","date":"not-a-date","amount":"-1.00","description":"y"}]"#;
        let client = FakeHttpClient::new()
            .get_returns(&format!("{BASE}/accounts"), accounts)
            .get_returns(
                &format!("{BASE}/accounts/acc_1/transactions?from_date=2026-04-01&count=500"),
                txns,
            );
        let err = adapter(&client)
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn http_error_on_accounts_propagates() {
        let client =
            FakeHttpClient::new().get_errors(&format!("{BASE}/accounts"), "HTTP 401 invalid_token");
        let err = adapter(&client).sync(None).unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("401")),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn description_used_as_payee_when_counterparty_absent() {
        let accounts = r#"[{"id":"acc_1","currency":"USD","name":"A"}]"#;
        let txns = r#"[{"id":"t","date":"2026-04-10","amount":"-10.00","description":"Raw memo"}]"#;
        let client = FakeHttpClient::new()
            .get_returns(&format!("{BASE}/accounts"), accounts)
            .get_returns(
                &format!("{BASE}/accounts/acc_1/transactions?from_date=2026-04-01&count=500"),
                txns,
            );
        let result = adapter(&client)
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();
        let t = &result[0].1[0];
        assert_eq!(t.payee.as_deref(), Some("Raw memo"));
        assert_eq!(t.description.as_deref(), Some("Raw memo"));
    }

    #[test]
    fn default_since_two_years_back() {
        let accounts = r#"[]"#;
        let client = FakeHttpClient::new().get_returns(&format!("{BASE}/accounts"), accounts);
        adapter(&client).sync(None).unwrap();
        // With no accounts we only hit /accounts. Assert one call.
        assert_eq!(client.call_count(), 1);
    }

    #[test]
    fn empty_transactions_stops_pagination_immediately() {
        let accounts = r#"[{"id":"acc_1","currency":"USD","name":"A"}]"#;
        let client = FakeHttpClient::new()
            .get_returns(&format!("{BASE}/accounts"), accounts)
            .get_returns(
                &format!("{BASE}/accounts/acc_1/transactions?from_date=2026-04-01&count=500"),
                "[]",
            );
        let result = adapter(&client)
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();
        assert!(result[0].1.is_empty());
        assert_eq!(client.call_count(), 2);
    }
}
