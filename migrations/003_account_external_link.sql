-- Link local accounts to remote bank-sync provider accounts (S04).
-- Partial unique index: unlinked accounts (NULL provider) coexist freely;
-- linked accounts cannot duplicate a (provider, external_id) pair.
ALTER TABLE accounts ADD COLUMN external_provider TEXT;
ALTER TABLE accounts ADD COLUMN external_account_id TEXT;
ALTER TABLE accounts ADD COLUMN last_sync_at TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_external_link
    ON accounts(external_provider, external_account_id)
    WHERE external_provider IS NOT NULL;
