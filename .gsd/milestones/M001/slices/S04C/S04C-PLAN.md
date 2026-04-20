# S04C: Browser Connect flow + multi-bank support (Teller + Pluggy)

**Goal:** Eliminate the manual-token-paste step from bank setup and unlock multi-bank per provider. `rtf teller connect` + `rtf pluggy connect` each open a browser to the provider's hosted Connect widget, capture the enrollment/item via a local HTTP callback server, and persist it. Each call creates ONE new enrollment; user runs the command once per bank. `rtf sync --provider <p>` iterates all stored enrollments/items and pulls transactions for each. New `provider_connections` table supports N enrollments per provider without schema conflict.
**Demo:** rtf teller connect opens a browser to Teller Connect, user links a bank, rtf captures the enrollment and stores it. Repeat for each bank — N enrollments per provider. rtf pluggy connect does the same for Brazilian banks. rtf sync --provider teller iterates all Teller enrollments; same for Pluggy. New provider_connections table supports multi-bank per provider without schema conflict.

## Must-Haves

- `rtf teller connect` (after one-time `teller setup --cert --key --app-id`) opens the default browser to a local URL serving Teller Connect, waits for the user to link a bank, captures the enrollment object from the widget's onSuccess callback via a POST to the local server, persists it, shuts down the server, and prints a success envelope naming the linked institution.
- Running `teller connect` a second time adds a second enrollment without overwriting the first.
- `rtf pluggy connect` does the same via Pluggy's Connect widget — creates a connect_token server-side, opens the browser with it, captures the itemId on success.
- `rtf sync --provider teller` iterates every stored Teller enrollment, fetches each one's accounts + transactions, aggregates the report: `{imported, duplicates, accounts_synced, enrollments_synced}`.
- `rtf sync --provider pluggy` does the same across all items.
- `rtf connections list --format json` shows every stored enrollment/item across providers for operator inspection. Each row: `{id, provider, external_id, institution_name, created_at}`. Access tokens are NOT exposed in this output.
- Existing `teller setup` / `pluggy setup` commands continue to work but are repurposed: they now store per-app data only (cert+key+app_id for Teller; client_id+client_secret for Pluggy). Per-bank access_token / item_id is no longer accepted on those commands — the Connect flow is the only path.
- Migration 005 creates `provider_connections`; migration 006 cleans per-bank data out of existing `provider_credentials` rows. Existing user's Teller enrollment is preserved by auto-migrating the stored access_token into a new `provider_connections` row using the access_token itself as a synthetic external_id (pending a real enrollment.id via a fresh `teller connect`).
- `cargo test` passes. Unit tests for the Connect server (mock browser POSTing a canned callback). Integration tests in `tests/sync_demo.rs` for new CLI commands + sync iteration. Two `#[ignore]`d live smokes: one for `teller connect` (spawns real browser), one for `pluggy connect`.
- Live verified: user adds at least one additional bank via `teller connect` and confirms `rtf sync --provider teller` pulls transactions across all enrolled banks.

## Proof Level

- This slice proves: contract — multi-bank per provider works end-to-end with a browser-driven UX that eliminates manual token-pasting. Both S04 providers (Teller, Pluggy) land on the same pattern. Downstream slices (S05 categorization onward) consume a DB where transactions from multiple linked banks per provider coexist correctly.

## Integration Closure

- Upstream surfaces consumed: everything from S04 + S04B — BankSyncAdapter trait, HttpClient (incl. with_mtls), SyncService, provider_credentials, TellerAdapter + PluggyAdapter (refactored to take per-enrollment credentials).
- New wiring introduced: `provider_connections` table + `ProviderConnection` domain type + `ProviderConnectionRepository`; reusable `ConnectServer` in `src/infrastructure/connect/` (local HTTP server + browser launcher); `rtf teller connect` + `rtf pluggy connect` + `rtf connections list`; refactored sync-per-provider that iterates enrollments; migrations 005 + 006.
- What remains: richer credential security (keychain, encryption) — follow-up. Pruning/unlinking old enrollments (`connections remove --id X`) — small follow-up. S05 (categorization) can now proceed.

## Verification

- `rtf teller connect` prints the local URL it opened (so the user can paste it manually if browser launch fails), and on callback prints the captured institution name + enrollment/item id.
- Sync envelope grows an `enrollments_synced` field per provider to disambiguate from `accounts_synced` (multiple accounts per enrollment are possible).
- `connections list` gives operators a safe view of stored links without exposing secrets.
- Errors during the Connect flow (port bind failure, browser launch failure, callback timeout, malformed enrollment payload) each surface as distinct error messages so the user knows which phase failed.
- Access tokens, client secrets, cert keys: never logged, never in the `connections list` output, only in the DB.

