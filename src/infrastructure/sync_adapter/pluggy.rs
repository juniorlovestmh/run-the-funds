//! Pluggy bank-sync adapter for Brazilian banks.
//!
//! Auth flow:
//!   1. POST https://api.pluggy.ai/auth with `{"clientId":..., "clientSecret":...}`
//!      returns `{"apiKey":"..."}`. Cache for the adapter's lifetime.
//!   2. Every subsequent call uses `X-API-KEY: <apiKey>` header.
//!
//! Fetch flow:
//!   3. GET /accounts?itemId=<ITEM_ID> — list of accounts under the item.
//!   4. For each account: GET /transactions?accountId=<ID>&from=YYYY-MM-DD&pageSize=500&page=N,
//!      looping across pages until `page == totalPages`.
//!
//! Sign convention: Pluggy emits positive amounts with a separate `type` field
//! (`DEBIT`/`CREDIT`). Our convention: debit = negative, credit = positive. We
//! always derive sign from `type` and use `abs(amount)` as magnitude — safer
//! than trusting whatever sign the connector happens to emit.

use std::cell::RefCell;
use std::str::FromStr;

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::infrastructure::http::HttpClient;

use super::{BankSyncAdapter, RemoteTransaction};

const DEFAULT_BASE_URL: &str = "https://api.pluggy.ai";
const PAGE_SIZE: usize = 500;

pub struct PluggyAdapter<'a, C: HttpClient> {
    client: &'a C,
    client_id: String,
    client_secret: String,
    item_id: String,
    base_url: String,
    api_key: RefCell<Option<String>>,
}

impl<'a, C: HttpClient> PluggyAdapter<'a, C> {
    pub fn new(client: &'a C, client_id: String, client_secret: String, item_id: String) -> Self {
        Self {
            client,
            client_id,
            client_secret,
            item_id,
            base_url: DEFAULT_BASE_URL.into(),
            api_key: RefCell::new(None),
        }
    }

    /// Test helper: override the base URL so a mock HTTP server can respond.
    #[cfg(test)]
    pub fn with_base_url(mut self, base: String) -> Self {
        self.base_url = base;
        self
    }

    fn authed_get(&self, url: &str) -> Result<String, DomainError> {
        let key = self.ensure_api_key()?;
        self.client.get_with_headers(
            url,
            &[
                ("X-API-KEY", &key),
                ("Accept", "application/json"),
            ],
        )
    }

