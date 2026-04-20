# S02: Transaction Import Pipeline

**Goal:** Implement a unified OFX importer that parses both OFX 1.x SGML (Chase QFX and similar US banks) and OFX 2.x XML (Brazilian banks like Nubank) through a single `OfxImporter` adapter. `rtf transactions import` persists normalized transactions with correct amounts/dates/payees/FITIDs, enforces currency match against the target account, and dedupes re-imports via `external_id`. CSV import is deferred to a later slice — the CLI's `--format csv` will return a clear "not yet supported" error.
**Demo:** Import a real Nubank OFX file and a real Chase CSV file. rtf transactions list --format json shows normalized transactions from both banks with correct amounts, dates, and payee names.

## Must-Haves

- `rtf transactions import --format ofx --file tests/fixtures/chase-sample.qfx --account-id acc-chase` returns `{status:"ok", data:{imported:N, duplicates:0}}` where N matches the count of `<STMTTRN>` records in the file.
- Same command against `tests/fixtures/nubank-sample.ofx` (OFX 2.x XML) with a BRL account returns an equivalent envelope.
- Re-importing either fixture yields `{imported:0, duplicates:N}` — no new rows, no error.
- `rtf transactions list --account-id <id> --format json` shows rows with correct `date`, `amount`, `amount.currency`, `payee`, `description`, `external_id` (the OFX `<FITID>`).
- Importing a USD file into a BRL account returns `{status:"error", message:"currency mismatch: expected BRL, got USD"}` and persists nothing.
- `rtf transactions import --format csv ...` returns a clear `ImportError::UnsupportedFormat("csv — deferred to a later slice")` envelope (CLI exits non-zero).
- `cargo test` passes, including unit tests for both OFX dialects and an end-to-end integration test that drives the real binary.

## Proof Level

- This slice proves: contract — a single OFX adapter parses real-shape fixtures in both SGML (1.x) and XML (2.x) dialects end-to-end, the service layer enforces currency invariants and deduplicates via FITID-backed `external_id`, and the CLI surfaces consistent JSON envelopes. Downstream slices (S03 currency conversion, S04 categorization) can assume transactions are normalized, persisted, and tagged to an account with stable external ids.

## Integration Closure

- Upstream surfaces consumed: `TransactionRepository` + `SqliteTransactionRepository` (S01), `Account` + `AccountRepository` (S01 — service now looks up the account to validate currency), `Importer` trait + `ImportError` (S02 scaffolding).
- New wiring introduced: concrete `OfxImporter::import` handling both OFX dialects; `ImportReport { imported, duplicates }` returned by `TransactionService::import_from`; `find_by_external_id` on the transaction repo; unique index on `(account_id, external_id)`; CLI `import` handler orchestrates adapter → service.
- What remains: CSV adapter (deferred to a later slice as a fallback for banks without QFX — e.g. Aidvantage); S03 (multi-currency rollup + SimpleFIN/Pluggy bank sync); S04 (categorization + transfer detection consuming FITIDs and transfer pairs); all-accounts listing (not blocking — S02 scoped to `--account-id` filter only).

## Verification

- Success envelope carries `{imported, duplicates, file, format, account_id}` so an operator or agent can tell re-runs from first-runs at a glance.
- `ImportError::Parse { format: "ofx", detail }` surfaces element path + FITID (if known) when a record fails to parse, so malformed exports are diagnosable without inspecting the raw file.
- `ImportError::UnsupportedFormat` explicitly names the deferred CSV case so operators aren't left guessing.
- Currency mismatch and account-not-found errors emit `DomainError` variants verbatim in the `message` field — no silent fallbacks.

## Tasks

- [x] **T01: Unified OFX adapter: SGML (1.x) + XML (2.x) with fixture-driven TDD** `est:2.5h`
  Replace the `NotImplemented` body of `OfxImporter::import` with a parser that handles both OFX dialects behind a single entry point.

Dialect detection (first ~200 bytes of the file):
- If file starts with `<?xml` or the first non-blank line is an XML prolog → OFX 2.x (well-formed XML) → use `quick_xml::reader::Reader`.
- Else if preamble contains `OFXHEADER:` or `DATA:OFXSGML` → OFX 1.x SGML → use a small line-oriented tokenizer (see below).
- Otherwise → `ImportError::Parse { format: "ofx", detail: "cannot detect OFX dialect" }`.

