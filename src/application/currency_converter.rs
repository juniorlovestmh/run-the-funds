//! CurrencyConverter — orchestrates exchange-rate lookups with a cache-first,
//! fetch-on-miss, walk-back-on-weekend flow.
//!
//! The service is generic over `ExchangeRateRepository` and `RateProvider`
//! so tests can inject a canned provider. The production wiring uses
//! `SqliteExchangeRateRepository` + `BcbPtaxProvider`.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::domain::exchange::{ExchangeRate, ExchangeRateRepository};
use crate::infrastructure::exchange::RateProvider;

/// Maximum number of days to walk back when the queried date has no rate
/// (weekend + long holiday run). PTAX gaps longer than a week are vanishingly
/// rare and usually indicate a real data issue worth surfacing as an error.
const MAX_WALK_BACK_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Conversion {
    pub amount: Decimal,
    pub currency: CurrencyCode,
    pub rate: Decimal,
    pub rate_date: NaiveDate,
    pub source: String,
    /// Present when the rate used comes from a date earlier than the queried
    /// date (weekend/holiday fallback). `None` means the rate matches the
    /// queried date exactly (or the conversion is same-currency).
    pub fallback_reason: Option<String>,
}

pub struct CurrencyConverter<R: ExchangeRateRepository, P: RateProvider> {
    repo: R,
    provider: P,
}

impl<R: ExchangeRateRepository, P: RateProvider> CurrencyConverter<R, P> {
    pub fn new(repo: R, provider: P) -> Self {
        Self { repo, provider }
    }

    pub fn convert(
        &self,
        amount: Decimal,
        from: CurrencyCode,
        to: CurrencyCode,
        queried_date: NaiveDate,
    ) -> Result<Conversion, DomainError> {
        if from == to {
            return Ok(Conversion {
                amount,
                currency: to,
                rate: Decimal::ONE,
                rate_date: queried_date,
                source: "identity".into(),
                fallback_reason: None,
            });
        }

        // Exact cache hit — fast path, no network.
        if let Some(row) = self.repo.find_by_pair_date(from, to, queried_date)? {
            return Ok(self.build_conversion(amount, to, &row, queried_date));
        }

        // Walk from the queried date backwards up to MAX_WALK_BACK_DAYS.
        // For each candidate date: check cache first, then hit the provider.
        // First rate found wins.
        for days_back in 0..=MAX_WALK_BACK_DAYS {
            let try_date = queried_date - chrono::Duration::days(days_back);

            // days_back==0 is already-cache-missed above, skip the redundant
            // cache check on the first iteration.
            let row = if days_back > 0 {
                if let Some(cached) = self.repo.find_by_pair_date(from, to, try_date)? {
                    Some(cached)
                } else {
                    self.try_fetch_and_save(from, to, try_date)?
                }
            } else {
                if days_back == 0 {
                    eprintln!(
                        "fetching {} rate for {try_date}...",
                        self.provider.source_name()
                    );
                }
                self.try_fetch_and_save(from, to, try_date)?
            };

            if let Some(r) = row {
                return Ok(self.build_conversion(amount, to, &r, queried_date));
            }
        }

        Err(DomainError::NotFound {
            entity: "ExchangeRate".into(),
            id: format!("{from}→{to} within {MAX_WALK_BACK_DAYS} days of {queried_date}"),
        })
    }

