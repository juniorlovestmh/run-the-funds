//! Pluggy per-app credential persistence + browser-based Connect flow (S04C).
//!
//! Two commands:
//!
//!   - `rtf pluggy setup --client-id <ID> --client-secret <SECRET>`
//!     stores the per-app credentials in `provider_credentials[provider='pluggy']`.
//!     Per-bank items come later via `pluggy connect`.
//!
//!   - `rtf pluggy connect` loads those credentials, exchanges
//!     them for an API key (`POST /auth`), mints a short-lived Connect Token
//!     (`POST /connect_token`), opens the default browser to an HTML page
//!     that embeds Pluggy Connect parameterized with that token, waits for
//!     the user to link a bank, captures the `itemData` payload, and
//!     stores the resulting `item.id` as a new `provider_connections` row.
//!     Can be run N times to link N banks.

use serde_json::Value;
use uuid::Uuid;

use crate::domain::connections::{ProviderConnection, ProviderConnectionRepository};
use crate::domain::credentials::{ProviderCredentials, ProviderCredentialsRepository};
use crate::domain::error::DomainError;
use crate::infrastructure::connect::{
    launch_browser, render_template, CallbackKind, ConnectServer,
};
use crate::infrastructure::http::{HttpClient, UreqHttpClient};
use crate::infrastructure::storage::{
    Database, SqliteProviderConnectionRepository, SqliteProviderCredentialsRepository,
};

use super::response::{CliResponse, ErrorResponse};

const CONNECT_HTML: &str = include_str!("pluggy_connect.html");
const CONNECT_TIMEOUT_SECS: u64 = 300;
const PLUGGY_API_BASE: &str = "https://api.pluggy.ai";

// ---- setup -----------------------------------------------------------------

