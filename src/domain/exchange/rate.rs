use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::currency::CurrencyCode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExchangeRate {
    pub id: String,
    pub from: CurrencyCode,
    pub to: CurrencyCode,
    pub rate: Decimal,
    pub date: NaiveDate,
    pub source: String,
    pub fetched_at: DateTime<Utc>,
}

impl ExchangeRate {
    pub fn new(
        id: String,
        from: CurrencyCode,
        to: CurrencyCode,
        rate: Decimal,
        date: NaiveDate,
        source: String,
    ) -> Self {
        Self {
            id,
            from,
            to,
            rate,
            date,
            source,
            fetched_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn new_sets_fetched_at_to_now() {
        let before = Utc::now();
        let r = ExchangeRate::new(
            "r1".into(),
            CurrencyCode::USD,
            CurrencyCode::BRL,
            dec!(5.1234),
            NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
            "BCB PTAX".into(),
        );
        let after = Utc::now();
        assert!(r.fetched_at >= before && r.fetched_at <= after);
        assert_eq!(r.rate, dec!(5.1234));
    }

    #[test]
    fn serde_roundtrip() {
        let r = ExchangeRate::new(
            "r1".into(),
            CurrencyCode::USD,
            CurrencyCode::BRL,
            dec!(5.1234),
            NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
            "BCB PTAX".into(),
        );
        let json = serde_json::to_string(&r).unwrap();
        let decoded: ExchangeRate = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, r);
    }
}
