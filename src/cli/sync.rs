//! CLI handler for `rtf sync [--provider <simplefin|pluggy>] [--since YYYY-MM-DD]`.
//!
//! Loads the provider's credentials from `provider_credentials`, constructs
//! the appropriate adapter, and runs it through `SyncService::sync_provider`.
//! In T02 `--provider` is required; T04 makes it optional and runs both.

use chrono::NaiveDate;
use serde::Serialize;

use crate::application::{MonarchSyncReport, MonarchSyncService, ProviderSyncReport, SyncService};
use crate::domain::connections::ProviderConnectionRepository;
use crate::domain::credentials::ProviderCredentialsRepository;
use crate::domain::error::DomainError;
use crate::infrastructure::http::UreqHttpClient;
use crate::infrastructure::storage::{
    Database, SqliteAccountRepository, SqliteCategoryRepository,
    SqliteProviderConnectionRepository, SqliteProviderCredentialsRepository, SqliteTagRepository,
    SqliteTransactionRepository,
};
use crate::infrastructure::sync_adapter::{
    MonarchAdapter, PluggyAdapter, SimpleFinAdapter, TellerAdapter,
};

use super::response::{CliResponse, ErrorResponse};

/// Aggregated result from a multi-provider sync.
/// Each provider slot is `None` when no credentials are configured for that
/// provider. `errors` carries failures from configured-but-failing providers
/// so one provider's outage doesn't mask the other's success.
#[derive(Debug, Clone, Serialize)]
pub struct UnifiedSyncReport {
    pub simplefin: Option<ProviderSyncReport>,
    pub pluggy: Option<ProviderSyncReport>,
    pub teller: Option<ProviderSyncReport>,
    pub monarch: Option<MonarchSyncReport>,
    pub errors: Vec<SyncErrorEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncErrorEntry {
    pub provider: String,
    pub message: String,
}

pub fn handle_sync(db: &Database, provider: Option<String>, since: Option<String>) {
    let since = match since {
        Some(s) => match NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(e) => {
                print_error(&format!(
                    "invalid --since date \"{s}\" (expected YYYY-MM-DD): {e}"
                ));
                std::process::exit(1);
            }
        },
        None => None,
    };

    let provider = provider.map(|p| p.to_lowercase());

    match provider.as_deref() {
        Some("simplefin") => run_simplefin(db, since),
        Some("pluggy") => run_pluggy(db, since),
        Some("teller") => run_teller(db, since),
        Some("monarch") => run_monarch(db, since),
        Some(other) => {
            print_error(&format!(
                "unknown provider \"{other}\" (expected \"monarch\", \"simplefin\", \"pluggy\", or \"teller\")"
            ));
            std::process::exit(1);
        }
        None => run_unified(db, since),
    }
}

