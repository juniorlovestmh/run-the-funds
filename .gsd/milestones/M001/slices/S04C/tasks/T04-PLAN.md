---
estimated_steps: 48
estimated_files: 7
skills_used: []
---

# T04: Pluggy: refactor to multi-item + `rtf pluggy connect` + HTML template

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

## Inputs

- `src/infrastructure/connect/ (T02)`
- `src/domain/connections/ (T01)`
- `src/infrastructure/sync_adapter/pluggy.rs (existing single-item)`
- `src/cli/teller.rs (T03 as template)`

## Expected Output

- `rtf pluggy setup now per-app-creds only`
- `rtf pluggy connect browser-based multi-item CLI`
- `Pluggy Connect HTML template`
- `Sync iterates all items`

## Verification

cargo test -- cli::pluggy && cargo test -- infrastructure::sync_adapter::pluggy && cargo test --test sync_demo
