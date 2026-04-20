---
estimated_steps: 1
estimated_files: 16
skills_used: []
---

# T01: Initialize Rust project with DDD module structure and dependencies

Create the rtf Rust binary crate with Cargo.toml, DDD-aligned module hierarchy (domain/, application/, infrastructure/, cli/), and all core dependencies. Set up the module tree so subsequent tasks can add entities and implementations without restructuring. Include a minimal main.rs with clap CLI skeleton that prints help.

## Inputs

- None specified.

## Expected Output

- `Cargo.toml`
- `src/main.rs`
- `src/lib.rs`
- `src/domain/mod.rs`
- `src/cli/mod.rs`
- `src/application/mod.rs`
- `src/infrastructure/mod.rs`

## Verification

cargo build && cargo test
