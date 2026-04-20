---
estimated_steps: 52
estimated_files: 7
skills_used: []
---

# T03: Teller: refactor to multi-enrollment + `rtf teller connect` + HTML template

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

## Inputs

- `src/infrastructure/connect/ (T02)`
- `src/domain/connections/ (T01)`
- `src/infrastructure/sync_adapter/teller.rs (existing single-enrollment)`

## Expected Output

- `rtf teller setup now only handles per-app creds`
- `rtf teller connect browser-based multi-enrollment CLI`
- `Teller Connect HTML template`
- `Sync iterates all enrollments, aggregates report`

## Verification

cargo test -- cli::teller && cargo test -- infrastructure::sync_adapter::teller && cargo test --test sync_demo
