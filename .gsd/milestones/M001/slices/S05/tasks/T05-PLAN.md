---
estimated_steps: 21
estimated_files: 7
skills_used: []
---

# T05: `rtf spending` report + categories/category-groups CLI

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

## Inputs

- `T01-T04 outputs`
- `src/domain/category/ + src/application/currency_converter.rs (S03)`

## Expected Output

- `SpendingService.compute with category/group/account rollups + transfer exclusion`
- `rtf spending CLI with date + format + currency flags`
- `rtf categories + rtf category-groups CLI`
- `~15 unit tests + 1 integration`

## Verification

cargo test -- application::spending_service && cargo test -- cli::spending && cargo test -- cli::categories
