---
estimated_steps: 1
estimated_files: 3
skills_used: []
---

# T04: SyncService Monarch wiring: taxonomy then accounts then transactions with ID resolution

Extend SyncService with run_monarch(). Phase 1 — import taxonomy: fetch category groups → upsert by external_id = Monarch's group id (build Monarch_group_id→rtf_group_id lookup). Same for categories, same for tags. Phase 2 — import accounts: upsert by (external_provider='monarch', external_account_id=monarch_id), creating new rtf accounts with reasonable defaults for type/currency from Monarch's type+subtype+currency. Phase 3 — import transactions: for each RemoteTransaction, resolve monarch_account_id + monarch_category_id + monarch_tag_ids via lookups, build domain Transaction with external_id=monarch_txn_id, external_provider='monarch', upsert via persist_batch. After persist, apply tags via TagRepository.set_tags_for_transaction. Emit MonarchSyncReport.

## Inputs

- `T02 TagRepository`
- `T03 MonarchAdapter`
- `existing SyncService + persist_batch from S02/S04`

## Expected Output

- `sync_service.rs with run_monarch(...) + 10+ tests`

## Verification

Unit tests: empty Monarch → empty import; re-running produces 0 duplicates; new category in Monarch creates new rtf category; tag-id reference resolution; transaction category resolved via lookup. Integration-ish test with FakeMmoneyRunner backed by survey/*.json fixtures.
