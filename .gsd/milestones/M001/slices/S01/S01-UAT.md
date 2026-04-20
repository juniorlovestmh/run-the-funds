# S01: Domain Foundation + Storage — UAT

**Milestone:** M001
**Written:** 2026-04-18T17:46:26.049Z

## S01: Domain Foundation + Storage — UAT

### Test: Create account via CLI
```bash
rtf --db /tmp/test.db accounts create --name "Nubank Checking" --type checking --currency BRL --owner "Sky" --institution "Nubank"
```
**Expected:** JSON response with `status: "ok"`, account data with UUID id, zero BRL balance, correct fields.
**Result:** PASS

### Test: Create USD account
```bash
rtf --db /tmp/test.db accounts create --name "Chase Checking" --type checking --currency USD --owner "Sky"
```
**Expected:** JSON response with USD account.
**Result:** PASS

### Test: List accounts as JSON
```bash
rtf --db /tmp/test.db accounts list --format json
```
**Expected:** JSON array with both BRL and USD accounts, sorted by name.
**Result:** PASS — Chase before Nubank (alphabetical)

### Test: List accounts as table
```bash
rtf --db /tmp/test.db accounts list
```
**Expected:** Human-readable table with columns for ID, Name, Type, Currency, Owner, Balance.
**Result:** PASS

### Test: Validation error returns JSON
```bash
rtf --db /tmp/test.db accounts create --name "" --type checking --currency USD --owner "Sky"
```
**Expected:** JSON error to stderr, exit code 1.
**Result:** PASS — `{status: "error", message: "validation error: account name is required"}`

### Test: Full test suite
```bash
cargo test
```
**Expected:** All tests pass.
**Result:** PASS — 131 passed, 0 failed
