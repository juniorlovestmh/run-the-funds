---
estimated_steps: 28
estimated_files: 12
skills_used: []
---

# T01: Rules table + domain + SQLite repo + `rtf rules` CLI (add/list/remove)

**Migration 007** (`migrations/007_rules.sql`):
```sql
CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    match_field TEXT NOT NULL,         -- 'payee' | 'description' | 'amount'
    match_pattern TEXT NOT NULL,       -- case-insensitive substring for payee/description; '>N'/'<N'/'=N' for amount
    category_id TEXT NOT NULL REFERENCES categories(id),
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rules_priority ON rules(priority DESC);
```

**Domain** (`src/domain/rules/`):
- `Rule { id, name, match_field: RuleField, match_pattern: String, category_id, priority, created_at, updated_at }`.
- `RuleField` enum: `Payee`, `Description`, `Amount` (stored as lowercase string).
- `RuleRepository` trait: `save`, `find_by_id`, `find_all` (ordered by priority DESC), `delete`.

**SQLite impl** (`src/infrastructure/storage/rules_repo.rs`): straightforward mirror of the existing repo pattern.

**CLI** (`src/cli/rules.rs`):
- `rtf rules add --name <n> --match-field <payee|description|amount> --pattern <p> --category-id <cat> [--priority <N>]`
- `rtf rules list [--format json|table]`
- `rtf rules remove --id <uuid>`
- All envelopes follow existing patterns.

**Tests:**
- Domain: Rule validation (empty name rejected, unknown field rejected, empty pattern rejected).
- Repo: save+find_by_id, find_all ordering, delete, upsert-by-id.
- CLI: rules list empty, add happy path, list shows added, remove works, not-found errors.

## Inputs

- `src/domain/credentials/ (repo pattern)`
- `src/domain/connections/ (repo pattern)`
- `src/domain/category/ (foreign key)`

## Expected Output

- `Migration 007 applied cleanly`
- `Rule domain + RuleRepository + SQLite impl`
- `rtf rules add/list/remove CLI`
- `~8 unit tests`

## Verification

cargo test -- domain::rules && cargo test -- infrastructure::storage::rules_repo && cargo test -- cli::rules
