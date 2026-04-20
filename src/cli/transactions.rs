//! CLI handlers for `rtf transactions {import,list,get}`.
//!
//! Shape: clap parses args, the handler opens repositories, instantiates the
//! application service, calls it, and prints a structured JSON envelope
//! (`{status, data}` / `{status, message}`).

use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::application::{CurrencyConverter, SplitAllocation, SplitService, TransactionService};
use crate::domain::currency::{CurrencyCode, Money};
use crate::domain::error::DomainError;
use crate::domain::transaction::{Transaction, TransactionRepository};
use crate::infrastructure::exchange::BcbPtaxProvider;
use crate::infrastructure::importer::OfxImporter;
use crate::infrastructure::storage::{
    Database, SqliteAccountRepository, SqliteExchangeRateRepository, SqliteTransactionRepository,
    SqliteTransactionSplitRepository,
};

use super::response::{CliResponse, ErrorResponse};

/// Presentation shape for `transactions list --format json`: wraps Transaction
/// and adds its USD equivalent + the rate that was used. `rate_status` tells
/// a consumer at a glance whether the USD value is exact, fallback, or missing.
#[derive(Serialize)]
struct TransactionView<'a> {
    #[serde(flatten)]
    txn: &'a Transaction,
    /// Stringified Decimal; `None` when no rate could be resolved.
    amount_usd: Option<String>,
    rate: Option<String>,
    rate_date: Option<NaiveDate>,
    /// One of: `ok` (exact-date rate), `fallback` (weekend walk-back),
    /// `unavailable` (no rate within 7 days), `same_currency` (USD account).
    rate_status: &'static str,
}

