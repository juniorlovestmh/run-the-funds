# S05: Categorization Engine

**Goal:** Turn the 1,500+ raw transactions into categorized spending insight. Rule-based auto-categorization (pattern → category), transaction splits (one Costco charge spans groceries + household), transfer detection (Wise USD→BRL, Chase→Capital One auto-paired and excluded from spending), and a `rtf spending` report that aggregates by category / group / account / person with optional date windows and transfer exclusion. Works on the real data that's already sitting in the DB.
**Demo:** After importing transactions, rtf categorize applies rules and shows categorization summary. Split a Costco transaction into groceries + household. Internal transfer between checking accounts detected and excluded from spending report.

## Must-Haves

- `rtf rules add --name "Student Loan" --match-payee "ADVS ED SERV" --category <cat-id> [--priority 100]` persists a rule. `rules list` / `rules remove` round-trip.
- `rtf categorize` walks every uncategorized transaction, applies rules by priority (highest first), sets `category_id` on matches, skips non-matches. Output: `{status:"ok", data:{categorized: N, skipped: M, rules_fired: [{rule_id, count}]}}`.
- `rtf categorize --dry-run` reports what WOULD change without writing.
- `rtf categorize --reset` clears all `category_id`s first, then runs rules.
- `rtf transactions split <txn-id> --category <cat> --amount <x> --category <cat2> --amount <y>` creates N split rows for one transaction. The original transaction's `category_id` gets cleared (it's represented as splits). Splits must sum to the transaction's absolute amount (within 1 cent tolerance).
- `rtf categorize --detect-transfers` finds debit/credit pairs (same absolute amount, dates within ±3 days, different accounts), sets `transfer_pair_id` on both, reports pair count.
- `rtf spending [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--by-category] [--by-group] [--by-account] [--exclude-transfers] [--format json|table]` produces a rollup.
- Spending excludes transfers by default; `--include-transfers` overrides. Excludes splits' parent transactions when splits exist.
- `rtf categories create --name <name> --group <group-id>`, `rtf categories list`, `rtf category-groups create --name`, `rtf category-groups list` round-trip.
- `cargo test` passes. Real-data validation: seed a handful of rules against the user's actual transactions, run categorize, verify spending report reflects genuine patterns (student loans, subscriptions, payroll, transfers).

## Proof Level

- This slice proves: contract — rule-based categorization runs against real data; splits and transfer detection cover the roadmap's Costco and internal-transfer scenarios; `rtf spending` produces meaningful aggregates. Downstream slices (S06 household attribution, S07 goals/debt/health, S08 agent query layer) can assume transactions are categorized and transfers are marked.

## Integration Closure

- Upstream surfaces consumed: `Transaction { category_id, transfer_pair_id }` domain fields (from S01 scaffolding, unused until now); `TransactionRepository::find_by_account` / `find_by_date_range`; `Category` + `CategoryGroup` domain types + SQLite repos (from S01, mostly dormant).
- New wiring introduced: migration 007 (rules table) + migration 008 (splits table); `Rule` domain + repo + `CategorizationService`; split domain + repo + service; transfer-detection algorithm; `SpendingReport` domain type; CLI commands (`rules`, `categorize`, `transactions split`, `spending`, `categories`, `category-groups`).
- What remains: S06 (household + per-person attribution — adds a `person_id` dimension to spending), S07 (goals/debt/health — consumes categorized spending), S08 (agent-ready JSON queries + export).

## Verification

- `categorize` reports per-rule fire counts so operators see which rules are doing work and which aren't firing.
- `--dry-run` lets operators preview impact before committing.
- `spending` output carries subtotals + grand total + category coverage (% of transactions categorized).
- Transfer detection reports pair count + any unmatched same-amount candidates (useful signal for "this looks like a transfer but didn't match").
- Rule-match errors (e.g., invalid regex) surface as `DomainError::Validation` with the pattern + failure reason.

## Tasks

- [x] **T01: Rules table + domain + SQLite repo + `rtf rules` CLI (add/list/remove)** `est:2h`
  **Migration 007** (`migrations/007_rules.sql`):
```sql
CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    match_field TEXT NOT NULL,         -- 'payee' | 'description' | 'amount'
    match_pattern TEXT NOT NULL,       -- case-insensitive substring for payee/description; '>N'/'<N'/'=N' for amount
    category_id TEXT NOT NULL REFERENCES categories(id),
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rules_priority ON rules(priority DESC);
```

