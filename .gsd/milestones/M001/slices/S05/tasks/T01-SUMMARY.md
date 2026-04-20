---
id: T01
parent: S05
milestone: M001
key_files:
  - migrations/007_rules.sql
  - src/infrastructure/storage/migrations.rs
  - src/infrastructure/storage/rules_repo.rs
  - src/infrastructure/storage/mod.rs
  - src/infrastructure/storage/database.rs
  - src/domain/rules/mod.rs
  - src/domain/rules/rule.rs
  - src/domain/rules/repository.rs
  - src/domain/mod.rs
  - src/cli/rules.rs
  - src/cli/mod.rs
  - src/main.rs
  - Cargo.toml
key_decisions:
  - Regex validated at rule-save time (not categorize-time). Bad patterns fail fast with a clear error; the hot path of categorization stays cheap.
  - Case-insensitive by default for both substring and regex. Regex patterns get a silent `(?i)` prefix; user-visible pattern stays clean.
  - Amount pattern DSL (>N, <=N, etc.) lives in the domain layer, parsed on Rule::new. Rejects bogus input (e.g. `not-a-number`) before it hits the DB.
duration: 
verification_result: passed
completed_at: 2026-04-20T00:28:03.720Z
blocker_discovered: false
---

# T01: Rules schema + domain + SQLite repo + `fintrack rules add/list/remove` CLI; regex support baked in (validated at save-time via the `regex` crate); 29 new tests; working-tree commit deferred until S05 completes as a rollup.

**Rules schema + domain + SQLite repo + `fintrack rules add/list/remove` CLI; regex support baked in (validated at save-time via the `regex` crate); 29 new tests; working-tree commit deferred until S05 completes as a rollup.**

## What Happened

Foundation layer for S05 categorization. Migration 007 + Rule domain with MatchField (payee|description|amount) and MatchKind (substring|regex) enums + amount DSL (>N, <N, =N, >=N, <=N). Regex patterns compile at save time via `regex::Regex::new(format!("(?i){pattern}"))` — invalid patterns rejected in Rule::new, not during the next `categorize` run. Amount patterns also pre-validated. Rules ordered by priority DESC, created_at ASC as stable tiebreaker.

SQLite repo with UPSERT-by-id semantics mirrors existing pattern. `fintrack rules add/list/remove` CLI. 29 new tests: 16 domain (field/kind FromStr, validation, substring + regex + amount matching across all operators, serde), 7 repo (CRUD + ordering + upsert + FK enforcement), 7 CLI (happy paths + regex validation + unknown fields + FK + amount DSL + ordering).

**Committed status:** working tree carries T01's code uncommitted per the end-of-slice cadence rule. Nothing regresses — existing CLIs all work. `fintrack rules list` returns empty on the user's real DB until categories exist (S05 T05 adds the categories CLI).

Per user request: pausing S05 here to live-verify Pluggy from S04C first. T02-T06 (~13h remaining) resume next session.

## Verification

`cargo test` → 347 passing, 5 ignored, 0 failed (+29 since S04D).

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 1250ms |

## Deviations

None. Plan called for substring-only initially; regex support added upfront at user request. Costs a `regex = \"1\"` dep + ~15 lines of handling code; zero compromise on the match correctness model."

## Known Issues

Working tree is dirty with T01 code until S05 completes as a rollup commit. Acceptable per commit-cadence rule."

## Files Created/Modified

- `migrations/007_rules.sql`
- `src/infrastructure/storage/migrations.rs`
- `src/infrastructure/storage/rules_repo.rs`
- `src/infrastructure/storage/mod.rs`
- `src/infrastructure/storage/database.rs`
- `src/domain/rules/mod.rs`
- `src/domain/rules/rule.rs`
- `src/domain/rules/repository.rs`
- `src/domain/mod.rs`
- `src/cli/rules.rs`
- `src/cli/mod.rs`
- `src/main.rs`
- `Cargo.toml`
