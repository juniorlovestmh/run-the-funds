//! End-to-end smoke tests for S04 — shells to the compiled binary.
//!
//! **Design choice:** the default test suite runs fully offline. It exercises
//! the CLI error paths and the credential/linkage round-trips — everything
//! that doesn't require a real HTTP call. Two `#[ignore]`d live smokes sit
//! at the bottom; they require real provider credentials in env vars and
//! are meant for manual verification.

use std::process::{Command, Output};

use rtf::infrastructure::storage::Database;
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

fn parse_stderr(output: &Output) -> Value {
    let stderr = std::str::from_utf8(&output.stderr).expect("stderr not utf-8");
    serde_json::from_str(stderr).unwrap_or_else(|e| {
        panic!("failed to parse stderr JSON: {e}\nstderr: {stderr}")
    })
}

fn temp_db() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("rtf.db");
    let s = path.to_str().unwrap().to_string();
    (dir, s)
}

#[test]
fn sync_missing_simplefin_credentials_has_clear_message() {
    let (_dir, db) = temp_db();
    // Force migrations by creating an account.
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);

    let out = run(&["--db", &db, "sync", "--provider", "simplefin"]);
    assert!(!out.status.success());
    let err = parse_stderr(&out);
    let msg = err["message"].as_str().unwrap();
    assert!(msg.contains("SimpleFIN not configured"), "got: {msg}");
    assert!(msg.contains("rtf simplefin setup"), "should name the setup command");
}

#[test]
fn sync_missing_pluggy_credentials_has_clear_message() {
    let (_dir, db) = temp_db();
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "BRL", "--owner", "Sky",
    ]);

    let out = run(&["--db", &db, "sync", "--provider", "pluggy"]);
    assert!(!out.status.success());
    let err = parse_stderr(&out);
    let msg = err["message"].as_str().unwrap();
    assert!(msg.contains("Pluggy not configured"), "got: {msg}");
    assert!(msg.contains("rtf pluggy setup"));
}

#[test]
fn sync_unknown_provider_errors() {
    let (_dir, db) = temp_db();
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);

    let out = run(&["--db", &db, "sync", "--provider", "bogus"]);
    assert!(!out.status.success());
    assert!(parse_stderr(&out)["message"]
        .as_str()
        .unwrap()
        .contains("unknown provider"));
}

#[test]
fn sync_unified_with_nothing_configured_guides_user() {
    let (_dir, db) = temp_db();
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);

    let out = run(&["--db", &db, "sync"]);
    assert!(!out.status.success());
    let msg = parse_stderr(&out)["message"].as_str().unwrap().to_string();
    assert!(
        msg.contains("no bank-sync providers configured"),
        "got: {msg}"
    );
    assert!(msg.contains("simplefin setup") || msg.contains("pluggy setup"));
}

#[test]
fn sync_malformed_since_errors() {
    let (_dir, db) = temp_db();
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);

    let out = run(&[
        "--db", &db, "sync",
        "--provider", "simplefin",
        "--since", "not-a-date",
    ]);
    assert!(!out.status.success());
    assert!(parse_stderr(&out)["message"]
        .as_str()
        .unwrap()
        .contains("invalid --since date"));
}

#[test]
fn accounts_link_roundtrips_through_cli() {
    let (_dir, db) = temp_db();

    // Create and capture id.
    let out = run(&[
        "--db", &db, "accounts", "create",
        "--name", "Chase", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);
    let id = parse_stdout(&out)["data"]["id"].as_str().unwrap().to_string();

    // Link.
    let out = run(&[
        "--db", &db, "accounts", "link",
        "--id", &id,
        "--provider", "simplefin",
        "--external-id", "ext-abc",
    ]);
    assert!(out.status.success(), "link failed: {}", String::from_utf8_lossy(&out.stderr));
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["external_provider"], "simplefin");
    assert_eq!(data["external_account_id"], "ext-abc");

    // Re-link without --force is rejected.
    let out = run(&[
        "--db", &db, "accounts", "link",
        "--id", &id,
        "--provider", "simplefin",
        "--external-id", "ext-xyz",
    ]);
    assert!(!out.status.success());
    assert!(parse_stderr(&out)["message"]
        .as_str()
        .unwrap()
        .contains("already linked"));

    // With --force it succeeds.
    let out = run(&[
        "--db", &db, "accounts", "link",
        "--id", &id,
        "--provider", "simplefin",
        "--external-id", "ext-xyz",
        "--force",
    ]);
    assert!(out.status.success());
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["external_account_id"], "ext-xyz");
}

