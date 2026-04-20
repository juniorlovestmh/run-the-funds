---
id: S02
parent: M001
milestone: M001
provides:
  - ["OfxImporter: parses OFX 1.x SGML + 2.x XML into Vec<Transaction>", "TransactionService::import_from: account lookup + currency guard + FITID dedup + ImportReport", "ImportReport { imported, duplicates }: Serialize-able report type", "DomainError::Import(String): adapter-level error carrier", "TransactionRepository::find_by_external_id: per-account FITID lookup", "migrations/002: partial unique index on (account_id, external_id)", "CLI: rtf transactions import --format ofx|qfx, with deferred CSV error", "tests/fixtures/chase-sample.qfx (523 txns) and nubank-sample.ofx (13 txns) for future regression coverage"]
requires:
  - slice: S01
    provides: TransactionRepository, AccountRepository, SqliteTransactionRepository, Account domain model, migrations framework, CLI envelope
affects:
  - ["src/infrastructure/importer/ofx.rs (stub \u2192 working)", "src/application/transaction_service.rs (generic on both repos, import_from added)", "src/cli/transactions.rs (import handler rewired)", "src/domain/error.rs (new Import variant)", "src/domain/transaction/repository.rs (new find_by_external_id method)", "src/infrastructure/storage/transaction_repo.rs (find_by_external_id impl, save flipped to INSERT)", "src/infrastructure/storage/migrations.rs (migration 002 registered)"]
key_files:
  - ["src/infrastructure/importer/ofx.rs", "src/application/transaction_service.rs", "src/cli/transactions.rs", "src/domain/error.rs", "src/domain/transaction/repository.rs", "src/infrastructure/storage/transaction_repo.rs", "src/infrastructure/storage/migrations.rs", "migrations/002_transaction_external_id_unique.sql", "tests/import_demo.rs", "tests/fixtures/chase-sample.qfx", "tests/fixtures/nubank-sample.ofx", ".gsd/milestones/M001/slices/S02/S02-UAT.md"]
key_decisions:
  - ["Unified one OFX adapter for both OFX 1.x SGML and OFX 2.x XML rather than two adapters.", "Line-tolerant character-level SGML tokenizer \u2014 handles Chase's unclosed leaves, Nubank's XML-closed leaves, and any future bank variant.", "Partial unique index on (account_id, external_id) WHERE external_id IS NOT NULL \u2014 keeps manually-entered txns permissive.", "Dedup scoped per-account \u2014 two banks can legitimately issue the same FITID.", "Flipped save from INSERT OR REPLACE to plain INSERT; REPLACE was neutering the unique index.", "Two-pass import_from: validate first, persist after. Atomic-on-mismatch without explicit DB transactions.", "Strict currency matching \u2014 no placeholder-overwrite; safer and alerts operators on ambiguous files.", "CLI treats `qfx` as an alias for `ofx` \u2014 matches Chase's file extension; no dialect knowledge required.", "CSV explicitly deferred with a 'not yet' error \u2014 hint instead of generic 'unsupported'.", "Mid-slice pivot from 5-task CSV-based plan to 4-task OFX-unified plan after seeing real Chase export options."]
patterns_established:
  - ["Character-level SGML tokenizer for legacy OFX 1.x \u2014 reusable template if we ever support QIF/QBO.", "Two-pass validate-then-persist flow in the service layer \u2014 template for future batch operations (S04 categorization may reuse).", "Integration test via env!(\"CARGO_BIN_EXE_rtf\") + serde_json parsing of stdout/stderr \u2014 template for E2E tests in later slices.", "Per-slice S##-UAT.md with a Roadmap Done-criteria coverage table \u2014 keeps slice boundaries traceable."]
observability_surfaces:
  - ["`{status:\"ok\", data:{imported, duplicates, file, format, account_id}}` envelope on successful import.", "`ImportError::Parse { format, detail }` carries FITID + element context for failing records.", "`DomainError::CurrencyMismatch { expected, got }` and `DomainError::NotFound { entity, id }` surface verbatim in the CLI error message.", "Explicit `\"csv import is deferred to a later slice\"` message for unsupported format."]
drill_down_paths:
  []
duration: ""
verification_result: passed
completed_at: 2026-04-19T19:22:36.681Z
blocker_discovered: false
---

# S02: Transaction Import Pipeline

**Unified OFX adapter (OFX 1.x SGML + 2.x XML) parses real Chase QFX (523 txns) and Nubank OFX (13 txns), persists with per-account FITID dedup, enforces currency match, and exposes a clean CLI envelope.**

## What Happened

S02 turned the scaffolding stubs into a working import pipeline across four tasks (plus a fifth legacy task closed as superseded during a mid-slice replan).

