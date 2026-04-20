---
estimated_steps: 10
estimated_files: 1
skills_used: []
---

# T05: Live-test both providers with multiple banks + S04C-UAT.md + commit

Manual verification + docs + single rollup commit.

**Live flow** (user-driven):
1. `rtf teller setup --app-id <USER_APP_ID> --cert ~/Downloads/teller/certificate.pem --key ~/Downloads/teller/private_key.pem`
2. `rtf teller connect` → browser opens, user links Chase (or another bank), success callback fires, enrollment persisted.
3. `rtf teller connect` → user links a DIFFERENT bank → second enrollment row.
4. `rtf connections list` → both enrollments listed with institution names.
5. `rtf sync --provider teller` → aggregated report across both enrollments; per-account transactions persisted for each linked account.
6. Same flow for Pluggy after setup: `rtf pluggy connect` → user links their Brazilian bank → `rtf sync --provider pluggy` works.

**S04C-UAT.md** mirrors the S04B shape. Scenarios: setup (per-app creds), connect (browser flow), multi-bank (N enrollments), connections list, sync aggregation, missing-creds errors, timeout handling. Roadmap coverage table. Notes the user flow from zero to synced-multi-bank.

**Commit**: one rollup covering T01–T05. Message notes the mid-slice clarification that Pluggy ships alongside Teller and the new `provider_connections` table is the foundation for multi-bank across both.

## Inputs

- `target/release/rtf (rebuilt after T04)`
- `user's Teller cert/key/app_id`
- `user's Pluggy client_id/secret`
- `.gsd/milestones/M001/slices/S04B/S04B-UAT.md (template)`

## Expected Output

- `Multi-bank live-verified across Teller + Pluggy`
- `S04C-UAT.md`
- `Single rollup commit`

## Verification

cargo test && live-test the connect flow for both providers
