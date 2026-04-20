//! End-to-end integration test for S02 — shells out to the compiled binary
//! and drives the full `accounts create` → `transactions import` → re-import
//! → `transactions list` flow against the real Chase QFX and Nubank OFX
//! fixtures. Also covers the currency-mismatch and CSV-deferred error paths.
//!
//! This is the slice's proof: if this test passes, the roadmap's S02 "Done"
//! criteria hold.

use std::path::Path;
use std::process::{Command, Output};

use chrono::{NaiveDate, Utc};
use rtf::domain::currency::CurrencyCode;
use rtf::domain::exchange::{ExchangeRate, ExchangeRateRepository};
use rtf::infrastructure::storage::{Database, SqliteExchangeRateRepository};
use rust_decimal_macros::dec;
use serde_json::Value;
use tempfile::TempDir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtf")
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("failed to invoke rtf binary")
}

fn parse_stdout(output: &Output) -> Value {
    let stdout = std::str::from_utf8(&output.stdout).expect("stdout not utf-8");
    serde_json::from_str(stdout).unwrap_or_else(|e| {
        panic!("failed to parse stdout as JSON: {e}\nstdout: {stdout}\nstderr: {}", String::from_utf8_lossy(&output.stderr))
    })
}

fn parse_stderr(output: &Output) -> Value {
    let stderr = std::str::from_utf8(&output.stderr).expect("stderr not utf-8");
    serde_json::from_str(stderr).unwrap_or_else(|e| {
        panic!("failed to parse stderr as JSON: {e}\nstderr: {stderr}")
    })
}

