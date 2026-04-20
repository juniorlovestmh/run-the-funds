# Changelog

All notable changes to **Run The Funds** (`rtf`) are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-04-20

Initial public release. Milestone **M001 "Core Financial Engine"** — slices
S01 through S06 — shipped. Slices S07–S09 (investments, goals/debt/health,
agent-ready query + export) are planned but not included in this release.

### Added

- **Domain model** (S01): accounts, transactions, categories, category
  groups, rules, transaction splits, tags, transfer pairs, exchange rates,
  persons. DDD layout with repository traits at the boundary. Money is
  modeled as `{ amount: Decimal, currency: CurrencyCode }` — `f64` is
  never used for money.
- **Transaction import pipeline** (S02): OFX / QFX / CSV. Custom SGML
  tokenizer for OFX 1.x, `quick-xml` for OFX 2.x, hybrid-dialect support
  for banks that emit XML-style closing tags over an SGML header (Nubank).
- **Multi-currency PTAX** (S03): lazy-fetches USD↔BRL rates from Banco
  Central do Brasil's public endpoint, caches per-date, walks back on
  weekends/holidays (up to 5 calendar days). Every BRL transaction gets
  annotated with its USD equivalent on `transactions list`.
- **Bank-sync adapters** (S04, S04B, S04C, S04D): SimpleFIN, Pluggy
  (with browser-based Connect widget), Teller (with mTLS client-cert
  support for Development and Production tiers), and the per-connection
  remove CLI. These are retained in-tree but dormant after S06; the
  Monarch adapter is the recommended path.
- **Categorization engine** (S05): user-defined rules across three match
  kinds (substring, regex, amount-DSL like `>100` / `<=50`), priority-
  ordered application with per-rule fire counts in the report;
  cross-account same-currency transfer-pair detection within ±3 days;
  transaction splits that expand cleanly into spending rollups.
- **Spending reports** (S05): by category, group, account, or currency,
  with optional date windows, default transfer exclusion, and table or
  JSON output.
- **Tags domain** (S06): first-class tags + `transaction_tags` junction
  with CASCADE deletes in both directions; `rtf tags list` (read-only —
  Monarch is the source of truth).
- **Monarch Money adapter** (S06): shells out to the
  [`mmoney-cli`](https://github.com/theFong/mmoney-cli) tool via a
  `MmoneyRunner` trait; `rtf sync --provider monarch` performs a
  3-phase taxonomy → accounts → transactions import with full
  `external_id` upsert resolution. Idempotent re-runs.

### CLI surface

```
rtf accounts create|list|link
rtf transactions import|list|get
rtf transaction-split
rtf category-groups create|list
rtf categories create|list
rtf tags list
rtf rules add|list|remove
rtf categorize [--dry-run --reset --account-id --detect-transfers]
rtf spending [--from --to --account-id --include-transfers --format]
rtf convert <amount> <from> --to <currency> --date YYYY-MM-DD
rtf sync --provider <simplefin|pluggy|teller|monarch> [--since YYYY-MM-DD]
rtf simplefin setup
rtf pluggy setup|connect
rtf teller setup|connect
rtf connections list|remove
```

Every subcommand accepts `--format json` when it returns data; every
command returns a stable `{status, data}` / `{status, message}` envelope.

### Verified

- `cargo test` — 441 passing / 6 ignored / 0 failed.
- Live Monarch sync against a 19-account, 2,185-transaction production
  Monarch account: 11-second first run with exact count parity against
  `mmoney` direct pagination; zero-drift idempotent re-run.
- PTAX integration verified against Banco Central do Brasil's live
  endpoint across weekend-walk-back cases.

### Licensing

This release is licensed under the
[GNU Affero General Public License, version 3](./LICENSE).

[0.1.0]: https://github.com/juniorlovestmh/run-the-funds/releases/tag/v0.1.0
