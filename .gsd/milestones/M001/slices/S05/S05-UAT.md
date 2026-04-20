# S05: Categorization Engine — UAT

**Milestone:** M001
**Written:** 2026-04-20T11:05:56.371Z

# S05: Categorization Engine — UAT

**Milestone:** M001  
**Written:** 2026-04-20  
**DB under test:** `~/rtf.db` (6 accounts, 1,508 transactions)

## Test: migrations auto-apply (007 rules, 008 transaction_splits)
First invocation migrates schema v7 → v8, creates `transaction_splits`. **PASS.**

## Test: category-groups + categories CRUD
4 groups, 15 categories persisted; `--format json` and `--format table` render. **PASS.**

## Test: rules add/list with priority
17 rules stored; priority ordering visible (pri 100 before pri 50). **PASS.**

## Test: rule-based categorization, dry-run then real
dry_run categorized=493, skipped=1015; apply matches. Priority precedence verified (Google One pri 100 before Google generic pri 50). UPSERT-on-id fix verified by re-running without unique-index collision. **PASS.**

## Test: transfer detection
8 pairs created, 16 rows got transfer_pair_id, 414 candidates skipped. **PASS.**

## Test: transaction split
$262.46 Casas Guanabara → $200 Groceries + $62.46 Shopping; parent category_id cleared. **PASS.**

## Test: spending rollup — table + JSON, date window, transfer exclusion
Totals by currency, by category (most-negative first), by group, uncategorized bucket. `transfers_excluded: 6` in YTD window. `--include-transfers` flips filter. Splits expand into line items. **PASS.**

## Test: full test suite
`cargo test` → 406 passed, 6 ignored, 0 failed.

## Known limitations
- Importer sign convention inconsistent — both income and spending rows positive. S06 fixes this via Monarch ingest.
- No cross-currency aggregation yet — S07 territory.
- No split query/delete CLI.

## Backup
Pre-S05 backup at `~/rtf.db.pre-S05-20260419-215841.bak`.