    fn ensure_api_key(&self) -> Result<String, DomainError> {
        if let Some(k) = self.api_key.borrow().as_ref() {
            return Ok(k.clone());
        }
        let body = serde_json::json!({
            "clientId": self.client_id,
            "clientSecret": self.client_secret,
        })
        .to_string();
        let response = self.client.post(
            &format!("{}/auth", self.base_url),
            &body,
            &[("Content-Type", "application/json")],
        )?;
        let parsed: Value = serde_json::from_str(&response).map_err(|e| {
            DomainError::Import(format!("Pluggy auth JSON parse: {e}"))
        })?;
        let key = parsed
            .get("apiKey")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DomainError::Import("Pluggy auth response missing 'apiKey'".into())
            })?
            .to_string();
        *self.api_key.borrow_mut() = Some(key.clone());
        Ok(key)
    }

    fn fetch_accounts(&self) -> Result<Vec<PluggyAccount>, DomainError> {
        let url = format!("{}/accounts?itemId={}", self.base_url, self.item_id);
        let body = self.authed_get(&url)?;
        let parsed: Value = serde_json::from_str(&body).map_err(|e| {
            DomainError::Import(format!("Pluggy accounts JSON parse: {e}"))
        })?;
        let results = parsed
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                DomainError::Import("Pluggy accounts response missing 'results'".into())
            })?;
        let mut out = Vec::with_capacity(results.len());
        for row in results {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| DomainError::Import("Pluggy account missing 'id'".into()))?
                .to_string();
            let currency_str = row
                .get("currencyCode")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    DomainError::Import(format!(
                        "Pluggy account {id} missing 'currencyCode'"
                    ))
                })?;
            let currency = CurrencyCode::from_str(currency_str).map_err(|e| {
                DomainError::Import(format!(
                    "Pluggy account {id} unsupported currency {currency_str}: {e}"
                ))
            })?;
            out.push(PluggyAccount { id, currency });
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
        let mut page = 1usize;
        loop {
            let url = format!(
                "{}/transactions?accountId={}&from={}&pageSize={}&page={}",
                self.base_url,
                account_id,
                since.format("%Y-%m-%d"),
                PAGE_SIZE,
                page,
            );
            let body = self.authed_get(&url)?;
            let parsed: Value = serde_json::from_str(&body).map_err(|e| {
                DomainError::Import(format!("Pluggy transactions JSON parse: {e}"))
            })?;
            let total_pages = parsed
                .get("totalPages")
                .and_then(Value::as_u64)
                .unwrap_or(1);
            let results = parsed
                .get("results")
                .and_then(Value::as_array)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            for row in results {
                all.push(Self::parse_transaction(row, currency)?);
            }

            if (page as u64) >= total_pages || total_pages == 0 {
                break;
            }
            page += 1;
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
            .ok_or_else(|| DomainError::Import("Pluggy transaction missing 'id'".into()))?
            .to_string();

        // `date` is an ISO datetime string like "2024-04-08T13:00:00.000Z".
        let date_str = row
            .get("date")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DomainError::Import(format!("Pluggy transaction {external_id} missing 'date'"))
            })?;
        let date_prefix = date_str.get(..10).ok_or_else(|| {
            DomainError::Import(format!(
                "Pluggy transaction {external_id} date too short: {date_str}"
            ))
        })?;
        let date = NaiveDate::parse_from_str(date_prefix, "%Y-%m-%d").map_err(|e| {
            DomainError::Import(format!(
                "Pluggy transaction {external_id} malformed date {date_str}: {e}"
            ))
        })?;

        // Amount can arrive as a JSON number OR string depending on connector;
        // accept both. We take the absolute value and let `type` determine sign.
        let raw_amount = row.get("amount").ok_or_else(|| {
            DomainError::Import(format!(
                "Pluggy transaction {external_id} missing 'amount'"
            ))
        })?;
        let amount_magnitude = if let Some(s) = raw_amount.as_str() {
            Decimal::from_str(s).map_err(|e| {
                DomainError::Import(format!(
                    "Pluggy transaction {external_id} amount parse {s}: {e}"
                ))
            })?
        } else if let Some(f) = raw_amount.as_f64() {
            // f64 → 4dp-safe Decimal via string formatting.
            Decimal::from_str(&format!("{f:.4}")).map_err(|e| {
                DomainError::Import(format!(
                    "Pluggy transaction {external_id} amount parse from f64: {e}"
                ))
            })?
        } else {
            return Err(DomainError::Import(format!(
                "Pluggy transaction {external_id} amount is neither string nor number"
            )));
        };
        let magnitude = amount_magnitude.abs();

        let tx_type = row
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DomainError::Import(format!(
                    "Pluggy transaction {external_id} missing 'type'"
                ))
            })?;
        let amount = match tx_type {
            "DEBIT" => -magnitude,
            "CREDIT" => magnitude,
            other => {
                return Err(DomainError::Import(format!(
                    "Pluggy transaction {external_id} unknown type '{other}' (expected DEBIT or CREDIT)"
                )));
            }
        };

        let description = row
            .get("description")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let payee = row
            .get("merchant")
            .and_then(|m| m.get("name"))
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

struct PluggyAccount {
    id: String,
    currency: CurrencyCode,
}

