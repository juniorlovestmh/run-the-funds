//! Bank-sync provider adapters (S04).
//!
//! Each provider implements `BankSyncAdapter` — returns a list of
//! `(external_account_id, transactions)` tuples for every account the
//! credentials can access. The service layer filters to locally-linked
//! accounts and persists via `TransactionService::persist_batch`.
//!
//! Providers are constructed with already-loaded credentials + an
//! `HttpClient`. No env-var reads in this layer — the CLI loads
//! credentials from the `provider_credentials` table and passes them in.

pub mod monarch;
pub mod pluggy;
pub mod simplefin;
pub mod teller;

pub use monarch::{
    MmoneyRunner, MonarchAccount, MonarchAdapter, MonarchCategory, MonarchCategoryGroup,
    MonarchTag, MonarchTransaction, SubprocessMmoneyRunner,
};
pub use pluggy::PluggyAdapter;
pub use simplefin::SimpleFinAdapter;
pub use teller::TellerAdapter;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::domain::currency::CurrencyCode;
use crate::domain::error::DomainError;

/// Minimal transaction shape returned by a bank-sync adapter.
/// `SyncService` converts these into `domain::Transaction` by assigning a
/// UUID, the local account id, status, and `imported_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTransaction {
    pub external_id: String,
    pub date: NaiveDate,
    pub amount: Decimal,
    pub currency: CurrencyCode,
    pub payee: Option<String>,
    pub description: Option<String>,
}

pub trait BankSyncAdapter {
    fn provider_name(&self) -> &'static str;

    /// Fetch transactions from the provider. `since` is an inclusive lower
    /// bound on the transaction date; `None` means use the provider's default
    /// window (the service layer always passes `Some(today - 2 years)` on
    /// first sync to force a real backfill).
    ///
    /// Returns `(external_account_id, transactions)` per account accessible
    /// by the adapter's credentials. Accounts the caller isn't locally
    /// linked to are still returned — the caller filters.
    fn sync(
        &self,
        since: Option<NaiveDate>,
    ) -> Result<Vec<(String, Vec<RemoteTransaction>)>, DomainError>;
}
