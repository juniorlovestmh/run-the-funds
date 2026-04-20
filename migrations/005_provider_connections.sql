-- Per-bank connections for bank-sync providers (S04C).
-- One row per linked bank. For Teller: the row's external_id is the Teller
-- enrollment id, and `data` holds {"access_token":"..."}. For Pluggy: the
-- row's external_id is the Pluggy itemId, and `data` is typically `{}`.
-- `provider_credentials` keeps per-app data (cert/key/app_id for Teller,
-- client_id/secret for Pluggy); this table holds the per-bank data that
-- can legitimately have multiple rows per provider.
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
