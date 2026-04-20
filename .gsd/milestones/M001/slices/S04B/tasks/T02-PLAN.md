---
estimated_steps: 10
estimated_files: 1
skills_used: []
---

# T02: Live-test Teller + S04B-UAT.md + commit (includes S04 leftover HTTP fix)

Manual verification + slice docs + rollup commit.

**Live flow** (user-driven):
1. User obtains an access token via Teller Connect (one-time browser step).
2. `rtf teller setup --access-token <TOKEN>`.
3. Discover account IDs via direct API call or curl (similar to our S04 discovery step).
4. `rtf accounts link --provider teller --external-id <id>` for each local account.
5. `rtf sync --provider teller` — real transactions come back (or clear error if mTLS is required by the tier).
6. Re-run — dedup.

**S04B-UAT.md**: mirrors S04's shape with setup/link/sync/unified/dedup/error scenarios.

**Commit**: one rollup containing T01 + T02 changes + the uncommitted S04 live-test HTTP fix. Message notes the HTTP improvement rides along.

## Inputs

- `target/release/rtf (rebuilt after T01)`
- `.gsd/milestones/M001/slices/S04/S04-UAT.md (template)`

## Expected Output

- `Live-verified Teller sync path (or documented mTLS blocker if applicable)`
- `S04B-UAT.md`
- `Single rollup commit`

## Verification

cargo test && manual live-sync once credentials are in place
