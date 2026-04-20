---
estimated_steps: 20
estimated_files: 5
skills_used: []
---

# T04: Transfer detection: algorithm + `rtf categorize --detect-transfers`

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

## Inputs

- `src/application/categorization_service.rs (T02)`
- `Transaction.transfer_pair_id field (S01)`

## Expected Output

- `detect_transfers method with greedy pairing + date tolerance`
- `Currency-aware pairing`
- `find_untagged + update_transfer_pair_id on repo`
- `rtf categorize --detect-transfers CLI`
- `~8 unit tests`

## Verification

cargo test -- application::categorization_service::transfer_detection