fn create_account(db: &str, name: &str, account_type: &str, currency: &str) -> String {
    let out = run(&[
        "--db",
        db,
        "accounts",
        "create",
        "--name",
        name,
        "--type",
        account_type,
        "--currency",
        currency,
        "--owner",
        "Sky",
    ]);
    assert!(
        out.status.success(),
        "create {name}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = parse_stdout(&out);
    assert_eq!(json["status"], "ok");
    json["data"]["id"]
        .as_str()
        .expect("account id should be a string")
        .to_string()
}

#[test]
fn end_to_end_import_demo() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("rtf.db");
    let db = db.to_str().unwrap();

    let chase_id = create_account(db, "Chase", "checking", "USD");
    let nubank_id = create_account(db, "Nubank", "checking", "BRL");

    // ---- Chase QFX import ---------------------------------------------------
    let chase_fixture = "tests/fixtures/chase-sample.qfx";
    assert!(
        Path::new(chase_fixture).exists(),
        "missing Chase fixture at {chase_fixture}"
    );

    let out = run(&[
        "--db",
        db,
        "transactions",
        "import",
        "--format",
        "ofx",
        "--file",
        chase_fixture,
        "--account-id",
        &chase_id,
    ]);
    assert!(out.status.success(), "Chase first import failed");
    let json = parse_stdout(&out);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["imported"], 3);
    assert_eq!(json["data"]["duplicates"], 0);
    assert_eq!(json["data"]["format"], "ofx");

    // Re-import the same file — every FITID should already exist.
    let out = run(&[
        "--db",
        db,
        "transactions",
        "import",
        "--format",
        "qfx", // alias for ofx — should also route to the OFX adapter
        "--file",
        chase_fixture,
        "--account-id",
        &chase_id,
    ]);
    assert!(out.status.success(), "Chase re-import failed");
    let json = parse_stdout(&out);
    assert_eq!(json["data"]["imported"], 0);
    assert_eq!(json["data"]["duplicates"], 3);
    assert_eq!(json["data"]["format"], "qfx");

    // ---- Nubank OFX import --------------------------------------------------
    let nubank_fixture = "tests/fixtures/nubank-sample.ofx";
    assert!(
        Path::new(nubank_fixture).exists(),
        "missing Nubank fixture at {nubank_fixture}"
    );

    let out = run(&[
        "--db",
        db,
        "transactions",
        "import",
        "--format",
        "ofx",
        "--file",
        nubank_fixture,
        "--account-id",
        &nubank_id,
    ]);
    assert!(out.status.success(), "Nubank first import failed");
    let json = parse_stdout(&out);
    assert_eq!(json["data"]["imported"], 3);
    assert_eq!(json["data"]["duplicates"], 0);

    // Re-import
    let out = run(&[
        "--db",
        db,
        "transactions",
        "import",
        "--format",
        "ofx",
        "--file",
        nubank_fixture,
        "--account-id",
        &nubank_id,
    ]);
    assert!(out.status.success());
    let json = parse_stdout(&out);
    assert_eq!(json["data"]["imported"], 0);
    assert_eq!(json["data"]["duplicates"], 3);

    // Pre-seed BRL↔USD rates for every Nubank fixture date so `transactions list`
    // never hits the live BCB API in this test. S03 enriches BRL rows with
    // `amount_usd` via the CurrencyConverter; without seeding, that would
    // trigger live HTTP calls and make this test flaky.
    {
        let db_handle = Database::open(db).unwrap();
        let repo = SqliteExchangeRateRepository::new(&db_handle);
        for day in 1..=3u32 {
            let d = NaiveDate::from_ymd_opt(2026, 1, day).unwrap();
            for (from, to, rate) in [
                (CurrencyCode::USD, CurrencyCode::BRL, dec!(5.10)),
                (CurrencyCode::BRL, CurrencyCode::USD, dec!(0.196)),
            ] {
                repo.save(&ExchangeRate {
                    id: format!("seed-{from}-{to}-{d}"),
                    from,
                    to,
                    rate,
                    date: d,
                    source: "SEEDED".into(),
                    fetched_at: Utc::now(),
                })
                .unwrap();
            }
        }
    }

    // ---- List and spot-check ------------------------------------------------
    let out = run(&[
        "--db",
        db,
        "transactions",
        "list",
        "--account-id",
        &chase_id,
        "--format",
        "json",
    ]);
    assert!(out.status.success());
    let json = parse_stdout(&out);
    assert_eq!(json["status"], "ok");
    let txns = json["data"].as_array().expect("data should be an array");
    assert_eq!(txns.len(), 3);
    // Most-recent row after ORDER BY date DESC is 2026-01-03, ACME COFFEE, FITID SAMPLE003.
    let first = &txns[0];
    assert_eq!(first["external_id"], "SAMPLE003");
    assert_eq!(first["amount"]["currency"], "USD");
    assert_eq!(first["date"], "2026-01-03");

    let out = run(&[
        "--db",
        db,
        "transactions",
        "list",
        "--account-id",
        &nubank_id,
        "--format",
        "json",
    ]);
    assert!(out.status.success());
    let json = parse_stdout(&out);
    let txns = json["data"].as_array().unwrap();
    assert_eq!(txns.len(), 3);
    assert!(
        txns.iter()
            .all(|t| t["amount"]["currency"] == "BRL"),
        "all Nubank rows must be tagged BRL"
    );
    // S03 enrichment: every BRL row gets amount_usd + rate metadata. Seeded
    // rates above guarantee rate_status is "ok" (exact date match).
    assert!(
        txns.iter().all(|t| t["amount_usd"].is_string()),
        "every BRL row should have a populated amount_usd"
    );
    assert!(
        txns.iter().all(|t| t["rate_status"] == "ok"),
        "seeded rates should give rate_status: ok for every row"
    );
    assert!(
        txns.iter().all(|t| t["rate"] == "0.196"),
        "every row should use the seeded BRL→USD rate"
    );

    // ---- Currency mismatch --------------------------------------------------
    let out = run(&[
        "--db",
        db,
        "transactions",
        "import",
        "--format",
        "ofx",
        "--file",
        chase_fixture, // USD file
        "--account-id",
        &nubank_id, // BRL account
    ]);
    assert!(!out.status.success(), "currency mismatch should fail");
    let err = parse_stderr(&out);
    assert_eq!(err["status"], "error");
    let msg = err["message"].as_str().unwrap();
    assert!(
        msg.contains("currency mismatch"),
        "expected 'currency mismatch' in error, got: {msg}"
    );

    // ---- CSV rejection ------------------------------------------------------
    let out = run(&[
        "--db",
        db,
        "transactions",
        "import",
        "--format",
        "csv",
        "--file",
        "/dev/null",
        "--account-id",
        &chase_id,
    ]);
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let err = parse_stderr(&out);
    let msg = err["message"].as_str().unwrap();
    assert!(
        msg.contains("deferred"),
        "expected 'deferred' in error, got: {msg}"
    );
}