For OFX 1.x SGML, write a tokenizer that:
- Skips header key:value lines until the first `<OFX>` tag.
- Walks the body token by token. A token is one of: `<TAG>` (container open), `</TAG>` (close), or `<TAG>value` (leaf with inline value, where `value` is everything on the line after `>`). Leaf tags do NOT have closing tags in 1.x.
- Tracks a stack of container tags. When the `<STMTTRN>` container opens, start accumulating leaf fields into a `HashMap<&str, String>`. When `</STMTTRN>` closes, emit one `Transaction` from the accumulated fields and clear the map.
- Also reads `<CURDEF>` from the `<STMTRS>` header — captured as the file's declared currency, returned alongside the transactions (see currency contract below).

For OFX 2.x XML, use `quick_xml::reader::Reader` with `trim_text(true)`. Walk Start/End/Text events with the same state machine (stack + STMTTRN accumulator). Import as `use quick_xml::reader::Reader; use quick_xml::events::Event;` — absolute paths only, since the module is named `ofx`.

Field mapping (identical across dialects):
- `<FITID>` → `transaction.external_id` (required — missing → `ImportError::Parse`).
- `<DTPOSTED>` → `transaction.date`. OFX encodes timestamps like `20260416120000[0:GMT]`; parse the first 8 chars as `%Y%m%d` via `NaiveDate::parse_from_str`.
- `<TRNAMT>` → `transaction.amount.amount` via `rust_decimal::Decimal::from_str`. Sign preserved (debits stay negative, credits positive).
- `<NAME>` → `transaction.payee` (optional).
- `<MEMO>` → `transaction.description` (optional).
- `<TRNTYPE>` → currently unused; log-skip. (S04 may consume it for categorization hints.)
- `transaction.id`: `uuid::Uuid::new_v4().to_string()`.
- `transaction.amount.currency`: parse from `<CURDEF>` if present; otherwise leave as `CurrencyCode::USD` placeholder. The service layer (T03) will validate/overwrite with the account's currency.
- `transaction.status`: `Pending`.
- `transaction.imported_at`: `Utc::now()`.

TDD — write failing tests first:
- `sgml_parses_single_stmttrn`: hand-written OFX 1.x string with one STMTTRN → correct Transaction fields.
- `sgml_parses_multiple_stmttrn`: order preserved.
- `xml_parses_single_stmttrn`: hand-written OFX 2.x XML string with one STMTTRN → same result as SGML equivalent.
- `detects_dialect_from_preamble`: feed 1.x header vs. XML prolog, confirm dialect router picks the right branch.
- `missing_fitid_errors`: STMTTRN without FITID → `ImportError::Parse`.
- `malformed_date_errors`: `<DTPOSTED>20261399...` → `ImportError::Parse`.
- `negative_amount_preserved`.
- `parses_curdef`: adapter captures `<CURDEF>USD` from the statement header and tags all transactions with USD.
- `parses_chase_qfx_fixture`: real Chase QFX at `tests/fixtures/chase-sample.qfx` parses without error; asserts `len() == <expected>` and spot-checks the first row's FITID + amount.
- `parses_nubank_ofx_fixture`: gated with `#[ignore]` (or conditionally compiled) until the Nubank fixture is provided by the user; when present, same shape as the Chase fixture test.

Fixture checked in at `tests/fixtures/chase-sample.qfx` — user explicitly OK'd not sanitizing PII. Nubank fixture path reserved at `tests/fixtures/nubank-sample.ofx` — fill in when the user drops the file.
  - Files: `src/infrastructure/importer/ofx.rs`, `src/infrastructure/importer/mod.rs`, `tests/fixtures/chase-sample.qfx`
  - Verify: cargo test -- infrastructure::importer::ofx

- [x] **T02: External_id dedup: repo lookup, unique index, migration 002** `est:1h`
  Today `SqliteTransactionRepository::save` uses `INSERT OR REPLACE`, which overwrites rows that share an id PK but doesn't prevent two rows with the same `external_id` under different UUIDs from coexisting. For real FITID-backed dedup we need a uniqueness constraint and a lookup helper.

Changes:
1. Add `migrations/002_transaction_external_id_unique.sql`:
   ```sql
   CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_account_external_id
       ON transactions(account_id, external_id)
       WHERE external_id IS NOT NULL;
   ```
   Update `infrastructure/storage/migrations.rs` to load and apply 002 after 001; bump `schema_version` row.
