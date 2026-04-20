# S06: Monarch adapter + retire Teller/SimpleFIN/Pluggy

**Goal:** Make Monarch Money the single upstream source for fintrack's US financial data. Retire direct-to-bank adapters (Teller, SimpleFIN, Pluggy) and Connect scaffolding. Import Monarch's category groups + categories + tags into fintrack's domain (new tags + transaction_tags tables). Build MonarchAdapter that shells out to the installed mmoney CLI for JSON, parses into fintrack's domain, resolves Monarch IDs to fintrack-local IDs via external_id upsert, and persists via the existing persist_batch pipeline. Nuke-and-rebuild migration (user confirmed zero attachment to current fintrack DB).
**Demo:** fintrack sync pulls every account/transaction/category/tag from Monarch Money via mmoney subprocess. fintrack transactions list matches mmoney count. fintrack spending matches Monarch's cashflow within 1%. Teller/SimpleFIN/Pluggy/Connect code removed.

## Must-Haves

- fintrack sync (Monarch-only, no --provider flag) pulls every Monarch account/transaction/category/tag into fintrack. Re-running is idempotent — no duplicates, updates reflect Monarch's latest. fintrack transactions list count matches mmoney transactions list within ±5. fintrack spending for a known month matches Monarch's cashflow within 1%. cargo build references no Teller/SimpleFIN/Pluggy symbols. cargo test all green.

## Proof Level

- This slice proves: live demo against the user's real Monarch account (2,376+ transactions, 19 accounts) with side-by-side count and spending verification vs mmoney-cli

## Integration Closure

fintrack's data plane switches to Monarch-only subprocess pull. Pre-S06 backup of fintrack.db taken before migration wipe. Monarch's server-side rules continue auto-categorizing new transactions; fintrack mirrors the resulting state.

## Verification

- fintrack sync emits MonarchSyncReport {accounts_imported, transactions_imported, categories_imported, tags_imported, duplicates_skipped, window_start, window_end}. Preserves {status, data} CLI envelope. On mmoney auth failure, surfaces 'run mmoney auth login first' guidance.

## Tasks