**T01 — Unified OFX adapter.** Replaced `OfxImporter::import`'s `NotImplemented` with a dialect-aware parser. Detects OFX 2.x (XML prolog) vs OFX 1.x (SGML preamble) in the first ~512 bytes. The SGML path is a small character-level tokenizer that handles both pure OFX 1.x (unclosed leaves, Chase QFX) and hybrid 1.x-header-with-XML-closed-leaves (Nubank) — closing tags are just no-ops in the STMTTRN state machine. The XML path uses `quick_xml::reader::Reader` with the same `ParseOutput` shape. Tag names uppercased for defensive matching. `<CURDEF>` captured for currency detection. 20 unit tests plus two real-fixture tests.

**T02 — external_id dedup.** Added `migrations/002_transaction_external_id_unique.sql` with a *partial* unique index on `(account_id, external_id) WHERE external_id IS NOT NULL` — partial so manually-entered transactions without FITIDs coexist. `find_by_external_id(account_id, external_id)` added to the repo trait + SQLite impl. **Flipped `save` from `INSERT OR REPLACE` to plain `INSERT`** — REPLACE was silently deleting conflicting rows and neutering the index. +6 tests.

**T03 — service + CLI wiring.** `TransactionService` now generic over both `TransactionRepository` and `AccountRepository`. New `import_from<I: Importer>` method with a **two-pass design**: pass 1 validates currencies (mismatch → `DomainError::CurrencyMismatch`, zero writes); pass 2 persists with dedup (FITID hit → `duplicates++`, miss → save + `imported++`). Atomic-on-mismatch without explicit DB transactions. `ImportReport { imported, duplicates }` is Serialize-able. New `DomainError::Import(String)` variant. CLI routes `ofx` + `qfx` to OfxImporter, rejects `csv` explicitly with a "deferred" message. +7 tests using an in-memory FakeImporter.

**T04 — end-to-end demo + UAT.** `tests/import_demo.rs` shells out to the compiled binary via `env!("CARGO_BIN_EXE_rtf")` against a tempdir DB. Drives the full flow (accounts create → Chase import 523/0 → Chase re-import via qfx alias 0/523 → Nubank import 13/0 → re-import 0/13 → list JSON spot-checks → currency mismatch → CSV deferral). `.gsd/milestones/M001/slices/S02/S02-UAT.md` documents every scenario with expected JSON shapes and the Roadmap Done-criteria coverage table.

**T05 — superseded.** Original 5-task plan had a CSV E2E demo at T05; after the mid-slice pivot to OFX-unified (see next paragraph), the coverage moved into the new T04. T05 closed with no work.

**Plan revision mid-slice.** After seeing the user's actual Chase export options (CSV + QFX + QIF + QBO) and reviewing the raw QFX content, pivoted from "CSV for Chase" to "QFX for Chase" — dropped the dedicated CSV adapter, unified on one OFX adapter for both countries, 5 tasks → 4. Reasons: real FITIDs vs synthesized hashes (stronger dedup), single parser covers US + Brazilian banks, most major US banks offer QFX. CSV becomes a future fallback adapter for banks that don't offer QFX (e.g. Aidvantage).

## Verification

`cargo test` → **171 unit tests + 1 integration test = 172 passed, 0 failed, 0 ignored**. `cargo build` → clean. End-to-end test (`end_to_end_import_demo`) drives the compiled binary through every scenario in S02-UAT.md — 523 Chase transactions and 13 Nubank transactions round-trip correctly through import → list → re-import with deterministic dedup counts. Currency mismatch (exit 1) and CSV rejection (exit 2) paths both surface structured error envelopes.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

"5-task plan reduced to 4 tasks mid-slice after seeing real Chase export options. Dropped the separate CsvImporter task; unified on a single OfxImporter handling both dialects. Replaced the 'Chase CSV' Done criterion with 'Chase QFX' \u2014 better dedup, less code. T05 closed as superseded. Decision + reasoning documented in S02-PLAN and the Roadmap coverage table in S02-UAT."

## Known Limitations

["CSV adapter not implemented \u2014 deferred to a later slice for banks without QFX support (Aidvantage et al.).", "`rtf transactions list` without `--account-id` still returns an empty list \u2014 carried over from S01 scaffolding; needs a new all-accounts repo method.", "OFX files with no `<CURDEF>` default to USD and surface a CurrencyMismatch for non-USD accounts. Safer than silent coercion but may need a `--assume-currency` flag later."]

## Follow-ups

["Add all-accounts listing to `rtf transactions list` (new repo method or service orchestration).", "Add CSV adapter as a fallback for banks that don't offer QFX.", "Consider surfacing the OFX adapter's captured `<CURDEF>` in debug logs for easier mismatch triage.", "S03 will consume the persisted transactions for multi-currency rollup \u2014 imported_at and external_id are load-bearing for that flow."]

## Files Created/Modified

None.
