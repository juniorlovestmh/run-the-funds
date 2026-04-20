---
estimated_steps: 1
estimated_files: 11
skills_used: []
---

# T02: Tags domain + schema + repository

Introduce tags as a first-class rtf concept mirroring Monarch. Migration 010 creates tags (id, name UNIQUE, color, order_index, external_id, external_provider, timestamps) + transaction_tags junction (transaction_id FK CASCADE, tag_id FK CASCADE, PK both). Domain: src/domain/tag/{mod,tag,repository}.rs with TagRepository trait (find_all, find_by_name, find_by_external_id, save, set_tags_for_transaction, find_tags_for_transaction). SqliteTagRepository impl with 5+ tests including CASCADE verification. CLI: rtf tags list [--format]. No create/delete CLI — Monarch is source of truth, rtf mirrors.

## Inputs

- `Monarch tag shape from monarch-cleanup/survey/tags.json`

## Expected Output

- `migrations/010_tags.sql`
- `src/domain/tag/* and tag_repo.rs with 5+ tests`

## Verification

cargo test: new tag_repo tests pass including CASCADE delete. rtf tags list --format json on empty DB returns empty array envelope.
