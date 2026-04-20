---
estimated_steps: 25
estimated_files: 8
skills_used: []
---

# T02: CategorizationService + `rtf categorize` with dry-run + reset + per-rule fire counts

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

## Inputs

- `src/domain/rules/ (T01)`
- `src/domain/transaction/repository.rs (existing)`

## Expected Output

- `CategorizationService with dry-run + reset + per-rule fire counts`
- `find_uncategorized + clear_categories on TransactionRepository`
- `rtf categorize CLI`
- `~10 unit tests + 1 integration`

## Verification

cargo test -- application::categorization_service && cargo test -- cli::categorize
