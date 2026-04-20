//! SimpleFIN bank-sync adapter for US banks.
//!
//! SimpleFIN's access URL embeds basic-auth credentials:
//!   `https://<user>:<pass>@<host>/<path>`
//! We parse the URL, extract user+pass, and GET `<base>/accounts?start-date=<unix>&pending=0`
//! with a `Authorization: Basic ...` header. Response body is JSON containing
//! `accounts[]`, each with embedded `transactions[]`.
//!
//! Sign convention matches ours: negative = debit, positive = credit. No
//! inversion needed. Amounts arrive as strings to preserve Decimal precision.

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::infrastructure::http::HttpClient;

use super::{BankSyncAdapter, RemoteTransaction};

pub struct SimpleFinAdapter<'a, C: HttpClient> {
    client: &'a C,
    access_url: String,
}

impl<'a, C: HttpClient> SimpleFinAdapter<'a, C> {
    pub fn new(client: &'a C, access_url: String) -> Self {
        Self { client, access_url }
    }

    /// Splits an access URL `https://user:pass@host/path` into
    /// `(base_url_without_creds, user, pass)`. Returns an Import error if
    /// the URL is malformed or missing credentials.
    fn split_access_url(access_url: &str) -> Result<(String, String, String), DomainError> {
        let (scheme, rest) = access_url
            .split_once("://")
            .ok_or_else(|| DomainError::Import(format!("SimpleFIN access URL missing scheme: {access_url}")))?;
        let (userinfo, host_and_path) = rest
            .split_once('@')
            .ok_or_else(|| DomainError::Import("SimpleFIN access URL missing user:pass@host".into()))?;
        let (user, pass) = userinfo
            .split_once(':')
            .ok_or_else(|| DomainError::Import("SimpleFIN access URL userinfo must be user:pass".into()))?;
        Ok((
            format!("{scheme}://{host_and_path}"),
            user.to_string(),
            pass.to_string(),
        ))
    }

    fn build_accounts_url(base: &str, since: NaiveDate) -> String {
        // SimpleFIN's start-date is a unix timestamp (seconds). We use the
        // UTC midnight of the queried date as a deterministic boundary.
        let start_ts = since
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let sep = if base.contains('?') { '&' } else { '?' };
        format!("{base}/accounts{sep}start-date={start_ts}&pending=0")
    }

    fn parse_body(
        body: &str,
    ) -> Result<Vec<(String, Vec<RemoteTransaction>)>, DomainError> {
        let parsed: Value = serde_json::from_str(body)
            .map_err(|e| DomainError::Import(format!("SimpleFIN JSON parse: {e}")))?;

        // SimpleFIN sometimes returns a top-level `errors` array. Surface
        // any non-empty entries so the operator can see provider-side issues.
        if let Some(errs) = parsed.get("errors").and_then(Value::as_array) {
            if !errs.is_empty() {
                return Err(DomainError::Import(format!(
                    "SimpleFIN reported errors: {errs:?}"
                )));
            }
        }

        let accounts = parsed
            .get("accounts")
            .and_then(Value::as_array)
            .ok_or_else(|| DomainError::Import("SimpleFIN response missing 'accounts'".into()))?;

        let mut out = Vec::with_capacity(accounts.len());
        for acc in accounts {
            let external_account_id = acc
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| DomainError::Import("SimpleFIN account missing 'id'".into()))?
                .to_string();
            let currency_str = acc
                .get("currency")
                .and_then(Value::as_str)
                .ok_or_else(|| DomainError::Import(format!(
                    "SimpleFIN account {external_account_id} missing 'currency'"
                )))?;
            let currency = CurrencyCode::from_str(currency_str).map_err(|e| {
                DomainError::Import(format!(
                    "SimpleFIN account {external_account_id} unsupported currency {currency_str}: {e}"
                ))
            })?;

            let transactions = acc
                .get("transactions")
                .and_then(Value::as_array)
                .map(|arr| arr.as_slice())
                .unwrap_or(&[]);

            let mut remote_txns = Vec::with_capacity(transactions.len());
            for tx in transactions {
                let external_id = tx
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| DomainError::Import(format!(
                        "SimpleFIN transaction under {external_account_id} missing 'id'"
                    )))?
                    .to_string();
                let posted = tx
                    .get("posted")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| DomainError::Import(format!(
                        "SimpleFIN transaction {external_id} missing 'posted'"
                    )))?;
                let date = DateTime::from_timestamp(posted, 0)
                    .ok_or_else(|| DomainError::Import(format!(
                        "SimpleFIN transaction {external_id} 'posted' out of range: {posted}"
                    )))?
                    .with_timezone(&Utc)
                    .date_naive();
                let amount_str = tx
                    .get("amount")
                    .and_then(Value::as_str)
                    .ok_or_else(|| DomainError::Import(format!(
                        "SimpleFIN transaction {external_id} missing 'amount'"
                    )))?;
                let amount = Decimal::from_str(amount_str).map_err(|e| {
                    DomainError::Import(format!(
                        "SimpleFIN transaction {external_id} amount parse {amount_str}: {e}"
                    ))
                })?;

