//! Teller per-app credential persistence + browser-based Connect flow (S04C).
//!
//! Two commands:
//!
//!   - `rtf teller setup --app-id <APP_ID> [--cert <path> --key <path>] [--environment <env>]`
//!     stores the per-app credentials in `provider_credentials[provider='teller']`.
//!     These are shared across every enrollment. Developer/Production tiers
//!     require cert+key (mTLS); Sandbox does not.
//!
//!   - `rtf teller connect` loads those credentials, spins up a local
//!     HTTP server, opens the default browser to an HTML page that embeds
//!     Teller Connect, waits for the user to link a bank, captures the
//!     enrollment payload from the widget's `onSuccess` callback, and
//!     stores it as a new row in `provider_connections`. Can be run N
//!     times to link N banks — no upsert on the stored enrollment id,
//!     so adding a second bank doesn't overwrite the first.

use uuid::Uuid;

use crate::domain::connections::{ProviderConnection, ProviderConnectionRepository};
use crate::domain::credentials::{ProviderCredentials, ProviderCredentialsRepository};
use crate::domain::error::DomainError;
use crate::infrastructure::connect::{
    CallbackKind, ConnectServer, launch_browser, render_template,
};
use crate::infrastructure::storage::{
    Database, SqliteProviderConnectionRepository, SqliteProviderCredentialsRepository,
};

use super::response::{CliResponse, ErrorResponse};

const CONNECT_HTML: &str = include_str!("teller_connect.html");
const CONNECT_TIMEOUT_SECS: u64 = 300;

// ---- setup -----------------------------------------------------------------

pub fn handle_setup(
    db: &Database,
    app_id: String,
    cert_path: Option<String>,
    key_path: Option<String>,
    environment: Option<String>,
) {
    match store_app_creds(db, app_id, cert_path, key_path, environment) {
        Ok(msg) => {
            let response = CliResponse::ok(serde_json::json!({ "message": msg }));
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn store_app_creds(
    db: &Database,
    app_id: String,
    cert_path: Option<String>,
    key_path: Option<String>,
    environment: Option<String>,
) -> Result<String, DomainError> {
    if app_id.trim().is_empty() {
        return Err(DomainError::Validation("--app-id is required".into()));
    }

    let (cert_pem, key_pem, default_env) = match (cert_path, key_path) {
        (Some(c), Some(k)) => {
            let cert_pem = std::fs::read_to_string(&c)
                .map_err(|e| DomainError::Import(format!("read --cert {c}: {e}")))?;
            let key_pem = std::fs::read_to_string(&k)
                .map_err(|e| DomainError::Import(format!("read --key {k}: {e}")))?;
            (Some(cert_pem), Some(key_pem), "development")
        }
        (None, None) => (None, None, "sandbox"),
        _ => {
            return Err(DomainError::Validation(
                "--cert and --key must be provided together (both, or neither)".into(),
            ));
        }
    };

    let env = environment.unwrap_or_else(|| default_env.to_string());
    if !matches!(env.as_str(), "sandbox" | "development" | "production") {
        return Err(DomainError::Validation(format!(
            "--environment must be sandbox|development|production; got {env}"
        )));
    }

    let mut blob = serde_json::Map::new();
    blob.insert("app_id".into(), serde_json::Value::String(app_id));
    blob.insert("environment".into(), serde_json::Value::String(env.clone()));
    if let (Some(c), Some(k)) = (cert_pem, key_pem) {
        blob.insert("cert_pem".into(), serde_json::Value::String(c));
        blob.insert("key_pem".into(), serde_json::Value::String(k));
    }
    let data = serde_json::Value::Object(blob).to_string();

    let creds = ProviderCredentials::new(Uuid::new_v4().to_string(), "teller".into(), data)?;
    let repo = SqliteProviderCredentialsRepository::new(db);
    repo.save(&creds)?;

    let tier_msg = match (env.as_str(), creds.data.contains("cert_pem")) {
        ("development", _) => "development tier (mTLS)",
        ("production", _) => "production tier (mTLS)",
        _ => "sandbox (Bearer-only)",
    };
    Ok(format!(
        "Teller app credentials saved ({tier_msg}); run `rtf teller connect` to link a bank."
    ))
}

// ---- connect ---------------------------------------------------------------

pub fn handle_connect(db: &Database) {
    match do_connect(db) {
        Ok(payload) => {
            let response = CliResponse::ok(payload);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn do_connect(db: &Database) -> Result<serde_json::Value, DomainError> {
    // 1. Load per-app credentials; app_id is required for Teller Connect.
    let creds_repo = SqliteProviderCredentialsRepository::new(db);
    let creds = creds_repo.find_by_provider("teller")?.ok_or_else(|| {
        DomainError::Import(
            "Teller app credentials not configured — run `rtf teller setup --app-id ... --cert ... --key ...` first"
                .into(),
        )
    })?;
    let (app_id, environment) = parse_app_id_and_env(&creds.data)?;

    // 2. Render the Connect HTML with the user's app_id + environment.
    let html = render_template(
        CONNECT_HTML,
        &[("APP_ID", &app_id), ("ENVIRONMENT", &environment)],
    );

    // 3. Spin up the callback server and point the browser at it.
    let server = ConnectServer::bind()?;
    let url = server.url();
    eprintln!("Teller Connect: opening {url}");
    eprintln!("(If the browser didn't launch, paste that URL yourself.)");
    launch_browser(&url);

    // 4. Block until the widget calls back.
    let captured = server.run_until_callback(&html, CONNECT_TIMEOUT_SECS)?;
    match captured.kind {
        CallbackKind::Success => {}
        CallbackKind::Failure => {
            return Err(DomainError::Import(format!(
                "Teller Connect widget reported a failure: {}",
                captured.body
            )));
        }
        CallbackKind::Exit => {
            return Err(DomainError::Import(
                "Teller Connect cancelled (user closed the widget)".into(),
            ));
        }
    }

    // 5. Parse the enrollment payload, upsert the connection row.
    let (enrollment_id, access_token, institution_name) = parse_enrollment(&captured.body)?;
    persist_enrollment(
        db,
        &enrollment_id,
        &access_token,
        institution_name.as_deref(),
    )?;

    Ok(serde_json::json!({
        "provider": "teller",
        "enrollment_id": enrollment_id,
        "institution_name": institution_name,
        "message": "Linked. Run `rtf sync --provider teller` to pull transactions.",
    }))
}

fn parse_app_id_and_env(data: &str) -> Result<(String, String), DomainError> {
    let parsed: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| DomainError::Import(format!("teller credentials JSON: {e}")))?;
    let app_id = parsed
        .get("app_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            DomainError::Import(
                "teller credentials missing `app_id` — re-run `rtf teller setup --app-id ...`"
                    .into(),
            )
        })?;
    let environment = parsed
        .get("environment")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "development".into());
    Ok((app_id, environment))
}

fn parse_enrollment(body: &str) -> Result<(String, String, Option<String>), DomainError> {
    let parsed: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| DomainError::Import(format!("Teller enrollment payload not JSON: {e}")))?;
    let access_token = parsed
        .get("accessToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            DomainError::Import("Teller enrollment payload missing `accessToken`".into())
        })?;
    let enrollment = parsed.get("enrollment").ok_or_else(|| {
        DomainError::Import("Teller enrollment payload missing `enrollment`".into())
    })?;
    let enrollment_id = enrollment
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            DomainError::Import("Teller enrollment payload missing `enrollment.id`".into())
        })?;
    let institution_name = enrollment
        .get("institution")
        .and_then(|i| i.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok((enrollment_id, access_token, institution_name))
}

