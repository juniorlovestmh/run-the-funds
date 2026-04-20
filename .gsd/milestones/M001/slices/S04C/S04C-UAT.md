# S04C: Browser Connect flow + multi-bank support (Teller + Pluggy) — UAT

**Milestone:** M001
**Written:** 2026-04-19T23:57:03.636Z

Scope: replace the manual-token-paste step from S04/S04B with a browser-driven Connect flow for both Teller and Pluggy, and add multi-bank-per-provider storage so each `connect` call adds a new enrollment rather than overwriting the previous one. New `provider_connections` table, reusable local HTTP callback server, per-provider HTML templates that embed the vendor's Connect widget.

Automated coverage: 311 unit + 3 convert_demo + 1 import_demo + 11 sync_demo = **326 passing tests**, 5 ignored (live smokes), 0 failed.

Live-verified with real banks: **Capital One (5 accounts, 984 transactions) + Chase (524 transactions, via legacy pre-S04C enrollment) = 1,508 transactions in one sync call, 2-year backfill**.

## Test: `connections list` shows stored bank connections (no secrets)

```bash
rtf connections list --format json
```

**Expected:** JSON array of `{id, provider, external_id, institution_name, created_at, updated_at}`. Access tokens are never exposed.
**Result:** PASS — verified against real DB with 2 Teller enrollments (Chase Legacy + Capital One).

## Test: Legacy enrollment migration preserves existing data

**Scenario:** User had a single pre-S04C Teller access_token stored in `provider_credentials` (S04B shape).

**Expected:** Migration 006 moves the access_token into a new `provider_connections` row with `external_id = <the-legacy-token>` and `institution_name = "Legacy (re-run `rtf teller connect`)"`. Per-app data (cert+key) stays in `provider_credentials`. `rtf sync --provider teller` continues to work post-migration — all 524 existing Chase transactions round-trip.
**Result:** PASS — user's production DB migrated cleanly; sync worked immediately after the migration with no manual intervention.

## Test: `rtf teller setup` saves per-app credentials (app_id + cert + key)

```bash
rtf teller setup \
  --app-id app_pr9uabthpvbhp573su000 \
  --cert ~/Downloads/teller/certificate.pem \
  --key ~/Downloads/teller/private_key.pem
```

**Expected:** `{status:"ok", data:{message:"Teller app credentials saved (development tier (mTLS)); ..."}}`. Stored blob has `app_id`, `cert_pem`, `key_pem`, `environment: "development"`. No `access_token` (that's per-bank now).
**Result:** PASS — environment auto-detected from cert presence; partial cert args rejected; empty app_id rejected.

## Test: `rtf teller connect` end-to-end browser flow

```bash
rtf teller connect
```

**Expected:** Local HTTP server binds to a random port. Default browser opens to `http://127.0.0.1:<port>/`. Teller Connect widget loads and the user links a bank. On success, the widget POSTs the enrollment to `/success`, the CLI parses and stores it in `provider_connections`, prints `{status:"ok", data:{enrollment_id, institution_name, message}}`. Server shuts down.
**Result:** PASS — user linked Capital One end-to-end in a single browser session.

## Test: Multiple `teller connect` calls add multiple enrollments

**Scenario:** User had one enrollment (Chase Legacy); ran `teller connect` a second time for Capital One.

**Expected:** Two rows in `provider_connections` for `provider='teller'`. No overwrite. Same `external_id` re-connected would upsert; different `external_id` (different enrollment) adds a row.
**Result:** PASS — 2 rows present, both sync on `rtf sync --provider teller`.

## Test: `sync --provider teller` iterates all enrollments

```bash
rtf sync --provider teller --since 2024-04-19
```

**Expected:** Walks every row in `provider_connections[provider='teller']`, builds one `TellerAdapter` per enrollment, sums results into a single `ProviderSyncReport`.
**Result:** PASS — reported `{imported: 984, duplicates: 524, accounts_synced: 6, window_start: 2024-04-19, window_end: 2026-04-19}`. 984 fresh Capital One transactions across 5 accounts; 524 Chase transactions seen as duplicates via FITID dedup; 6 accounts covered in one call.

## Test: Per-account transaction distribution (real data)

| Account | Txns |
|---------|------|
| Capital One Venture | 932 |
| Chase Day Spending | 524 |
| Capital One Spark Cash Select | 37 |
| Capital One 360 Savings | 14 |
| Capital One 360 Checking | 1 |
| Capital One Quicksilver | 0 |

## Test: `rtf pluggy setup` saves per-app credentials only

```bash
rtf pluggy setup --client-id <ID> --client-secret <SECRET>
```

**Result:** PASS — unit + integration tested. Stored blob has only `client_id` + `client_secret`. Live verification deferred until user obtains Pluggy credentials.

## Test: `rtf pluggy connect` end-to-end browser flow

```bash
rtf pluggy connect
```

**Expected:** Mints an apiKey via `POST /auth`, then a Connect Token via `POST /connect_token`. Opens Pluggy Connect widget. On success, widget POSTs item payload; CLI parses `item.id` + `connector.name`, stores new `provider_connections` row.
**Result:** PENDING LIVE VERIFICATION — user hasn't obtained Pluggy credentials yet. Code path fully covered by unit tests.

## Test: Unified `rtf sync` runs every configured provider

```bash
rtf sync
```

**Expected:** `data.{simplefin, pluggy, teller, errors}`. Each provider slot independently Some/None; one provider's outage doesn't block others.
**Result:** PASS.

## Test: Full test suite

```bash
cargo test
```

**Result:** PASS — 326 tests, 5 ignored, 0 failed.

## Roadmap "Done" criteria coverage

| Criterion | Status |
|-----------|--------|
| `teller connect` opens browser + captures enrollment | ✅ Live (Capital One) |
| Multi-enrollment (N banks) | ✅ Live (Chase + Capital One) |
| `pluggy connect` mirrors the flow | ⏳ Code shipped; live pending |
| `sync --provider teller` iterates all enrollments | ✅ Live (984 + 524 across 2 enrollments) |
| `sync --provider pluggy` iterates all items | ⏳ Code shipped |
| `connections list` exposes metadata without secrets | ✅ Live |
| Legacy data auto-migrates to new schema | ✅ Live |
| No user-visible regressions from S04/S04B | ✅ Live (unified sync + Teller both fine) |

## Known limitations

- **Pluggy live verification pending.** Code path fully tested via unit tests; user flips it on once they have Pluggy credentials + have linked their first Brazilian bank via Connect.
- **Legacy enrollment row.** User still has one `provider_connections` row with `institution_name: "Legacy (re-run ...)"` using the old access_token as synthetic external_id. Harmless but noisy. Can be deleted via sqlite; no CLI command for this yet (filed as follow-up).
- **No `rtf connections remove` yet.** User can delete via `sqlite3` directly. Small follow-up.
- **Wise and Novo.** Not covered by Teller's institution list. Deferred per user decision: Wise QFX export works via S02 OFX importer; Novo needs a CSV importer (deferred to a later slice).
- **Credentials at rest still plaintext.** Unchanged from S04 SECURITY.md.