#[test]
fn accounts_link_rejects_unknown_provider() {
    let (_dir, db) = temp_db();
    let out = run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);
    let id = parse_stdout(&out)["data"]["id"].as_str().unwrap().to_string();

    let out = run(&[
        "--db", &db, "accounts", "link",
        "--id", &id,
        "--provider", "bogus",
        "--external-id", "x",
    ]);
    assert!(!out.status.success());
    assert!(parse_stderr(&out)["message"]
        .as_str()
        .unwrap()
        .contains("unknown provider"));
}

#[test]
fn pluggy_setup_stores_app_credentials() {
    let (_dir, db) = temp_db();
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "BRL", "--owner", "Sky",
    ]);

    let out = run(&[
        "--db", &db, "pluggy", "setup",
        "--client-id", "cid-xyz",
        "--client-secret", "csec-xyz",
    ]);
    assert!(out.status.success(), "setup failed: {}", String::from_utf8_lossy(&out.stderr));

    let db_handle = Database::open(&db).unwrap();
    use rtf::domain::credentials::ProviderCredentialsRepository;
    use rtf::infrastructure::storage::SqliteProviderCredentialsRepository;
    let repo = SqliteProviderCredentialsRepository::new(&db_handle);
    let creds = repo.find_by_provider("pluggy").unwrap().unwrap();
    let data: serde_json::Value = serde_json::from_str(&creds.data).unwrap();
    assert_eq!(data["client_id"], "cid-xyz");
    assert_eq!(data["client_secret"], "csec-xyz");
    // Per-bank item_id now lives in provider_connections, not here.
    assert!(data.get("item_id").is_none());
}

// ---- Live smokes --------------------------------------------------------
// These require real credentials in env vars and are ignored by default.
// Run with: `cargo test --test sync_demo -- --ignored`
// Each test expects a fresh temp DB so it won't touch a real user database.

#[test]
fn sync_missing_teller_credentials_has_clear_message() {
    let (_dir, db) = temp_db();
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);
    let out = run(&["--db", &db, "sync", "--provider", "teller"]);
    assert!(!out.status.success());
    let msg = parse_stderr(&out)["message"].as_str().unwrap().to_string();
    assert!(msg.contains("Teller not configured"), "got: {msg}");
    assert!(msg.contains("rtf teller setup"));
}

#[test]
fn teller_setup_stores_app_credentials() {
    let (_dir, db) = temp_db();
    run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);
    let out = run(&[
        "--db", &db, "teller", "setup",
        "--app-id", "app_test_xyz",
    ]);
    assert!(out.status.success(), "setup failed: {}", String::from_utf8_lossy(&out.stderr));

    let db_handle = Database::open(&db).unwrap();
    use rtf::domain::credentials::ProviderCredentialsRepository;
    use rtf::infrastructure::storage::SqliteProviderCredentialsRepository;
    let repo = SqliteProviderCredentialsRepository::new(&db_handle);
    let creds = repo.find_by_provider("teller").unwrap().unwrap();
    let data: serde_json::Value = serde_json::from_str(&creds.data).unwrap();
    assert_eq!(data["app_id"], "app_test_xyz");
    assert_eq!(data["environment"], "sandbox");
    // Per-bank access_token lives in provider_connections, not here.
    assert!(data.get("access_token").is_none());
}