pub fn handle_setup(db: &Database, client_id: String, client_secret: String) {
    match store_app_creds(db, client_id, client_secret) {
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
    client_id: String,
    client_secret: String,
) -> Result<String, DomainError> {
    if client_id.trim().is_empty() {
        return Err(DomainError::Validation("--client-id is required".into()));
    }
    if client_secret.trim().is_empty() {
        return Err(DomainError::Validation("--client-secret is required".into()));
    }

    let data = serde_json::json!({
        "client_id": client_id,
        "client_secret": client_secret,
    })
    .to_string();

    let creds = ProviderCredentials::new(Uuid::new_v4().to_string(), "pluggy".into(), data)?;
    SqliteProviderCredentialsRepository::new(db).save(&creds)?;

    Ok("Pluggy app credentials saved; run `rtf pluggy connect` to link a bank.".into())
}

// ---- connect ---------------------------------------------------------------

pub fn handle_connect(db: &Database) {
    let http = UreqHttpClient::new();
    match do_connect(db, &http) {
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

fn do_connect<C: HttpClient>(
    db: &Database,
    http: &C,
) -> Result<serde_json::Value, DomainError> {
    // 1. Load per-app credentials.
    let (client_id, client_secret) = load_app_creds(db)?;

    // 2. Exchange for an API key, then mint a Connect Token.
    let api_key = fetch_api_key(http, &client_id, &client_secret)?;
    let connect_token = fetch_connect_token(http, &api_key)?;

    // 3. Render the widget HTML with the connect_token and include-sandbox
    // flag (default true for dev tier — lets sandbox institutions show up
    // alongside real ones).
    let html = render_template(
        CONNECT_HTML,
        &[
            ("CONNECT_TOKEN", &connect_token),
            ("INCLUDE_SANDBOX", "true"),
        ],
    );

    // 4. Open browser + wait for callback.
    let server = ConnectServer::bind()?;
    let url = server.url();
    eprintln!("Pluggy Connect: opening {url}");
    eprintln!("(If the browser didn't launch, paste that URL yourself.)");
    launch_browser(&url);

    let captured = server.run_until_callback(&html, CONNECT_TIMEOUT_SECS)?;
    match captured.kind {
        CallbackKind::Success => {}
        CallbackKind::Failure => {
            return Err(DomainError::Import(format!(
                "Pluggy Connect widget reported a failure: {}",
                captured.body
            )));
        }
        CallbackKind::Exit => {
            return Err(DomainError::Import(
                "Pluggy Connect cancelled (user closed the widget)".into(),
            ));
        }
    }

    // 5. Parse the captured item + persist.
    let (item_id, institution_name) = parse_item(&captured.body)?;
    persist_item(db, &item_id, institution_name.as_deref())?;

    Ok(serde_json::json!({
        "provider": "pluggy",
        "item_id": item_id,
        "institution_name": institution_name,
        "message": "Linked. Run `rtf sync --provider pluggy` to pull transactions.",
    }))
}

fn load_app_creds(db: &Database) -> Result<(String, String), DomainError> {
    let repo = SqliteProviderCredentialsRepository::new(db);
    let creds = repo.find_by_provider("pluggy")?.ok_or_else(|| {
        DomainError::Import(
            "Pluggy app credentials not configured — run `rtf pluggy setup --client-id ... --client-secret ...` first"
                .into(),
        )
    })?;
    let parsed: Value = serde_json::from_str(&creds.data)
        .map_err(|e| DomainError::Import(format!("pluggy credentials JSON: {e}")))?;
    let client_id = parsed
        .get("client_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DomainError::Import("pluggy credentials missing client_id".into()))?
        .to_string();
    let client_secret = parsed
        .get("client_secret")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DomainError::Import("pluggy credentials missing client_secret".into()))?
        .to_string();
    Ok((client_id, client_secret))
}

fn fetch_api_key<C: HttpClient>(
    http: &C,
    client_id: &str,
    client_secret: &str,
) -> Result<String, DomainError> {
    let body = serde_json::json!({
        "clientId": client_id,
        "clientSecret": client_secret,
    })
    .to_string();
    let response = http.post(
        &format!("{PLUGGY_API_BASE}/auth"),
        &body,
        &[("Content-Type", "application/json")],
    )?;
    let parsed: Value = serde_json::from_str(&response)
        .map_err(|e| DomainError::Import(format!("Pluggy auth JSON parse: {e}")))?;
    parsed
        .get("apiKey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DomainError::Import("Pluggy auth response missing 'apiKey'".into()))
}

fn fetch_connect_token<C: HttpClient>(
    http: &C,
    api_key: &str,
) -> Result<String, DomainError> {
    let response = http.post(
        &format!("{PLUGGY_API_BASE}/connect_token"),
        "{}",
        &[
            ("X-API-KEY", api_key),
            ("Content-Type", "application/json"),
        ],
    )?;
    let parsed: Value = serde_json::from_str(&response).map_err(|e| {
        DomainError::Import(format!("Pluggy connect_token JSON parse: {e}"))
    })?;
    parsed
        .get("accessToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            DomainError::Import("Pluggy connect_token response missing 'accessToken'".into())
        })
}

fn parse_item(body: &str) -> Result<(String, Option<String>), DomainError> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|e| DomainError::Import(format!("Pluggy item payload not JSON: {e}")))?;
    // Pluggy's widget onSuccess payload is typically shaped either:
    //   { "item": { "id": "...", "connector": { "name": "..." } } }
    // or (some SDK versions):
    //   { "id": "...", "connector": { ... } }
    // Accept both.
    let item = parsed.get("item").unwrap_or(&parsed);
    let item_id = item
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DomainError::Import("Pluggy item payload missing item.id".into()))?;
    let institution_name = item
        .get("connector")
        .and_then(|c| c.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok((item_id, institution_name))
}

fn persist_item(
    db: &Database,
    item_id: &str,
    institution_name: Option<&str>,
) -> Result<(), DomainError> {
    // Pluggy items don't have a per-item access_token — all auth goes through
    // the per-app apiKey reminted on each sync. So `data` stays as an empty
    // JSON object.
    let conn = ProviderConnection::new(
        Uuid::new_v4().to_string(),
        "pluggy".into(),
        item_id.to_string(),
        "{}".into(),
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
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct FakeHttp {
        responses: RefCell<HashMap<String, String>>,
        calls: RefCell<Vec<(String, String)>>,
    }
    impl FakeHttp {
        fn new() -> Self {
            Self {
                responses: RefCell::new(HashMap::new()),
                calls: RefCell::new(Vec::new()),
            }
        }
        fn post_returns(self, url: &str, body: &str) -> Self {
            self.responses
                .borrow_mut()
                .insert(url.to_string(), body.to_string());
            self
        }
    }
    impl HttpClient for FakeHttp {
        fn get(&self, _: &str) -> Result<String, DomainError> {
            unreachable!()
        }
        fn post(
            &self,
            url: &str,
            body: &str,
            _headers: &[(&str, &str)],
        ) -> Result<String, DomainError> {
            self.calls
                .borrow_mut()
                .push((url.to_string(), body.to_string()));
            self.responses
                .borrow()
                .get(url)
                .cloned()
                .ok_or_else(|| DomainError::Import(format!("no canned POST for {url}")))
        }
    }

    // ---- setup tests --------------------------------------------------------

    #[test]
    fn store_app_creds_happy_path() {
        let db = Database::in_memory().unwrap();
        store_app_creds(&db, "cid".into(), "csec".into()).unwrap();
        let creds = SqliteProviderCredentialsRepository::new(&db)
            .find_by_provider("pluggy")
            .unwrap()
            .unwrap();
        let parsed: Value = serde_json::from_str(&creds.data).unwrap();
        assert_eq!(parsed["client_id"], "cid");
        assert_eq!(parsed["client_secret"], "csec");
        // No item_id at this layer — that lives in provider_connections now.
        assert!(parsed.get("item_id").is_none());
    }

    #[test]
    fn store_app_creds_rejects_empty_fields() {
        let db = Database::in_memory().unwrap();
        assert!(store_app_creds(&db, "".into(), "x".into()).is_err());
        assert!(store_app_creds(&db, "x".into(), "".into()).is_err());
    }

    // ---- connect flow tests -------------------------------------------------

    #[test]
    fn fetch_api_key_roundtrips() {
        let http = FakeHttp::new()
            .post_returns(&format!("{PLUGGY_API_BASE}/auth"), r#"{"apiKey":"kx"}"#);
        let key = fetch_api_key(&http, "cid", "csec").unwrap();
        assert_eq!(key, "kx");
    }

    #[test]
    fn fetch_api_key_missing_field_errors() {
        let http = FakeHttp::new()
            .post_returns(&format!("{PLUGGY_API_BASE}/auth"), r#"{"wrong":"shape"}"#);
        assert!(fetch_api_key(&http, "cid", "csec").is_err());
    }

    #[test]
    fn fetch_connect_token_roundtrips() {
        let http = FakeHttp::new().post_returns(
            &format!("{PLUGGY_API_BASE}/connect_token"),
            r#"{"accessToken":"ct_xyz"}"#,
        );
        let ct = fetch_connect_token(&http, "api_key_here").unwrap();
        assert_eq!(ct, "ct_xyz");
    }

    #[test]
    fn parse_item_accepts_wrapped_shape() {
        let payload = r#"{"item":{"id":"item-abc","connector":{"name":"Nubank"}}}"#;
        let (id, inst) = parse_item(payload).unwrap();
        assert_eq!(id, "item-abc");
        assert_eq!(inst.as_deref(), Some("Nubank"));
    }

    #[test]
    fn parse_item_accepts_flat_shape() {
        let payload = r#"{"id":"item-abc","connector":{"name":"Itau"}}"#;
        let (id, inst) = parse_item(payload).unwrap();
        assert_eq!(id, "item-abc");
        assert_eq!(inst.as_deref(), Some("Itau"));
    }

    #[test]
    fn parse_item_missing_id_errors() {
        assert!(parse_item(r#"{"item":{"connector":{"name":"X"}}}"#).is_err());
    }

    #[test]
    fn parse_item_allows_missing_connector_name() {
        let (id, inst) = parse_item(r#"{"item":{"id":"item-abc"}}"#).unwrap();
        assert_eq!(id, "item-abc");
        assert!(inst.is_none());
    }

    #[test]
    fn persist_item_upserts_same_item_id() {
        let db = Database::in_memory().unwrap();
        persist_item(&db, "item-1", Some("Nubank")).unwrap();
        persist_item(&db, "item-1", Some("Nubank Rotated")).unwrap();
        let rows = SqliteProviderConnectionRepository::new(&db)
            .find_by_provider("pluggy")
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].institution_name.as_deref(), Some("Nubank Rotated"));
    }

    #[test]
    fn persist_item_adds_row_for_different_item_id() {
        let db = Database::in_memory().unwrap();
        persist_item(&db, "item-1", Some("Nubank")).unwrap();
        persist_item(&db, "item-2", Some("Itau")).unwrap();
        let rows = SqliteProviderConnectionRepository::new(&db)
            .find_by_provider("pluggy")
            .unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn load_app_creds_errors_when_missing() {
        let db = Database::in_memory().unwrap();
        assert!(load_app_creds(&db).is_err());
    }

    #[test]
    fn load_app_creds_roundtrip() {
        let db = Database::in_memory().unwrap();
        store_app_creds(&db, "cid".into(), "csec".into()).unwrap();
        let (cid, csec) = load_app_creds(&db).unwrap();
        assert_eq!(cid, "cid");
        assert_eq!(csec, "csec");
    }
}
