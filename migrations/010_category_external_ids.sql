-- S06 T03: add external_id + external_provider to categories + category_groups
-- so an upstream source (currently Monarch Money) can be the single owner of
-- taxonomy and re-sync idempotently by matching on (external_provider, external_id).
--
-- Locally-created rows (created before this migration, or created by
-- `rtf category-groups create` / `rtf categories create`) leave these columns
-- NULL and are untouched by sync.
ALTER TABLE category_groups ADD COLUMN external_id TEXT;
ALTER TABLE category_groups ADD COLUMN external_provider TEXT;
ALTER TABLE categories ADD COLUMN external_id TEXT;
ALTER TABLE categories ADD COLUMN external_provider TEXT;

CREATE INDEX IF NOT EXISTS idx_category_groups_external
    ON category_groups(external_provider, external_id)
    WHERE external_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_categories_external
    ON categories(external_provider, external_id)
    WHERE external_id IS NOT NULL;