    fn try_fetch_and_save(
        &self,
        from: CurrencyCode,
        to: CurrencyCode,
        date: NaiveDate,
    ) -> Result<Option<ExchangeRate>, DomainError> {
        match self.provider.fetch(from, to, date) {
            Ok(rate) => {
                let row = ExchangeRate::new(
                    Uuid::new_v4().to_string(),
                    from,
                    to,
                    rate,
                    date,
                    self.provider.source_name().into(),
                );
                self.repo.save(&row)?;
                Ok(Some(row))
            }
            // NotFound from the provider means "no publication on this date";
            // caller keeps walking. Any other error propagates immediately.
            Err(DomainError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn build_conversion(
        &self,
        amount: Decimal,
        to: CurrencyCode,
        row: &ExchangeRate,
        queried_date: NaiveDate,
    ) -> Conversion {
        let fallback_reason = if row.date != queried_date {
            let days = (queried_date - row.date).num_days();
            Some(format!(
                "no rate published for {}; used {} ({} day{} earlier)",
                queried_date,
                row.date,
                days,
                if days == 1 { "" } else { "s" }
            ))
        } else {
            None
        };

        Conversion {
            amount: amount * row.rate,
            currency: to,
            rate: row.rate,
            rate_date: row.date,
            source: row.source.clone(),
            fallback_reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::storage::{Database, SqliteExchangeRateRepository};
    use rust_decimal_macros::dec;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Canned rate provider. Keyed on `(from, to, date)` so tests can simulate
    /// weekend/holiday gaps (missing key → NotFound) or hard errors.
    struct FakeProvider {
        rates: HashMap<(CurrencyCode, CurrencyCode, NaiveDate), Decimal>,
        calls: RefCell<Vec<(CurrencyCode, CurrencyCode, NaiveDate)>>,
        fail_on_next_call: RefCell<Option<String>>,
    }

    impl FakeProvider {
        fn new() -> Self {
            Self {
                rates: HashMap::new(),
                calls: RefCell::new(Vec::new()),
                fail_on_next_call: RefCell::new(None),
            }
        }
        fn with(
            mut self,
            from: CurrencyCode,
            to: CurrencyCode,
            date: (i32, u32, u32),
            rate: Decimal,
        ) -> Self {
            self.rates.insert(
                (
                    from,
                    to,
                    NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
                ),
                rate,
            );
            self
        }
        fn set_next_error(&self, msg: &str) {
            *self.fail_on_next_call.borrow_mut() = Some(msg.to_string());
        }
        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl RateProvider for FakeProvider {
        fn source_name(&self) -> &'static str {
            "FAKE"
        }
        fn fetch(
            &self,
            from: CurrencyCode,
            to: CurrencyCode,
            date: NaiveDate,
        ) -> Result<Decimal, DomainError> {
            self.calls.borrow_mut().push((from, to, date));
            if let Some(msg) = self.fail_on_next_call.borrow_mut().take() {
                return Err(DomainError::Import(msg));
            }
            self.rates
                .get(&(from, to, date))
                .copied()
                .ok_or(DomainError::NotFound {
                    entity: "ExchangeRate".into(),
                    id: format!("FAKE {from}→{to} {date}"),
                })
        }
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn same_currency_short_circuits() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let provider = FakeProvider::new();
        let svc = CurrencyConverter::new(repo, provider);

        let c = svc
            .convert(
                dec!(1000),
                CurrencyCode::USD,
                CurrencyCode::USD,
                date(2026, 4, 7),
            )
            .unwrap();
        assert_eq!(c.amount, dec!(1000));
        assert_eq!(c.rate, Decimal::ONE);
        assert_eq!(c.currency, CurrencyCode::USD);
        assert_eq!(c.source, "identity");
        assert!(c.fallback_reason.is_none());
        assert_eq!(svc.provider.call_count(), 0);
    }

    #[test]
    fn cache_miss_fetches_and_persists() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let provider = FakeProvider::new().with(
            CurrencyCode::USD,
            CurrencyCode::BRL,
            (2026, 4, 7),
            dec!(5.12),
        );
        let svc = CurrencyConverter::new(repo, provider);

        let c = svc
            .convert(
                dec!(100),
                CurrencyCode::USD,
                CurrencyCode::BRL,
                date(2026, 4, 7),
            )
            .unwrap();
        assert_eq!(c.amount, dec!(512.00));
        assert_eq!(c.rate, dec!(5.12));
        assert_eq!(c.rate_date, date(2026, 4, 7));
        assert!(c.fallback_reason.is_none());
        assert_eq!(svc.provider.call_count(), 1);

        // And the rate is now persisted — a direct repo lookup should find it.
        let cached = SqliteExchangeRateRepository::new(&db)
            .find_by_pair_date(CurrencyCode::USD, CurrencyCode::BRL, date(2026, 4, 7))
            .unwrap()
            .unwrap();
        assert_eq!(cached.rate, dec!(5.12));
        assert_eq!(cached.source, "FAKE");
    }

    #[test]
    fn second_call_hits_cache() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let provider = FakeProvider::new().with(
            CurrencyCode::USD,
            CurrencyCode::BRL,
            (2026, 4, 7),
            dec!(5.00),
        );
        let svc = CurrencyConverter::new(repo, provider);

        svc.convert(
            dec!(1),
            CurrencyCode::USD,
            CurrencyCode::BRL,
            date(2026, 4, 7),
        )
        .unwrap();
        let call_after_first = svc.provider.call_count();

        svc.convert(
            dec!(2),
            CurrencyCode::USD,
            CurrencyCode::BRL,
            date(2026, 4, 7),
        )
        .unwrap();
        assert_eq!(
            svc.provider.call_count(),
            call_after_first,
            "second call should hit cache, not provider"
        );
    }

    #[test]
    fn walk_back_finds_prior_business_day() {
        // Provider publishes only for Fri 2026-04-03. Querying Sun 2026-04-05
        // must walk back Sun→Sat→Fri and return Friday's rate with fallback.
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let provider = FakeProvider::new().with(
            CurrencyCode::USD,
            CurrencyCode::BRL,
            (2026, 4, 3),
            dec!(5.10),
        );
        let svc = CurrencyConverter::new(repo, provider);

        let c = svc
            .convert(
                dec!(100),
                CurrencyCode::USD,
                CurrencyCode::BRL,
                date(2026, 4, 5),
            )
            .unwrap();
        assert_eq!(c.rate, dec!(5.10));
        assert_eq!(c.rate_date, date(2026, 4, 3));
        let reason = c.fallback_reason.expect("expected fallback reason");
        assert!(reason.contains("2026-04-05"));
        assert!(reason.contains("2026-04-03"));
        assert!(reason.contains("2 days earlier"));
        // Provider was called 3 times: Sun, Sat, Fri.
        assert_eq!(svc.provider.call_count(), 3);
    }

    #[test]
    fn walk_back_uses_cache_for_prior_dates_when_available() {
        // Pre-seed a Friday rate via a fresh service call. Then, in a new
        // service instance (same DB), query Sunday — walk-back should hit
        // the cache for Friday WITHOUT calling the provider.
        let db = Database::in_memory().unwrap();

        // Seed: seed rate by directly saving (simulates a prior import).
        let seed = ExchangeRate::new(
            "seed".into(),
            CurrencyCode::USD,
            CurrencyCode::BRL,
            dec!(5.10),
            date(2026, 4, 3),
            "FAKE".into(),
        );
        SqliteExchangeRateRepository::new(&db).save(&seed).unwrap();

        let provider = FakeProvider::new(); // no canned rates → always NotFound
        let svc = CurrencyConverter::new(SqliteExchangeRateRepository::new(&db), provider);

        let c = svc
            .convert(
                dec!(100),
                CurrencyCode::USD,
                CurrencyCode::BRL,
                date(2026, 4, 5),
            )
            .unwrap();
        assert_eq!(c.rate, dec!(5.10));
        assert_eq!(c.rate_date, date(2026, 4, 3));
        // Provider called for Sunday + Saturday (both NotFound), then Friday
        // hits cache — so 2 calls, not 3.
        assert_eq!(svc.provider.call_count(), 2);
    }

    #[test]
    fn walk_back_exhausted_returns_not_found() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let provider = FakeProvider::new(); // every date returns NotFound
        let svc = CurrencyConverter::new(repo, provider);

        let err = svc
            .convert(
                dec!(100),
                CurrencyCode::USD,
                CurrencyCode::BRL,
                date(2026, 4, 5),
            )
            .unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[test]
    fn provider_import_error_propagates_without_walking_back() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let provider = FakeProvider::new();
        provider.set_next_error("HTTP 503");
        let svc = CurrencyConverter::new(repo, provider);

        let err = svc
            .convert(
                dec!(100),
                CurrencyCode::USD,
                CurrencyCode::BRL,
                date(2026, 4, 7),
            )
            .unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("503")),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn zero_amount_returns_zero() {
        let db = Database::in_memory().unwrap();
        let provider = FakeProvider::new().with(
            CurrencyCode::USD,
            CurrencyCode::BRL,
            (2026, 4, 7),
            dec!(5.12),
        );
        let svc = CurrencyConverter::new(SqliteExchangeRateRepository::new(&db), provider);
        let c = svc
            .convert(
                dec!(0),
                CurrencyCode::USD,
                CurrencyCode::BRL,
                date(2026, 4, 7),
            )
            .unwrap();
        assert_eq!(c.amount, Decimal::ZERO);
    }

    #[test]
    fn negative_amount_preserves_sign() {
        let db = Database::in_memory().unwrap();
        let provider = FakeProvider::new().with(
            CurrencyCode::BRL,
            CurrencyCode::USD,
            (2026, 4, 7),
            dec!(0.2),
        );
        let svc = CurrencyConverter::new(SqliteExchangeRateRepository::new(&db), provider);
        let c = svc
            .convert(
                dec!(-500),
                CurrencyCode::BRL,
                CurrencyCode::USD,
                date(2026, 4, 7),
            )
            .unwrap();
        assert_eq!(c.amount, dec!(-100.0));
    }

    #[test]
    fn same_date_exact_cache_hit_has_no_fallback_reason() {
        let db = Database::in_memory().unwrap();
        let seed = ExchangeRate::new(
            "seed".into(),
            CurrencyCode::USD,
            CurrencyCode::BRL,
            dec!(5.0),
            date(2026, 4, 7),
            "FAKE".into(),
        );
        SqliteExchangeRateRepository::new(&db).save(&seed).unwrap();

        let svc =
            CurrencyConverter::new(SqliteExchangeRateRepository::new(&db), FakeProvider::new());
        let c = svc
            .convert(
                dec!(10),
                CurrencyCode::USD,
                CurrencyCode::BRL,
                date(2026, 4, 7),
            )
            .unwrap();
        assert!(c.fallback_reason.is_none());
        assert_eq!(svc.provider.call_count(), 0);
    }
}