2. Extend `TransactionRepository` trait with `fn find_by_external_id(&self, account_id: &str, external_id: &str) -> Result<Option<Transaction>, DomainError>`.
3. Implement it in `SqliteTransactionRepository` via `SELECT * FROM transactions WHERE account_id = ?1 AND external_id = ?2 LIMIT 1`.
4. Leave `save` semantics unchanged — the service layer (T03) will call `find_by_external_id` first and skip on hit.

TDD:
- `find_by_external_id_returns_none_when_absent`.
- `find_by_external_id_roundtrips`.
- `unique_index_enforced`: insert two rows under the same `(account_id, external_id)` but different PKs → second insert fails with a UNIQUE constraint violation surfaced as `DomainError::Storage`.
- `null_external_id_not_constrained`: two rows under the same account with `external_id = None` both insert successfully (partial index keeps NULLs permissive).
- Regression: existing S01 tests still pass (notably `roundtrip_optional_fields`).
  - Files: `migrations/002_transaction_external_id_unique.sql`, `src/infrastructure/storage/migrations.rs`, `src/domain/transaction/repository.rs`, `src/infrastructure/storage/transaction_repo.rs`
  - Verify: cargo test -- infrastructure::storage::transaction_repo

- [x] **T03: Wire import pipeline: account lookup, currency guard, ImportReport, CLI** `est:1.5h`
  Glue the adapter and service together so the CLI gets a real import flow and reports dedup counts.

Changes to `TransactionService`:
- Parameterize on both `TransactionRepository` and `AccountRepository` so it can look up the destination account.
- New method:
  ```rust
  pub fn import_from<I: Importer>(
      &self,
      importer: &I,
      source: &Path,
      account_id: &str,
  ) -> Result<ImportReport, DomainError>
  ```
  Flow:
  1. Look up the account — `DomainError::NotFound` if missing.
  2. Call `importer.import(source, account_id)`. Wrap non-domain `ImportError` variants into a new `DomainError::Import(String)` carrier.
  3. For each returned `Transaction`:
     - If the adapter filled in a currency and it doesn't match the account currency → `DomainError::CurrencyMismatch` (abort the whole import; zero rows persisted).
     - Otherwise overwrite `amount.currency` with the account's currency (adapter currency may be a placeholder).
  4. Second pass (only when step 3 validated all rows): for each txn, `repo.find_by_external_id(account_id, external_id)` — if `Some`, increment `duplicates`; else `repo.save(&txn)` and increment `imported`.
  5. Return `ImportReport { imported, duplicates }`.
- Add `pub struct ImportReport { pub imported: usize, pub duplicates: usize }` with `Serialize`.
- Add `DomainError::Import(String)` variant + a test for its `Display` impl.

CLI (`src/cli/transactions.rs::handle_import`):
- Build both repos, instantiate `TransactionService`, call `svc.import_from(...)`.
- On success, emit `{status:"ok", data:{imported, duplicates, file, format, account_id}}`.
- Add an explicit branch for `format == "csv"` → emit an `UnsupportedFormat` error envelope with the deferred-to-later-slice message, exit 2.
- Keep other exit codes: 0 success, 1 domain error, 2 bad input (unsupported format).

TDD (use a lightweight in-test `Importer` impl to avoid file I/O coupling):
- `import_from_unknown_account_errors` → `DomainError::NotFound`.
- `import_from_currency_mismatch` → `DomainError::CurrencyMismatch`, zero rows persisted.
- `import_from_dedup_counts`: three txns, two with FITIDs already in DB → report `{imported:1, duplicates:2}`.
- `import_from_overwrites_placeholder_currency`: adapter returns txn with USD placeholder + account USD → row persisted with account currency.
- `import_from_atomic_on_mismatch`: mismatch on the 2nd of 3 rows → NO rows persisted (the two-pass design ensures this).
  - Files: `src/application/transaction_service.rs`, `src/domain/error.rs`, `src/cli/transactions.rs`
  - Verify: cargo test -- application::transaction_service && cargo test -- cli

- [x] **T04: End-to-end demo: import Chase + Nubank fixtures via the built binary; write S02-UAT.md** `est:1.5h`
  Lock the slice with an integration test that shells out to the compiled `rtf` binary and runs the full import → list → re-import path. Mirrors the S01-UAT structure for consistency.

