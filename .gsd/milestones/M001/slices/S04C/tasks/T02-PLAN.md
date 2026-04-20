---
estimated_steps: 41
estimated_files: 3
skills_used: []
---

# T02: Connect-flow infrastructure: local HTTP callback server + browser launcher + HTML template engine

Reusable chunk of code that both `teller connect` and `pluggy connect` use.

**New module** (`src/infrastructure/connect/mod.rs`):
```rust
pub struct ConnectServer {
    port: u16,
    // internal state
}

pub struct CapturedCallback {
    pub body: String,            // raw JSON the widget POSTed
    pub kind: CallbackKind,
}

pub enum CallbackKind {
    Success,  // POST to /success
    Failure,  // POST to /failure (widget failed)
    Exit,     // POST to /exit (user closed the widget)
}

impl ConnectServer {
    /// Bind 127.0.0.1:<random-port>, serve `html_page` at `/`, and wait for the
    /// browser to POST a JSON body back to `/success` / `/failure` / `/exit`.
    /// Returns the captured callback (or a timeout error).
    pub fn run(html_page: &str, timeout_secs: u64) -> Result<CapturedCallback, DomainError>;

    /// URL to open in the browser (http://127.0.0.1:<port>/).
    pub fn url(&self) -> String;
}

/// Launch the default browser with `url`. Falls back to printing the URL for
/// the user to paste if the launch fails (headless environment, SSH, etc.).
pub fn launch_browser(url: &str) -> Result<(), DomainError>;
```

Implementation uses **`tiny_http`** for the server (tiny blocking HTTP, no dep drag) and **`webbrowser`** for the launcher. Random port via `tiny_http::Server::http("127.0.0.1:0")`. Server handles:
- `GET /` → respond with the provided HTML.
- `GET /favicon.ico` → 204.
- `POST /success`, `/failure`, `/exit` → capture JSON body, respond with a small HTML “You can close this tab” page, queue the callback to the caller, then break the accept loop.
- Any other path → 404.

**HTML template** for each provider will be embedded via `include_str!` in T03/T04 (Teller's page wires up Teller Connect; Pluggy's page wires up Pluggy Connect). For T02, just ship the reusable infrastructure.

**Deps**: `tiny_http = "0.12"`, `webbrowser = "1"`.

**TDD**:
- `run()` happy path: spin up the server on a random port, POST `/success` from a test HTTP client (use `ureq`), assert the returned `CapturedCallback.body` matches what was posted. Test takes <100ms.
- Timeout: `run()` with a 1-second timeout and no callback → returns `DomainError::Import("connect timeout")`.
- Failure callback: POST `/failure` → returned `CapturedCallback.kind == Failure`.
- Multiple non-callback GETs don't complete the flow.
- `launch_browser` is tested trivially — just verify the function signature compiles and returns Ok on a well-formed URL (no real browser launch in tests).

## Inputs

- `src/infrastructure/http.rs (pattern for mockable HTTP)`

## Expected Output

- `ConnectServer abstraction with random-port bind + /success/failure/exit POST capture + timeout`
- `launch_browser helper with graceful fallback to stderr print`
- `Deps: tiny_http + webbrowser`

## Verification

cargo test -- infrastructure::connect
