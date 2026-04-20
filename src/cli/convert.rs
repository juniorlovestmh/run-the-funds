//! CLI handler for `rtf convert <amount> <from> --to <currency> [--date <YYYY-MM-DD>]`.

use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::application::CurrencyConverter;
use crate::domain::currency::CurrencyCode;
use crate::infrastructure::exchange::BcbPtaxProvider;
use crate::infrastructure::storage::{Database, SqliteExchangeRateRepository};

use super::response::{CliResponse, ErrorResponse};

pub fn handle_convert(
    db: &Database,
    amount: String,
    from: String,
    to: String,
    date: Option<String>,
) {
    let amount = match Decimal::from_str(&amount) {
        Ok(a) => a,
        Err(e) => {
            print_error(&format!("invalid amount \"{amount}\": {e}"));
            std::process::exit(1);
        }
    };

    let from = match CurrencyCode::from_str(&from) {
        Ok(c) => c,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let to = match CurrencyCode::from_str(&to) {
        Ok(c) => c,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let date = match date {
        Some(s) => match NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                print_error(&format!("invalid date \"{s}\" (expected YYYY-MM-DD): {e}"));
                std::process::exit(1);
            }
        },
        None => chrono::Local::now().date_naive(),
    };

    let svc = CurrencyConverter::new(
        SqliteExchangeRateRepository::new(db),
        BcbPtaxProvider::new(),
    );

    match svc.convert(amount, from, to, date) {
        Ok(conversion) => {
            let response = CliResponse::ok(serde_json::json!({
                "amount": conversion.amount.to_string(),
                "currency": conversion.currency.to_string(),
                "rate": conversion.rate.to_string(),
                "rate_date": conversion.rate_date.format("%Y-%m-%d").to_string(),
                "source": conversion.source,
                "fallback_reason": conversion.fallback_reason,
            }));
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}
