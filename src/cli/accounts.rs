use std::str::FromStr;

use crate::application::AccountService;
use crate::domain::account::AccountType;
use crate::domain::currency::CurrencyCode;
use crate::infrastructure::storage::{Database, SqliteAccountRepository};

use super::response::{CliResponse, ErrorResponse};

pub fn handle_create(
    db: &Database,
    name: String,
    type_str: String,
    currency_str: String,
    owner: String,
    institution: Option<String>,
) {
    let account_type = match AccountType::from_str(&type_str) {
        Ok(t) => t,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let currency = match CurrencyCode::from_str(&currency_str) {
        Ok(c) => c,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    let repo = SqliteAccountRepository::new(db);
    let svc = AccountService::new(repo);

    match svc.create_account(name, account_type, currency, owner, institution) {
        Ok(account) => {
            let response = CliResponse::ok(account);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

pub fn handle_link(
    db: &Database,
    id: String,
    provider: String,
    external_id: String,
    force: bool,
) {
    let provider_lower = provider.to_lowercase();
    if provider_lower != "simplefin" && provider_lower != "pluggy" && provider_lower != "teller" {
        print_error(&format!(
            "unknown provider \"{provider}\" (expected \"simplefin\", \"pluggy\", or \"teller\")"
        ));
        std::process::exit(1);
    }

    let repo = SqliteAccountRepository::new(db);
    let svc = AccountService::new(repo);

    match svc.link_account(&id, provider_lower, external_id, force) {
        Ok(account) => {
            let response = CliResponse::ok(account);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

pub fn handle_list(db: &Database, format: String) {
    let repo = SqliteAccountRepository::new(db);
    let svc = AccountService::new(repo);

    match svc.list_accounts() {
        Ok(accounts) => {
            if format == "json" {
                let response = CliResponse::ok(&accounts);
                println!("{}", serde_json::to_string_pretty(&response).unwrap());
            } else {
                print_table(&accounts);
            }
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn print_table(accounts: &[crate::domain::account::Account]) {
    if accounts.is_empty() {
        println!("No accounts found.");
        return;
    }

    println!(
        "{:<36}  {:<20}  {:<12}  {:<5}  {:<15}  {:<10}",
        "ID", "Name", "Type", "Cur", "Owner", "Balance"
    );
    println!("{}", "-".repeat(105));
    for acc in accounts {
        println!(
            "{:<36}  {:<20}  {:<12}  {:<5}  {:<15}  {}",
            acc.id, acc.name, acc.account_type, acc.currency, acc.owner, acc.balance
        );
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}