- [x] **T01: Retire Teller + SimpleFIN + Pluggy adapters and Connect scaffolding** `est:medium`
  Delete all code for the three direct-to-bank adapters (teller.rs, simplefin.rs, pluggy.rs, pluggy_connect.rs, connect/*, cli/{teller,simplefin,pluggy,connections}.rs, pluggy_connect.html). Remove TellerCommands/SimplefinCommands/PluggyCommands/ConnectionsCommands enums from cli/mod.rs + corresponding main.rs dispatch arms. Drop unused deps from Cargo.toml (rustls/ring, base64 if only SimpleFIN used it, tiny_http, webbrowser, quick-xml). Write migration 009_monarch_reset.sql that wipes provider-sourced state: DROP TABLE provider_credentials; DELETE FROM accounts WHERE external_provider IN ('teller','simplefin','pluggy'); DELETE FROM transactions; DELETE FROM categories; DELETE FROM category_groups; DELETE FROM rules; DELETE FROM transaction_splits. Preserve exchange_rates (PTAX cache) + persons (beneficiary dimension, future). Keep existing sync_service scaffolding that a new Monarch adapter will plug into.
  - Files: `src/infrastructure/sync_adapter/teller.rs`, `src/infrastructure/sync_adapter/simplefin.rs`, `src/infrastructure/sync_adapter/pluggy.rs`, `src/infrastructure/sync_adapter/pluggy_connect.rs`, `src/infrastructure/connect/`, `src/cli/teller.rs`, `src/cli/simplefin.rs`, `src/cli/pluggy.rs`, `src/cli/pluggy_connect.html`, `src/cli/connections.rs`, `src/cli/mod.rs`, `src/main.rs`, `Cargo.toml`, `migrations/009_monarch_reset.sql`
  - Verify: cargo build clean (no dead symbols). cargo test passes (removed tests for retired code). sqlite3 on a throwaway DB: after running migration, accounts/transactions/categories/category_groups/rules/transaction_splits/provider_credentials empty; exchange_rates + persons preserved.

- [x] **T02: Tags domain + schema + repository** `est:medium`
  Introduce tags as a first-class fintrack concept mirroring Monarch. Migration 010 creates tags (id, name UNIQUE, color, order_index, external_id, external_provider, timestamps) + transaction_tags junction (transaction_id FK CASCADE, tag_id FK CASCADE, PK both). Domain: src/domain/tag/{mod,tag,repository}.rs with TagRepository trait (find_all, find_by_name, find_by_external_id, save, set_tags_for_transaction, find_tags_for_transaction). SqliteTagRepository impl with 5+ tests including CASCADE verification. CLI: fintrack tags list [--format]. No create/delete CLI — Monarch is source of truth, fintrack mirrors.
  - Files: `migrations/010_tags.sql`, `src/domain/tag/mod.rs`, `src/domain/tag/tag.rs`, `src/domain/tag/repository.rs`, `src/domain/mod.rs`, `src/infrastructure/storage/tag_repo.rs`, `src/infrastructure/storage/mod.rs`, `src/infrastructure/storage/migrations.rs`, `src/cli/tags.rs`, `src/cli/mod.rs`, `src/main.rs`
  - Verify: cargo test: new tag_repo tests pass including CASCADE delete. fintrack tags list --format json on empty DB returns empty array envelope.

- [x] **T03: MonarchAdapter — subprocess-based mmoney ingest** `est:medium`
  src/infrastructure/sync_adapter/monarch.rs: shells out to mmoney CLI (path configurable via MMONEY_BIN env, default /Users/sky/.local/bin/mmoney) via std::process::Command. Uses serde_json to parse stdout. Exposes fetch_accounts(), fetch_category_groups(), fetch_categories(), fetch_tags(), fetch_transactions(start_date, end_date). RemoteTransaction carries monarch_txn_id (used as external_id), account external_id, merchant, payee, amount, currency, date, monarch_category_id (external, resolved later), monarch_tag_ids (external). Paginates transactions with --limit 500 --offset N until < 500 returned. Clean error on missing mmoney ('run mmoney auth login first'). Trait abstraction (MmoneyRunner) so tests can inject canned JSON fixtures without subprocess.
  - Files: `src/infrastructure/sync_adapter/monarch.rs`, `src/infrastructure/sync_adapter/mod.rs`
  - Verify: Unit tests with FakeMmoneyRunner returning canned JSON from survey fixtures. One #[ignore]d live smoke that fetches from real Monarch.

- [x] **T04: SyncService Monarch wiring: taxonomy then accounts then transactions with ID resolution** `est:large`
  Extend SyncService with run_monarch(). Phase 1 — import taxonomy: fetch category groups → upsert by external_id = Monarch's group id (build Monarch_group_id→fintrack_group_id lookup). Same for categories, same for tags. Phase 2 — import accounts: upsert by (external_provider='monarch', external_account_id=monarch_id), creating new fintrack accounts with reasonable defaults for type/currency from Monarch's type+subtype+currency. Phase 3 — import transactions: for each RemoteTransaction, resolve monarch_account_id + monarch_category_id + monarch_tag_ids via lookups, build domain Transaction with external_id=monarch_txn_id, external_provider='monarch', upsert via persist_batch. After persist, apply tags via TagRepository.set_tags_for_transaction. Emit MonarchSyncReport.
  - Files: `src/application/sync_service.rs`, `src/application/mod.rs`, `src/domain/category/repository.rs`
  - Verify: Unit tests: empty Monarch → empty import; re-running produces 0 duplicates; new category in Monarch creates new fintrack category; tag-id reference resolution; transaction category resolved via lookup. Integration-ish test with FakeMmoneyRunner backed by survey/*.json fixtures.

- [x] **T05: CLI rewire + live demo + UAT + rollup commit** `est:medium`
  Rewire fintrack sync to call Monarch directly (no --provider flag). Remove accounts link --provider references to retired providers; keep command but default to monarch. Write S06-UAT.md covering: migration 009+010 on real DB, fresh fintrack sync against live Monarch, fintrack transactions list count ≈ mmoney count, fintrack spending matches Monarch cashflow for March ±1%, idempotency (re-running sync = 0 new rows). Single commit covering T01-T05.
  - Files: `src/cli/sync.rs`, `src/cli/mod.rs`, `src/main.rs`, `.gsd/milestones/M001/slices/S06/S06-UAT.md`
  - Verify: cargo test green. Live demo against real Monarch. Commit on main.

## Files Likely Touched

- src/infrastructure/sync_adapter/teller.rs
- src/infrastructure/sync_adapter/simplefin.rs
- src/infrastructure/sync_adapter/pluggy.rs
- src/infrastructure/sync_adapter/pluggy_connect.rs
- src/infrastructure/connect/
- src/cli/teller.rs
- src/cli/simplefin.rs
- src/cli/pluggy.rs
- src/cli/pluggy_connect.html
- src/cli/connections.rs
- src/cli/mod.rs
- src/main.rs
- Cargo.toml
- migrations/009_monarch_reset.sql
- migrations/010_tags.sql
- src/domain/tag/mod.rs
- src/domain/tag/tag.rs
- src/domain/tag/repository.rs
- src/domain/mod.rs
- src/infrastructure/storage/tag_repo.rs
- src/infrastructure/storage/mod.rs
- src/infrastructure/storage/migrations.rs
- src/cli/tags.rs
- src/infrastructure/sync_adapter/monarch.rs
- src/infrastructure/sync_adapter/mod.rs
- src/application/sync_service.rs
- src/application/mod.rs
- src/domain/category/repository.rs
- src/cli/sync.rs
- .gsd/milestones/M001/slices/S06/S06-UAT.md
