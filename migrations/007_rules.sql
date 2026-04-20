-- Categorization rules (S05). One rule per (pattern, field, kind) combo.
-- Higher `priority` wins ties when multiple rules match the same transaction.
-- `match_kind` distinguishes substring matching (default, case-insensitive)
-- from regex matching (RE2-style via the `regex` crate). For amount-field
-- rules, match_kind is stored but unused — amount patterns are a tiny DSL
-- of `>N`, `<N`, `=N`, `>=N`, `<=N`.
CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    match_field TEXT NOT NULL,
    match_kind TEXT NOT NULL DEFAULT 'substring',
    match_pattern TEXT NOT NULL,
    category_id TEXT NOT NULL REFERENCES categories(id),
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rules_priority ON rules(priority DESC);