fn persist_enrollment(
    db: &Database,
    enrollment_id: &str,
    access_token: &str,
    institution_name: Option<&str>,
) -> Result<(), DomainError> {
    let data = serde_json::json!({ "access_token": access_token }).to_string();
    let conn = ProviderConnection::new(
        Uuid::new_v4().to_string(),
        "teller".into(),
        enrollment_id.to_string(),
        data,
        institution_name.map(|s| s.to_string()),
    )?;
    SqliteProviderConnectionRepository::new(db).save(&conn)?;
    Ok(())
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_db_with_app_creds(env: &str, with_cert: bool) -> Database {
        let db = Database::in_memory().unwrap();
        let mut blob = serde_json::Map::new();
        blob.insert("app_id".into(), serde_json::Value::String("app_xxx".into()));
        blob.insert("environment".into(), serde_json::Value::String(env.into()));
        if with_cert {
            blob.insert("cert_pem".into(), serde_json::Value::String("C".into()));
            blob.insert("key_pem".into(), serde_json::Value::String("K".into()));
        }
        let data = serde_json::Value::Object(blob).to_string();
        let creds =
            ProviderCredentials::new(Uuid::new_v4().to_string(), "teller".into(), data).unwrap();
        SqliteProviderCredentialsRepository::new(&db)
            .save(&creds)
            .unwrap();
        db
    }

    // ---- setup tests --------------------------------------------------------

    #[test]
    fn store_app_creds_sandbox_no_cert() {
        let db = Database::in_memory().unwrap();
        let msg = store_app_creds(&db, "app_xxx".into(), None, None, None).unwrap();
        assert!(msg.contains("sandbox"));
        let stored = SqliteProviderCredentialsRepository::new(&db)
            .find_by_provider("teller")
            .unwrap()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&stored.data).unwrap();
        assert_eq!(parsed["app_id"], "app_xxx");
        assert_eq!(parsed["environment"], "sandbox");
        assert!(parsed.get("cert_pem").is_none());
    }

    #[test]
    fn store_app_creds_with_cert_defaults_to_development() {
        use std::io::Write;
        let db = Database::in_memory().unwrap();
        let cert_file = tempfile::NamedTempFile::new().unwrap();
        let key_file = tempfile::NamedTempFile::new().unwrap();
        cert_file.as_file().write_all(b"CERT").unwrap();
        key_file.as_file().write_all(b"KEY").unwrap();
        let msg = store_app_creds(
            &db,
            "app_xxx".into(),
            Some(cert_file.path().to_str().unwrap().into()),
            Some(key_file.path().to_str().unwrap().into()),
            None,
        )
        .unwrap();
        assert!(msg.contains("development"));
        let stored = SqliteProviderCredentialsRepository::new(&db)
            .find_by_provider("teller")
            .unwrap()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&stored.data).unwrap();
        assert_eq!(parsed["environment"], "development");
        assert_eq!(parsed["cert_pem"], "CERT");
        assert_eq!(parsed["key_pem"], "KEY");
    }

    #[test]
    fn store_app_creds_rejects_partial_cert_args() {
        let db = Database::in_memory().unwrap();
        assert!(store_app_creds(&db, "app_xxx".into(), Some("cert".into()), None, None).is_err());
    }

    #[test]
    fn store_app_creds_rejects_empty_app_id() {
        let db = Database::in_memory().unwrap();
        assert!(store_app_creds(&db, "".into(), None, None, None).is_err());
    }

    #[test]
    fn store_app_creds_rejects_bogus_environment() {
        let db = Database::in_memory().unwrap();
        assert!(
            store_app_creds(&db, "app_xxx".into(), None, None, Some("wild-west".into())).is_err()
        );
    }

    // ---- connect parsing tests ----------------------------------------------

    #[test]
    fn parse_enrollment_extracts_id_token_and_institution() {
        let payload = r#"{
            "accessToken": "token_abc",
            "user": { "id": "usr_1" },
            "enrollment": {
                "id": "enr_xyz",
                "institution": { "name": "Chase", "id": "chase" }
            }
        }"#;
        let (id, token, inst) = parse_enrollment(payload).unwrap();
        assert_eq!(id, "enr_xyz");
        assert_eq!(token, "token_abc");
        assert_eq!(inst.as_deref(), Some("Chase"));
    }

    #[test]
    fn parse_enrollment_errors_on_missing_fields() {
        assert!(parse_enrollment(r#"{"enrollment":{"id":"x"}}"#).is_err());
        assert!(parse_enrollment(r#"{"accessToken":"t"}"#).is_err());
        assert!(parse_enrollment(r#"{"accessToken":"t","enrollment":{}}"#).is_err());
    }

    #[test]
    fn parse_enrollment_allows_missing_institution() {
        let payload = r#"{
            "accessToken": "token_abc",
            "enrollment": { "id": "enr_xyz" }
        }"#;
        let (_, _, inst) = parse_enrollment(payload).unwrap();
        assert!(inst.is_none());
    }

    #[test]
    fn parse_app_id_and_env_roundtrip() {
        let db = seeded_db_with_app_creds("production", true);
        let creds = SqliteProviderCredentialsRepository::new(&db)
            .find_by_provider("teller")
            .unwrap()
            .unwrap();
        let (app_id, env) = parse_app_id_and_env(&creds.data).unwrap();
        assert_eq!(app_id, "app_xxx");
        assert_eq!(env, "production");
    }

    #[test]
    fn persist_enrollment_upserts_on_same_enrollment_id() {
        let db = Database::in_memory().unwrap();
        persist_enrollment(&db, "enr_1", "token_v1", Some("Chase")).unwrap();
        persist_enrollment(&db, "enr_1", "token_v2", Some("Chase Rotated")).unwrap();

        let repo = SqliteProviderConnectionRepository::new(&db);
        let rows = repo.find_by_provider("teller").unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].data.contains("token_v2"));
        assert_eq!(rows[0].institution_name.as_deref(), Some("Chase Rotated"));
    }

    #[test]
    fn persist_enrollment_adds_second_row_for_different_enrollment_id() {
        let db = Database::in_memory().unwrap();
        persist_enrollment(&db, "enr_1", "token_a", Some("Chase")).unwrap();
        persist_enrollment(&db, "enr_2", "token_b", Some("Capital One")).unwrap();

        let repo = SqliteProviderConnectionRepository::new(&db);
        let rows = repo.find_by_provider("teller").unwrap();
        assert_eq!(rows.len(), 2);
    }
}
