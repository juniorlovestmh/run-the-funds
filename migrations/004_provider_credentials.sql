-- Per-provider credentials blob for bank sync (S04).
-- `data` holds a provider-specific JSON shape:
--   simplefin: {"access_url": "https://user:pass@host/path"}
--   pluggy:    {"client_id": "...", "client_secret": "...", "item_id": "..."}
-- UNIQUE on provider so there's exactly one cred set per provider; re-running
-- `rtf <provider> setup` upserts.
CREATE TABLE IF NOT EXISTS provider_credentials (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL UNIQUE,
    data TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
