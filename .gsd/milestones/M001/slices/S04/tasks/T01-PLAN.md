---
estimated_steps: 38
estimated_files: 16
skills_used: []
---

# T01: Account linkage + provider credentials storage (migrations 003 + 004, repos, `accounts link` CLI)

Foundation for both sync providers: schema + domain + repos for account↔remote mapping AND for stored credentials.

**Migration 003** (`migrations/003_account_external_link.sql`):
```sql
ALTER TABLE accounts ADD COLUMN external_provider TEXT;
ALTER TABLE accounts ADD COLUMN external_account_id TEXT;
ALTER TABLE accounts ADD COLUMN last_sync_at TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_external_link
    ON accounts(external_provider, external_account_id)
    WHERE external_provider IS NOT NULL;
```
Partial unique index — unlinked accounts (NULL provider) coexist freely; linked accounts cannot duplicate a (provider, external_id) pair.

**Migration 004** (`migrations/004_provider_credentials.sql`):
```sql
CREATE TABLE IF NOT EXISTS provider_credentials (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL UNIQUE,
    data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```
`data` is a JSON blob whose shape depends on the provider (SimpleFIN: `{"access_url":"https://user:pass@host/path"}`; Pluggy: `{"client_id":...,"client_secret":...,"item_id":...}`). UNIQUE on `provider` so there's only one cred set per provider.

Register both migrations in `src/infrastructure/storage/migrations.rs`.

**Domain** (`src/domain/account/account.rs`): extend `Account` with `external_provider: Option<String>`, `external_account_id: Option<String>`, `last_sync_at: Option<DateTime<Utc>>`. Helpers: `link(provider, external_id)`, `mark_synced(at)`.

**Domain** (`src/domain/credentials/`, new module): new `ProviderCredentials` domain struct `{ id, provider, data: String (JSON), created_at, updated_at }`. Trait `ProviderCredentialsRepository` with `save`, `find_by_provider(provider) -> Option<ProviderCredentials>`, `delete(provider)`. Keep the JSON opaque at this layer — each adapter parses its own shape in T02/T03.

**Repos** (`src/infrastructure/storage/`):
- `account_repo.rs`: update row mapping + save SQL for the three new columns. Add `find_by_external_link(provider, external_id)` and `find_by_provider(provider)` methods to the trait + impl.
- New `credentials_repo.rs`: `SqliteProviderCredentialsRepository` implementing the trait. `save` uses INSERT OR REPLACE on the (provider) UNIQUE to upsert.

**CLI** (`src/cli/accounts.rs` + wiring): new `accounts link` subcommand:
- `rtf accounts link --id <local-uuid> --provider <simplefin|pluggy> --external-id <remote-id>`
- Validates provider string, looks up local account, calls `link`, saves. Rejects a second link on an already-linked account unless `--force`.
- `{status:"ok", data: <account>}` envelope.

**Security posture** — add a one-paragraph note to `.gsd/milestones/M001/slices/S04/SECURITY.md`: credentials are plaintext in the SQLite DB; treat `rtf.db` like `.aws/credentials` (user-only file perms, not checked in, encrypted-volume recommended). Keychain integration is an out-of-scope future enhancement.

**TDD**:
- Domain: `link` sets both fields; `mark_synced` updates timestamp; serde round-trips.
- Account repo: migration 003 applied; `find_by_external_link` roundtrip; `find_by_provider` returns only matching rows; partial unique index rejects dup (provider, external_id); multiple NULL-provider accounts coexist.
- Credentials repo: migration 004 applied; save/find_by_provider roundtrip; upsert on duplicate provider; delete removes the row.
- CLI `accounts link`: success path, unknown provider rejected, double-link rejected without `--force`, `--force` overrides.

## Inputs

- `migrations/001_initial.sql`
- `migrations/002_transaction_external_id_unique.sql`
- `src/infrastructure/storage/account_repo.rs`
- `src/domain/account/account.rs`

## Expected Output

- `Migrations 003+004 applied and idempotent`
- `Account domain + repo gain linkage columns and lookup methods`
- `ProviderCredentials domain + SQLite repo with upsert semantics`
- ``rtf accounts link` CLI with --force guard`
- `SECURITY.md documenting the plaintext-credentials posture`

## Verification

cargo test -- domain::account && cargo test -- domain::credentials && cargo test -- infrastructure::storage::account_repo && cargo test -- infrastructure::storage::credentials_repo && cargo test -- cli::accounts
