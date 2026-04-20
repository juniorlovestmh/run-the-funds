use chrono::NaiveDate;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;

use super::ExchangeRate;

pub trait ExchangeRateRepository {
    fn save(&self, rate: &ExchangeRate) -> Result<(), DomainError>;

    /// Exact match on `(from, to, date)`. Returns None if no row exists.
    fn find_by_pair_date(
        &self,
        from: CurrencyCode,
        to: CurrencyCode,
        date: NaiveDate,
    ) -> Result<Option<ExchangeRate>, DomainError>;

    /// Most-recent rate with `date <= queried`. Used for weekend/holiday
    /// fallback when BCB doesn't publish on non-business days.
    fn find_nearest_on_or_before(
        &self,
        from: CurrencyCode,
        to: CurrencyCode,
        date: NaiveDate,
    ) -> Result<Option<ExchangeRate>, DomainError>;
}
