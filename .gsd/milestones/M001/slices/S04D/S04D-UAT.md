# S04D: Connections remove CLI + polish — UAT

**Milestone:** M001
**Written:** 2026-04-20T00:03:52.454Z

# S04D: Connections remove CLI + polish — UAT

Single-task slice. Closes the S04C-UAT known-limitation gap.

## Test: Remove by id

```bash
rtf connections remove --id <uuid>
```

**Result:** PASS.

## Test: Remove by provider + external-id

```bash
rtf connections remove --provider teller --external-id <enr_id>
```

**Result:** PASS — live-verified by removing user's Legacy Teller row.

## Test: Transactions preserved across connection removal

**Expected:** Deleting a connection does NOT cascade to transactions. They're keyed to local account_id, not the enrollment.
**Result:** PASS — 1,508 transactions intact across 6 accounts after Legacy removal.

## Test: Argument validation

- `--id` + `--provider` together → Validation error.
- Neither → Validation error with guidance.
- Partial (provider w/o external_id or vice versa) → Validation error.

**Result:** PASS (7 unit tests).

## Test: Not-found surfaces clearly

- `--id missing` → `DomainError::NotFound`, exit 1.
- `--provider teller --external-id nope` → same.

**Result:** PASS.

## Test: Full test suite

```bash
cargo test
```

**Result:** PASS — 333 tests, 5 ignored, 0 failed.

