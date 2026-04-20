---
estimated_steps: 17
estimated_files: 2
skills_used: []
---

# T06: End-to-end demo on real data + S05-UAT.md + commit

Use the user's 1,508 real transactions to seed rules, run categorize + detect-transfers + spending, verify the numbers make sense.

**Demo flow** (`tests/categorize_demo.rs` or a new integration test):
1. Seed 8-10 realistic rules based on the ad-hoc SQL findings from earlier:
   - Student loan: match_field=payee, pattern="ADVS ED SERV", category=Debt
   - Payroll (Aptitude 8): pattern="APTITUDE 8 PAYROLL", category=Income
   - T-Mobile: pattern="METRO BY T-MOBILE", category=Subscriptions
   - Google One: pattern="GOOGLE ONE", category=Subscriptions
   - Kraken: pattern="KRAKEN EXCHANGE", category=Investments
   - Claude.ai: pattern="CLAUDE.AI", category=Subscriptions
   - Capital One payments: pattern="CAPITAL ONE CRCARDPMT", category=Transfer
   - Wise: pattern="WISE INC", category=Transfer
2. Run `rtf categorize`.
3. Run `rtf categorize --detect-transfers` — verify Wise + Capital One payment pairs.
4. Run `rtf spending --by-category --from 2025-01-01 --to 2025-12-31`.
5. Assert non-zero categorized counts; spending > 0; transfer_volume > 0; uncategorized < total.

**S05-UAT.md** mirrors the S04C shape: full scenarios for rules CRUD, categorize happy + dry-run + reset, splits, transfer detection, spending, categories/groups. Roadmap coverage table.

**Commit**: single rollup commit covering T01-T06.

## Inputs

- `target/release/rtf (rebuilt after T05)`
- `user's real DB at ~/rtf.db`
- `ad-hoc SQL findings from S04C post-mortem (payroll, subscriptions, transfers)`

## Expected Output

- `Integration test covering the full categorize→detect→spending flow`
- `S05-UAT.md with real spending breakdown`
- `Single rollup commit`

## Verification

cargo test --test categorize_demo && cargo test && live: rtf categorize + rtf spending against ~/rtf.db
