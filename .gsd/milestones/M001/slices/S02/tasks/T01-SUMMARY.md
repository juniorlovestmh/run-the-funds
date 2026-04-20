---
id: T01
parent: S02
milestone: M001
key_files:
  - src/infrastructure/importer/ofx.rs
  - tests/fixtures/chase-sample.qfx
key_decisions:
  - Unified single OfxImporter covering both 1.x SGML and 2.x XML rather than two adapters — shared Record/ParseOutput reduces duplication; dialect detection is ~15 lines.
  - Line-tolerant character-level SGML lexer (not strictly line-oriented) so downstream banks whose OFX 1.x runs tags together on one line still parse.
  - Uppercase tag normalization in both paths so lowercase variants don't silently drop to the match arm's default.
  - <CURDEF> captured as the adapter's declared currency; service layer (T03) will validate/override against the account currency rather than the adapter doing it.
  - Kept Chase fixture with PII intact — user explicitly said PII is fine.
duration: 
verification_result: passed
completed_at: 2026-04-19T19:08:59.703Z
blocker_discovered: false
---

# T01: Unified OFX adapter: detects SGML (1.x) vs XML (2.x), parses both, real 523-record Chase QFX fixture passes.

**Unified OFX adapter: detects SGML (1.x) vs XML (2.x), parses both, real 523-record Chase QFX fixture passes.**

## What Happened

Replaced the `NotImplemented` stub in `src/infrastructure/importer/ofx.rs` with a single adapter that handles both OFX dialects behind one entry point.

**Dialect detection** (first 512 bytes of the file): `<?xml` prolog → XML path; `OFXHEADER:`/`DATA:OFXSGML` preamble → SGML path; bare `<OFX>` root with no OFX 1.x markers → XML; otherwise `ImportError::Parse` with a descriptive message.

**SGML (1.x) tokenizer** — a line-tolerant character-level lexer (`SgmlLexer`) that skips past the header block to the first `<`, then at each tag decides: if the trailing non-markup run is whitespace-only it's an `Open`, otherwise a `Leaf` with the inline value. Container closes produce `Close`. State machine walks the tokens tracking "inside STMTTRN" — container opens start a new `Record`, closes push it. Leaf tags map to record fields (FITID/DTPOSTED/TRNAMT/NAME/MEMO). `<CURDEF>` outside STMTTRN captures the statement currency. Minimal HTML entity decoding (`&amp;` `&lt;` `&gt;` `&quot;` `&apos;`) for SGML leaves.

**XML (2.x) parser** — `quick_xml::reader::Reader` with `trim_text(true)`. Same state machine but driven by Start/End/Text events and an open-tag stack. Text events attribute to the innermost open tag. Tag names uppercased in both paths for defensive matching.

**Shared pipeline** — both paths feed a `ParseOutput { records, currency }` which is then mapped to `Transaction` values: `FITID → external_id`, `DTPOSTED[..8] → date` (parsed with `%Y%m%d`), `TRNAMT → amount.amount` (via `Decimal::from_str`), `NAME → payee`, `MEMO → description`. Currency defaults to USD when no `<CURDEF>` was seen. `id` is a fresh UUID v4; `status = Pending`; `imported_at = Utc::now()`.

**Tests (19 new, 1 ignored):** dialect detection (SGML, XML prolog, bare `<OFX>`, garbage); SGML happy path + multi-record order + CURDEF=BRL + missing FITID + malformed date + malformed amount + negative/positive sign preservation + entity decoding; XML happy path + multi-record + missing FITID; IO error for missing file; **real Chase QFX fixture (523 records) asserts count + spot-checks first row (FITID `202604160`, -200.00 USD, 2026-04-16, Wise payee)**; Nubank fixture test slot reserved with `#[ignore]` pending user-supplied file.

Kept the Chase QFX fixture at `tests/fixtures/chase-sample.qfx` unsanitized per user instruction.

## Verification

`cargo test -- infrastructure::importer::ofx` → 19 passed, 1 ignored (Nubank fixture pending), 0 failed. Full `cargo test` → 157 passed, 1 ignored, 0 failed (was 140 → +17 new OFX tests + 1 ignored-Nubank slot). `cargo build` → clean.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test -- infrastructure::importer::ofx` | 0 | pass | 10ms |
| 2 | `cargo test` | 0 | pass | 70ms |
| 3 | `cargo build` | 0 | pass | 1170ms |

## Deviations

None from the planned scope. Added a defensive `sgml_positive_amount_preserved` test beyond the minimum plan, and an `io_error_surfaces_missing_file` test — both cheap.

## Known Issues

Nubank fixture not yet supplied — `parses_nubank_ofx_fixture` is `#[ignore]`d. Once the file lands at `tests/fixtures/nubank-sample.ofx` the test becomes live with a one-line flag removal.

## Files Created/Modified

- `src/infrastructure/importer/ofx.rs`
- `tests/fixtures/chase-sample.qfx`