Create `tests/import_demo.rs` (cargo integration test — separate crate, so it uses `env!("CARGO_BIN_EXE_rtf")` to find the binary):
1. Spawn a temp DB via `tempfile::NamedTempFile` — pass its path to every invocation via `--db`.
2. `accounts create --name Chase --type checking --currency USD --owner Sky` — parse stdout JSON, capture the returned account id.
3. `accounts create --name Nubank --type checking --currency BRL --owner Sky` — same.
4. `transactions import --format ofx --file tests/fixtures/chase-sample.qfx --account-id <chase_id>` — assert `status=="ok"`, `data.imported == <expected>`, `data.duplicates == 0`.
5. Re-run the same import — assert `data.imported == 0`, `data.duplicates == <expected>`.
6. Same two-step dance for Nubank fixture (gate with `if Path::new("tests/fixtures/nubank-sample.ofx").exists()` — skip if missing so CI doesn't fail before the user supplies it).
7. `transactions list --account-id <chase_id> --format json` — parse JSON array, assert first entry has non-empty `external_id`, an `amount.currency == "USD"`, and a parseable `date`.
8. Currency-mismatch path: `transactions import --format ofx --file tests/fixtures/chase-sample.qfx --account-id <nubank_id>` — assert `status=="error"`, message contains `currency mismatch`.
9. CSV rejection: `transactions import --format csv --file /dev/null --account-id <chase_id>` — exit 2, message contains `deferred`.

Use `std::process::Command::new(env!("CARGO_BIN_EXE_rtf"))` and `serde_json::from_slice` to parse each stdout.

Also write `.gsd/milestones/M001/slices/S02/S02-UAT.md` mirroring the shape of S01-UAT: list the exact CLI commands, the expected JSON shapes, and the roadmap Done criteria this proves.
  - Files: `tests/import_demo.rs`, `.gsd/milestones/M001/slices/S02/S02-UAT.md`
  - Verify: cargo test --test import_demo

- [x] **T05: End-to-end demo: import Nubank + Chase fixtures and assert CLI output** `est:1.5h`
  Lock down the slice with an integration test that exercises the whole CLI path end-to-end and with a manual demo run matching the roadmap's Done criteria.

Create `tests/import_demo.rs` (integration test — separate crate, so it can shell out to the built binary). Structure:
1. Create a temp DB via `tempfile::NamedTempFile`.
2. Exec `rtf accounts create --db $DB --name "Nubank" --type checking --currency BRL --owner "Sky"` (capture the account id from the JSON output).
3. Exec a second `accounts create` for Chase (USD).
4. Exec `rtf transactions import --db $DB --format ofx --file tests/fixtures/nubank-sample.ofx --account-id $NUBANK_ID` — assert stdout JSON has `status=="ok"` and `data.imported == <expected N>`, `data.duplicates == 0`.
5. Re-run the same import — assert `data.imported == 0`, `data.duplicates == N`.
6. Same for Chase CSV.
7. Exec `rtf transactions list --db $DB --account-id $NUBANK_ID --format json` — parse JSON, assert first transaction has expected `date`, `amount`, `payee`, `external_id`, and `amount.currency == "BRL"`.
8. Currency-mismatch case: try importing the Chase CSV against the Nubank account → assert `status=="error"` and message contains `currency mismatch`.

Use `std::process::Command` with `env!("CARGO_BIN_EXE_rtf")` to invoke the binary — that's the cargo-blessed way to integration-test a bin crate.

Fixtures: use the user-provided sanitized `tests/fixtures/nubank-sample.ofx` and `tests/fixtures/chase-sample.csv`. If either is missing, the integration test uses `#[ignore]` with a clear reason so CI still runs but the demo is explicit.

Finally, document the demo in `.gsd/milestones/M001/slices/S02/S02-UAT.md` (mirror the S01-UAT pattern) with the exact commands and expected JSON shapes.
  - Files: `tests/import_demo.rs`, `tests/fixtures/nubank-sample.ofx`, `tests/fixtures/chase-sample.csv`, `.gsd/milestones/M001/slices/S02/S02-UAT.md`
  - Verify: cargo test --test import_demo

## Files Likely Touched

- src/infrastructure/importer/ofx.rs
- src/infrastructure/importer/mod.rs
- tests/fixtures/chase-sample.qfx
- migrations/002_transaction_external_id_unique.sql
- src/infrastructure/storage/migrations.rs
- src/domain/transaction/repository.rs
- src/infrastructure/storage/transaction_repo.rs
- src/application/transaction_service.rs
- src/domain/error.rs
- src/cli/transactions.rs
- tests/import_demo.rs
- .gsd/milestones/M001/slices/S02/S02-UAT.md
- tests/fixtures/nubank-sample.ofx
- tests/fixtures/chase-sample.csv
