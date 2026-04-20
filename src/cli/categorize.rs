//! CLI handler for `rtf categorize` (S05 T02 + T04).
//!
//! Two modes:
//! - Default: apply rules to uncategorized transactions (rule-based).
//! - `--detect-transfers`: pair same-amount debits/credits across accounts
//!   within ±3 days (transfer detection).
//!
//! `--dry-run` previews either mode without writing. `--reset` only applies
//! to the rule path (clears existing category_ids).

use serde::Serialize;

use crate::application::{CategorizationService, CategorizeOptions};
use crate::infrastructure::storage::{Database, SqliteRuleRepository, SqliteTransactionRepository};

use super::response::{CliResponse, ErrorResponse};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum CategorizeOutput {
    Rules(crate::application::CategorizeReport),
    Transfers(crate::application::TransferPairingReport),
}

pub fn handle_categorize(
    db: &Database,
    dry_run: bool,
    reset: bool,
    account_id: Option<String>,
    detect_transfers: bool,
) {
    let svc = CategorizationService::new(
        SqliteRuleRepository::new(db),
        SqliteTransactionRepository::new(db),
    );

    let result = if detect_transfers {
        svc.detect_transfers(dry_run)
            .map(CategorizeOutput::Transfers)
    } else {
        let opts = CategorizeOptions {
            dry_run,
            reset,
            account_id: account_id.as_deref(),
        };
        svc.categorize(opts).map(CategorizeOutput::Rules)
    };

    match result {
        Ok(output) => {
            let response = CliResponse::ok(output);
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
