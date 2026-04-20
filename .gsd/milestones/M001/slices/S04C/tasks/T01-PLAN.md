---
estimated_steps: 34
estimated_files: 13
skills_used: []
---

# T01: provider_connections schema + domain + repo + migration from legacy single-enrollment storage

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

## Inputs

- `migrations/001-004 (existing)`
- `src/domain/credentials/ (pattern to mirror)`

## Expected Output

- `Migration 005 + 006 applied cleanly`
- `ProviderConnection domain + repo`
- `rtf connections list [--format json|table]`
- `Existing user's Teller enrollment preserved by migration`

## Verification

cargo test -- domain::connections && cargo test -- infrastructure::storage::connections_repo && cargo test -- cli::connections