#[test]
fn accounts_link_accepts_teller_provider() {
    let (_dir, db) = temp_db();
    let out = run(&[
        "--db", &db, "accounts", "create",
        "--name", "X", "--type", "checking",
        "--currency", "USD", "--owner", "Sky",
    ]);
    let id = parse_stdout(&out)["data"]["id"].as_str().unwrap().to_string();

    let out = run(&[
        "--db", &db, "accounts", "link",
        "--id", &id,
        "--provider", "teller",
        "--external-id", "acc_teller_1",
    ]);
    assert!(out.status.success(), "link failed: {}", String::from_utf8_lossy(&out.stderr));
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["external_provider"], "teller");
    assert_eq!(data["external_account_id"], "acc_teller_1");
}

#[test]
#[ignore = "requires SIMPLEFIN_SETUP_TOKEN env; hits live SimpleFIN API"]
fn live_simplefin_setup_smoke() {
    let token = std::env::var("SIMPLEFIN_SETUP_TOKEN")
        .expect("SIMPLEFIN_SETUP_TOKEN must be set for this test");
    let (_dir, db) = temp_db();

    let out = run(&["--db", &db, "simplefin", "setup", &token]);
    assert!(
        out.status.success(),
        "setup failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = parse_stdout(&out);
    let msg = json["data"]["message"].as_str().unwrap();
    assert!(msg.contains("configured"));
}

#[test]
#[ignore = "requires TELLER_APP_ID + cert + key env; manual browser flow"]
fn live_teller_connect_smoke() {
    let app_id = std::env::var("TELLER_APP_ID").expect("TELLER_APP_ID required");
    let cert = std::env::var("TELLER_CERT_PATH").expect("TELLER_CERT_PATH required");
    let key = std::env::var("TELLER_KEY_PATH").expect("TELLER_KEY_PATH required");
    let (_dir, db) = temp_db();

    let out = run(&[
        "--db", &db, "teller", "setup",
        "--app-id", &app_id,
        "--cert", &cert,
        "--key", &key,
    ]);
    assert!(out.status.success());

    // The connect call opens a browser and blocks until the user links a bank.
    // Skipped in automated CI; run manually with --ignored.
}

#[test]
#[ignore = "requires TELLER_ACCESS_TOKEN env; hits live Teller API"]
fn live_teller_sync_smoke() {
    let token = std::env::var("TELLER_ACCESS_TOKEN")
        .expect("TELLER_ACCESS_TOKEN must be set for this test");
    let (_dir, db) = temp_db();

    let out = run(&["--db", &db, "teller", "setup", "--access-token", &token]);
    assert!(
        out.status.success(),
        "setup failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // With no accounts locally linked, sync returns imported:0 without error.
    let out = run(&["--db", &db, "sync", "--provider", "teller"]);
    assert!(
        out.status.success(),
        "sync failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["provider"], "teller");
    assert_eq!(data["accounts_synced"], 0);
}

#[test]
#[ignore = "requires PLUGGY_CLIENT_ID + PLUGGY_CLIENT_SECRET + PLUGGY_ITEM_ID env; hits live Pluggy"]
fn live_pluggy_sync_smoke() {
    let client_id = std::env::var("PLUGGY_CLIENT_ID").expect("PLUGGY_CLIENT_ID required");
    let client_secret =
        std::env::var("PLUGGY_CLIENT_SECRET").expect("PLUGGY_CLIENT_SECRET required");
    let item_id = std::env::var("PLUGGY_ITEM_ID").expect("PLUGGY_ITEM_ID required");
    let (_dir, db) = temp_db();

    // Create a BRL account + link it to one of the provider's accounts.
    // For the smoke we just run setup + sync and assert the JSON shape.
    run(&[
        "--db", &db, "pluggy", "setup",
        "--client-id", &client_id,
        "--client-secret", &client_secret,
        "--item-id", &item_id,
    ]);

    let out = run(&["--db", &db, "sync", "--provider", "pluggy"]);
    // With no accounts locally linked, sync returns imported:0 without error.
    assert!(
        out.status.success(),
        "sync failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data = &parse_stdout(&out)["data"];
    assert_eq!(data["provider"], "pluggy");
    // accounts_synced should be 0 since we didn't link any yet.
    assert_eq!(data["accounts_synced"], 0);
}
