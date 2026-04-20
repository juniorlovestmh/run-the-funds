-- Tags (S06 T01). Orthogonal cross-cut dimension for transactions —
-- beneficiary (For Kids / For Wife / Joint / Family BR), location (BR),
-- recurring-type (Subscription), reimbursability (Work reimbursable / Tax
-- deductible / Refund pending), etc. Separate from categories, which answer
-- "what kind of spending"; tags answer anything that cross-cuts categories.
--
-- `external_id` + `external_provider` are populated when the tag is
-- imported from an upstream source (currently Monarch Money). Name is
-- UNIQUE so the Monarch import can upsert by name and keep a stable lookup.
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT,
    order_index INTEGER,
    external_id TEXT,
    external_provider TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tags_external
    ON tags(external_provider, external_id)
    WHERE external_id IS NOT NULL;

-- Many-to-many: a transaction can carry N tags.
CREATE TABLE IF NOT EXISTS transaction_tags (
    transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (transaction_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_transaction_tags_tag ON transaction_tags(tag_id);
