//! OFX importer — handles both OFX 1.x (SGML, Chase QFX-style) and
//! OFX 2.x (well-formed XML, typical of Brazilian banks like Nubank).
//!
//! Dialect is detected from the preamble:
//! - An `<?xml` prolog (or bare `<OFX>` root with no OFX 1.x header block)
//!   routes to the `quick-xml` path.
//! - A `OFXHEADER:` / `DATA:OFXSGML` preamble routes to the line-oriented
//!   SGML tokenizer defined in this module.
//!
//! Both paths feed a common `Record` accumulator and produce `Transaction`
//! values with FITID-backed `external_id`. Dedup via external_id is handled
//! one layer up, in `TransactionService::import_from`.

use std::fs;
use std::path::Path;
use std::str::FromStr;

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::currency::{CurrencyCode, Money};
use crate::domain::transaction::Transaction;

use super::{ImportError, Importer};

/// OFX file importer. Stateless — constructed per-import by the service layer.
pub struct OfxImporter;

impl OfxImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OfxImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Importer for OfxImporter {
    fn name(&self) -> &'static str {
        "ofx"
    }

    fn import(&self, source: &Path, account_id: &str) -> Result<Vec<Transaction>, ImportError> {
        let bytes = fs::read(source).map_err(|e| ImportError::Io {
            path: source.display().to_string(),
            source: e,
        })?;
        // OFX 1.x is USASCII / Latin-1; OFX 2.x is typically UTF-8. `from_utf8_lossy`
        // keeps us working on both without pulling in an encoding crate.
        let text = String::from_utf8_lossy(&bytes).into_owned();
        parse_ofx(&text, account_id)
    }
}

fn parse_ofx(text: &str, account_id: &str) -> Result<Vec<Transaction>, ImportError> {
    let output = match detect_dialect(text)? {
        Dialect::Sgml => parse_sgml(text)?,
        Dialect::Xml => parse_xml(text)?,
    };

    let currency = output.currency.unwrap_or(CurrencyCode::USD);
    output
        .records
        .into_iter()
        .map(|r| r.into_transaction(account_id, currency))
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum Dialect {
    Sgml,
    Xml,
}

fn detect_dialect(text: &str) -> Result<Dialect, ImportError> {
    let head_len = text.len().min(512);
    let head = &text[..head_len];
    let trimmed = head.trim_start();

    if trimmed.starts_with("<?xml") {
        return Ok(Dialect::Xml);
    }
    if head.contains("OFXHEADER:") || head.contains("DATA:OFXSGML") {
        return Ok(Dialect::Sgml);
    }
    if trimmed.starts_with("<OFX>") {
        return Ok(Dialect::Xml);
    }
    Err(ImportError::Parse {
        format: "ofx",
        detail: "cannot detect OFX dialect (no OFXHEADER, DATA:OFXSGML, or <?xml prolog)".into(),
    })
}

#[derive(Debug, Default)]
struct Record {
    fitid: Option<String>,
    dtposted: Option<String>,
    trnamt: Option<String>,
    name: Option<String>,
    memo: Option<String>,
}

impl Record {
    fn set(&mut self, tag: &str, value: &str) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        match tag {
            "FITID" => self.fitid = Some(trimmed.to_string()),
            "DTPOSTED" => self.dtposted = Some(trimmed.to_string()),
            "TRNAMT" => self.trnamt = Some(trimmed.to_string()),
            "NAME" => self.name = Some(trimmed.to_string()),
            "MEMO" => self.memo = Some(trimmed.to_string()),
            _ => {}
        }
    }

    fn into_transaction(
        self,
        account_id: &str,
        currency: CurrencyCode,
    ) -> Result<Transaction, ImportError> {
        let fitid = self.fitid.ok_or_else(|| ImportError::Parse {
            format: "ofx",
            detail: "STMTTRN missing <FITID>".into(),
        })?;
        let dtposted = self.dtposted.ok_or_else(|| ImportError::Parse {
            format: "ofx",
            detail: format!("STMTTRN {fitid} missing <DTPOSTED>"),
        })?;
        let trnamt = self.trnamt.ok_or_else(|| ImportError::Parse {
            format: "ofx",
            detail: format!("STMTTRN {fitid} missing <TRNAMT>"),
        })?;

        let date_slice = dtposted.get(..8).ok_or_else(|| ImportError::Parse {
            format: "ofx",
            detail: format!("STMTTRN {fitid} DTPOSTED too short: {dtposted}"),
        })?;
        let date =
            NaiveDate::parse_from_str(date_slice, "%Y%m%d").map_err(|e| ImportError::Parse {
                format: "ofx",
                detail: format!("STMTTRN {fitid} malformed DTPOSTED {dtposted}: {e}"),
            })?;
        let amount = Decimal::from_str(trnamt.trim()).map_err(|e| ImportError::Parse {
            format: "ofx",
            detail: format!("STMTTRN {fitid} malformed TRNAMT {trnamt}: {e}"),
        })?;

        let mut txn = Transaction::new(
            Uuid::new_v4().to_string(),
            account_id.to_string(),
            date,
            Money::new(amount, currency),
        )?;
        txn.external_id = Some(fitid);
        txn.payee = self.name;
        txn.description = self.memo;
        txn.imported_at = Some(Utc::now());
        Ok(txn)
    }
}

