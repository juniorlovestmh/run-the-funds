//! End-to-end integration test for S03 — exercises `rtf convert` + the
//! multi-currency `transactions list` enrichment against the compiled binary.
//!
//! Rates are seeded directly into the SQLite DB before each CLI invocation
//! so the test runs fully offline (no BCB round-trips). One `#[ignore]`d
//! live variant at the bottom exists for manual verification.

use std::process::{Command, Output};

use chrono::{NaiveDate, Utc};
use rtf::domain::currency::CurrencyCode;
use rtf::domain::exchange::{ExchangeRate, ExchangeRateRepository};
use rtf::infrastructure::storage::{Database, SqliteExchangeRateRepository};
use rust_decimal::Decimal;
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
        panic!(
            "failed to parse stdout JSON: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn seed_rate(db_path: &str, from: CurrencyCode, to: CurrencyCode, date: NaiveDate, rate: Decimal) {
    let db = Database::open(db_path).unwrap();
    let repo = SqliteExchangeRateRepository::new(&db);
    repo.save(&ExchangeRate {
        id: format!("seed-{from}-{to}-{date}"),
        from,
        to,
        rate,
        date,
        source: "SEEDED".into(),
        fetched_at: Utc::now(),
    })
    .unwrap();
}

/// Convenience: seed both directions (USD→BRL and BRL→USD) for a date.
fn seed_pair(db_path: &str, date: NaiveDate, usd_to_brl: Decimal, brl_to_usd: Decimal) {
    seed_rate(
        db_path,
        CurrencyCode::USD,
        CurrencyCode::BRL,
        date,
        usd_to_brl,
    );
    seed_rate(
        db_path,
        CurrencyCode::BRL,
        CurrencyCode::USD,
        date,
        brl_to_usd,
    );
}

#[test]
fn convert_cli_end_to_end_with_seeded_rates() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("rtf.db");
    let db_path = db_path.to_str().unwrap();

    // Create any account — forces migrations to run.
    let out = run(&[
        "--db",
        db_path,
        "accounts",
        "create",
        "--name",
        "NubankTest",
        "--type",
        "checking",
        "--currency",
        "BRL",
        "--owner",
        "Sky",
    ]);
    assert!(out.status.success());
    let nubank_id = parse_stdout(&out)["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Seed rates for 2026-04-07 (a Tuesday) and 2026-04-03 (the Friday
    // before the 04-05 Sunday test).
    seed_pair(
        db_path,
        NaiveDate::from_ymd_opt(2026, 4, 7).unwrap(),
        dec!(5.12),
        dec!(0.195),
    );
    seed_pair(
        db_path,
        NaiveDate::from_ymd_opt(2026, 4, 3).unwrap(),
        dec!(5.08),
        dec!(0.197),
    );

    // ---- convert USD → BRL (exact cache hit) -------------------------------
    let out = run(&[
        "--db",
        db_path,
        "convert",
        "1000",
        "USD",
        "--to",
        "BRL",
        "--date",
        "2026-04-07",
    ]);
    assert!(
        out.status.success(),
        "convert USD→BRL failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = parse_stdout(&out);
    assert_eq!(json["status"], "ok");
    let data = &json["data"];
    assert_eq!(data["currency"], "BRL");
    assert_eq!(data["rate"], "5.12");
    assert_eq!(data["rate_date"], "2026-04-07");
    assert_eq!(data["source"], "SEEDED");
    assert!(data["fallback_reason"].is_null());
    // 1000 * 5.12 = 5120.00
    let amount: Decimal = data["amount"].as_str().unwrap().parse().unwrap();
    assert_eq!(amount, dec!(5120.00));

    // ---- convert BRL → USD (exact cache hit) -------------------------------
    let out = run(&[
        "--db",
        db_path,
        "convert",
        "500",
        "BRL",
        "--to",
        "USD",
        "--date",
        "2026-04-07",
    ]);
    assert!(out.status.success());
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["currency"], "USD");
    assert_eq!(data["rate"], "0.195");
    // 500 * 0.195 = 97.500
    let amount: Decimal = data["amount"].as_str().unwrap().parse().unwrap();
    assert_eq!(amount, dec!(97.500));

    // ---- Same-currency identity --------------------------------------------
    let out = run(&[
        "--db",
        db_path,
        "convert",
        "100",
        "USD",
        "--to",
        "USD",
        "--date",
        "2026-04-07",
    ]);
    assert!(out.status.success());
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["rate"], "1");
    assert_eq!(data["source"], "identity");
    assert!(data["fallback_reason"].is_null());
    let amount: Decimal = data["amount"].as_str().unwrap().parse().unwrap();
    assert_eq!(amount, dec!(100));

    // ---- Weekend walk-back (Sunday → Friday) -------------------------------
    // 2026-04-05 is a Sunday with no seeded rate; walk-back finds 2026-04-03 (Fri).
    let out = run(&[
        "--db",
        db_path,
        "convert",
        "200",
        "USD",
        "--to",
        "BRL",
        "--date",
        "2026-04-05",
    ]);
    assert!(out.status.success());
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["rate_date"], "2026-04-03");
    assert_eq!(data["rate"], "5.08");
    let reason = data["fallback_reason"]
        .as_str()
        .expect("fallback_reason should be set");
    assert!(reason.contains("2026-04-05"));
    assert!(reason.contains("2026-04-03"));
    assert!(reason.contains("2 days earlier"));

    // ---- Negative amount preserves sign ------------------------------------
    let out = run(&[
        "--db",
        db_path,
        "convert",
        "-150",
        "BRL",
        "--to",
        "USD",
        "--date",
        "2026-04-07",
    ]);
    assert!(out.status.success());
    let amount: Decimal = parse_stdout(&out)["data"]["amount"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    // -150 * 0.195 = -29.250
    assert_eq!(amount, dec!(-29.250));

    // ---- Multi-currency list enrichment ------------------------------------
    // Import the Nubank fixture and seed every fixture date. After the import,
    // list --format json should give every row a populated amount_usd with
    // rate_status == "ok".
    for day in 1..=3 {
        let d = NaiveDate::from_ymd_opt(2026, 1, day).unwrap();
        seed_pair(db_path, d, dec!(5.10), dec!(0.196));
    }

    let out = run(&[
        "--db",
        db_path,
        "transactions",
        "import",
        "--format",
        "ofx",
        "--file",
        "tests/fixtures/nubank-sample.ofx",
        "--account-id",
        &nubank_id,
    ]);
    assert!(
        out.status.success(),
        "nubank import failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run(&[
        "--db",
        db_path,
        "transactions",
        "list",
        "--account-id",
        &nubank_id,
        "--format",
        "json",
    ]);
    assert!(out.status.success());
    let txns = parse_stdout(&out)["data"]
        .as_array()
        .expect("data should be an array")
        .clone();
    assert_eq!(txns.len(), 3);
    for t in &txns {
        let status = t["rate_status"].as_str().unwrap();
        assert_eq!(status, "ok", "expected rate_status=ok, got {status}: {t}");
        assert!(t["amount_usd"].is_string(), "amount_usd must be populated");
        assert_eq!(t["rate"], "0.196");
        assert_eq!(t["amount"]["currency"], "BRL");
    }
}

