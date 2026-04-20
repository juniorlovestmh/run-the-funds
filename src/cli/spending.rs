//! CLI handler for `rtf spending [--from --to --account-id --include-transfers --format]` (S05 T05).

use chrono::NaiveDate;

use crate::application::{SpendingOptions, SpendingReport, SpendingService};
use crate::infrastructure::storage::{
    Database, SqliteCategoryRepository, SqliteTransactionRepository,
    SqliteTransactionSplitRepository,
};

use super::response::{CliResponse, ErrorResponse};

pub fn handle_spending(
    db: &Database,
    from: Option<String>,
    to: Option<String>,
    account_id: Option<String>,
    include_transfers: bool,
    format: String,
) {
    let from = parse_date(from, "--from").unwrap_or_else(|e| {
        print_error(&e);
        std::process::exit(1);
    });
    let to = parse_date(to, "--to").unwrap_or_else(|e| {
        print_error(&e);
        std::process::exit(1);
    });

    let svc = SpendingService::new(
        SqliteTransactionRepository::new(db),
        SqliteTransactionSplitRepository::new(db),
        SqliteCategoryRepository::new(db),
    );

    let opts = SpendingOptions {
        from,
        to,
        exclude_transfers: !include_transfers,
        account_id: account_id.as_deref(),
    };

    match svc.compute(opts) {
        Ok(report) => {
            if format == "json" {
                let response = CliResponse::ok(&report);
                println!("{}", serde_json::to_string_pretty(&response).unwrap());
            } else {
                print_table(&report);
            }
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn parse_date(s: Option<String>, label: &str) -> Result<Option<NaiveDate>, String> {
    match s {
        None => Ok(None),
        Some(raw) => NaiveDate::parse_from_str(&raw, "%Y-%m-%d")
            .map(Some)
            .map_err(|e| format!("invalid {label} date \"{raw}\" (expected YYYY-MM-DD): {e}")),
    }
}

fn print_table(report: &SpendingReport) {
    println!("=== Totals by currency ===");
    for t in &report.totals_by_currency {
        println!("  {} {}  ({} lines)", t.currency, t.amount, t.line_count);
    }
    println!();
    println!("=== By category (most-negative first) ===");
    for line in &report.by_category {
        let group = line.group_name.as_deref().unwrap_or("-");
        println!(
            "  {:<30}  {:<15}  {} {}  ({} lines)",
            line.category_name, group, line.currency, line.amount, line.line_count
        );
    }
    println!();
    println!("=== By group ===");
    for line in &report.by_group {
        println!(
            "  {:<30}  {} {}  ({} lines)",
            line.group_name, line.currency, line.amount, line.line_count
        );
    }
    println!();
    println!("=== Uncategorized ===");
    if report.uncategorized.is_empty() {
        println!("  (none)");
    }
    for u in &report.uncategorized {
        println!("  {} {}  ({} lines)", u.currency, u.amount, u.line_count);
    }
    println!();
    println!(
        "transactions: {}, splits: {}, transfers excluded: {}",
        report.transaction_count, report.split_count, report.transfers_excluded
    );
    if let (Some(f), Some(t)) = (report.window_start, report.window_end) {
        println!("window: {f} → {t}");
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}
