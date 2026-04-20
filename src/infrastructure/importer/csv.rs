//! CSV importer — targets US bank exports (Chase, Capital One, Discover,
//! Aidvantage). Each bank emits slightly different column layouts, so the
//! final implementation will either ship a handful of bank-specific layout
//! strategies or accept a layout descriptor as a parameter.
//!
//! Heads-up for the implementer: this module is named `csv`, which shadows
//! the external `csv` crate for any `use csv::...` written here. When you
//! import the parser, write the absolute path: `use ::csv::Reader;` (or
//! `extern crate csv as csv_crate;` and rename).
//!
//! Planned behavior (to be filled in during S02):
//! - Parse CSV via the `csv` crate with `Reader::from_path`.
//! - First row is a header; match columns case-insensitively against a
//!   per-bank layout table (date, amount, description, payee, transaction id).
//! - Convert amounts through `rust_decimal::Decimal::from_str` — never
//!   `f64`. Negative numbers stay negative (expense); positive stays
//!   positive (income).
//! - Dates typically arrive as `MM/DD/YYYY` — parse via
//!   `NaiveDate::parse_from_str`.
//! - Surface parse failures as `ImportError::Parse { format: "csv", ... }`.

use std::path::Path;

use crate::domain::transaction::Transaction;

use super::{ImportError, Importer};

/// CSV file importer. Stateless — constructed per-import by the service layer.
pub struct CsvImporter;

impl CsvImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CsvImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Importer for CsvImporter {
    fn name(&self) -> &'static str {
        "csv"
    }

    fn import(
        &self,
        _source: &Path,
        _account_id: &str,
    ) -> Result<Vec<Transaction>, ImportError> {
        Err(ImportError::NotImplemented("csv"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_csv() {
        assert_eq!(CsvImporter::new().name(), "csv");
    }

    #[test]
    fn import_returns_not_implemented() {
        let imp = CsvImporter::new();
        let result = imp.import(Path::new("/dev/null"), "acc-001");
        assert!(matches!(result, Err(ImportError::NotImplemented("csv"))));
    }
}
