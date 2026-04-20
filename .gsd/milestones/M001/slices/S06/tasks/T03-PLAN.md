---
estimated_steps: 1
estimated_files: 2
skills_used: []
---

# T03: MonarchAdapter — subprocess-based mmoney ingest

src/infrastructure/sync_adapter/monarch.rs: shells out to mmoney CLI (path configurable via MMONEY_BIN env, default /Users/sky/.local/bin/mmoney) via std::process::Command. Uses serde_json to parse stdout. Exposes fetch_accounts(), fetch_category_groups(), fetch_categories(), fetch_tags(), fetch_transactions(start_date, end_date). RemoteTransaction carries monarch_txn_id (used as external_id), account external_id, merchant, payee, amount, currency, date, monarch_category_id (external, resolved later), monarch_tag_ids (external). Paginates transactions with --limit 500 --offset N until < 500 returned. Clean error on missing mmoney ('run mmoney auth login first'). Trait abstraction (MmoneyRunner) so tests can inject canned JSON fixtures without subprocess.

## Inputs

- `mmoney CLI already installed at /Users/sky/.local/bin/mmoney`
- `Monarch JSON shape from monarch-cleanup/survey/*.json`

## Expected Output

- `src/infrastructure/sync_adapter/monarch.rs with MonarchAdapter + 5 fetch methods + 10+ tests`

## Verification

Unit tests with FakeMmoneyRunner returning canned JSON from survey fixtures. One #[ignore]d live smoke that fetches from real Monarch.