## Tasks

- [x] **T01: provider_connections schema + domain + repo + migration from legacy single-enrollment storage** `est:2h`
  Foundation for multi-bank.

**Migration 005** (`migrations/005_provider_connections.sql`):
```sql
CREATE TABLE IF NOT EXISTS provider_connections (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    external_id TEXT NOT NULL,
    data TEXT NOT NULL DEFAULT '{}',
    institution_name TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_connections_provider_ext
    ON provider_connections(provider, external_id);
CREATE INDEX IF NOT EXISTS idx_provider_connections_provider
    ON provider_connections(provider);
```

**Migration 006** (`migrations/006_migrate_connections.sql`): data migration from the existing single-enrollment storage.
- For any `provider_credentials` row where `provider='teller'` and `data.access_token` is present: INSERT a `provider_connections` row with `external_id = access_token` (synthetic, will be replaced on next fresh connect), `data = {"access_token": <value>}`, `institution_name = 'Legacy'`. Then UPDATE the provider_credentials row to remove access_token from its data blob, keeping cert_pem + key_pem + (adds `app_id` as null placeholder).
- Same pattern for `provider='pluggy'`: migrate `data.item_id` to a provider_connections row; strip item_id from provider_credentials.
- SQLite doesn't have native JSON manipulation in pre-3.38 versions, but recent rusqlite bundles 3.45+ which supports `json_extract`/`json_remove`. Use those.

**Domain** (`src/domain/connections/`):
- `ProviderConnection { id, provider, external_id, data: String (JSON), institution_name: Option<String>, created_at, updated_at }`.
- `ProviderConnectionRepository` trait: `save`, `find_by_provider(provider) -> Vec<ProviderConnection>`, `find_by_external_id(provider, external_id) -> Option`, `delete(id)`.

**SQLite impl** (`src/infrastructure/storage/connections_repo.rs`):
- Straightforward. UPSERT on `(provider, external_id)` so re-running connect for the same bank replaces rather than duplicates.

