-- Data migration from single-enrollment storage to provider_connections (S04C).
-- For any existing Teller/Pluggy credentials that hold per-bank data inside
-- provider_credentials.data, split that data into a provider_connections row.
-- Per-app data (cert/key for Teller; client_id/secret for Pluggy) stays in
-- provider_credentials.

-- Teller: access_token → provider_connections row.
-- external_id = the access_token itself (synthetic; will be replaced on fresh
-- `rtf teller connect`). Keeps existing sync working post-upgrade.
INSERT OR IGNORE INTO provider_connections (id, provider, external_id, data, institution_name, created_at, updated_at)
SELECT
    lower(hex(randomblob(16))),
    'teller',
    json_extract(data, '$.access_token'),
    json_object('access_token', json_extract(data, '$.access_token')),
    'Legacy (re-run `rtf teller connect`)',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM provider_credentials
WHERE provider = 'teller' AND json_extract(data, '$.access_token') IS NOT NULL;

-- Strip access_token from provider_credentials.data (keep cert_pem + key_pem).
UPDATE provider_credentials
SET data = json_remove(data, '$.access_token'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
WHERE provider = 'teller' AND json_extract(data, '$.access_token') IS NOT NULL;

-- Pluggy: item_id → provider_connections row.
INSERT OR IGNORE INTO provider_connections (id, provider, external_id, data, institution_name, created_at, updated_at)
SELECT
    lower(hex(randomblob(16))),
    'pluggy',
    json_extract(data, '$.item_id'),
    '{}',
    'Legacy (re-run `rtf pluggy connect`)',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM provider_credentials
WHERE provider = 'pluggy' AND json_extract(data, '$.item_id') IS NOT NULL;

-- Strip item_id from provider_credentials.data (keep client_id + client_secret).
UPDATE provider_credentials
SET data = json_remove(data, '$.item_id'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
WHERE provider = 'pluggy' AND json_extract(data, '$.item_id') IS NOT NULL;