pub fn handle_import(db: &Database, file: String, format: String, account_id: String) {
    let fmt = format.to_lowercase();

    // CSV is reserved for a later slice — surface an explicit "not yet" error
    // rather than a generic "unsupported" so operators aren't left guessing.
    if fmt == "csv" {
        print_error("csv import is deferred to a later slice — use an OFX/QFX export for now");
        std::process::exit(2);
    }
    // "ofx" and "qfx" both route to the OFX adapter (QFX is OFX 1.x SGML).
    if fmt != "ofx" && fmt != "qfx" {
        print_error(&format!(
            "unsupported import format: {format} (expected \"ofx\" or \"qfx\")"
        ));
        std::process::exit(2);
    }

    let path = std::path::PathBuf::from(&file);
    let svc = TransactionService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
    );
    let importer = OfxImporter::new();

    match svc.import_from(&importer, &path, &account_id) {
        Ok(report) => {
            let response = CliResponse::ok(serde_json::json!({
                "imported": report.imported,
                "duplicates": report.duplicates,
                "file": file,
                "format": fmt,
                "account_id": account_id,
            }));
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

pub fn handle_list(db: &Database, account_id: Option<String>, format: String) {
    let svc = TransactionService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
    );

    let txns = match account_id.as_deref() {
        Some(id) => svc.list_by_account(id),
        None => {
            // S02 follow-up: a proper all-accounts listing will need a
            // new repository method (or a multi-account orchestration in
            // the service). For now fall back to an empty list and let
            // the user scope by --account-id.
            Ok(Vec::new())
        }
    };

    match txns {
        Ok(list) => {
            if format == "json" {
                match build_views(db, &list) {
                    Ok(views) => {
                        let response = CliResponse::ok(&views);
                        println!("{}", serde_json::to_string_pretty(&response).unwrap());
                    }
                    Err(e) => {
                        print_error(&e.to_string());
                        std::process::exit(1);
                    }
                }
            } else {
                print_table(&list);
            }
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

/// Enrich each transaction with its USD equivalent. BRL → USD conversions go
/// through the CurrencyConverter (cache-first, auto-fetch-on-miss); per-date
/// results are naturally deduplicated by the converter's caching, so a 50-row
/// BRL listing with 10 unique dates incurs at most 10 BCB round-trips — and
/// zero on subsequent runs.
///
/// Hard errors (HTTP failure, storage error) abort the listing with a
/// structured error envelope. Date-specific `NotFound` (no rate within the
/// walk-back window) downgrades to `rate_status: "unavailable"` for that
/// row only — other rows still render.
fn build_views<'a>(
    db: &Database,
    txns: &'a [Transaction],
) -> Result<Vec<TransactionView<'a>>, DomainError> {
    let converter = CurrencyConverter::new(
        SqliteExchangeRateRepository::new(db),
        BcbPtaxProvider::new(),
    );

    let mut views = Vec::with_capacity(txns.len());
    for txn in txns {
        if txn.amount.currency == CurrencyCode::USD {
            views.push(TransactionView {
                txn,
                amount_usd: Some(txn.amount.amount.to_string()),
                rate: Some("1".into()),
                rate_date: Some(txn.date),
                rate_status: "same_currency",
            });
            continue;
        }

        match converter.convert(
            txn.amount.amount,
            txn.amount.currency,
            CurrencyCode::USD,
            txn.date,
        ) {
            Ok(c) => {
                let status = if c.fallback_reason.is_some() {
                    "fallback"
                } else {
                    "ok"
                };
                views.push(TransactionView {
                    txn,
                    amount_usd: Some(c.amount.to_string()),
                    rate: Some(c.rate.to_string()),
                    rate_date: Some(c.rate_date),
                    rate_status: status,
                });
            }
            Err(DomainError::NotFound { .. }) => {
                // Rate genuinely unavailable for this date — degrade this row,
                // keep the listing going.
                views.push(TransactionView {
                    txn,
                    amount_usd: None,
                    rate: None,
                    rate_date: None,
                    rate_status: "unavailable",
                });
            }
            Err(e) => return Err(e),
        }
    }

    Ok(views)
}

/// Handler for `rtf transactions split <txn-id> --split cat-id:amount[:notes] --split ...`.
/// Each `--split` arg is parsed as `category_id:amount[:notes]` (colon-separated).
pub fn handle_split(db: &Database, transaction_id: String, split_args: Vec<String>) {
    // First load the parent to know its currency — splits use the same.
    let txn_repo = SqliteTransactionRepository::new(db);
    let txn = match load_transaction_or_die(&txn_repo, &transaction_id) {
        Ok(t) => t,
        Err(code) => std::process::exit(code),
    };

    let allocations = match parse_split_args(&split_args, txn.amount.currency) {
        Ok(a) => a,
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };

    let svc = SplitService::new(
        SqliteTransactionRepository::new(db),
        SqliteTransactionSplitRepository::new(db),
    );

    match svc.split_transaction(&transaction_id, allocations) {
        Ok(splits) => {
            let data = serde_json::json!({
                "transaction_id": transaction_id,
                "splits_created": splits.len(),
                "splits": splits,
            });
            let response = CliResponse::ok(data);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn load_transaction_or_die(
    repo: &SqliteTransactionRepository,
    id: &str,
) -> Result<Transaction, i32> {
    match repo.find_by_id(id) {
        Ok(Some(t)) => Ok(t),
        Ok(None) => {
            print_error(&format!("transaction not found: {id}"));
            Err(1)
        }
        Err(e) => {
            print_error(&e.to_string());
            Err(1)
        }
    }
}

fn parse_split_args(
    args: &[String],
    parent_currency: CurrencyCode,
) -> Result<Vec<SplitAllocation>, DomainError> {
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        // Syntax: `cat-id:amount[:notes]`. Split on FIRST two colons so notes can
        // contain colons.
        let mut parts = arg.splitn(3, ':');
        let cat = parts.next().unwrap_or("").trim();
        let amount_str = parts.next().ok_or_else(|| {
            DomainError::Validation(format!(
                "split arg `{arg}` missing amount (expected cat-id:amount[:notes])"
            ))
        })?;
        let notes = parts.next().map(|s| s.to_string());

        if cat.is_empty() {
            return Err(DomainError::Validation(format!(
                "split arg `{arg}` has empty category id"
            )));
        }
        let amount = Decimal::from_str(amount_str.trim()).map_err(|e| {
            DomainError::Validation(format!(
                "split arg `{arg}` amount `{amount_str}` not a decimal: {e}"
            ))
        })?;

        out.push(SplitAllocation {
            category_id: cat.to_string(),
            amount: Money::new(amount, parent_currency),
            notes,
        });
    }
    Ok(out)
}

pub fn handle_get(db: &Database, id: String) {
    let svc = TransactionService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
    );

    match svc.get_transaction(&id) {
        Ok(txn) => {
            let response = CliResponse::ok(txn);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn print_table(txns: &[Transaction]) {
    if txns.is_empty() {
        println!("No transactions found.");
        return;
    }

    println!(
        "{:<36}  {:<10}  {:<12}  {:<20}  {:<10}",
        "ID", "Date", "Amount", "Payee", "Status"
    );
    println!("{}", "-".repeat(95));
    for txn in txns {
        let payee = txn.payee.as_deref().unwrap_or("-");
        println!(
            "{:<36}  {:<10}  {:<12}  {:<20}  {}",
            txn.id, txn.date, txn.amount, payee, txn.status
        );
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}
