---
id: T02
parent: S06
milestone: M001
key_files:
  - src/infrastructure/sync_adapter/monarch.rs
  - src/infrastructure/sync_adapter/mod.rs
  - examples/monarch_smoke.rs
key_decisions:
  - Own trait surface (not BankSyncAdapter) — Monarch returns accounts + categories + tags in addition to transactions; squeezing into BankSyncAdapter.sync() would lose the richer shape T03 needs.
  - Subprocess to mmoney vs reimplementing the GraphQL client in Rust — reuses the already-installed + already-authenticated CLI, keeps Monarch GraphQL fragility contained to the Python side, zero duplication of their query library.
  - MmoneyRunner trait for testability — canned JSON fixtures > actual subprocess for 12 unit tests that run in 20ms.
  - Amount parsing via serde_json::Value + Decimal::from_str(number.to_string()) preserves precision where f64 would round — Monarch's amounts come through as JSON numbers.
  - Binary path resolution: env > ~/.local/bin > PATH. Keeps the common case (uv tool install) zero-config and gives an override for CI/alternative installs.
  - Currency hardcoded to USD at the adapter boundary — all observed live accounts are USD; T03 can extend when a BR bank link surfaces non-USD data.
duration: 
verification_result: untested
completed_at: 2026-04-20T11:41:34.816Z
blocker_discovered: false
---

# T02: MonarchAdapter with subprocess-based mmoney ingest: fetch accounts / category groups / categories / tags / paginated transactions. 12 unit tests + live smoke against real Monarch (19 accounts, 61 April txns).

**MonarchAdapter with subprocess-based mmoney ingest: fetch accounts / category groups / categories / tags / paginated transactions. 12 unit tests + live smoke against real Monarch (19 accounts, 61 April txns).**

## What Happened

New adapter in src/infrastructure/sync_adapter/monarch.rs. MmoneyRunner trait abstracts the subprocess call so tests inject canned JSON without spawning binaries. SubprocessMmoneyRunner is the real impl, resolving the mmoney binary in order: MMONEY_BIN env var → ~/.local/bin/mmoney (default uv-tool install location) → PATH lookup. Auth-failure detection surfaces 'mmoney is not authenticated — run mmoney auth login first' with useful install guidance on binary-missing. Five public methods: fetch_accounts() → Vec<MonarchAccount>, fetch_category_groups() → Vec<MonarchCategoryGroup>, fetch_categories() → Vec<MonarchCategory>, fetch_tags() → Vec<MonarchTag>, fetch_transactions(start, end) → Vec<MonarchTransaction> with internal 500-row pagination and a 1M-row safety cap. MonarchTransaction preserves Monarch's category_external_id and tag_external_ids as Monarch-side IDs — T03 will resolve these against fintrack's own category/tag tables via external_id upsert lookups. Amount parsing via decimal_from_json handles JSON numbers (`-8.06`), strings (`'3.14'`), and nulls (→ zero) since mmoney's JSON envelope varies across endpoints. Live smoke (examples/monarch_smoke.rs) against the real Monarch account returned 19 accounts, 15 category groups, 66 categories, 13 tags, 61 April-to-date transactions with correct merchant names / Decimal amounts / category external IDs.

## Verification

cargo build clean. cargo test: 432 passing / 6 ignored / 0 failed (+12 over T01's 420 — 12 new adapter tests). Live smoke `cargo run --example monarch_smoke` succeeds end-to-end against the user's real Monarch account via mmoney subprocess.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| — | No verification commands discovered | — | — | — |

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `src/infrastructure/sync_adapter/monarch.rs`
- `src/infrastructure/sync_adapter/mod.rs`
- `examples/monarch_smoke.rs`
