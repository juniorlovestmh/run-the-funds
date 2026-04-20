# S04B: Teller Adapter (third bank-sync provider) — UAT

**Milestone:** M001
**Written:** 2026-04-19T22:33:35.038Z

Scope: Teller added as a third bank-sync provider alongside SimpleFIN and Pluggy. One adapter, one setup CLI, one dispatch branch. Live-verified against the user's real Chase account via Teller's Developer tier (mTLS client cert + key).

## Test: `teller setup` with mTLS cert

```bash
rtf teller setup \
  --access-token "token_..." \
  --cert ~/Downloads/teller/certificate.pem \
  --key ~/Downloads/teller/private_key.pem
```

**Expected:** `{status:"ok", data:{message:"Teller configured with mTLS (Development/Production tier); ..."}}`. Cert + key PEM contents embedded in the stored credentials; source files can be deleted.
**Result:** PASS

## Test: `teller setup` Sandbox (no cert)

```bash
rtf teller setup --access-token "<sandbox-token>"
```

**Expected:** Message reads *"Teller configured (Sandbox / Bearer-only)"*.
**Result:** PASS (unit test).

## Test: Link local account to Teller

```bash
rtf accounts link --id <local-uuid> --provider teller --external-id acc_pr9ubcf1l6jujrg5m8000
```

**Expected:** `external_provider: "teller"`, `external_account_id` populated. Provider string "teller" accepted at the CLI boundary.
**Result:** PASS

## Test: Live sync pulls real Chase data via mTLS

```bash
rtf sync --provider teller
```

**Result:** PASS — 524 real Chase transactions pulled in ~2.8s. `window_start: 2024-04-19`, `window_end: 2026-04-19` (2-year backfill honored). `accounts_synced: 1`, `last_sync_at` advanced.

## Test: Immediate re-sync is a true no-op

```bash
rtf sync --provider teller
```

**Result:** PASS — `imported:0, duplicates:0`. `last_sync_at = today` means the adapter asks Teller for nothing new.

## Test: Forced full re-pull exercises dedup

```bash
rtf sync --provider teller --since 2024-04-19
```

**Result:** PASS — `imported:0, duplicates:524`. Migration 002's partial unique index rejects every FITID.

## Test: Multi-currency enrichment on real data

```bash
rtf transactions list --account-id <chase-uuid> --format json
```

**Result:** PASS — 524/524 rows carry `rate_status: "same_currency"` (USD account). Spot-checked first row: 2026-04-18, Bull & Bowtie, -$10.00, `external_id` starts with `txn_`, `payee` from Teller's `details.counterparty.name`.

## Test: Unified sync with 3 providers + error isolation

```bash
rtf sync
```

**Expected:** `data.{simplefin, pluggy, teller, errors}`. One provider failing doesn't block others.
**Result:** PASS (structure unit-tested; live unified not run since SimpleFIN billing is unresolved and Pluggy isn't configured yet).

## Test: Full test suite

```bash
cargo test
```

**Result:** PASS — **293 tests**, 5 ignored, 0 failed.

## Rollup fixes in this slice

- **HTTP body on 4xx/5xx** (from S04 live-test): `UreqHttpClient::collapse` now preserves the response body when ureq returns `Error::Status(code, response)`. Changes `"status code 402"` → `"HTTP 402: {\"errors\":[\"Payment required.\"]}"`.
- **mTLS client auth**: `UreqHttpClient::with_mtls(cert_pem, key_pem)` with rustls + ring crypto provider + webpki-roots.
- **Teller auth fix**: HTTP Basic with token-as-username + empty password (not Bearer). Caught on first live-sync.

## Roadmap "Done" criteria coverage

| Criterion | Proven by |
|-----------|-----------|
| Teller configured as a bank-sync provider alongside SimpleFIN + Pluggy | All tests above |
| mTLS for Developer/Production tiers | Live 524-row sync with user's real cert |
| 2-year backfill | Live window_start=2024-04-19 |
| FITID dedup | Re-run with `--since 2024-04-19` reported 524 duplicates |
| Zero env-var management | Credentials (incl. cert+key PEM contents) in `provider_credentials` |
| No service-layer changes | `SyncService` untouched |

## Known limitations

- Enrollment access token is obtained externally (user's own app or curl against `/connect_token`) and pasted into `teller setup`. `rtf teller connect` — a browser-launching callback-server flow — is a future slice. Biggest remaining UX gap.
- One Teller enrollment per configuration (same as Pluggy single-itemId). Multi-enrollment needs richer credential storage.
- Credentials (incl. cert+key PEM contents) remain plaintext in the local DB. See SECURITY.md from S04 for posture.

