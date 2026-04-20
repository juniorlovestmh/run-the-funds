---
id: S04D
parent: M001
milestone: M001
provides:
  - ["rtf connections remove --id <uuid>", "rtf connections remove --provider <p> --external-id <id>", "ProviderConnectionRepository::find_by_id"]
requires:
  - slice: S04C
    provides: provider_connections schema + ProviderConnectionRepository
affects:
  []
key_files:
  - ["src/cli/connections.rs", "src/cli/mod.rs", "src/main.rs", "src/domain/connections/repository.rs", "src/infrastructure/storage/connections_repo.rs"]
key_decisions:
  - (none)
patterns_established:
  - (none)
observability_surfaces:
  - none
drill_down_paths:
  []
duration: ""
verification_result: passed
completed_at: 2026-04-20T00:03:52.454Z
blocker_discovered: false
---

# S04D: Connections remove CLI + polish

**Single-task follow-up: `rtf connections remove` CLI closes the S04C polish gap; live-verified by removing user's Legacy Teller row while preserving all 1,508 persisted transactions.**

## What Happened

One task, ~30 minutes. New `ConnectionsCommands::Remove` variant supports both `--id <uuid>` and `--provider <p> --external-id <id>` lookup paths; enforces mutually-exclusive arg groups; surfaces clear Validation and NotFound errors. `ProviderConnectionRepository` gained `find_by_id`. 7 unit tests + live verification against the user's real DB. 333 tests passing post-slice.

No schema changes, no adapter changes. User can now safely clean up stale enrollments (including the S04C Legacy row) without touching SQLite.

## Verification

`cargo test` → 333 passing. Live: removed Legacy Teller row successfully; all 1,508 transactions preserved; `connections list` shows only the fresh Capital One enrollment.

## Requirements Advanced

None.

## Requirements Validated

None.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Operational Readiness

None.

## Deviations

None.

## Known Limitations

["Deletion is hard (no undo). User would need to re-run `teller connect` / `pluggy connect` to restore a removed enrollment."]

## Follow-ups

None.

## Files Created/Modified

None.
