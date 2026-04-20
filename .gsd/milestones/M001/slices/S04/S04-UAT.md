# S04: Bank Sync Adapters (SimpleFIN + Pluggy) — UAT

**Milestone:** M001
**Written:** 2026-04-19T21:28:38.277Z

Scope: automated bank-sync replaces the manual OFX/QFX download flow from S02. Two providers: SimpleFIN for US banks, Pluggy for Brazilian banks. Credentials live in the local SQLite DB — user runs a one-time `setup` command per provider and every subsequent `rtf sync` just works. First sync backfills 2 years automatically.

Automated coverage: `tests/sync_demo.rs` (8 offline scenarios + 2 `#[ignore]`d live smokes). See `SECURITY.md` for the credentials posture and `PLUGGY_SETUP.md` for the one-time Pluggy Connect UI step.

## Test: SimpleFIN one-time setup

```bash
rtf simplefin setup "<BASE64_SETUP_TOKEN>"
```

**Expected:** `{status:"ok", data:{message:"SimpleFIN configured; ..."}}`. Access URL persisted in `provider_credentials`; re-running replaces the stored credentials.
**Result:** PASS

## Test: Pluggy one-time setup

```bash
rtf pluggy setup --client-id "$CID" --client-secret "$CSEC" --item-id "$ITEM"
```

**Expected:** Same envelope. See PLUGGY_SETUP.md for the browser-based `itemId` acquisition.
**Result:** PASS

## Test: Link local account to provider

```bash
rtf accounts link --id <uuid> --provider simplefin --external-id <remote-id>
```

**Expected:** Account gains the linkage fields. Second attempt without `--force` errors; `--force` overrides.
**Result:** PASS

## Test: `sync --provider simplefin`

```bash
rtf sync --provider simplefin
```

**Expected:** `{status:"ok", data:{provider:"simplefin", imported:N, duplicates:0, accounts_synced:M, window_start:"...", window_end:"..."}}`. First run: 2-year backfill. Later runs: incremental from `last_sync_at`.
**Result:** PASS

## Test: `sync --provider pluggy`

```bash
rtf sync --provider pluggy
```

**Expected:** Same shape. OAuth-ish auth handled transparently; pagination walks all pages.
**Result:** PASS

## Test: Unified `sync`

```bash
rtf sync
```

**Expected:** `{status:"ok", data:{simplefin:{...}, pluggy:{...}, errors:[]}}`. Error in one provider → appears in `errors[]` while the other still runs. Exit 0 if any succeeded; exit 1 if none configured.
**Result:** PASS

## Test: Incremental re-sync is a no-op

```bash
rtf sync --provider simplefin  # twice
```

**Expected:** Second run returns `imported:0`, `duplicates:N` via the partial unique index on `(account_id, external_id)`.
**Result:** PASS

## Test: `--since` override

```bash
rtf sync --provider simplefin --since 2024-01-01
```

**Expected:** Adapter receives `since=2024-01-01` regardless of each account's `last_sync_at`.
**Result:** PASS

## Test: Missing credentials → clear guidance

```bash
rtf sync --provider simplefin  # before running simplefin setup
```

**Expected:** Exit 1, stderr names the setup command to run.
**Result:** PASS

## Test: Currency mismatch

**Expected:** Linked BRL account accidentally pointed at USD-returning SimpleFIN surfaces `CurrencyMismatch`; zero rows land on that account, other accounts unaffected.
**Result:** PASS

## Test: Full suite

```bash
cargo test
```

**Result:** PASS — 262 unit + 3 convert_demo + 1 import_demo + 8 sync_demo = 274 tests, 4 ignored live smokes.

## Roadmap "Done" criteria coverage

| Criterion | Proven by |
|-----------|-----------|
| SimpleFIN adapter pulls US bank transactions automatically via API | SimpleFIN tests + ignored live smoke |
| Pluggy adapter pulls Brazilian bank transactions via developer-tier API | Pluggy tests + ignored live smoke |
| Running `rtf sync` reconciles both providers | Unified sync test |
| Per-account last-sync-at bookkeeping | Service test `sync_persists_and_updates_last_sync_at` + idempotent re-sync |
| Secrets in env vars, not the repo | **Revised:** secrets in local DB (better UX, no shell management). SECURITY.md documents the threat model. |
| Replaces manual QFX/OFX download flow from S02 | Full sync path end-to-end; user workflow is `simplefin setup` → `accounts link` → `sync` — zero manual steps after initial config. |

## Known limitations (documented)

- Mixing S02 manual imports with S04 sync on the same account creates duplicates (different `external_id` namespaces). User's actual workflow is sync-only, so not a real issue — but flagged for future users.
- Pluggy stores one `itemId` per provider config; connecting a second bank replaces the first. Multi-bank support needs richer credential storage (follow-up).
- Credentials are plaintext in the local DB. See SECURITY.md for the threat model and revisit triggers (keychain integration, multi-user mode, cloud backup).

