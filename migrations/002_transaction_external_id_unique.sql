-- Enforce per-account uniqueness of OFX FITIDs so re-imports are genuinely
-- deduplicated at the storage layer rather than only at the service layer.
-- The partial predicate keeps NULLs permissive — manually-entered transactions
-- (no external id) are not constrained.
CREATE UNIQUE INDEX IF NOT EXISTS idx_transactions_account_external_id
    ON transactions(account_id, external_id)
    WHERE external_id IS NOT NULL;