**CLI — `rtf connections list [--format json|table]`** (`src/cli/connections.rs`):
- Reads all rows across providers. Redacts `data` (doesn't include it in the output — only metadata).
- JSON shape: `[{id, provider, external_id, institution_name, created_at}, ...]`. Table shape: columns ID | Provider | Institution | External ID | Created.

**TDD**:
- Migrations apply cleanly on a fresh DB; version advances to 6.
- Migration from legacy data: seed a `provider_credentials` row with an access_token, run migrations, assert a matching `provider_connections` row exists and the provider_credentials row no longer has access_token.
- Repo: save+find_by_provider, find_by_external_id returns Some/None, delete removes, UPSERT behavior on same (provider, external_id).
- CLI: connections list returns expected JSON shape across 2 Teller + 1 Pluggy row; access tokens NOT in output.
  - Files: `migrations/005_provider_connections.sql`, `migrations/006_migrate_connections.sql`, `src/infrastructure/storage/migrations.rs`, `src/infrastructure/storage/connections_repo.rs`, `src/infrastructure/storage/mod.rs`, `src/infrastructure/storage/database.rs`, `src/domain/connections/mod.rs`, `src/domain/connections/connection.rs`, `src/domain/connections/repository.rs`, `src/domain/mod.rs`, `src/cli/connections.rs`, `src/cli/mod.rs`, `src/main.rs`
  - Verify: cargo test -- domain::connections && cargo test -- infrastructure::storage::connections_repo && cargo test -- cli::connections

- [x] **T02: Connect-flow infrastructure: local HTTP callback server + browser launcher + HTML template engine** `est:2.5h`
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
  - Files: `src/infrastructure/connect/mod.rs`, `src/infrastructure/mod.rs`, `Cargo.toml`
  - Verify: cargo test -- infrastructure::connect

- [x] **T03: Teller: refactor to multi-enrollment + `rtf teller connect` + HTML template** `est:3h`
  Wire the Connect flow for Teller and switch the adapter/sync path to iterate enrollments.

**`teller setup` refactor** (`src/cli/teller.rs`):
- Now takes `--app-id <APP_ID> --cert <path> --key <path>` (drops `--access-token` — obsolete). App ID is stored in `provider_credentials.data` alongside cert/key.
- Output message: “Teller app credentials saved; run `rtf teller connect` to link each bank.”

**New `rtf teller connect`** (`src/cli/teller.rs`):
1. Load cert+key+app_id from `provider_credentials[provider='teller']`. Error if missing.
2. Render the Teller Connect HTML template with the app_id injected.
3. `ConnectServer::run(html, timeout=300s)` — returns the captured enrollment JSON.
4. Parse the enrollment: extract `id`, `accessToken`, `institution.name`.
5. UPSERT a `provider_connections` row: `(provider='teller', external_id=enrollment.id, data={"access_token":"..."}, institution_name=institution.name)`.
6. Print `{status:"ok", data:{enrollment_id, institution_name, message:"Linked; run `rtf sync --provider teller`."}}`.

**Teller Connect HTML template** (`src/cli/teller_connect.html` via `include_str!`):
```html
<!DOCTYPE html>
<html>
  <head><title>rtf · Teller Connect</title></head>
  <body>
    <p>Loading Teller Connect…</p>
    <script src="https://cdn.teller.io/connect/connect.js"></script>
    <script>
      const tellerConnect = TellerConnect.setup({
        applicationId: "{{APP_ID}}",
        environment: "development",  // or production; sandbox would skip mTLS path
        onSuccess: function(enrollment) {
          fetch("/success", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(enrollment) })
            .then(() => { document.body.innerHTML = "<p>Linked. You can close this tab.</p>"; });
        },
        onFailure: function(failure) {
          fetch("/failure", { method: "POST", body: JSON.stringify(failure) });
          document.body.innerHTML = "<p>Connect failed. Close this tab and check the rtf CLI.</p>";
        },
        onExit: function() {
          fetch("/exit", { method: "POST", body: "{}" });
          document.body.innerHTML = "<p>Cancelled. You can close this tab.</p>";
        },
      });
      tellerConnect.open();
    </script>
  </body>
</html>
```
{{APP_ID}} substituted at runtime via simple string replacement.

**Sync refactor**:
- `run_teller` (and `run_teller_inline`) now loads ALL `provider_connections` for `provider='teller'`. For each: build a TellerAdapter with its access_token + the shared cert+key HTTP client, call `SyncService::sync_provider`, aggregate reports.
- Aggregated `ProviderSyncReport` sums `imported` + `duplicates` + `accounts_synced` across enrollments, adds `enrollments_synced: usize`.

**TDD**:
- `teller setup` without access_token succeeds with cert/key/app_id.
- `teller connect` end-to-end: mock the ConnectServer response with a canned enrollment JSON, assert a provider_connections row is inserted.
- Missing-credentials error path (connect before setup).
- Sync aggregation: 2 enrollments linked, each returns N txns → aggregated report has sum.
- Fresh connect replaces the legacy-migrated enrollment (external_id moves from access_token to real enrollment.id).
- `#[ignore]`d live smoke: `teller_connect_live` — runs the full flow against real Teller (spawns browser). Documented, requires user presence.
  - Files: `src/cli/teller.rs`, `src/cli/teller_connect.html`, `src/cli/mod.rs`, `src/cli/sync.rs`, `src/main.rs`, `src/infrastructure/sync_adapter/teller.rs`, `tests/sync_demo.rs`
  - Verify: cargo test -- cli::teller && cargo test -- infrastructure::sync_adapter::teller && cargo test --test sync_demo

- [x] **T04: Pluggy: refactor to multi-item + `rtf pluggy connect` + HTML template** `est:3h`
  Mirror T03 for Pluggy. Same connect-server infrastructure; different widget shape.

**`pluggy setup` refactor** (`src/cli/pluggy.rs`):
- Now takes `--client-id <ID> --client-secret <SECRET>` only (drops `--item-id`).
- Output message: “Pluggy app credentials saved; run `rtf pluggy connect` to link each bank.”

**`rtf pluggy connect`**:
1. Load client_id+client_secret from `provider_credentials[provider='pluggy']`.
2. Server-side: POST https://api.pluggy.ai/auth to get apiKey, then POST /connect_token to get a short-lived widget token.
3. Render the Pluggy Connect HTML template with the connect_token injected.
4. `ConnectServer::run(html, timeout=300s)`.
5. Parse the captured item: extract `item.id`, `item.connector.name` (institution).
6. UPSERT `provider_connections` row: `(provider='pluggy', external_id=item.id, data={}, institution_name=connector.name)`.

**Pluggy Connect HTML template** (`src/cli/pluggy_connect.html`):
```html
<!DOCTYPE html>
<html>
  <head><title>rtf · Pluggy Connect</title></head>
  <body>
    <p>Loading Pluggy Connect…</p>
    <script src="https://cdn.pluggy.ai/pluggy-connect/v2.9.0/pluggy-connect.js"></script>
    <script>
      const pluggyConnect = PluggyConnect.init({
        connectToken: "{{CONNECT_TOKEN}}",
        includeSandbox: true,
        onSuccess: function(itemData) {
          fetch("/success", { method:"POST", headers:{"Content-Type":"application/json"}, body: JSON.stringify(itemData) })
            .then(() => { document.body.innerHTML = "<p>Linked. You can close this tab.</p>"; });
        },
        onError: function(err) {
          fetch("/failure", { method:"POST", body: JSON.stringify(err) });
          document.body.innerHTML = "<p>Connect failed. Close this tab.</p>";
        },
        onClose: function() {
          fetch("/exit", { method:"POST", body: "{}" });
          document.body.innerHTML = "<p>Cancelled. You can close this tab.</p>";
        },
      });
      pluggyConnect.open();
    </script>
  </body>
</html>
```
The CDN version number + Connect API shape need to be verified against Pluggy's current docs at build time.

**Sync refactor**: same pattern as Teller — iterate pluggy items, aggregate reports.

**TDD**:
- pluggy setup without item-id succeeds.
- pluggy connect happy path: mock connect_token issuance (canned POST /auth + /connect_token responses), then mock the ConnectServer callback with a canned item JSON. Assert provider_connections row inserted.
- Sync aggregation across 2 items.
- `#[ignore]`d `pluggy_connect_live` live smoke.
  - Files: `src/cli/pluggy.rs`, `src/cli/pluggy_connect.html`, `src/cli/mod.rs`, `src/cli/sync.rs`, `src/main.rs`, `src/infrastructure/sync_adapter/pluggy.rs`, `tests/sync_demo.rs`
  - Verify: cargo test -- cli::pluggy && cargo test -- infrastructure::sync_adapter::pluggy && cargo test --test sync_demo

- [x] **T05: Live-test both providers with multiple banks + S04C-UAT.md + commit** `est:2h`
  Manual verification + docs + single rollup commit.

**Live flow** (user-driven):
1. `rtf teller setup --app-id <USER_APP_ID> --cert ~/Downloads/teller/certificate.pem --key ~/Downloads/teller/private_key.pem`
2. `rtf teller connect` → browser opens, user links Chase (or another bank), success callback fires, enrollment persisted.
3. `rtf teller connect` → user links a DIFFERENT bank → second enrollment row.
4. `rtf connections list` → both enrollments listed with institution names.
5. `rtf sync --provider teller` → aggregated report across both enrollments; per-account transactions persisted for each linked account.
6. Same flow for Pluggy after setup: `rtf pluggy connect` → user links their Brazilian bank → `rtf sync --provider pluggy` works.

**S04C-UAT.md** mirrors the S04B shape. Scenarios: setup (per-app creds), connect (browser flow), multi-bank (N enrollments), connections list, sync aggregation, missing-creds errors, timeout handling. Roadmap coverage table. Notes the user flow from zero to synced-multi-bank.

**Commit**: one rollup covering T01–T05. Message notes the mid-slice clarification that Pluggy ships alongside Teller and the new `provider_connections` table is the foundation for multi-bank across both.
  - Files: `.gsd/milestones/M001/slices/S04C/S04C-UAT.md`
  - Verify: cargo test && live-test the connect flow for both providers

## Files Likely Touched

- migrations/005_provider_connections.sql
- migrations/006_migrate_connections.sql
- src/infrastructure/storage/migrations.rs
- src/infrastructure/storage/connections_repo.rs
- src/infrastructure/storage/mod.rs
- src/infrastructure/storage/database.rs
- src/domain/connections/mod.rs
- src/domain/connections/connection.rs
- src/domain/connections/repository.rs
- src/domain/mod.rs
- src/cli/connections.rs
- src/cli/mod.rs
- src/main.rs
- src/infrastructure/connect/mod.rs
- src/infrastructure/mod.rs
- Cargo.toml
- src/cli/teller.rs
- src/cli/teller_connect.html
- src/cli/sync.rs
- src/infrastructure/sync_adapter/teller.rs
- tests/sync_demo.rs
- src/cli/pluggy.rs
- src/cli/pluggy_connect.html
- src/infrastructure/sync_adapter/pluggy.rs
- .gsd/milestones/M001/slices/S04C/S04C-UAT.md