#[derive(Debug, Default)]
struct ParseOutput {
    records: Vec<Record>,
    currency: Option<CurrencyCode>,
}

// --- OFX 1.x SGML path -------------------------------------------------------

enum SgmlToken {
    Open(String),
    Close(String),
    Leaf(String, String),
}

/// Line-tolerant tokenizer for OFX 1.x SGML. Leaf tags have no closing form
/// (`<TRNAMT>-45.99` on its own line); container tags do (`<STMTTRN>...</STMTTRN>`).
///
/// Strategy: skip everything before the first `<`, then at each `<…>` decide
/// whether the following non-markup run is whitespace (Open) or text (Leaf).
struct SgmlLexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> SgmlLexer<'a> {
    fn new(input: &'a str) -> Self {
        let body_start = input.find('<').unwrap_or(input.len());
        Self {
            input,
            pos: body_start,
        }
    }
}

impl Iterator for SgmlLexer<'_> {
    type Item = Result<SgmlToken, String>;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        if self.pos >= bytes.len() {
            return None;
        }
        if bytes[self.pos] != b'<' {
            return Some(Err(format!("expected '<' at byte {}", self.pos)));
        }
        self.pos += 1;

        let is_close = self.pos < bytes.len() && bytes[self.pos] == b'/';
        if is_close {
            self.pos += 1;
        }

        let tag_start = self.pos;
        while self.pos < bytes.len() && bytes[self.pos] != b'>' {
            self.pos += 1;
        }
        if self.pos >= bytes.len() {
            return Some(Err("unterminated tag".into()));
        }
        let tag = self.input[tag_start..self.pos].trim().to_uppercase();
        self.pos += 1; // consume '>'

        if is_close {
            return Some(Ok(SgmlToken::Close(tag)));
        }

        let val_start = self.pos;
        while self.pos < bytes.len() && bytes[self.pos] != b'<' {
            self.pos += 1;
        }
        let raw = &self.input[val_start..self.pos];
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            Some(Ok(SgmlToken::Open(tag)))
        } else {
            Some(Ok(SgmlToken::Leaf(tag, decode_entities(trimmed))))
        }
    }
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn parse_sgml(text: &str) -> Result<ParseOutput, ImportError> {
    let mut output = ParseOutput::default();
    let mut current: Option<Record> = None;

    for token in SgmlLexer::new(text) {
        let token = token.map_err(|e| ImportError::Parse {
            format: "ofx",
            detail: e,
        })?;

        match token {
            SgmlToken::Open(tag) if tag == "STMTTRN" => {
                current = Some(Record::default());
            }
            SgmlToken::Close(tag) if tag == "STMTTRN" => {
                if let Some(record) = current.take() {
                    output.records.push(record);
                }
            }
            SgmlToken::Leaf(tag, value) => {
                if let Some(rec) = current.as_mut() {
                    rec.set(&tag, &value);
                } else if tag == "CURDEF" {
                    if let Ok(c) = CurrencyCode::from_str(value.trim()) {
                        output.currency = Some(c);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(output)
}

// --- OFX 2.x XML path --------------------------------------------------------

fn parse_xml(text: &str) -> Result<ParseOutput, ImportError> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    let mut output = ParseOutput::default();
    let mut current: Option<Record> = None;
    let mut stack: Vec<String> = Vec::new();
    let mut buf = Vec::new();

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| ImportError::Parse {
                format: "ofx",
                detail: format!("xml parse error: {e}"),
            })?;

        match event {
            Event::Start(e) => {
                let tag = tag_name(&e.name().as_ref())?;
                if tag == "STMTTRN" {
                    current = Some(Record::default());
                }
                stack.push(tag);
            }
            Event::End(e) => {
                let tag = tag_name(&e.name().as_ref())?;
                if tag == "STMTTRN" {
                    if let Some(record) = current.take() {
                        output.records.push(record);
                    }
                }
                stack.pop();
            }
            Event::Text(e) => {
                let text = e
                    .unescape()
                    .map_err(|err| ImportError::Parse {
                        format: "ofx",
                        detail: format!("xml text decode: {err}"),
                    })?
                    .into_owned();
                if let Some(tag) = stack.last() {
                    let tag = tag.clone();
                    if let Some(rec) = current.as_mut() {
                        rec.set(&tag, &text);
                    } else if tag == "CURDEF" {
                        if let Ok(c) = CurrencyCode::from_str(text.trim()) {
                            output.currency = Some(c);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(output)
}

fn tag_name(bytes: &[u8]) -> Result<String, ImportError> {
    std::str::from_utf8(bytes)
        .map(|s| s.to_uppercase())
        .map_err(|e| ImportError::Parse {
            format: "ofx",
            detail: format!("xml non-utf8 tag: {e}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    fn sgml_with(stmttrns: &str, curdef: &str) -> String {
        format!(
            "\nOFXHEADER:100\nDATA:OFXSGML\nVERSION:102\nSECURITY:NONE\nENCODING:USASCII\n\n\
             <OFX>\n<BANKMSGSRSV1>\n<STMTTRNRS>\n<STMTRS>\n<CURDEF>{curdef}\n\
             <BANKTRANLIST>\n{stmttrns}\n</BANKTRANLIST>\n</STMTRS>\n</STMTTRNRS>\n</BANKMSGSRSV1>\n</OFX>\n"
        )
    }

    #[test]
    fn name_is_ofx() {
        assert_eq!(OfxImporter::new().name(), "ofx");
    }

    #[test]
    fn detects_sgml_from_header() {
        let content = sgml_with("", "USD");
        let dialect = detect_dialect(&content).unwrap();
        assert!(matches!(dialect, Dialect::Sgml));
    }

    #[test]
    fn detects_xml_from_prolog() {
        let content = "<?xml version=\"1.0\"?>\n<OFX></OFX>";
        let dialect = detect_dialect(content).unwrap();
        assert!(matches!(dialect, Dialect::Xml));
    }

    #[test]
    fn detects_xml_from_bare_ofx_root() {
        let content = "<OFX><BANKMSGSRSV1></BANKMSGSRSV1></OFX>";
        let dialect = detect_dialect(content).unwrap();
        assert!(matches!(dialect, Dialect::Xml));
    }

    #[test]
    fn detect_fails_on_garbage() {
        let err = detect_dialect("not ofx").unwrap_err();
        assert!(matches!(err, ImportError::Parse { .. }));
    }

    #[test]
    fn sgml_parses_single_stmttrn() {
        let content = sgml_with(
            "<STMTTRN>\n<TRNTYPE>DEBIT\n<DTPOSTED>20260315120000[0:GMT]\n\
             <TRNAMT>-45.99\n<FITID>abc123\n<NAME>Costco\n<MEMO>groceries\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let txns = OfxImporter::new().import(f.path(), "acc-001").unwrap();

        assert_eq!(txns.len(), 1);
        let t = &txns[0];
        assert_eq!(t.account_id, "acc-001");
        assert_eq!(t.external_id.as_deref(), Some("abc123"));
        assert_eq!(t.date, NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());
        assert_eq!(t.amount.amount, dec!(-45.99));
        assert_eq!(t.amount.currency, CurrencyCode::USD);
        assert_eq!(t.payee.as_deref(), Some("Costco"));
        assert_eq!(t.description.as_deref(), Some("groceries"));
        assert!(t.imported_at.is_some());
    }

    #[test]
    fn sgml_parses_multiple_stmttrn_preserving_order() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20260315\n<TRNAMT>-10.00\n<FITID>t1\n</STMTTRN>\n\
             <STMTTRN>\n<DTPOSTED>20260316\n<TRNAMT>-20.00\n<FITID>t2\n</STMTTRN>\n\
             <STMTTRN>\n<DTPOSTED>20260317\n<TRNAMT>+30.00\n<FITID>t3\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let txns = OfxImporter::new().import(f.path(), "acc-001").unwrap();
        let ids: Vec<&str> = txns
            .iter()
            .filter_map(|t| t.external_id.as_deref())
            .collect();
        assert_eq!(ids, vec!["t1", "t2", "t3"]);
    }

    #[test]
    fn sgml_captures_curdef_brl() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20260315\n<TRNAMT>-150.00\n<FITID>nu1\n</STMTTRN>",
            "BRL",
        );
        let f = write_temp(&content);
        let txns = OfxImporter::new().import(f.path(), "acc-brl").unwrap();
        assert_eq!(txns[0].amount.currency, CurrencyCode::BRL);
    }

    #[test]
    fn sgml_missing_fitid_errors() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20260315\n<TRNAMT>-10.00\n<NAME>X\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let err = OfxImporter::new().import(f.path(), "acc-001").unwrap_err();
        match err {
            ImportError::Parse { format, detail } => {
                assert_eq!(format, "ofx");
                assert!(detail.contains("FITID"));
            }
            other => panic!("expected Parse error, got {other:?}"),
        }
    }

    #[test]
    fn sgml_malformed_date_errors() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20261399\n<TRNAMT>-10.00\n<FITID>x1\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let err = OfxImporter::new().import(f.path(), "acc-001").unwrap_err();
        assert!(matches!(err, ImportError::Parse { .. }));
    }

    #[test]
    fn sgml_malformed_amount_errors() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20260315\n<TRNAMT>NOPE\n<FITID>x1\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let err = OfxImporter::new().import(f.path(), "acc-001").unwrap_err();
        assert!(matches!(err, ImportError::Parse { .. }));
    }

    #[test]
    fn sgml_negative_amount_preserved() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20260315\n<TRNAMT>-1234.56\n<FITID>x1\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let txns = OfxImporter::new().import(f.path(), "acc-001").unwrap();
        assert_eq!(txns[0].amount.amount, dec!(-1234.56));
        assert!(txns[0].amount.is_negative());
    }

    #[test]
    fn sgml_positive_amount_preserved() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20260315\n<TRNAMT>1516.41\n<FITID>x1\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let txns = OfxImporter::new().import(f.path(), "acc-001").unwrap();
        assert_eq!(txns[0].amount.amount, dec!(1516.41));
        assert!(!txns[0].amount.is_negative());
    }

    #[test]
    fn sgml_decodes_common_entities() {
        let content = sgml_with(
            "<STMTTRN>\n<DTPOSTED>20260315\n<TRNAMT>-10.00\n<FITID>x1\n\
             <NAME>Barnes &amp; Noble\n</STMTTRN>",
            "USD",
        );
        let f = write_temp(&content);
        let txns = OfxImporter::new().import(f.path(), "acc-001").unwrap();
        assert_eq!(txns[0].payee.as_deref(), Some("Barnes & Noble"));
    }

    #[test]
    fn xml_parses_single_stmttrn() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<OFX>
  <BANKMSGSRSV1><STMTTRNRS><STMTRS>
    <CURDEF>BRL</CURDEF>
    <BANKTRANLIST>
      <STMTTRN>
        <TRNTYPE>DEBIT</TRNTYPE>
        <DTPOSTED>20260310120000</DTPOSTED>
        <TRNAMT>-150.00</TRNAMT>
        <FITID>nu-987</FITID>
        <MEMO>Supermercado</MEMO>
      </STMTTRN>
    </BANKTRANLIST>
  </STMTRS></STMTTRNRS></BANKMSGSRSV1>
</OFX>"#;
        let f = write_temp(content);
        let txns = OfxImporter::new().import(f.path(), "acc-002").unwrap();

        assert_eq!(txns.len(), 1);
        let t = &txns[0];
        assert_eq!(t.external_id.as_deref(), Some("nu-987"));
        assert_eq!(t.date, NaiveDate::from_ymd_opt(2026, 3, 10).unwrap());
        assert_eq!(t.amount.amount, dec!(-150.00));
        assert_eq!(t.amount.currency, CurrencyCode::BRL);
        assert_eq!(t.description.as_deref(), Some("Supermercado"));
    }

    #[test]
    fn xml_parses_multiple_stmttrn() {
        let content = r#"<?xml version="1.0"?>
<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS>
<CURDEF>USD</CURDEF>
<BANKTRANLIST>
<STMTTRN><DTPOSTED>20260315</DTPOSTED><TRNAMT>-10.00</TRNAMT><FITID>a</FITID></STMTTRN>
<STMTTRN><DTPOSTED>20260316</DTPOSTED><TRNAMT>-20.00</TRNAMT><FITID>b</FITID></STMTTRN>
</BANKTRANLIST>
</STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>"#;
        let f = write_temp(content);
        let txns = OfxImporter::new().import(f.path(), "acc-002").unwrap();
        assert_eq!(txns.len(), 2);
        assert_eq!(txns[0].external_id.as_deref(), Some("a"));
        assert_eq!(txns[1].external_id.as_deref(), Some("b"));
    }

    #[test]
    fn xml_missing_fitid_errors() {
        let content = r#"<?xml version="1.0"?>
<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS>
<CURDEF>USD</CURDEF>
<BANKTRANLIST>
<STMTTRN><DTPOSTED>20260315</DTPOSTED><TRNAMT>-10.00</TRNAMT></STMTTRN>
</BANKTRANLIST>
</STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>"#;
        let f = write_temp(content);
        let err = OfxImporter::new().import(f.path(), "acc-001").unwrap_err();
        assert!(matches!(err, ImportError::Parse { .. }));
    }

    #[test]
    fn io_error_surfaces_missing_file() {
        let err = OfxImporter::new()
            .import(Path::new("/this/does/not/exist.ofx"), "acc-001")
            .unwrap_err();
        assert!(matches!(err, ImportError::Io { .. }));
    }

    #[test]
    fn parses_chase_qfx_fixture() {
        let path = Path::new("tests/fixtures/chase-sample.qfx");
        if !path.exists() {
            panic!("expected fixture at {}", path.display());
        }
        let txns = OfxImporter::new().import(path, "acc-chase").unwrap();

        assert_eq!(
            txns.len(),
            3,
            "Chase QFX fixture has 3 synthetic transactions"
        );
        assert!(txns.iter().all(|t| t.external_id.is_some()));
        assert!(txns.iter().all(|t| t.amount.currency == CurrencyCode::USD));
        assert!(txns.iter().all(|t| t.account_id == "acc-chase"));

        // First record in the file is 2026-01-03, ACME COFFEE debit -12.34.
        let first = &txns[0];
        assert_eq!(first.external_id.as_deref(), Some("SAMPLE003"));
        assert_eq!(first.date, NaiveDate::from_ymd_opt(2026, 1, 3).unwrap());
        assert_eq!(first.amount.amount, dec!(-12.34));
        assert!(first.payee.as_deref().unwrap().contains("ACME COFFEE"));
    }

    #[test]
    fn parses_nubank_ofx_fixture() {
        // Nubank's real exports emit an OFX 1.x header (`OFXHEADER:100`,
        // `DATA:OFXSGML`) but XML-style closing tags on every leaf. Our
        // dialect detector routes it to the SGML tokenizer and closes are
        // no-ops in the record state machine — the hybrid "Just Works".
        let path = Path::new("tests/fixtures/nubank-sample.ofx");
        let txns = OfxImporter::new().import(path, "acc-nubank").unwrap();

        assert_eq!(
            txns.len(),
            3,
            "Nubank OFX fixture has 3 synthetic transactions"
        );
        assert!(txns.iter().all(|t| t.external_id.is_some()));
        assert!(txns.iter().all(|t| t.amount.currency == CurrencyCode::BRL));
        assert!(txns.iter().all(|t| t.account_id == "acc-nubank"));

        // First record (2026-01-01, +2000.00 BRL, incoming Pix transfer).
        let first = &txns[0];
        assert_eq!(
            first.external_id.as_deref(),
            Some("00000000-0000-0000-0000-000000000001")
        );
        assert_eq!(first.date, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(first.amount.amount, dec!(2000.00));
        // Portuguese accents must round-trip cleanly (UTF-8 encoded file).
        assert!(
            first
                .description
                .as_deref()
                .unwrap()
                .contains("Transferência")
        );
        assert!(first.description.as_deref().unwrap().contains("Câmbio"));
    }
}
