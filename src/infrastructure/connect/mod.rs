//! Local HTTP callback server + browser launcher for provider Connect
//! widgets (S04C).
//!
//! Both Teller Connect and Pluggy Connect are browser-embedded JavaScript
//! widgets: the user lands on a web page that loads the widget SDK, links
//! their bank, and the widget fires an `onSuccess` callback with the
//! enrollment/item JSON. For a CLI tool, we want that callback to come
//! back to the CLI process — so we spin up a minimal HTTP server on a
//! random localhost port, point the browser at it, serve the provider's
//! Connect HTML template, and wait for the widget to POST the payload to
//! `/success` (or `/failure` / `/exit`).
//!
//! The server shuts down as soon as it receives a terminal callback, so
//! the CLI can proceed with persisting the captured enrollment.

use std::time::{Duration, Instant};

use crate::domain::error::DomainError;

/// Captured payload from a widget callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedCallback {
    pub kind: CallbackKind,
    /// Raw request body (the widget's JSON payload).
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackKind {
    /// Bank linked successfully.
    Success,
    /// The widget reported a failure (bank auth, MFA, etc.).
    Failure,
    /// The user closed the widget without completing.
    Exit,
}

pub struct ConnectServer {
    server: tiny_http::Server,
    port: u16,
}

impl ConnectServer {
    /// Bind 127.0.0.1 on a random free port and prepare to serve `html_page`
    /// at `GET /`. The server runs synchronously in `run_until_callback` and
    /// shuts down as soon as a terminal (Success/Failure/Exit) POST arrives
    /// — or when `timeout_secs` elapses, whichever comes first.
    pub fn bind() -> Result<Self, DomainError> {
        let server = tiny_http::Server::http("127.0.0.1:0")
            .map_err(|e| DomainError::Import(format!("connect server bind 127.0.0.1:0: {e}")))?;
        let port = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| DomainError::Import("connect server: no local addr".into()))?
            .port();
        Ok(Self { server, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    /// Serve requests until a `/success`, `/failure`, or `/exit` POST arrives,
    /// or until `timeout_secs` passes. GET / returns the supplied HTML page.
    /// Every other request 404s.
    pub fn run_until_callback(
        &self,
        html_page: &str,
        timeout_secs: u64,
    ) -> Result<CapturedCallback, DomainError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);

        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    DomainError::Import("connect flow timed out before any callback".into())
                })?;

            let mut request = match self
                .server
                .recv_timeout(remaining)
                .map_err(|e| DomainError::Import(format!("connect server recv: {e}")))?
            {
                Some(req) => req,
                None => {
                    // recv_timeout returned without a request — deadline hit.
                    return Err(DomainError::Import(
                        "connect flow timed out before any callback".into(),
                    ));
                }
            };

            let method = request.method().clone();
            let url = request.url().to_string();
            let path = url.split('?').next().unwrap_or("/").to_string();

            match (method, path.as_str()) {
                (tiny_http::Method::Get, "/") => {
                    let response = tiny_http::Response::from_string(html_page.to_string())
                        .with_header(
                            "Content-Type: text/html; charset=utf-8"
                                .parse::<tiny_http::Header>()
                                .unwrap(),
                        );
                    let _ = request.respond(response);
                }
                (tiny_http::Method::Get, "/favicon.ico") => {
                    let _ = request.respond(tiny_http::Response::empty(204));
                }
                (tiny_http::Method::Post, "/success")
                | (tiny_http::Method::Post, "/failure")
                | (tiny_http::Method::Post, "/exit") => {
                    let kind = match path.as_str() {
                        "/success" => CallbackKind::Success,
                        "/failure" => CallbackKind::Failure,
                        _ => CallbackKind::Exit,
                    };
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);
                    let tab_closer = tiny_http::Response::from_string(
                        "<html><body><p>You can close this tab.</p></body></html>",
                    )
                    .with_header(
                        "Content-Type: text/html; charset=utf-8"
                            .parse::<tiny_http::Header>()
                            .unwrap(),
                    );
                    let _ = request.respond(tab_closer);
                    return Ok(CapturedCallback { kind, body });
                }
                _ => {
                    let _ = request.respond(tiny_http::Response::empty(404));
                }
            }
        }
    }
}

/// Launch the default browser at `url`. On failure (no display, SSH, etc.),
/// print the URL to stderr so the user can paste it into a browser manually.
/// Never errors — falls back silently to the stderr hint.
pub fn launch_browser(url: &str) {
    if webbrowser::open(url).is_err() {
        eprintln!("Could not auto-launch a browser. Open this URL manually: {url}");
    }
}

