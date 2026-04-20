---
estimated_steps: 33
estimated_files: 3
skills_used: []
---

# T03: Wire import pipeline: account lookup, currency guard, ImportReport, CLI

Glue the adapter and service together so the CLI gets a real import flow and reports dedup counts.

Changes to `TransactionService`:
- Parameterize on both `TransactionRepository` and `AccountRepository` so it can look up the destination account.
- New method:
  ```rust
  pub fn import_from<I: Importer>(
      &self,
      importer: &I,
      source: &Path,
      account_id: &str,
  ) -> Result<ImportReport, DomainError>
  ```
  Flow:
  1. Look up the account — `DomainError::NotFound` if missing.
  2. Call `importer.import(source, account_id)`. Wrap non-domain `ImportError` variants into a new `DomainError::Import(String)` carrier.
  3. For each returned `Transaction`:
     - If the adapter filled in a currency and it doesn't match the account currency → `DomainError::CurrencyMismatch` (abort the whole import; zero rows persisted).
     - Otherwise overwrite `amount.currency` with the account's currency (adapter currency may be a placeholder).
  4. Second pass (only when step 3 validated all rows): for each txn, `repo.find_by_external_id(account_id, external_id)` — if `Some`, increment `duplicates`; else `repo.save(&txn)` and increment `imported`.
  5. Return `ImportReport { imported, duplicates }`.
- Add `pub struct ImportReport { pub imported: usize, pub duplicates: usize }` with `Serialize`.
- Add `DomainError::Import(String)` variant + a test for its `Display` impl.

CLI (`src/cli/transactions.rs::handle_import`):
- Build both repos, instantiate `TransactionService`, call `svc.import_from(...)`.
- On success, emit `{status:"ok", data:{imported, duplicates, file, format, account_id}}`.
- Add an explicit branch for `format == "csv"` → emit an `UnsupportedFormat` error envelope with the deferred-to-later-slice message, exit 2.
- Keep other exit codes: 0 success, 1 domain error, 2 bad input (unsupported format).

TDD (use a lightweight in-test `Importer` impl to avoid file I/O coupling):
- `import_from_unknown_account_errors` → `DomainError::NotFound`.
- `import_from_currency_mismatch` → `DomainError::CurrencyMismatch`, zero rows persisted.
- `import_from_dedup_counts`: three txns, two with FITIDs already in DB → report `{imported:1, duplicates:2}`.
- `import_from_overwrites_placeholder_currency`: adapter returns txn with USD placeholder + account USD → row persisted with account currency.
- `import_from_atomic_on_mismatch`: mismatch on the 2nd of 3 rows → NO rows persisted (the two-pass design ensures this).

## Inputs

- `src/application/transaction_service.rs`
- `src/cli/transactions.rs`
- `src/domain/error.rs`
- `src/infrastructure/importer/mod.rs`

## Expected Output

- ``TransactionService::import_from` + `ImportReport``
- ``DomainError::Import` variant`
- `CLI import handler reports `{imported, duplicates}` and rejects `--format csv` cleanly`
- `Tests covering atomic-on-mismatch + dedup counting`

## Verification

cargo test -- application::transaction_service && cargo test -- cli
