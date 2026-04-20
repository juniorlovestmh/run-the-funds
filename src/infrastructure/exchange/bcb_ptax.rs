//! Banco Central do Brasil PTAX rate provider.
//!
//! Hits the Olinda OData endpoint:
//!   https://olinda.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata/CotacaoDolarDia(dataCotacao=@dataCotacao)?@dataCotacao='MM-DD-YYYY'&$format=json
//!
//! Response shape:
//!   { "value": [ { "cotacaoCompra": 5.12, "cotacaoVenda": 5.13, "dataHoraCotacao": "..." } ] }
//!
//! Rate semantics: BCB publishes BRL-per-USD. We compute the midpoint of
//! compra (buy) and venda (sell) for a neutral rate, then invert for BRL→USD.

use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::infrastructure::http::{HttpClient, UreqHttpClient};

use super::RateProvider;

pub struct BcbPtaxProvider<C: HttpClient> {
    client: C,
}

impl BcbPtaxProvider<UreqHttpClient> {
    pub fn new() -> Self {
        Self {
            client: UreqHttpClient::new(),
        }
    }
}

impl Default for BcbPtaxProvider<UreqHttpClient> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: HttpClient> BcbPtaxProvider<C> {
    pub fn with_client(client: C) -> Self {
        Self { client }
    }

    fn build_url(date: NaiveDate) -> String {
        // BCB's endpoint wants MM-DD-YYYY — odd, but documented.
        format!(
            "https://olinda.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata/CotacaoDolarDia(dataCotacao=@dataCotacao)?@dataCotacao='{}'&$format=json",
            date.format("%m-%d-%Y"),
        )
    }

    fn parse_midpoint_brl_per_usd(body: &str, date: NaiveDate) -> Result<Decimal, DomainError> {
        let parsed: Value = serde_json::from_str(body)
            .map_err(|e| DomainError::Import(format!("BCB PTAX JSON parse: {e}")))?;

        let values = parsed
            .get("value")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                DomainError::Import("BCB PTAX response missing 'value' array".into())
            })?;

        if values.is_empty() {
            // No publication on this date (weekend/holiday) — the caller
            // walks back to find the nearest available rate.
            return Err(DomainError::NotFound {
                entity: "ExchangeRate".into(),
                id: format!("BCB PTAX {date}"),
            });
        }

        let row = &values[0];
        let compra = row
            .get("cotacaoCompra")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                DomainError::Import("BCB PTAX row missing cotacaoCompra".into())
            })?;
        let venda = row
            .get("cotacaoVenda")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                DomainError::Import("BCB PTAX row missing cotacaoVenda".into())
            })?;

        // Format to 6 decimal places, then parse as Decimal. f64 is fine for
        // rate magnitudes (<< 2^53) and 6dp is well within what PTAX publishes.
        let midpoint = (compra + venda) / 2.0;
        let as_str = format!("{midpoint:.6}");
        Decimal::from_str(&as_str)
            .map_err(|e| DomainError::Import(format!("BCB PTAX rate decimal parse: {e}")))
    }
}