fn run_monarch(db: &Database, since: Option<NaiveDate>) {
    match run_monarch_inline(db, since) {
        Ok(report) => {
            let response = CliResponse::ok(report);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

/// Runs the Monarch sync via the installed `mmoney` CLI. No stored
/// credentials needed — mmoney handles auth itself (macOS keychain). If
/// mmoney is missing or not logged in, the adapter surfaces a clear error.
fn run_monarch_inline(
    db: &Database,
    since: Option<NaiveDate>,
) -> Result<MonarchSyncReport, DomainError> {
    let adapter = MonarchAdapter::subprocess();
    let svc = MonarchSyncService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
        SqliteCategoryRepository::new(db),
        SqliteTagRepository::new(db),
    );
    svc.run(&adapter, since)
}

fn run_unified(db: &Database, since: Option<NaiveDate>) {
    let mut report = UnifiedSyncReport {
        simplefin: None,
        pluggy: None,
        teller: None,
        monarch: None,
        errors: Vec::new(),
    };

    match run_simplefin_inline(db, since) {
        Ok(Some(r)) => report.simplefin = Some(r),
        Ok(None) => {}
        Err(e) => report.errors.push(SyncErrorEntry {
            provider: "simplefin".into(),
            message: e.to_string(),
        }),
    }

    match run_pluggy_inline(db, since) {
        Ok(Some(r)) => report.pluggy = Some(r),
        Ok(None) => {}
        Err(e) => report.errors.push(SyncErrorEntry {
            provider: "pluggy".into(),
            message: e.to_string(),
        }),
    }

    match run_teller_inline(db, since) {
        Ok(Some(r)) => report.teller = Some(r),
        Ok(None) => {}
        Err(e) => report.errors.push(SyncErrorEntry {
            provider: "teller".into(),
            message: e.to_string(),
        }),
    }

    // Monarch is NOT auto-run in the unified path. mmoney is a system-wide
    // tool holding the user's global auth — pulling it silently on every
    // `rtf sync` is surprising behavior. Users opt in explicitly via
    // `rtf sync --provider monarch`.

    if report.simplefin.is_none()
        && report.pluggy.is_none()
        && report.teller.is_none()
        && report.errors.is_empty()
    {
        print_error(
            "no bank-sync providers configured; run `rtf simplefin setup <token>`, `rtf pluggy setup ...`, `rtf teller setup --access-token <token>`, or for Monarch: `mmoney auth login` then `rtf sync --provider monarch`",
        );
        std::process::exit(1);
    }

    let any_success =
        report.simplefin.is_some() || report.pluggy.is_some() || report.teller.is_some();
    let response = CliResponse::ok(report);
    println!("{}", serde_json::to_string_pretty(&response).unwrap());

    if !any_success {
        std::process::exit(1);
    }
}

/// Inline variant of `run_simplefin` that returns `Ok(None)` when the
/// provider isn't configured, `Ok(Some(report))` on success, and `Err` on
/// real failures. Used by `run_unified` so one provider's absence doesn't
/// kill the other's sync.
fn run_simplefin_inline(
    db: &Database,
    since: Option<NaiveDate>,
) -> Result<Option<ProviderSyncReport>, DomainError> {
    let creds_repo = SqliteProviderCredentialsRepository::new(db);
    let Some(creds) = creds_repo.find_by_provider("simplefin")? else {
        return Ok(None);
    };
    let access_url = parse_access_url(&creds.data)?;
    let http = UreqHttpClient::new();
    let adapter = SimpleFinAdapter::new(&http, access_url);
    let svc = SyncService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
    );
    svc.sync_provider(&adapter, since).map(Some)
}

fn run_pluggy(db: &Database, since: Option<NaiveDate>) {
    match run_pluggy_inline(db, since) {
        Ok(Some(report)) => {
            let response = CliResponse::ok(report);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Ok(None) => {
            print_error(
                "Pluggy not configured — run `rtf pluggy setup --client-id ... --client-secret ...` first",
            );
            std::process::exit(1);
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn run_pluggy_inline(
    db: &Database,
    since: Option<NaiveDate>,
) -> Result<Option<ProviderSyncReport>, DomainError> {
    // 1. Per-app credentials (client_id + client_secret).
    let creds_repo = SqliteProviderCredentialsRepository::new(db);
    let Some(creds) = creds_repo.find_by_provider("pluggy")? else {
        return Ok(None);
    };
    let (client_id, client_secret) = parse_pluggy_app_creds(&creds.data)?;

    // 2. All per-bank items (one row per linked bank).
    let connections_repo = SqliteProviderConnectionRepository::new(db);
    let connections = connections_repo.find_by_provider("pluggy")?;

    if connections.is_empty() {
        return Ok(Some(ProviderSyncReport {
            provider: "pluggy".into(),
            imported: 0,
            duplicates: 0,
            accounts_synced: 0,
            window_start: None,
            window_end: None,
        }));
    }

    // 3. One HTTP client + SyncService — reused across items. PluggyAdapter
    // caches the apiKey per-instance; each item gets its own POST /auth,
    // which is acceptable noise (the per-item accounts + transactions calls
    // dominate).
    let http = UreqHttpClient::new();
    let svc = SyncService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
    );

    // 4. Iterate items, aggregate.
    let mut imported = 0usize;
    let mut duplicates = 0usize;
    let mut accounts_synced = 0usize;
    let mut window_start: Option<NaiveDate> = None;
    let mut window_end: Option<NaiveDate> = None;

    for conn in &connections {
        let adapter = PluggyAdapter::new(
            &http,
            client_id.clone(),
            client_secret.clone(),
            conn.external_id.clone(),
        );
        let report = svc.sync_provider(&adapter, since)?;
        imported += report.imported;
        duplicates += report.duplicates;
        accounts_synced += report.accounts_synced;
        window_start = min_date(window_start, report.window_start);
        window_end = max_date(window_end, report.window_end);
    }

    Ok(Some(ProviderSyncReport {
        provider: "pluggy".into(),
        imported,
        duplicates,
        accounts_synced,
        window_start,
        window_end,
    }))
}

fn parse_pluggy_app_creds(data: &str) -> Result<(String, String), DomainError> {
    let parsed: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| DomainError::Import(format!("pluggy app credentials JSON: {e}")))?;
    let fetch = |key: &str| -> Result<String, DomainError> {
        parsed
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| DomainError::Import(format!("missing {key} in pluggy credentials")))
    };
    Ok((fetch("client_id")?, fetch("client_secret")?))
}

fn run_teller(db: &Database, since: Option<NaiveDate>) {
    match run_teller_inline(db, since) {
        Ok(Some(report)) => {
            let response = CliResponse::ok(report);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Ok(None) => {
            print_error(
                "Teller not configured — run `rtf teller setup --app-id ... --cert ... --key ...` first",
            );
            std::process::exit(1);
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn run_teller_inline(
    db: &Database,
    since: Option<NaiveDate>,
) -> Result<Option<ProviderSyncReport>, DomainError> {
    // 1. Per-app credentials (cert + key for mTLS, app_id for setup, etc.).
    let creds_repo = SqliteProviderCredentialsRepository::new(db);
    let Some(creds) = creds_repo.find_by_provider("teller")? else {
        return Ok(None);
    };
    let app = parse_teller_app_creds(&creds.data)?;

    // 2. All per-bank enrollments (one row per linked bank).
    let connections_repo = SqliteProviderConnectionRepository::new(db);
    let connections = connections_repo.find_by_provider("teller")?;

    // No enrollments yet — caller treats this as "configured but empty".
    // Return a zero report so unified sync can still show Teller as a slot.
    if connections.is_empty() {
        return Ok(Some(ProviderSyncReport {
            provider: "teller".into(),
            imported: 0,
            duplicates: 0,
            accounts_synced: 0,
            window_start: None,
            window_end: None,
        }));
    }

    // 3. Build one HTTP client — mTLS if cert+key present, plain otherwise.
    // Reused across every enrollment.
    let http = build_teller_http(&app)?;
    let svc = SyncService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
    );

    // 4. Iterate enrollments, aggregate the per-enrollment reports.
    let mut imported = 0usize;
    let mut duplicates = 0usize;
    let mut accounts_synced = 0usize;
    let mut window_start: Option<NaiveDate> = None;
    let mut window_end: Option<NaiveDate> = None;

    for conn in &connections {
        let token = parse_connection_access_token(&conn.data).map_err(|e| {
            DomainError::Import(format!(
                "connection {} (institution {:?}): {}",
                conn.id, conn.institution_name, e
            ))
        })?;
        let adapter = TellerAdapter::new(&http, token);
        let report = svc.sync_provider(&adapter, since)?;
        imported += report.imported;
        duplicates += report.duplicates;
        accounts_synced += report.accounts_synced;
        window_start = min_date(window_start, report.window_start);
        window_end = max_date(window_end, report.window_end);
    }

    Ok(Some(ProviderSyncReport {
        provider: "teller".into(),
        imported,
        duplicates,
        accounts_synced,
        window_start,
        window_end,
    }))
}

struct TellerAppCreds {
    cert_pem: Option<String>,
    key_pem: Option<String>,
}

fn parse_teller_app_creds(data: &str) -> Result<TellerAppCreds, DomainError> {
    let parsed: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| DomainError::Import(format!("teller app credentials JSON: {e}")))?;
    Ok(TellerAppCreds {
        cert_pem: parsed
            .get("cert_pem")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        key_pem: parsed
            .get("key_pem")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

fn parse_connection_access_token(data: &str) -> Result<String, DomainError> {
    let parsed: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| DomainError::Import(format!("connection data JSON: {e}")))?;
    parsed
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DomainError::Import("connection missing access_token".into()))
}

fn build_teller_http(app: &TellerAppCreds) -> Result<UreqHttpClient, DomainError> {
    match (&app.cert_pem, &app.key_pem) {
        (Some(c), Some(k)) => UreqHttpClient::with_mtls(c, k),
        (None, None) => Ok(UreqHttpClient::new()),
        _ => Err(DomainError::Import(
            "Teller app credentials have cert_pem XOR key_pem; expected both or neither".into(),
        )),
    }
}

fn min_date(a: Option<NaiveDate>, b: Option<NaiveDate>) -> Option<NaiveDate> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

fn max_date(a: Option<NaiveDate>, b: Option<NaiveDate>) -> Option<NaiveDate> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

fn run_simplefin(db: &Database, since: Option<NaiveDate>) {
    let creds_repo = SqliteProviderCredentialsRepository::new(db);
    let creds = match creds_repo.find_by_provider("simplefin") {
        Ok(Some(c)) => c,
        Ok(None) => {
            print_error("SimpleFIN not configured — run `rtf simplefin setup <setup-token>` first");
            std::process::exit(1);
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };

    let access_url = match parse_access_url(&creds.data) {
        Ok(u) => u,
        Err(e) => {
            print_error(&format!("SimpleFIN credentials unreadable: {e}"));
            std::process::exit(1);
        }
    };

    let http = UreqHttpClient::new();
    let adapter = SimpleFinAdapter::new(&http, access_url);
    let svc = SyncService::new(
        SqliteTransactionRepository::new(db),
        SqliteAccountRepository::new(db),
    );

    match svc.sync_provider(&adapter, since) {
        Ok(report) => {
            let response = CliResponse::ok(report);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn parse_access_url(data: &str) -> Result<String, DomainError> {
    let parsed: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| DomainError::Import(format!("credentials JSON: {e}")))?;
    parsed
        .get("access_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DomainError::Import("missing access_url in credentials".into()))
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}
