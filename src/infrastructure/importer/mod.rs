//! Bank import adapters — S02 (Transaction Import Pipeline).
//!
//! Each adapter reads a file exported from a bank (OFX for Brazilian banks
//! like Nubank/Itaú, CSV for US banks like Chase/Capital One/Discover),
//! parses it, and produces a `Vec<Transaction>` bound to a caller-supplied
//! account. The application layer is responsible for persisting the
//! resulting transactions and handling duplicate detection via `external_id`.
//!
//! All importers return domain-level errors via `ImportError` so the CLI
//! can emit consistent JSON error envelopes.

pub mod csv;
pub mod ofx;

pub use csv::CsvImporter;
pub use ofx::OfxImporter;

use std::path::Path;

use thiserror::Error;

use crate::domain::error::DomainError;
use crate::domain::transaction::Transaction;

/// Adapter trait: given a source file and the destination account id,
/// parse the file and return domain-level `Transaction` values.
///
/// Implementations must:
/// - Preserve transaction dates exactly as they appear in the source.
/// - Populate `external_id` with the bank's stable transaction identifier
///   so later re-imports can be deduplicated.
/// - Use `rust_decimal` for all monetary amounts — never `f64`.
/// - Set `amount.currency` to the account's currency (caller supplies the
///   account id; the service layer will cross-check).
pub trait Importer {
    /// Short human-readable name for the adapter (e.g. "nubank-ofx").
    fn name(&self) -> &'static str;

    /// Parse `source` and return the transactions it contains.
    fn import(&self, source: &Path, account_id: &str) -> Result<Vec<Transaction>, ImportError>;
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("importer not yet implemented: {0}")]
    NotImplemented(&'static str),

    #[error("failed to read source file {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse {format} file: {detail}")]
    Parse {
        format: &'static str,
        detail: String,
    },

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error(transparent)]
    Domain(#[from] DomainError),
}