impl<C: HttpClient> RateProvider for BcbPtaxProvider<C> {
    fn source_name(&self) -> &'static str {
        "BCB PTAX"
    }

    fn fetch(
        &self,
        from: CurrencyCode,
        to: CurrencyCode,
        date: NaiveDate,
    ) -> Result<Decimal, DomainError> {
        if from == to {
            return Ok(Decimal::ONE);
        }

        // BCB only publishes USD↔BRL. Reject anything else up front.
        let is_usd_brl = matches!(
            (from, to),
            (CurrencyCode::USD, CurrencyCode::BRL) | (CurrencyCode::BRL, CurrencyCode::USD)
        );
        if !is_usd_brl {
            return Err(DomainError::Import(format!(
                "BCB PTAX only supports USD↔BRL; got {from}→{to}"
            )));
        }

        let url = Self::build_url(date);
        let body = self.client.get(&url)?;
        let brl_per_usd = Self::parse_midpoint_brl_per_usd(&body, date)?;

        match (from, to) {
            (CurrencyCode::USD, CurrencyCode::BRL) => Ok(brl_per_usd),
            (CurrencyCode::BRL, CurrencyCode::USD) => {
                // Invert: 1 BRL = 1 / (BRL per USD) USD.
                if brl_per_usd.is_zero() {
                    return Err(DomainError::Import(
                        "BCB PTAX returned zero rate; cannot invert".into(),
                    ));
                }
                Ok(Decimal::ONE / brl_per_usd)
            }
            _ => unreachable!("is_usd_brl guarded above"),
        }
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
        responses: RefCell<std::collections::HashMap<String, Canned>>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeHttpClient {
        fn new() -> Self {
            Self {
                responses: RefCell::new(std::collections::HashMap::new()),
                calls: RefCell::new(Vec::new()),
            }
        }
        fn with_response(self, url: &str, body: &str) -> Self {
            self.responses
                .borrow_mut()
                .insert(url.to_string(), Canned::Body(body.to_string()));
            self
        }
        fn with_error(self, url: &str, msg: &str) -> Self {
            self.responses
                .borrow_mut()
                .insert(url.to_string(), Canned::Error(msg.to_string()));
            self
        }
        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl HttpClient for FakeHttpClient {
        fn get(&self, url: &str) -> Result<String, DomainError> {
            self.calls.borrow_mut().push(url.to_string());
            match self.responses.borrow().get(url) {
                Some(Canned::Body(s)) => Ok(s.clone()),
                Some(Canned::Error(msg)) => Err(DomainError::Import(msg.clone())),
                None => Err(DomainError::Import(format!(
                    "FakeHttpClient: no canned response for {url}"
                ))),
            }
        }
    }

    fn body_with(compra: f64, venda: f64) -> String {
        format!(
            r#"{{"value":[{{"cotacaoCompra":{compra},"cotacaoVenda":{venda},"dataHoraCotacao":"2026-04-07 13:00:00.000"}}]}}"#
        )
    }

    fn empty_body() -> String {
        r#"{"value":[]}"#.to_string()
    }

    #[test]
    fn same_currency_short_circuits_to_one() {
        let client = FakeHttpClient::new();
        let provider = BcbPtaxProvider::with_client(client);

        let rate = provider
            .fetch(
                CurrencyCode::USD,
                CurrencyCode::USD,
                NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
            )
            .unwrap();
        assert_eq!(rate, Decimal::ONE);
    }

    #[test]
    fn usd_to_brl_uses_midpoint() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 7).unwrap();
        let url = BcbPtaxProvider::<FakeHttpClient>::build_url(date);
        let client = FakeHttpClient::new().with_response(&url, &body_with(5.10, 5.14));

        let provider = BcbPtaxProvider::with_client(client);
        let rate = provider.fetch(CurrencyCode::USD, CurrencyCode::BRL, date).unwrap();
        // midpoint = (5.10 + 5.14) / 2 = 5.12
        assert_eq!(rate, dec!(5.120000));
    }

    #[test]
    fn brl_to_usd_inverts_midpoint() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 7).unwrap();
        let url = BcbPtaxProvider::<FakeHttpClient>::build_url(date);
        let client = FakeHttpClient::new().with_response(&url, &body_with(5.00, 5.00));
        let provider = BcbPtaxProvider::with_client(client);

        let rate = provider.fetch(CurrencyCode::BRL, CurrencyCode::USD, date).unwrap();
        // 1 / 5.00 = 0.20
        assert_eq!(rate, dec!(0.2));
    }

    #[test]
    fn empty_value_array_returns_not_found() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 5).unwrap(); // Sunday
        let url = BcbPtaxProvider::<FakeHttpClient>::build_url(date);
        let client = FakeHttpClient::new().with_response(&url, &empty_body());
        let provider = BcbPtaxProvider::with_client(client);

        let err = provider
            .fetch(CurrencyCode::USD, CurrencyCode::BRL, date)
            .unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[test]
    fn malformed_json_returns_import_error() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 7).unwrap();
        let url = BcbPtaxProvider::<FakeHttpClient>::build_url(date);
        let client = FakeHttpClient::new().with_response(&url, "not json");
        let provider = BcbPtaxProvider::with_client(client);

        let err = provider
            .fetch(CurrencyCode::USD, CurrencyCode::BRL, date)
            .unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn http_error_propagates() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 7).unwrap();
        let url = BcbPtaxProvider::<FakeHttpClient>::build_url(date);
        let client = FakeHttpClient::new().with_error(&url, "HTTP 503");
        let provider = BcbPtaxProvider::with_client(client);

        let err = provider
            .fetch(CurrencyCode::USD, CurrencyCode::BRL, date)
            .unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("503")),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn source_name_is_bcb_ptax() {
        let client = FakeHttpClient::new();
        let provider = BcbPtaxProvider::with_client(client);
        assert_eq!(provider.source_name(), "BCB PTAX");
    }

    #[test]
    fn same_currency_does_not_hit_http() {
        let client = FakeHttpClient::new();
        // Hold a reference we can inspect after the call.
        let provider = BcbPtaxProvider::with_client(client);
        let _ = provider
            .fetch(
                CurrencyCode::BRL,
                CurrencyCode::BRL,
                NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
            )
            .unwrap();
        assert_eq!(provider.client.call_count(), 0);
    }

    /// Live network test — skipped by default. Flip on for manual verification
    /// that the real BCB endpoint still matches our parser. Picks a known past
    /// business day to minimize flakiness.
    #[test]
    #[ignore = "hits live BCB API; run with `cargo test -- --ignored` for manual verification"]
    fn live_bcb_fetch_returns_reasonable_rate() {
        let provider = BcbPtaxProvider::new();
        let rate = provider
            .fetch(
                CurrencyCode::USD,
                CurrencyCode::BRL,
                NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            )
            .unwrap();
        // Rates have been in the 4–7 BRL/USD band for the past decade; a very
        // loose check that survives real-world rate drift without being noisy.
        assert!(rate > dec!(1.0));
        assert!(rate < dec!(20.0));
    }
}
