//! Currency exchange rate adapters.
//!
//! `RateProvider` is the domain boundary — an abstraction over any source
//! of historical rates. `BcbPtaxProvider` is the concrete implementation
//! that fetches from Banco Central do Brasil's PTAX endpoint. Tests inject
//! a `FakeProvider` to avoid network coupling.

pub mod bcb_ptax;

pub use bcb_ptax::BcbPtaxProvider;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;

/// Fetches a rate for `from → to` on `date` from an external source.
///
/// Implementations should:
/// - Short-circuit `from == to` to `Decimal::ONE` before any network call.
/// - Return `DomainError::NotFound { entity: "ExchangeRate", ... }` when
///   the source has no rate for that exact date (weekend, holiday, pre-history).
///   The service layer handles the walk-back.
/// - Return `DomainError::Import(..)` for HTTP/parse failures.
pub trait RateProvider {
    fn fetch(
        &self,
        from: CurrencyCode,
        to: CurrencyCode,
        date: NaiveDate,
    ) -> Result<Decimal, DomainError>;

    fn source_name(&self) -> &'static str;
}