**Domain** (`src/domain/rules/`):
- `Rule { id, name, match_field: RuleField, match_pattern: String, category_id, priority, created_at, updated_at }`.
- `RuleField` enum: `Payee`, `Description`, `Amount` (stored as lowercase string).
- `RuleRepository` trait: `save`, `find_by_id`, `find_all` (ordered by priority DESC), `delete`.

**SQLite impl** (`src/infrastructure/storage/rules_repo.rs`): straightforward mirror of the existing repo pattern.

**CLI** (`src/cli/rules.rs`):
- `rtf rules add --name <n> --match-field <payee|description|amount> --pattern <p> --category-id <cat> [--priority <N>]`
- `rtf rules list [--format json|table]`
- `rtf rules remove --id <uuid>`
- All envelopes follow existing patterns.

**Tests:**
- Domain: Rule validation (empty name rejected, unknown field rejected, empty pattern rejected).
- Repo: save+find_by_id, find_all ordering, delete, upsert-by-id.
- CLI: rules list empty, add happy path, list shows added, remove works, not-found errors.
  - Files: `migrations/007_rules.sql`, `src/infrastructure/storage/migrations.rs`, `src/infrastructure/storage/rules_repo.rs`, `src/infrastructure/storage/mod.rs`, `src/infrastructure/storage/database.rs`, `src/domain/rules/mod.rs`, `src/domain/rules/rule.rs`, `src/domain/rules/repository.rs`, `src/domain/mod.rs`, `src/cli/rules.rs`, `src/cli/mod.rs`, `src/main.rs`
  - Verify: cargo test -- domain::rules && cargo test -- infrastructure::storage::rules_repo && cargo test -- cli::rules

- [x] **T02: CategorizationService + `rtf categorize` with dry-run + reset + per-rule fire counts** `est:3h`
  **Service** (`src/application/categorization_service.rs`):
- `CategorizationService` generic over `RuleRepository` + `TransactionRepository`.
- `categorize(options: CategorizeOptions) -> Result<CategorizeReport, DomainError>`.
- `CategorizeOptions { dry_run: bool, reset: bool, account_id: Option<String> }`.
- Flow:
  1. If `reset`, UPDATE transactions SET category_id = NULL WHERE ... (scoped to account_id if provided).
  2. Load all rules ordered by priority DESC.
  3. Walk uncategorized transactions (paginated by account for memory safety).
  4. For each transaction, find FIRST matching rule (rules with higher priority win ties).
  5. If `dry_run`, accumulate the would-be updates; else apply via repo.save.
  6. Track per-rule fire counts.
- Matching logic:
  - `match_field == "payee"` / `"description"`: case-insensitive substring match.
  - `match_field == "amount"`: parse pattern as `>N` / `<N` / `=N` / `>=N` / `<=N` where N is a Decimal.
- `CategorizeReport { categorized: usize, skipped: usize, reset: usize, rules_fired: Vec<{rule_id, rule_name, count}> }`. Serialize.

**CLI** (`src/cli/categorize.rs`):
- `rtf categorize [--dry-run] [--reset] [--account-id <id>] [--format json|table]`
- Prints report. Exit 0 even if `categorized == 0` (not an error).

**TransactionRepository extensions:**
- `find_uncategorized(account_id: Option<&str>) -> Vec<Transaction>`.
- `clear_categories(account_id: Option<&str>) -> usize` (returns count cleared).
- Both straightforward SQL. Add to trait + SQLite impl + tests.

**Tests:**
- Service: no rules → nothing categorized; one rule matches all → all categorized; priority ordering (rule A priority 100 matches, rule B priority 50 also matches → A wins); dry-run doesn't write; reset clears then re-runs; amount pattern parsing (>100, <=5, =0).
- CLI: smoke test via integration test with seeded rules.
  - Files: `src/application/categorization_service.rs`, `src/application/mod.rs`, `src/domain/transaction/repository.rs`, `src/infrastructure/storage/transaction_repo.rs`, `src/cli/categorize.rs`, `src/cli/mod.rs`, `src/main.rs`, `tests/sync_demo.rs`
  - Verify: cargo test -- application::categorization_service && cargo test -- cli::categorize

- [x] **T03: Transaction splits: migration 008 + domain + repo + `rtf transactions split` CLI** `est:3h`
  **Migration 008** (`migrations/008_transaction_splits.sql`):