                let description = tx
                    .get("description")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string());
                let payee = tx
                    .get("payee")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
                    .or_else(|| description.clone());

                remote_txns.push(RemoteTransaction {
                    external_id,
                    date,
                    amount,
                    currency,
                    payee,
                    description,
                });
            }

            out.push((external_account_id, remote_txns));
        }

        Ok(out)
    }
}

impl<C: HttpClient> BankSyncAdapter for SimpleFinAdapter<'_, C> {
    fn provider_name(&self) -> &'static str {
        "simplefin"
    }

    fn sync(
        &self,
        since: Option<NaiveDate>,
    ) -> Result<Vec<(String, Vec<RemoteTransaction>)>, DomainError> {
        let (base, user, pass) = Self::split_access_url(&self.access_url)?;
        // Default to today - 2 years on first sync.
        let effective_since = since
            .unwrap_or_else(|| Utc::now().date_naive() - chrono::Duration::days(730));
        let url = Self::build_accounts_url(&base, effective_since);
        let body = self.client.get_with_basic_auth(&url, &user, &pass)?;
        Self::parse_body(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::cell::RefCell;

    enum Canned {
        Body(String),
        Error(String),
    }

    struct FakeHttpClient {
        responses: RefCell<Vec<Canned>>,
        calls: RefCell<Vec<(String, Option<(String, String)>)>>,
    }

    impl FakeHttpClient {
        fn new() -> Self {
            Self {
                responses: RefCell::new(Vec::new()),
                calls: RefCell::new(Vec::new()),
            }
        }
        fn respond(self, body: &str) -> Self {
            self.responses.borrow_mut().push(Canned::Body(body.to_string()));
            self
        }
        fn error(self, msg: &str) -> Self {
            self.responses.borrow_mut().push(Canned::Error(msg.to_string()));
            self
        }
        fn last_call(&self) -> (String, Option<(String, String)>) {
            self.calls.borrow().last().cloned().expect("no calls recorded")
        }
    }

    impl HttpClient for FakeHttpClient {
        fn get(&self, url: &str) -> Result<String, DomainError> {
            self.calls.borrow_mut().push((url.to_string(), None));
            match self.responses.borrow_mut().remove(0) {
                Canned::Body(s) => Ok(s),
                Canned::Error(e) => Err(DomainError::Import(e)),
            }
        }
        fn get_with_basic_auth(
            &self,
            url: &str,
            user: &str,
            pass: &str,
        ) -> Result<String, DomainError> {
            self.calls
                .borrow_mut()
                .push((url.to_string(), Some((user.to_string(), pass.to_string()))));
            match self.responses.borrow_mut().remove(0) {
                Canned::Body(s) => Ok(s),
                Canned::Error(e) => Err(DomainError::Import(e)),
            }
        }
    }

    fn sample_body(accounts_json: &str) -> String {
        format!(r#"{{"errors":[],"accounts":[{accounts_json}]}}"#)
    }

    fn account_json(id: &str, currency: &str, txns: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"Test","currency":"{currency}","balance":"0","transactions":[{txns}]}}"#
        )
    }

    fn txn_json(id: &str, posted: i64, amount: &str, description: &str) -> String {
        format!(
            r#"{{"id":"{id}","posted":{posted},"amount":"{amount}","description":"{description}"}}"#
        )
    }

    #[test]
    fn split_access_url_extracts_creds() {
        let (base, user, pass) = SimpleFinAdapter::<FakeHttpClient>::split_access_url(
            "https://user1:pass2@bridge.simplefin.org/simplefin",
        )
        .unwrap();
        assert_eq!(base, "https://bridge.simplefin.org/simplefin");
        assert_eq!(user, "user1");
        assert_eq!(pass, "pass2");
    }

    #[test]
    fn split_access_url_rejects_missing_scheme() {
        let err = SimpleFinAdapter::<FakeHttpClient>::split_access_url("user:pass@host/path")
            .unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn split_access_url_rejects_missing_userinfo() {
        let err = SimpleFinAdapter::<FakeHttpClient>::split_access_url("https://host/path")
            .unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn sync_uses_basic_auth_and_start_date() {
        let txn = txn_json("t1", 1_712_534_400, "-45.99", "Whole Foods");
        let account = account_json("sf-chase-123", "USD", &txn);
        let body = sample_body(&account);

        let client = FakeHttpClient::new().respond(&body);
        let adapter = SimpleFinAdapter::new(
            &client,
            "https://u:p@bridge.simplefin.org/simplefin".into(),
        );

        let result = adapter
            .sync(Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()))
            .unwrap();
        let (url, auth) = client.last_call();
        assert!(url.starts_with("https://bridge.simplefin.org/simplefin/accounts"));
        assert!(url.contains("start-date=")); // unix ts present
        assert!(url.contains("pending=0"));
        assert_eq!(auth, Some(("u".into(), "p".into())));

        assert_eq!(result.len(), 1);
        let (ext_id, txns) = &result[0];
        assert_eq!(ext_id, "sf-chase-123");
        assert_eq!(txns.len(), 1);
        let t = &txns[0];
        assert_eq!(t.external_id, "t1");
        assert_eq!(t.amount, dec!(-45.99));
        assert_eq!(t.currency, CurrencyCode::USD);
        assert_eq!(t.description.as_deref(), Some("Whole Foods"));
        // Unix 1712534400 = 2024-04-08 UTC
        assert_eq!(t.date, NaiveDate::from_ymd_opt(2024, 4, 8).unwrap());
    }

    #[test]
    fn sync_default_since_is_two_years_back() {
        let body = sample_body(&account_json("a", "USD", ""));
        let client = FakeHttpClient::new().respond(&body);
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        adapter.sync(None).unwrap();
        let (url, _) = client.last_call();
        // 730 days back from today: start-date should be < today-unix - 720 days
        // (loose check that survives running on any date).
        let min_expected = (Utc::now() - chrono::Duration::days(735)).timestamp();
        let max_expected = (Utc::now() - chrono::Duration::days(725)).timestamp();
        let start_ts_str: &str = url.split("start-date=").nth(1).unwrap();
        let start_ts: i64 = start_ts_str.split('&').next().unwrap().parse().unwrap();
        assert!(
            start_ts >= min_expected && start_ts <= max_expected,
            "start-date {start_ts} not within [{min_expected}, {max_expected}]"
        );
    }

    #[test]
    fn sync_parses_multiple_accounts_and_transactions() {
        let a1 = account_json(
            "a1",
            "USD",
            &format!(
                "{},{}",
                txn_json("t1", 1_712_534_400, "-10.00", "A"),
                txn_json("t2", 1_712_620_800, "100.00", "B")
            ),
        );
        let a2 = account_json("a2", "USD", &txn_json("t3", 1_712_707_200, "-20.00", "C"));
        let body = sample_body(&format!("{a1},{a2}"));

        let client = FakeHttpClient::new().respond(&body);
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        let result = adapter.sync(None).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "a1");
        assert_eq!(result[0].1.len(), 2);
        assert_eq!(result[1].0, "a2");
        assert_eq!(result[1].1.len(), 1);
    }

    #[test]
    fn sync_surfaces_provider_errors() {
        let body = r#"{"errors":["Connection issue with bank ACME"],"accounts":[]}"#;
        let client = FakeHttpClient::new().respond(body);
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        let err = adapter.sync(None).unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("ACME")),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn sync_account_missing_currency_errors() {
        let body = r#"{"errors":[],"accounts":[{"id":"a","name":"x","transactions":[]}]}"#;
        let client = FakeHttpClient::new().respond(body);
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        let err = adapter.sync(None).unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn sync_transaction_missing_id_errors() {
        let body = r#"{"errors":[],"accounts":[{"id":"a","currency":"USD","transactions":[{"posted":1,"amount":"1.00"}]}]}"#;
        let client = FakeHttpClient::new().respond(body);
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        let err = adapter.sync(None).unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn sync_preserves_sign_on_credits_and_debits() {
        let body = sample_body(&account_json(
            "a",
            "USD",
            &format!(
                "{},{}",
                txn_json("debit", 1_712_534_400, "-123.45", "debit"),
                txn_json("credit", 1_712_620_800, "1516.41", "payroll")
            ),
        ));
        let client = FakeHttpClient::new().respond(&body);
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        let result = adapter.sync(None).unwrap();
        let (_, txns) = &result[0];
        assert_eq!(txns[0].amount, dec!(-123.45));
        assert!(txns[0].amount.is_sign_negative());
        assert_eq!(txns[1].amount, dec!(1516.41));
        assert!(!txns[1].amount.is_sign_negative());
    }

    #[test]
    fn provider_name_is_simplefin() {
        let client = FakeHttpClient::new();
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        assert_eq!(adapter.provider_name(), "simplefin");
    }

    #[test]
    fn http_error_propagates() {
        let client = FakeHttpClient::new().error("HTTP 503");
        let adapter = SimpleFinAdapter::new(&client, "https://u:p@host/".into());
        let err = adapter.sync(None).unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("503")),
            other => panic!("expected Import, got {other:?}"),
        }
    }
}
