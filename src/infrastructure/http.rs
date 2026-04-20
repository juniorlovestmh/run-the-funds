//! Shared HTTP client abstraction for infrastructure adapters.
//!
//! Three methods keep the trait small but cover every shape our adapters
//! need: plain GET (BCB PTAX), GET with basic auth (SimpleFIN access URL),
//! and POST with custom headers (SimpleFIN setup-token exchange, Pluggy
//! auth + data fetch).
//!
//! Production uses `UreqHttpClient`. `UreqHttpClient::with_mtls(cert_pem,
//! key_pem)` builds one configured with a client certificate for providers
//! that require mutual TLS (Teller's Developer and Production tiers).
//! Tests inject fakes that return canned responses keyed by URL.

use std::io::Cursor;
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

use crate::domain::error::DomainError;

pub trait HttpClient {
    fn get(&self, url: &str) -> Result<String, DomainError>;

    /// Default panic-impl lets test fakes only implement the methods they
    /// actually exercise. Production `UreqHttpClient` overrides all three.
    fn get_with_basic_auth(
        &self,
        _url: &str,
        _user: &str,
        _pass: &str,
    ) -> Result<String, DomainError> {
        Err(DomainError::Import(
            "get_with_basic_auth not implemented on this HttpClient".into(),
        ))
    }

    fn post(
        &self,
        _url: &str,
        _body: &str,
        _headers: &[(&str, &str)],
    ) -> Result<String, DomainError> {
        Err(DomainError::Import(
            "post not implemented on this HttpClient".into(),
        ))
    }

    fn get_with_headers(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
    ) -> Result<String, DomainError> {
        Err(DomainError::Import(
            "get_with_headers not implemented on this HttpClient".into(),
        ))
    }
}

pub struct UreqHttpClient {
    agent: ureq::Agent,
}

impl Default for UreqHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqHttpClient {
    /// Plain HTTPS client — the default rustls agent that ureq builds with
    /// its `tls` feature. Works for every provider that doesn't require
    /// mutual TLS (BCB, SimpleFIN, Pluggy).
    pub fn new() -> Self {
        Self {
            agent: ureq::Agent::new(),
        }
    }

    /// Build a client that presents a client certificate on every request.
    /// Needed by Teller's Developer and Production tiers; Sandbox does not
    /// require it. `cert_pem` and `key_pem` are the raw file contents —
    /// callers typically read these from `provider_credentials.data` where
    /// they were embedded by `rtf teller setup --cert --key`.
    pub fn with_mtls(cert_pem: &str, key_pem: &str) -> Result<Self, DomainError> {
        // rustls 0.23 needs a process-wide CryptoProvider. Install ring
        // on first use; subsequent calls are no-ops. `install_default`
        // returns an error if one is already installed — ignore it.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
            rustls_pemfile::certs(&mut Cursor::new(cert_pem))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| DomainError::Import(format!("parse client cert PEM: {e}")))?;
        if certs.is_empty() {
            return Err(DomainError::Import(
                "client cert PEM contained no certificates".into(),
            ));
        }

        let key = rustls_pemfile::private_key(&mut Cursor::new(key_pem))
            .map_err(|e| DomainError::Import(format!("parse client key PEM: {e}")))?
            .ok_or_else(|| DomainError::Import("client key PEM contained no private key".into()))?;

        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_client_auth_cert(certs, key)
            .map_err(|e| DomainError::Import(format!("build TLS config: {e}")))?;

        let agent = ureq::AgentBuilder::new()
            .tls_config(Arc::new(config))
            .build();

        Ok(Self { agent })
    }

    /// Collapse a `ureq::Result<Response>` into `Result<String, DomainError>`.
    /// Critically, on non-2xx we extract the response BODY (not just the
    /// status code) so messages like `{"errors":["Payment required."]}` reach
    /// the operator. Without this, the user sees `status code 402` and has
    /// no idea why.
    fn collapse(
        context: &str,
        url: &str,
        result: Result<ureq::Response, ureq::Error>,
    ) -> Result<String, DomainError> {
        match result {
            Ok(response) => response
                .into_string()
                .map_err(|e| DomainError::Import(format!("{context} {url}: read body: {e}"))),
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                let preview: String = body.chars().take(300).collect();
                Err(DomainError::Import(format!(
                    "{context} {url}: HTTP {code}: {preview}"
                )))
            }
            Err(ureq::Error::Transport(t)) => Err(DomainError::Import(format!(
                "{context} {url}: transport error: {t}"
            ))),
        }
    }
}

impl HttpClient for UreqHttpClient {
    fn get(&self, url: &str) -> Result<String, DomainError> {
        Self::collapse("GET", url, self.agent.get(url).call())
    }

    fn get_with_basic_auth(
        &self,
        url: &str,
        user: &str,
        pass: &str,
    ) -> Result<String, DomainError> {
        let encoded = B64.encode(format!("{user}:{pass}"));
        let result = self
            .agent
            .get(url)
            .set("Authorization", &format!("Basic {encoded}"))
            .call();
        Self::collapse("GET (basic auth)", url, result)
    }

    fn post(&self, url: &str, body: &str, headers: &[(&str, &str)]) -> Result<String, DomainError> {
        let mut req = self.agent.post(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        Self::collapse("POST", url, req.send_string(body))
    }

    fn get_with_headers(&self, url: &str, headers: &[(&str, &str)]) -> Result<String, DomainError> {
        let mut req = self.agent.get(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        Self::collapse("GET (headers)", url, req.call())
    }
}
