---
estimated_steps: 1
estimated_files: 14
skills_used: []
---

# T01: Retire Teller + SimpleFIN + Pluggy adapters and Connect scaffolding

Delete all code for the three direct-to-bank adapters (teller.rs, simplefin.rs, pluggy.rs, pluggy_connect.rs, connect/*, cli/{teller,simplefin,pluggy,connections}.rs, pluggy_connect.html). Remove TellerCommands/SimplefinCommands/PluggyCommands/ConnectionsCommands enums from cli/mod.rs + corresponding main.rs dispatch arms. Drop unused deps from Cargo.toml (rustls/ring, base64 if only SimpleFIN used it, tiny_http, webbrowser, quick-xml). Write migration 009_monarch_reset.sql that wipes provider-sourced state: DROP TABLE provider_credentials; DELETE FROM accounts WHERE external_provider IN ('teller','simplefin','pluggy'); DELETE FROM transactions; DELETE FROM categories; DELETE FROM category_groups; DELETE FROM rules; DELETE FROM transaction_splits. Preserve exchange_rates (PTAX cache) + persons (beneficiary dimension, future). Keep existing sync_service scaffolding that a new Monarch adapter will plug into.

## Inputs

- `existing S04/S04B/S04C/S04D code`

## Expected Output

- `cleaner repo with Teller/SimpleFIN/Pluggy/Connect gone`
- `migrations/009_monarch_reset.sql`

## Verification

cargo build clean (no dead symbols). cargo test passes (removed tests for retired code). sqlite3 on a throwaway DB: after running migration, accounts/transactions/categories/category_groups/rules/transaction_splits/provider_credentials empty; exchange_rates + persons preserved.