```sql
CREATE TABLE IF NOT EXISTS transaction_splits (
    id TEXT PRIMARY KEY,
    transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id),
    amount_value TEXT NOT NULL,
    amount_currency TEXT NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_transaction_splits_txn ON transaction_splits(transaction_id);
```

**Domain** (`src/domain/transaction/split.rs`):
- `TransactionSplit { id, transaction_id, category_id, amount: Money, notes: Option<String>, created_at }`.
- `TransactionSplitRepository` trait: `save`, `find_by_transaction(txn_id) -> Vec`, `delete_by_transaction(txn_id)`, `delete(id)`.

**SQLite impl** (`src/infrastructure/storage/transaction_split_repo.rs`).

**SplitService** (`src/application/split_service.rs`):
- `split_transaction(txn_id: &str, splits: Vec<(CategoryId, Money, Option<Notes>)>) -> Result<Vec<TransactionSplit>, DomainError>`.
- Flow:
  1. Load the transaction. Error if missing.
  2. Validate: sum of split absolute amounts equals transaction absolute amount (±1 cent tolerance for rounding).
  3. Validate: all splits share the transaction's currency.
  4. In a transaction: delete any existing splits for this txn_id; insert the new ones; set transaction.category_id = NULL (it's now represented by splits).
- `delete_splits(txn_id) -> Result<(), DomainError>` — unwind splits, restore behavior.

**CLI** (`src/cli/transactions.rs` extended):
- `rtf transactions split <txn-id> --split <cat-id>:<amount>[:<notes>] [--split ...]`
- Parses each `--split` arg as `cat_id:amount[:notes]` (colon-separated).
- Validates at least 2 splits (single-split makes no sense).
- Prints the resulting splits envelope.

**List command update:** `rtf transactions list --format json` gains a `splits` field on each row (empty array when no splits).

**Tests:**
- Domain: split validates non-zero amount, currency matches txn.
- Service: happy path (2-way split); sum-mismatch rejected; currency-mismatch rejected; unknown txn_id rejected; re-splitting an already-split txn replaces old splits.
- CLI: split happy path via integration test.
  - Files: `migrations/008_transaction_splits.sql`, `src/infrastructure/storage/migrations.rs`, `src/infrastructure/storage/transaction_split_repo.rs`, `src/infrastructure/storage/mod.rs`, `src/domain/transaction/split.rs`, `src/domain/transaction/mod.rs`, `src/application/split_service.rs`, `src/application/mod.rs`, `src/cli/transactions.rs`, `src/cli/mod.rs`, `src/main.rs`, `tests/sync_demo.rs`
  - Verify: cargo test -- domain::transaction::split && cargo test -- application::split_service && cargo test -- cli::transactions

- [x] **T04: Transfer detection: algorithm + `rtf categorize --detect-transfers`** `est:2h`
  Pair up debit/credit transactions across accounts that look like internal transfers (e.g., Wise USD→BRL, Chase→Capital One). Sets `transfer_pair_id` on both sides.

**Algorithm** (in `CategorizationService::detect_transfers`):
1. Load all transactions with `category_id IS NULL AND transfer_pair_id IS NULL` (don't re-pair).
2. Group by absolute amount. For each group with ≥2 entries spanning multiple accounts:
   - Sort by date.
   - Greedy pair: for each debit, find the first credit within ±3 days from a different account; mark them paired.
   - If multiple candidates, prefer the closest date.
3. For each pair: generate a shared UUID, set both transactions' `transfer_pair_id` to it via repo.save.
4. Return `TransferPairingReport { pairs_created: N, candidates_skipped: M }` (skipped = same-amount groups with odd count or all-one-account).

**CLI flag:** `rtf categorize --detect-transfers` runs this step instead of (or in addition to) rule-based categorization. Combine flags: `--detect-transfers --dry-run` is valid.

**Transaction repository additions:**
- `find_untagged() -> Vec<Transaction>` — no category_id + no transfer_pair_id. Used by detection.
- `update_transfer_pair_id(txn_id: &str, pair_id: &str) -> ()`.

**Tests:**
- Simple pair: $500 debit on Chase on 2026-04-10 + $500 credit on Capital One on 2026-04-10 → one pair created.
- Date tolerance: ±3 days works (pair), ±4 days doesn't (no pair).
- Skip same-account: $500 debit + $500 credit both on Chase → not paired.
- Odd count: three $500 txns → 2 paired, 1 unpaired.
- Already-paired txns ignored on re-run (idempotent).
- Currency match: only pair same-currency txns (a $500 and a BRL 500 don't pair).
  - Files: `src/application/categorization_service.rs`, `src/domain/transaction/repository.rs`, `src/infrastructure/storage/transaction_repo.rs`, `src/cli/categorize.rs`, `tests/sync_demo.rs`
  - Verify: cargo test -- application::categorization_service::transfer_detection

- [x] **T05: `rtf spending` report + categories/category-groups CLI** `est:3h`
  Turn categorized transactions into readable aggregates.

**SpendingService** (`src/application/spending_service.rs`):
- `SpendingReport { total: Money, by_category: Vec<CategoryLine>, by_group: Vec<GroupLine>, by_account: Vec<AccountLine>, uncategorized: Money, transfer_volume: Money, window_start, window_end }`.
- `CategoryLine { category_id, category_name, group_name, amount: Money, transaction_count: usize }`.
- Method `compute(opts: SpendingOptions) -> Result<SpendingReport, DomainError>`.
- `SpendingOptions { from: Option<NaiveDate>, to: Option<NaiveDate>, exclude_transfers: bool, currency: CurrencyCode }`.
- Flow: load txns in window; for each txn (or split line when splits exist), add to category/group/account buckets; skip transfers unless `exclude_transfers == false`; convert to reporting currency via S03's CurrencyConverter.

**CLI** (`src/cli/spending.rs`):
- `rtf spending [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--by-category] [--by-group] [--by-account] [--include-transfers] [--currency USD|BRL] [--format json|table]`
- Default: `--by-category`, `--exclude-transfers`, `--currency USD`, window = last 30 days.
- Table output: category name + amount, sorted descending, with uncategorized + transfer-volume footer rows.

**Categories + CategoryGroups CLI** (`src/cli/categories.rs`):
- `rtf categories list [--format]`
- `rtf categories create --name <n> --group-id <g>`
- `rtf category-groups list [--format]`
- `rtf category-groups create --name <n>`
- `rtf categories remove --id <uuid>` (fails if any rule or transaction references it).

**Category + CategoryGroup domain:** these already exist from S01 but CLI hasn't been wired. Add the CLI surface now.

**Tests:**
- Spending: empty DB → zero totals; one category with 3 txns → correct sum; split txn counted by split lines not parent; excluded transfers subtract out; date window filters; currency conversion integrated.
- Categories CLI: create, list, duplicate name rejected (via repo unique index if applicable).
  - Files: `src/application/spending_service.rs`, `src/application/mod.rs`, `src/cli/spending.rs`, `src/cli/categories.rs`, `src/cli/mod.rs`, `src/main.rs`, `tests/sync_demo.rs`
  - Verify: cargo test -- application::spending_service && cargo test -- cli::spending && cargo test -- cli::categories

- [x] **T06: End-to-end demo on real data + S05-UAT.md + commit** `est:2h`
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
  - Files: `tests/categorize_demo.rs`, `.gsd/milestones/M001/slices/S05/S05-UAT.md`
  - Verify: cargo test --test categorize_demo && cargo test && live: rtf categorize + rtf spending against ~/rtf.db

## Files Likely Touched

- migrations/007_rules.sql
- src/infrastructure/storage/migrations.rs
- src/infrastructure/storage/rules_repo.rs
- src/infrastructure/storage/mod.rs
- src/infrastructure/storage/database.rs
- src/domain/rules/mod.rs
- src/domain/rules/rule.rs
- src/domain/rules/repository.rs
- src/domain/mod.rs
- src/cli/rules.rs
- src/cli/mod.rs
- src/main.rs
- src/application/categorization_service.rs
- src/application/mod.rs
- src/domain/transaction/repository.rs
- src/infrastructure/storage/transaction_repo.rs
- src/cli/categorize.rs
- tests/sync_demo.rs
- migrations/008_transaction_splits.sql
- src/infrastructure/storage/transaction_split_repo.rs
- src/domain/transaction/split.rs
- src/domain/transaction/mod.rs
- src/application/split_service.rs
- src/cli/transactions.rs
- src/application/spending_service.rs
- src/cli/spending.rs
- src/cli/categories.rs
- tests/categorize_demo.rs
- .gsd/milestones/M001/slices/S05/S05-UAT.md
