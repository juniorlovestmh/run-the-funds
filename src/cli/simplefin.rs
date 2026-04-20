//! SimpleFIN setup-token exchange + persistence (S04).
//!
//! One-time flow per SimpleFIN subscription:
//!   1. User pastes their SimpleFIN setup token (base64-encoded URL) into
//!      `rtf simplefin setup <token>`.
//!   2. We decode the token into the claim URL.
//!   3. POST to that URL with an empty body.
//!   4. Response body IS the access URL (contains embedded basic-auth creds).
//!   5. Persist `{"access_url": "<url>"}` into `provider_credentials` keyed
//!      on `provider = "simplefin"`.
//!
//! Re-running replaces the stored credentials (upsert by provider).

use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE as B64_URL};
use base64::Engine;
use uuid::Uuid;

use crate::domain::credentials::{ProviderCredentials, ProviderCredentialsRepository};
use crate::domain::error::DomainError;
use crate::infrastructure::http::{HttpClient, UreqHttpClient};
use crate::infrastructure::storage::{Database, SqliteProviderCredentialsRepository};

use super::response::{CliResponse, ErrorResponse};

pub fn handle_setup(db: &Database, token: String) {
    let client = UreqHttpClient::new();
    match exchange_and_store(db, &client, &token) {
        Ok(_) => {
            let response = CliResponse::ok(serde_json::json!({
                "message": "SimpleFIN configured; run `rtf sync --provider simplefin` to pull transactions.",
            }));
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn exchange_and_store<C: HttpClient>(
    db: &Database,
    client: &C,
    token: &str,
) -> Result<(), DomainError> {
    let claim_url = decode_setup_token(token)?;
    let access_url = client.post(&claim_url, "", &[])?;
    let access_url = access_url.trim().to_string();
    if access_url.is_empty() {
        return Err(DomainError::Import(
            "SimpleFIN claim returned an empty access URL".into(),
        ));
    }

    let data = serde_json::json!({ "access_url": access_url }).to_string();
    let creds =
        ProviderCredentials::new(Uuid::new_v4().to_string(), "simplefin".into(), data)?;
    let repo = SqliteProviderCredentialsRepository::new(db);
    repo.save(&creds)?;
    Ok(())
}

fn decode_setup_token(token: &str) -> Result<String, DomainError> {
    let trimmed = token.trim();
    // SimpleFIN tokens are base64-encoded URLs. Try standard first, fall back
    // to URL-safe (for tokens that may use `-` / `_` instead of `+` / `/`).
    let bytes = B64
        .decode(trimmed)
        .or_else(|_| B64_URL.decode(trimmed))
        .map_err(|e| {
            DomainError::Import(format!("SimpleFIN setup token base64 decode: {e}"))
        })?;
    let url = std::str::from_utf8(&bytes)
        .map_err(|e| DomainError::Import(format!("SimpleFIN setup token not utf-8: {e}")))?
        .trim()
        .to_string();
    if !url.starts_with("http") {
        return Err(DomainError::Import(format!(
            "SimpleFIN setup token did not decode to an http(s) URL: got {url:?}"
        )));
    }
    Ok(url)
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeHttpClient {
        response: RefCell<Option<String>>,
    }
    impl FakeHttpClient {
        fn new(response: &str) -> Self {
            Self {
                response: RefCell::new(Some(response.to_string())),
            }
        }
    }
    impl HttpClient for FakeHttpClient {
        fn get(&self, _url: &str) -> Result<String, DomainError> {
            unreachable!()
        }
        fn post(
            &self,
            _url: &str,
            _body: &str,
            _headers: &[(&str, &str)],
        ) -> Result<String, DomainError> {
            Ok(self.response.borrow_mut().take().unwrap())
        }
    }

    #[test]
    fn decode_setup_token_standard_base64() {
        let url = "https://bridge.simplefin.org/simplefin/claim/X1b2";
        let token = B64.encode(url);
        assert_eq!(decode_setup_token(&token).unwrap(), url);
    }

    #[test]
    fn decode_setup_token_url_safe_base64() {
        let url = "https://bridge.simplefin.org/simplefin/claim/abc_def-ghi";
        let token = B64_URL.encode(url);
        assert_eq!(decode_setup_token(&token).unwrap(), url);
    }

    #[test]
    fn decode_setup_token_rejects_garbage() {
        let err = decode_setup_token("not-base64!!!").unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn decode_setup_token_rejects_non_http_result() {
        let token = B64.encode("ftp://not-a-simplefin-url");
        let err = decode_setup_token(&token).unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }

    #[test]
    fn exchange_and_store_persists_access_url() {
        let db = Database::in_memory().unwrap();
        let access_url = "https://u:p@bridge.simplefin.org/simplefin";
        let client = FakeHttpClient::new(access_url);
        let token = B64.encode("https://bridge.simplefin.org/claim/xyz");

        exchange_and_store(&db, &client, &token).unwrap();

        let repo = SqliteProviderCredentialsRepository::new(&db);
        let creds = repo.find_by_provider("simplefin").unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&creds.data).unwrap();
        assert_eq!(parsed["access_url"], access_url);
    }

    #[test]
    fn exchange_and_store_rejects_empty_response() {
        let db = Database::in_memory().unwrap();
        let client = FakeHttpClient::new("   ");
        let token = B64.encode("https://bridge.simplefin.org/claim/xyz");
        let err = exchange_and_store(&db, &client, &token).unwrap_err();
        assert!(matches!(err, DomainError::Import(_)));
    }
}
