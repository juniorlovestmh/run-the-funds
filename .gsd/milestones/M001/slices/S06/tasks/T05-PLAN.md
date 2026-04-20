---
estimated_steps: 1
estimated_files: 4
skills_used: []
---

# T05: CLI rewire + live demo + UAT + rollup commit

Rewire rtf sync to call Monarch directly (no --provider flag). Remove accounts link --provider references to retired providers; keep command but default to monarch. Write S06-UAT.md covering: migration 009+010 on real DB, fresh rtf sync against live Monarch, rtf transactions list count ≈ mmoney count, rtf spending matches Monarch cashflow for March ±1%, idempotency (re-running sync = 0 new rows). Single commit covering T01-T05.

## Inputs

- `T01-T04`
- `live Monarch account with cleaned taxonomy from today`

## Expected Output

- `S06-UAT.md`
- `single commit`

## Verification

cargo test green. Live demo against real Monarch. Commit on main.
