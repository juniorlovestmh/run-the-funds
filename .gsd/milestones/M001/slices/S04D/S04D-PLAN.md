# S04D: Connections remove CLI + polish

**Goal:** Add `rtf connections remove` CLI so users can delete a stored bank enrollment/item without touching SQLite directly. Supports both id-based and (provider, external_id)-based lookup. Unblocks cleaning up the Legacy enrollment row from S04C's migration-006, and any stale enrollment from a bank the user no longer wants synced.
**Demo:** rtf connections remove --id <uuid> deletes a stored enrollment/item. rtf connections remove --provider teller --external-id <id> deletes by lookup. Legacy row from S04C's migration 006 can be cleaned up via this command. No other code changes.

## Must-Haves

- `rtf connections remove --id <uuid>` deletes the matching row and prints `{status:"ok", data:{removed: 1, ...}}`.
- `rtf connections remove --provider teller --external-id <enr_id>` deletes by (provider, external_id) lookup.
- Unknown id → exit 1 with a clear "connection not found" error.
- Neither flag → exit 1 with guidance.
- Both flags → exit 1, ambiguous-args error.
- Unit tests cover all four paths. One integration test in `tests/sync_demo.rs`.
- User can clean up their Legacy Teller row: `rtf connections remove --provider teller --external-id token_2czxt73goplp3kmpfogkbj53vm`.

## Proof Level

- This slice proves: contract — user-facing CLI for a safe, reversible-via-re-connect operation. No service-layer or adapter changes.

## Integration Closure

- Upstream surfaces consumed: `ProviderConnectionRepository::delete` (already exists) and `find_by_external_id` (already exists).
- New wiring: `ConnectionsCommands::Remove` variant + handler.
- What remains: nothing — this fully closes the S04C polish gap.

## Verification

- Success envelope reports `{removed: 1, provider, external_id, institution_name}` — operator can confirm what got deleted.
- Not-found surfaces as a clear error, not a silent no-op.
- No secrets logged (operation touches no credentials).

## Tasks

- [x] **T01: `rtf connections remove` CLI + tests + slice commit** `est:1h`
  Single-task slice.

**CLI:**
- New `ConnectionsCommands::Remove { id: Option<String>, provider: Option<String>, external_id: Option<String> }` variant in `src/cli/mod.rs`.
- Handler `handle_remove(db, id, provider, external_id)` in `src/cli/connections.rs`:
  - Validate mutually-exclusive args: require exactly one of `--id` or (`--provider` + `--external-id`).
  - Look up the row via repo (by id → fetch all then filter, OR add a find_by_id method). Use find_by_external_id for the (provider, external_id) path.
  - If not found → exit 1 with `{status:"error", message:"connection not found: ..."}`.
  - Else call `repo.delete(id)` and emit `{status:"ok", data:{removed:1, provider, external_id, institution_name}}`.
- Wire `Commands::Connections → ConnectionsCommands::Remove { ... }` in `src/main.rs`.

**Repository addition (if needed):**
- `ProviderConnectionRepository` already has `find_by_external_id(provider, external_id) -> Option<ProviderConnection>` and `delete(id) -> ()`. Check if we need `find_by_id(id)` — probably yes for the `--id` path. Add it to the trait + SQLite impl.

**Tests:**
- Unit tests in `src/cli/connections.rs` for happy paths (remove by id, remove by provider+ext-id), not-found error, ambiguous-args error, missing-args error.
- Integration test in `tests/sync_demo.rs`: connect a fake enrollment via direct DB insert, run `rtf connections remove --id <uuid>`, verify the row is gone.

**Commit**: single commit covering T01 (the whole slice).
  - Files: `src/cli/connections.rs`, `src/cli/mod.rs`, `src/main.rs`, `src/domain/connections/repository.rs`, `src/infrastructure/storage/connections_repo.rs`, `tests/sync_demo.rs`
  - Verify: cargo test && manual: rtf connections remove --id <legacy-row-id>

## Files Likely Touched

- src/cli/connections.rs
- src/cli/mod.rs
- src/main.rs
- src/domain/connections/repository.rs
- src/infrastructure/storage/connections_repo.rs
- tests/sync_demo.rs
