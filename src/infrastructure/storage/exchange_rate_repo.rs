use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;
use crate::domain::exchange::{ExchangeRate, ExchangeRateRepository};

use super::database::Database;

pub struct SqliteExchangeRateRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteExchangeRateRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_rate(row: &rusqlite::Row) -> rusqlite::Result<ExchangeRate> {
        let id: String = row.get("id")?;
        let from_str: String = row.get("from_currency")?;
        let to_str: String = row.get("to_currency")?;
        let rate_str: String = row.get("rate")?;
        let date_str: String = row.get("date")?;
        let source: String = row.get("source")?;
        let fetched_at_str: String = row.get("fetched_at")?;

        let from = CurrencyCode::from_str(&from_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let to = CurrencyCode::from_str(&to_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let rate = Decimal::from_str(&rate_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let fetched_at = DateTime::parse_from_rfc3339(&fetched_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;

        Ok(ExchangeRate {
            id,
            from,
            to,
            rate,
            date,
            source,
            fetched_at,
        })
    }
}

impl ExchangeRateRepository for SqliteExchangeRateRepository<'_> {
    fn save(&self, rate: &ExchangeRate) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute(
                "INSERT OR REPLACE INTO exchange_rates (
                    id, from_currency, to_currency, rate, date, source, fetched_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    rate.id,
                    rate.from.to_string(),
                    rate.to.to_string(),
                    rate.rate.to_string(),
                    rate.date.format("%Y-%m-%d").to_string(),
                    rate.source,
                    rate.fetched_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save exchange_rate: {e}")))?;
        Ok(())
    }

    fn find_by_pair_date(
        &self,
        from: CurrencyCode,
        to: CurrencyCode,
        date: NaiveDate,
    ) -> Result<Option<ExchangeRate>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM exchange_rates \
                 WHERE from_currency = ?1 AND to_currency = ?2 AND date = ?3 LIMIT 1",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row(
                [
                    from.to_string(),
                    to.to_string(),
                    date.format("%Y-%m-%d").to_string(),
                ],
                Self::row_to_rate,
            )
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_pair_date: {e}")))
    }

    fn find_nearest_on_or_before(
        &self,
        from: CurrencyCode,
        to: CurrencyCode,
        date: NaiveDate,
    ) -> Result<Option<ExchangeRate>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM exchange_rates \
                 WHERE from_currency = ?1 AND to_currency = ?2 AND date <= ?3 \
                 ORDER BY date DESC LIMIT 1",
            )
            .map_err(|e| DomainError::Storage(format!("prepare: {e}")))?
            .query_row(
                [
                    from.to_string(),
                    to.to_string(),
                    date.format("%Y-%m-%d").to_string(),
                ],
                Self::row_to_rate,
            )
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_nearest_on_or_before: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_rate(date: (i32, u32, u32), value: Decimal) -> ExchangeRate {
        ExchangeRate::new(
            format!("rate-{}-{}-{}", date.0, date.1, date.2),
            CurrencyCode::USD,
            CurrencyCode::BRL,
            value,
            NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            "BCB PTAX".into(),
        )
    }

    #[test]
    fn save_and_find_by_pair_date() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let r = make_rate((2026, 4, 7), dec!(5.1234));
        repo.save(&r).unwrap();

        let found = repo
            .find_by_pair_date(
                CurrencyCode::USD,
                CurrencyCode::BRL,
                NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
            )
            .unwrap()
            .expect("expected row");
        assert_eq!(found.rate, dec!(5.1234));
        assert_eq!(found.source, "BCB PTAX");
    }

    #[test]
    fn find_by_pair_date_returns_none_when_absent() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        assert!(
            repo.find_by_pair_date(
                CurrencyCode::USD,
                CurrencyCode::BRL,
                NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn find_nearest_on_or_before_picks_latest_prior() {
        // Seed Friday + Thursday rates; querying Sunday must return Friday.
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        repo.save(&make_rate((2026, 4, 2), dec!(5.10))).unwrap(); // Thu
        repo.save(&make_rate((2026, 4, 3), dec!(5.15))).unwrap(); // Fri

        let found = repo
            .find_nearest_on_or_before(
                CurrencyCode::USD,
                CurrencyCode::BRL,
                NaiveDate::from_ymd_opt(2026, 4, 5).unwrap(), // Sun
            )
            .unwrap()
            .unwrap();
        assert_eq!(found.date, NaiveDate::from_ymd_opt(2026, 4, 3).unwrap());
        assert_eq!(found.rate, dec!(5.15));
    }

    #[test]
    fn find_nearest_on_or_before_returns_none_when_no_earlier_row() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        repo.save(&make_rate((2026, 4, 10), dec!(5.15))).unwrap();
        assert!(
            repo.find_nearest_on_or_before(
                CurrencyCode::USD,
                CurrencyCode::BRL,
                NaiveDate::from_ymd_opt(2026, 4, 5).unwrap(),
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn save_upserts_on_duplicate_pair_date() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        repo.save(&make_rate((2026, 4, 7), dec!(5.10))).unwrap();
        // Same pair+date but different rate value — should overwrite, not error.
        let mut updated = make_rate((2026, 4, 7), dec!(5.20));
        updated.id = "rate-updated".into();
        repo.save(&updated).unwrap();

        let found = repo
            .find_by_pair_date(
                CurrencyCode::USD,
                CurrencyCode::BRL,
                NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(found.rate, dec!(5.20));
    }

    #[test]
    fn separate_directions_are_independent_rows() {
        // USD→BRL and BRL→USD are different rows (different rate values).
        let db = Database::in_memory().unwrap();
        let repo = SqliteExchangeRateRepository::new(&db);
        let usd_to_brl = make_rate((2026, 4, 7), dec!(5.1234));
        let mut brl_to_usd = ExchangeRate::new(
            "rev".into(),
            CurrencyCode::BRL,
            CurrencyCode::USD,
            dec!(0.19518),
            NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
            "BCB PTAX".into(),
        );
        brl_to_usd.fetched_at = Utc::now();

        repo.save(&usd_to_brl).unwrap();
        repo.save(&brl_to_usd).unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 4, 7).unwrap();
        assert_eq!(
            repo.find_by_pair_date(CurrencyCode::USD, CurrencyCode::BRL, date)
                .unwrap()
                .unwrap()
                .rate,
            dec!(5.1234)
        );
        assert_eq!(
            repo.find_by_pair_date(CurrencyCode::BRL, CurrencyCode::USD, date)
                .unwrap()
                .unwrap()
                .rate,
            dec!(0.19518)
        );
    }
}
