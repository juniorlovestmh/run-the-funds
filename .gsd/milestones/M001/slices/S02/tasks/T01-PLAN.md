---
estimated_steps: 34
estimated_files: 3
skills_used: []
---

# T01: Unified OFX adapter: SGML (1.x) + XML (2.x) with fixture-driven TDD

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

## Inputs

- `src/infrastructure/importer/ofx.rs (current stub, with quick-xml absolute-path guidance)`
- `src/domain/transaction/transaction.rs`
- `src/domain/currency/money.rs`
- `/Users/sky/Downloads/sample.qfx (source for the fixture)`

## Expected Output

- `Working `OfxImporter::import` that dispatches on dialect and returns `Vec<Transaction>``
- `Line-oriented SGML tokenizer + quick-xml XML path, sharing field mapping`
- `Unit tests covering dialect detection, happy path, two failure modes, negative amounts, CURDEF`
- `Fixture test parsing the real Chase QFX`
- `Nubank fixture test slot reserved (ignored) pending user-supplied file`

## Verification

cargo test -- infrastructure::importer::ofx
