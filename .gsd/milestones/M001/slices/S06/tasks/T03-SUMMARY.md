---
id: T03
parent: S06
milestone: M001
key_files:
  - migrations/010_category_external_ids.sql
  - src/application/monarch_sync_service.rs
  - src/domain/category/category.rs
  - src/domain/category/category_group.rs
  - src/domain/category/repository.rs
  - src/domain/account/account_type.rs
  - src/infrastructure/storage/category_repo.rs
  - src/infrastructure/storage/migrations.rs
key_decisions:
  - External IDs added to categories + category_groups via new columns (not a side table) — simpler joins, matches the pattern already established on accounts and tags.
  - Upsert by external_id, not by name — Monarch IDs are stable across renames; name-matching would break if the user renames a category in Monarch.
  - New MonarchSyncService (separate from SyncService) rather than extending SyncService's generics — Monarch's surface (accounts, groups, cats, tags, txns) is too different from the BankSyncAdapter.sync() shape to force.
  - Account mapping falls back to Other for unrecognized types instead of erroring — better to import with a best-guess type than skip an account entirely.
  - semantic_changed detection compares payee / description / category_id / amount / date — only fields Monarch controls, so a local-only edit (e.g. future beneficiary_id tagging) wouldn't flip a txn into 'updated'.
  - DEFAULT_BACKFILL_DAYS = 730 matches the existing SyncService default — consistency across providers.
duration: 
verification_result: untested
completed_at: 2026-04-20T12:25:51.275Z
blocker_discovered: false
---

# T03: MonarchSyncService: 3-phase taxonomy → accounts → transactions import with external_id resolution. Idempotent re-run. 7 service tests + schema/domain extensions.

**MonarchSyncService: 3-phase taxonomy → accounts → transactions import with external_id resolution. Idempotent re-run. 7 service tests + schema/domain extensions.**

## What Happened

New MonarchSyncService<T, A, C, Tg> generic over 4 repos. Three-phase orchestration: (1) import category groups → categories → tags from Monarch; upsert by (external_provider='monarch', external_id); build Monarch-id→fintrack-id lookup for each. (2) Import accounts; upsert by (provider, external_id); map Monarch's (type.name, subtype.name) pair to fintrack's AccountType via helper that falls back to Other. (3) Import transactions within [since, today] (default: 2 years back); resolve Monarch category_id + tag_ids via the lookups built in phase 1; UPSERT by (local_account_id, external_id) so re-sync updates rather than duplicates. Returns MonarchSyncReport with per-phase counts + duplicates + updates + window. Schema changes required for ID resolution: migration 010 adds external_id + external_provider columns to categories + category_groups (with partial indexes). Domain types Category and CategoryGroup gained from_external() constructors + optional external_id/external_provider fields. AccountType enum extended with Brokerage + Other variants (Monarch has brokerage / cryptocurrency / roth / vehicle subtypes that don't map to Checking/Savings/CreditCard/Loan).

## Verification

cargo build clean. cargo test 441 passing / 6 ignored / 0 failed (+9 over T02's 432). 7 new MonarchSyncService tests cover: first-run taxonomy import, idempotent re-run (zero new imports on second call, all counted as duplicates), category + tag ID resolution on transactions, category-less transactions leaving category_id NULL, ghost-account transactions skipped without error, account upsert updating balance + name, and the monarch_to_account_type mapping table.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `migrations/010_category_external_ids.sql`
- `src/application/monarch_sync_service.rs`
- `src/domain/category/category.rs`
- `src/domain/category/category_group.rs`
- `src/domain/category/repository.rs`
- `src/domain/account/account_type.rs`
- `src/infrastructure/storage/category_repo.rs`
- `src/infrastructure/storage/migrations.rs`
