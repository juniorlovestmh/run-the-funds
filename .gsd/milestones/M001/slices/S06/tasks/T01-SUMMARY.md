---
id: T01
parent: S06
milestone: M001
key_files:
  - migrations/009_tags.sql
  - src/domain/tag/mod.rs
  - src/domain/tag/tag.rs
  - src/domain/tag/repository.rs
  - src/infrastructure/storage/tag_repo.rs
  - src/cli/tags.rs
key_decisions:
  - Name UNIQUE constraint — Monarch tag names are already globally unique (one tag named 'BR' in the whole household), and upsert-by-name is the fallback if external_id matching fails.
  - Color + order_index preserved from Monarch verbatim — no translation, no normalization.
  - set_tags_for_transaction replaces the full set atomically (delete + re-insert) — simpler semantics than partial attach/detach, matches Monarch's set_transaction_tags.
  - CLI read-only (no create/delete) — Monarch is source of truth; fintrack mirrors on sync.
duration: 
verification_result: untested
completed_at: 2026-04-20T11:36:38.532Z
blocker_discovered: false
---

# T01: Tags domain + migration 009 + SqliteTagRepository + `fintrack tags list` CLI — 420 tests passing (+14).

**Tags domain + migration 009 + SqliteTagRepository + `fintrack tags list` CLI — 420 tests passing (+14).**

## What Happened

New domain module src/domain/tag/{mod,tag,repository}.rs with Tag struct (id, name, color, order_index, external_id, external_provider, timestamps) + TagRepository trait (save, find_by_id/name/external, find_all, set_tags_for_transaction, find_tags_for_transaction). Migration 009_tags.sql creates tags (name UNIQUE, external_id+provider for upstream provenance) + transaction_tags junction table with CASCADE on both sides. SqliteTagRepository with ON CONFLICT(id) DO UPDATE upsert + 10 tests covering save/find/upsert/CASCADE-on-transaction-delete/CASCADE-on-tag-delete/name-unique-constraint. CLI: `fintrack tags list [--format table|json]` — read-only, Monarch is the source of truth going forward. Schema-version test bumped 8→9.

## Verification

cargo build clean. cargo test 420 passing / 6 ignored / 0 failed (+14 over S05's 406). `./target/release/fintrack --db /tmp/t01-smoke.db tags list --format json` returns `{status: ok, data: []}` on fresh DB (migration 009 auto-applied).

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `migrations/009_tags.sql`
- `src/domain/tag/mod.rs`
- `src/domain/tag/tag.rs`
- `src/domain/tag/repository.rs`
- `src/infrastructure/storage/tag_repo.rs`
- `src/cli/tags.rs`