#[test]
fn convert_cli_rejects_malformed_amount() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("rtf.db");
    let db_path = db_path.to_str().unwrap();

    // Force migration by creating an account first.
    run(&[
        "--db",
        db_path,
        "accounts",
        "create",
        "--name",
        "X",
        "--type",
        "checking",
        "--currency",
        "USD",
        "--owner",
        "Sky",
    ]);

    let out = run(&[
        "--db",
        db_path,
        "convert",
        "not-a-number",
        "USD",
        "--to",
        "BRL",
        "--date",
        "2026-04-07",
    ]);
    assert!(!out.status.success());
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    let msg = err["message"].as_str().unwrap();
    assert!(msg.contains("invalid amount"), "unexpected: {msg}");
}

#[test]
fn convert_cli_rejects_malformed_date() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("rtf.db");
    let db_path = db_path.to_str().unwrap();

    run(&[
        "--db",
        db_path,
        "accounts",
        "create",
        "--name",
        "X",
        "--type",
        "checking",
        "--currency",
        "USD",
        "--owner",
        "Sky",
    ]);

    let out = run(&[
        "--db",
        db_path,
        "convert",
        "100",
        "USD",
        "--to",
        "BRL",
        "--date",
        "not-a-date",
    ]);
    assert!(!out.status.success());
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert!(err["message"].as_str().unwrap().contains("invalid date"));
}

/// Live-BCB smoke test — hits the real PTAX API for a known past business day
/// and asserts the returned rate is within a sane band. Ignored by default
/// so CI doesn't depend on BCB's availability. Run with
/// `cargo test --test convert_demo -- --ignored` for manual verification.
#[test]
#[ignore = "hits live BCB API; run with `cargo test -- --ignored` for manual verification"]
fn convert_cli_live_bcb_smoke_test() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("rtf.db");
    let db_path = db_path.to_str().unwrap();

    run(&[
        "--db",
        db_path,
        "accounts",
        "create",
        "--name",
        "X",
        "--type",
        "checking",
        "--currency",
        "USD",
        "--owner",
        "Sky",
    ]);

    // Pick a known past business day.
    let out = run(&[
        "--db",
        db_path,
        "convert",
        "1",
        "USD",
        "--to",
        "BRL",
        "--date",
        "2025-01-02",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["source"], "BCB PTAX");
    let rate: Decimal = data["rate"].as_str().unwrap().parse().unwrap();
    assert!(
        rate > dec!(1.0) && rate < dec!(20.0),
        "rate out of sane band: {rate}"
    );
}