impl<C: HttpClient> BankSyncAdapter for PluggyAdapter<'_, C> {
    fn provider_name(&self) -> &'static str {
        "pluggy"
    }

    fn sync(
        &self,
        since: Option<NaiveDate>,
    ) -> Result<Vec<(String, Vec<RemoteTransaction>)>, DomainError> {
        let effective_since = since
            .unwrap_or_else(|| Utc::now().date_naive() - chrono::Duration::days(730));
        let accounts = self.fetch_accounts()?;
        let mut out = Vec::with_capacity(accounts.len());
        for acc in accounts {
            let txns = self.fetch_transactions_for_account(&acc.id, acc.currency, effective_since)?;
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
        // Keyed by URL. Returns responses in order for repeat calls to same URL.
        responses: RefCell<HashMap<String, Vec<Canned>>>,
        post_responses: RefCell<Vec<Canned>>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeHttpClient {
        fn new() -> Self {
            Self {
                responses: RefCell::new(HashMap::new()),
                post_responses: RefCell::new(Vec::new()),
                calls: RefCell::new(Vec::new()),
            }
        }
        fn get_returns(self, url: &str, body: &str) -> Self {
            self.responses
                .borrow_mut()
                .entry(url.to_string())
                .or_insert_with(Vec::new)
                .push(Canned::Body(body.to_string()));
            self
        }
        fn post_returns(self, body: &str) -> Self {
            self.post_responses
                .borrow_mut()
                .push(Canned::Body(body.to_string()));
            self
        }
        fn post_errors(self, msg: &str) -> Self {
            self.post_responses
                .borrow_mut()
                .push(Canned::Error(msg.to_string()));
            self
        }
        fn get_errors(self, url: &str, msg: &str) -> Self {
            self.responses
                .borrow_mut()
                .entry(url.to_string())
                .or_insert_with(Vec::new)
                .push(Canned::Error(msg.to_string()));
            self
        }
        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl HttpClient for FakeHttpClient {
        fn get(&self, _: &str) -> Result<String, DomainError> {
            unreachable!("pluggy adapter uses get_with_headers")
        }
        fn get_with_headers(
            &self,
            url: &str,
            _headers: &[(&str, &str)],
        ) -> Result<String, DomainError> {
            self.calls.borrow_mut().push(url.to_string());
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
                None => Err(DomainError::Import(format!(
                    "FakeHttpClient: no canned GET for {url}"
                ))),
            }
        }
        fn post(
            &self,
            url: &str,
            _body: &str,
            _headers: &[(&str, &str)],
        ) -> Result<String, DomainError> {
            self.calls.borrow_mut().push(format!("POST {url}"));
            match self.post_responses.borrow_mut().remove(0) {
                Canned::Body(s) => Ok(s),
                Canned::Error(e) => Err(DomainError::Import(e)),
            }
        }
    }

    const BASE: &str = "http://localhost:9999";

    fn adapter<'a>(client: &'a FakeHttpClient) -> PluggyAdapter<'a, FakeHttpClient> {
        PluggyAdapter::new(client, "cid".into(), "csec".into(), "item-42".into())
            .with_base_url(BASE.into())
    }

    fn auth_body(api_key: &str) -> String {
        format!(r#"{{"apiKey":"{api_key}"}}"#)
    }

    fn accounts_body(account_json: &str) -> String {
        format!(
            r#"{{"page":1,"total":1,"totalPages":1,"results":[{account_json}]}}"#
        )
    }

    fn txns_body(results: &str, total_pages: usize) -> String {
        format!(
            r#"{{"page":1,"total":{total_pages},"totalPages":{total_pages},"results":[{results}]}}"#
        )
    }

    #[test]
    fn provider_name_is_pluggy() {
        let client = FakeHttpClient::new();
        let a = adapter(&client);
        assert_eq!(a.provider_name(), "pluggy");
    }

    #[test]
    fn happy_path_parses_debit_with_sign_inversion() {
        let acc = r#"{"id":"a1","currencyCode":"BRL","name":"Conta"}"#;
        let txn = r#"{"id":"t1","date":"2026-04-10T13:00:00.000Z","amount":150.00,"type":"DEBIT","description":"Supermercado"}"#;
        let client = FakeHttpClient::new()
            .post_returns(&auth_body("key-xyz"))
            .get_returns(&format!("{BASE}/accounts?itemId=item-42"), &accounts_body(acc))
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=1"),
                &txns_body(txn, 1),
            );
        let a = adapter(&client);
        let result = a
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();
        assert_eq!(result.len(), 1);
        let (account_id, txns) = &result[0];
        assert_eq!(account_id, "a1");
        assert_eq!(txns.len(), 1);
        let t = &txns[0];
        assert_eq!(t.amount, dec!(-150.00));
        assert_eq!(t.currency, CurrencyCode::BRL);
        assert_eq!(t.date, NaiveDate::from_ymd_opt(2026, 4, 10).unwrap());
        assert_eq!(t.description.as_deref(), Some("Supermercado"));
    }

    #[test]
    fn credit_keeps_positive_sign() {
        let acc = r#"{"id":"a1","currencyCode":"BRL","name":"Conta"}"#;
        let txn = r#"{"id":"t1","date":"2026-04-10T13:00:00Z","amount":2000.00,"type":"CREDIT","description":"Payroll"}"#;
        let client = FakeHttpClient::new()
            .post_returns(&auth_body("k"))
            .get_returns(&format!("{BASE}/accounts?itemId=item-42"), &accounts_body(acc))
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=1"),
                &txns_body(txn, 1),
            );
        let a = adapter(&client);
        let result = a
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap();
        let t = &result[0].1[0];
        assert_eq!(t.amount, dec!(2000.00));
        assert!(!t.amount.is_sign_negative());
    }

    #[test]
    fn already_negative_amount_still_respects_type() {
        // Some connectors pre-sign amounts. Our logic always derives sign from
        // `type`, so a DEBIT with a negative number stays negative.
        let acc = r#"{"id":"a1","currencyCode":"BRL","name":"Conta"}"#;
        let txn = r#"{"id":"t1","date":"2026-04-10","amount":-50.00,"type":"DEBIT","description":"X"}"#;
        let client = FakeHttpClient::new()
            .post_returns(&auth_body("k"))
            .get_returns(&format!("{BASE}/accounts?itemId=item-42"), &accounts_body(acc))
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=1"),
                &txns_body(txn, 1),
            );
        let a = adapter(&client);
        let result = a.sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap())).unwrap();
        assert_eq!(result[0].1[0].amount, dec!(-50.00));
    }

    #[test]
    fn auth_is_cached_across_calls() {
        // Two accounts → one auth call, one accounts call, two transactions calls.
        let a1 = r#"{"id":"a1","currencyCode":"BRL","name":"A1"}"#;
        let a2 = r#"{"id":"a2","currencyCode":"BRL","name":"A2"}"#;
        let client = FakeHttpClient::new()
            .post_returns(&auth_body("k"))
            .get_returns(
                &format!("{BASE}/accounts?itemId=item-42"),
                &format!(
                    r#"{{"page":1,"total":2,"totalPages":1,"results":[{a1},{a2}]}}"#
                ),
            )
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=1"),
                &txns_body("", 1),
            )
            .get_returns(
                &format!("{BASE}/transactions?accountId=a2&from=2026-04-01&pageSize=500&page=1"),
                &txns_body("", 1),
            );
        let a = adapter(&client);
        a.sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap())).unwrap();
        // Exactly 4 calls: 1 POST /auth, 1 GET /accounts, 2 GET /transactions.
        assert_eq!(client.call_count(), 4);
    }

    #[test]
    fn pagination_walks_all_pages() {
        let acc = r#"{"id":"a1","currencyCode":"BRL","name":"A"}"#;
        let p1_txn = r#"{"id":"p1","date":"2026-04-10","amount":10,"type":"DEBIT","description":"one"}"#;
        let p2_txn = r#"{"id":"p2","date":"2026-04-11","amount":20,"type":"DEBIT","description":"two"}"#;
        let p3_txn = r#"{"id":"p3","date":"2026-04-12","amount":30,"type":"DEBIT","description":"three"}"#;

        let client = FakeHttpClient::new()
            .post_returns(&auth_body("k"))
            .get_returns(&format!("{BASE}/accounts?itemId=item-42"), &accounts_body(acc))
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=1"),
                &format!(r#"{{"page":1,"total":3,"totalPages":3,"results":[{p1_txn}]}}"#),
            )
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=2"),
                &format!(r#"{{"page":2,"total":3,"totalPages":3,"results":[{p2_txn}]}}"#),
            )
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=3"),
                &format!(r#"{{"page":3,"total":3,"totalPages":3,"results":[{p3_txn}]}}"#),
            );
        let a = adapter(&client);
        let result = a.sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap())).unwrap();
        assert_eq!(result[0].1.len(), 3);
        let ids: Vec<&str> = result[0].1.iter().map(|t| t.external_id.as_str()).collect();
        assert_eq!(ids, vec!["p1", "p2", "p3"]);
    }

    #[test]
    fn auth_failure_propagates() {
        let client = FakeHttpClient::new().post_errors("HTTP 401 invalid credentials");
        let a = adapter(&client);
        let err = a.sync(None).unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("401")),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn auth_response_missing_api_key_errors() {
        let client = FakeHttpClient::new().post_returns(r#"{"wrong":"shape"}"#);
        let a = adapter(&client);
        let err = a.sync(None).unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn merchant_name_preferred_as_payee() {
        let acc = r#"{"id":"a1","currencyCode":"BRL","name":"A"}"#;
        let txn = r#"{"id":"t1","date":"2026-04-10","amount":10,"type":"DEBIT","description":"raw memo","merchant":{"name":"Real Merchant"}}"#;
        let client = FakeHttpClient::new()
            .post_returns(&auth_body("k"))
            .get_returns(&format!("{BASE}/accounts?itemId=item-42"), &accounts_body(acc))
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=1"),
                &txns_body(txn, 1),
            );
        let a = adapter(&client);
        let result = a.sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap())).unwrap();
        let t = &result[0].1[0];
        assert_eq!(t.payee.as_deref(), Some("Real Merchant"));
        assert_eq!(t.description.as_deref(), Some("raw memo"));
    }

    #[test]
    fn unknown_type_errors() {
        let acc = r#"{"id":"a1","currencyCode":"BRL","name":"A"}"#;
        let txn = r#"{"id":"t1","date":"2026-04-10","amount":10,"type":"UNKNOWN","description":"x"}"#;
        let client = FakeHttpClient::new()
            .post_returns(&auth_body("k"))
            .get_returns(&format!("{BASE}/accounts?itemId=item-42"), &accounts_body(acc))
            .get_returns(
                &format!("{BASE}/transactions?accountId=a1&from=2026-04-01&pageSize=500&page=1"),
                &txns_body(txn, 1),
            );
        let a = adapter(&client);
        let err = a
            .sync(Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()))
            .unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn http_error_on_accounts_propagates() {
        let client = FakeHttpClient::new()
            .post_returns(&auth_body("k"))
            .get_errors(&format!("{BASE}/accounts?itemId=item-42"), "HTTP 429 rate limited");
        let a = adapter(&client);
        let err = a.sync(None).unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("429")),
            other => panic!("expected Import, got {other:?}"),
        }
    }
}