/// Inject runtime values into the provided HTML template. Supports simple
/// `{{KEY}}` substitution — no escaping, caller is responsible for passing
/// safe values (UUIDs, app IDs, etc. are all safe).
pub fn render_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn post_json(url: &str, body: &str) -> ureq::Response {
        ureq::post(url)
            .set("Content-Type", "application/json")
            .send_string(body)
            .expect("POST should succeed")
    }

    #[test]
    fn run_captures_success_callback() {
        let server = ConnectServer::bind().unwrap();
        let success_url = format!("{}success", server.url());

        let handle = thread::spawn(move || server.run_until_callback("<html>test</html>", 5));

        // Give the server a moment to reach recv_timeout.
        thread::sleep(Duration::from_millis(50));
        let response = post_json(&success_url, r#"{"accessToken":"token_xyz"}"#);
        assert!(response.status() < 300);

        let captured = handle.join().unwrap().unwrap();
        assert_eq!(captured.kind, CallbackKind::Success);
        assert!(captured.body.contains("token_xyz"));
    }

    #[test]
    fn run_captures_failure_callback() {
        let server = ConnectServer::bind().unwrap();
        let failure_url = format!("{}failure", server.url());
        let handle = thread::spawn(move || server.run_until_callback("<html/>", 5));
        thread::sleep(Duration::from_millis(50));
        post_json(&failure_url, r#"{"type":"bank_auth_failed"}"#);
        let captured = handle.join().unwrap().unwrap();
        assert_eq!(captured.kind, CallbackKind::Failure);
    }

    #[test]
    fn run_captures_exit_callback() {
        let server = ConnectServer::bind().unwrap();
        let exit_url = format!("{}exit", server.url());
        let handle = thread::spawn(move || server.run_until_callback("<html/>", 5));
        thread::sleep(Duration::from_millis(50));
        post_json(&exit_url, "{}");
        let captured = handle.join().unwrap().unwrap();
        assert_eq!(captured.kind, CallbackKind::Exit);
    }

    #[test]
    fn run_times_out_when_no_callback_arrives() {
        let server = ConnectServer::bind().unwrap();
        let err = server.run_until_callback("<html/>", 1).unwrap_err();
        match err {
            DomainError::Import(msg) => assert!(msg.contains("timed out"), "got: {msg}"),
            other => panic!("expected timeout Import error, got {other:?}"),
        }
    }

    #[test]
    fn serves_html_on_get_root() {
        let server = ConnectServer::bind().unwrap();
        let root_url = server.url();
        let success_url = format!("{root_url}success");

        let html = "<html><body>rtf teller connect</body></html>";
        let handle = thread::spawn(move || server.run_until_callback(html, 5));

        thread::sleep(Duration::from_millis(50));
        // Hit GET / first — should return the HTML page and keep running.
        let body = ureq::get(&root_url).call().unwrap().into_string().unwrap();
        assert!(body.contains("rtf teller connect"));

        // Then POST /success to shut it down.
        post_json(&success_url, "{}");
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn returns_404_for_unknown_paths_and_keeps_running() {
        let server = ConnectServer::bind().unwrap();
        let junk_url = format!("{}random/path", server.url());
        let success_url = format!("{}success", server.url());

        let handle = thread::spawn(move || server.run_until_callback("<html/>", 5));
        thread::sleep(Duration::from_millis(50));

        // Unknown path 404s but server stays up.
        let resp = ureq::get(&junk_url).call().ok();
        // ureq returns Err on 4xx by default; tolerate either.
        if let Some(r) = resp {
            assert_eq!(r.status(), 404);
        }
        // Server still accepts the terminal callback.
        post_json(&success_url, "{}");
        handle.join().unwrap().unwrap();
    }

    #[test]
    fn render_template_substitutes_vars() {
        let rendered = render_template(
            "Hello {{NAME}}, token is {{TOKEN}}.",
            &[("NAME", "Sky"), ("TOKEN", "abc123")],
        );
        assert_eq!(rendered, "Hello Sky, token is abc123.");
    }

    #[test]
    fn render_template_leaves_unknown_vars_untouched() {
        let rendered = render_template("{{KNOWN}} and {{UNKNOWN}}", &[("KNOWN", "yes")]);
        assert_eq!(rendered, "yes and {{UNKNOWN}}");
    }
}
