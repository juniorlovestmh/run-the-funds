# Decisions Register

<!-- Append-only. Never edit or remove existing rows.
     To reverse a decision, add a new row that supersedes it.
     Read this file at the start of any planning or research phase. -->

| # | When | Scope | Decision | Choice | Rationale | Revisable? | Made By |
|---|------|-------|----------|--------|-----------|------------|---------|
| D001 | M001 | arch | Implementation language | Rust | Strong type system for DDD (enums, pattern matching, From/Into traits). Borrow checker catches bugs. User explicitly chose Rust after considering Go. | No | human |
| D002 | M001 | data | Money representation and storage | Store in original currency with currency_code, convert at query time using transaction-date PTAX rates | Simpler than dual-store, no data duplication, always accurate. If rates corrected, historical queries auto-reflect. | No | collaborative |
| D003 | M001 | arch | Storage backend | rusqlite with SQLite | Local-first, no server dependency, fast reads. Follows Actual Budget proven approach. Adapter interface allows Postgres swap for managed service. | Yes — if managed service needs Postgres | collaborative |
| D004 | M001 | arch | Bank sync strategy | SimpleFIN for US ($15/yr), Pluggy/Meu Pluggy for Brazil (free), OFX/CSV file import as universal fallback | Coverage across both countries at minimal cost. File import as baseline ensures any bank works. | Yes — if cheaper Brazilian sync options emerge | collaborative |
| D005 | M001 | pattern | Category system structure | Two-level hierarchy (group → category) with regex rules on payee/description | Proven model from YNAB/Actual. Two-phase pipeline: rules auto-categorize, agent reviews remainder. | Yes — if ML categorization added later | collaborative |
| D006 | M001 | arch | Household model | Account has owner field, transactions have optional beneficiary field. No user/auth system. | Owner tracks account ownership, beneficiary tracks who spend was for. Simple tags, sufficient for household use. | Yes — if multi-tenant auth added | collaborative |
| D007 | M001 | arch | Agent integration approach | CLI subcommands with structured JSON output, designed for external agent orchestration | Agent (Ductor/Telegram/Claude) is external. Tool is the data backend with composable subcommands. No built-in conversation loop needed. | No | collaborative |
| D008 | M001 | library | Decimal arithmetic library | rust_decimal | Never use f64 for money. rust_decimal provides arbitrary precision decimal arithmetic. | No | agent |
