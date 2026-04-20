---
estimated_steps: 15
estimated_files: 6
skills_used: []
---

# T01: `rtf connections remove` CLI + tests + slice commit

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

## Inputs

- `src/domain/connections/repository.rs (existing trait)`
- `src/infrastructure/storage/connections_repo.rs (existing impl)`

## Expected Output

- `rtf connections remove --id <uuid> CLI`
- `rtf connections remove --provider <p> --external-id <id> CLI`
- `find_by_id method on the repo if not present`
- `+4-5 unit tests`
- `+1 integration test`
- `Single slice commit`

## Verification

cargo test && manual: rtf connections remove --id <legacy-row-id>
