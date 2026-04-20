# M001: Core Financial Engine

**Gathered:** 2026-04-18
**Status:** Ready for planning

## Project Description

rtf is a personal finance CLI tool in Rust that unifies transaction data from ~10 accounts across the US and Brazil into a single SQLite database. It handles multi-currency normalization (USD↔BRL using historically accurate BCB PTAX rates), auto-categorization via a rules engine, goal tracking, debt payoff modeling, and household spending attribution. The primary interface is an external AI agent that orchestrates CLI subcommands — the user never touches dashboards, they ask questions like "how am I doing on my savings goal?" or "what did we spend on per person this month?"

## Why This Milestone

The user has no financial tracking system today. Money flows through ~10 accounts in two countries, two currencies, across two household members — completely untracked. This milestone delivers the foundational engine: get all transactions into one place, normalize currencies, categorize spending, and expose structured queries for an agent to answer financial questions.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Import transactions from all bank accounts (OFX/CSV files for Brazilian banks, SimpleFIN for US banks, Pluggy API for Brazilian banks)
- Ask an agent "how much did we spend on food this month across all accounts?" and get an accurate answer in USD or BRL
- Track progress toward savings goals with accurate multi-currency rollups
- See per-person spending attribution (husband, wife, baby)
- Get a financial health score and debt payoff recommendations
- View upcoming bill schedule

### Entry point / environment

- Entry point: `rtf` CLI binary with subcommands
- Environment: local dev (macOS terminal), invoked by external agent (Ductor/Telegram → Claude → CLI)
- Live dependencies involved: BCB PTAX API (exchange rates), SimpleFIN API (US bank sync), Pluggy API (Brazilian bank sync)

## Completion Class

- Contract complete means: all domain entities tested, all adapters tested against fixtures, all CLI subcommands return correct JSON
- Integration complete means: real OFX/CSV files from Nubank/Itau/Chase/CapitalOne/Discover parse correctly; SimpleFIN and Pluggy adapters connect to real APIs
- Operational complete means: end-to-end flow works — import → categorize → query → structured JSON response

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A real Nubank OFX file and a real Chase CSV file import into the same database with accurate BRL→USD conversion
- `rtf spending --by-category --currency USD` produces correct cross-currency totals
- `rtf goals progress` shows accurate savings tracking across both currencies
- The agent can compose CLI subcommands to answer multi-step financial questions

## Architectural Decisions

### Implementation Language

**Decision:** Rust

**Rationale:** Strong type system for DDD domain modeling (enums with data, pattern matching for state machines, From/Into traits for adapter boundaries). Borrow checker catches a class of bugs other languages can't. User explicitly chose Rust after considering Go.

**Alternatives Considered:**
- Go — faster compile times, simpler concurrency, but weaker type system for domain modeling. User initially chose Go then switched to Rust.

### Money Representation

**Decision:** Store amounts in original currency with currency_code field, convert on-the-fly at query time using transaction-date PTAX rates.

**Rationale:** Simpler than dual-store (original + converted). No data duplication. Always accurate — if PTAX rates are updated or corrected, all historical queries automatically reflect the correction. BRL transactions stay BRL, USD stays USD.

**Alternatives Considered:**
- Dual-store (store both original and converted amounts) — more storage, risk of stale converted values, harder to maintain consistency.

### SQLite Storage

**Decision:** rusqlite with SQLite as the sole storage backend for MVP

**Rationale:** Local-first, no server dependency, fast reads for financial queries, portable database file. Follows Actual Budget's proven approach. Adapter interface allows swapping to Postgres for managed service later.

**Alternatives Considered:**
- Postgres — overkill for personal use, requires running a server
- sqlx — async overhead unnecessary for CLI tool that does synchronous queries

### Bank Sync Strategy

**Decision:** File import (OFX/CSV) as baseline for all banks. SimpleFIN ($15/yr) for US automated sync. Pluggy via Meu Pluggy (free developer tool) for Brazilian automated sync.

**Rationale:** Coverage across both countries. SimpleFIN is cheap and covers US/Canada. Meu Pluggy provides free access to 50+ Brazilian institutions. File import is the universal fallback for any bank.

**Alternatives Considered:**
- Plaid for US — free sandbox but paid production. SimpleFIN is cheaper and sufficient for personal use.
- Manual-only for Brazil — user wants minimal manual work. Meu Pluggy solves this.

### Category System

**Decision:** Two-level hierarchy (group → category) with regex-based rules on payee/description for auto-categorization.

**Rationale:** Matches proven model from YNAB/Actual Budget. Simple enough for MVP, extensible for ML-based categorization later. Two-phase pipeline: rules engine handles obvious matches, agent reviews uncategorized transactions.

**Alternatives Considered:**
- Flat categories — too coarse for meaningful spending analysis
- Three-level — unnecessary complexity for MVP

### Household Model

**Decision:** Account has `owner` field, transactions have optional `beneficiary` field. Per-person spending queries aggregate by beneficiary.

**Rationale:** Your wife owns a Nubank account, but a transaction on your card might be for the baby. Owner tracks account ownership; beneficiary tracks who the spend was for. Simple tags, no auth system.

**Alternatives Considered:**
- Full user/profile system — overkill for personal/household use

## Error Handling Strategy

- Malformed OFX/CSV → clear error message naming the file and line number, skip bad records with warning
- Duplicate transaction detection on re-import using composite key (date + amount + description + account)
- Missing exchange rate for a date → use nearest available rate, flag the transaction
- SQLite migration failures → backup database before migration, rollback on failure
- API failures (SimpleFIN/Pluggy/PTAX) → retry with exponential backoff, clear error message, graceful degradation to file import
- Two-phase categorization: rules engine auto-categorizes what it can, uncategorized transactions flagged for agent review

## Risks and Unknowns

- OFX format inconsistency across Brazilian banks — each bank may have OFX quirks
- Pluggy Meu Pluggy API stability — free developer tool, may change terms
- PTAX rate availability for weekends/holidays — need nearest-date fallback logic
- SimpleFIN API reliability and coverage for specific US institutions

## Existing Codebase / Prior Art

- No existing code — new project
- Reference: Actual Budget (local-first SQLite, envelope budgeting, bank sync architecture)
- Reference: hilm.ai (agent-driven financial tracking via Telegram)

## Relevant Requirements

- R001–R021 (all active requirements) — this milestone covers the full MVP capability set
- R022 (Investment/crypto) — explicitly deferred to M002

## Scope

### In Scope

- Transaction import from OFX/CSV files (Nubank, Itau, Chase, Capital One, Discover, Aidvantage)
- Automated US bank sync via SimpleFIN
- Automated Brazilian bank sync via Pluggy/Meu Pluggy
- Multi-currency normalization (USD↔BRL) using BCB PTAX historical rates
- Unified SQLite database with DDD domain model
- Category hierarchy with rules-based auto-categorization
- Transfer detection between own accounts
- Split transaction support
- Transaction reconciliation
- Household model with per-person spending attribution
- Goal tracking with progress measurement
- Debt payoff modeling (snowball/avalanche)
- Recurring transaction detection
- Financial health scoring
- Net worth tracking
- Bill scheduling (due dates, upcoming bills)
- Spending forecasting from historical patterns
- Structured JSON CLI output for agent orchestration
- Data export (CSV/JSON)
- Country-agnostic adapter architecture

### Out of Scope / Non-Goals

- Investment/crypto tracking (M002)
- Web UI (future)
- Multi-tenant managed service (future)
- Tax reporting
- Bill payment execution
- Multi-user authentication

## Technical Constraints

- rust_decimal for all monetary calculations — never f64
- All external API calls behind trait-based adapters for testability and swappability
- Country-specific logic (locale, date formats, bank-specific parsing) isolated behind adapters
- CLI output must be structured JSON for agent consumption (human-readable format as secondary option)

## Integration Points

- BCB PTAX API — daily USD↔BRL exchange rates (free, no auth)
- SimpleFIN API — US bank transaction sync ($15/yr)
- Pluggy/Meu Pluggy API — Brazilian bank transaction sync (free developer tool)
- External agent (Ductor/Telegram/Claude) — consumes CLI JSON output

## Testing Requirements

- TDD red/green for all domain entities, value objects, and services
- Integration tests with real OFX/CSV sample files (anonymized) from each bank
- Repository tests for SQLite round-trips on every entity type
- CLI subcommand tests verifying JSON output format
- Currency conversion tests against known BCB PTAX rates for specific dates
- Duplicate detection tests (re-import same file → zero new transactions)
- Transfer detection tests (internal transfers excluded from spending)

## Acceptance Criteria

### S01 — Domain Foundation + Storage
- Domain entities compile and pass unit tests
- SQLite repositories persist and retrieve all entity types
- `rtf accounts create` works end-to-end

### S02 — Transaction Import Pipeline
- Nubank OFX file parses into normalized transactions
- Chase CSV file parses into normalized transactions
- `rtf transactions list --format json` returns correct data

### S03 — Multi-Currency + Bank Sync
- BRL transactions display with accurate USD equivalent at PTAX rate for transaction date
- SimpleFIN adapter pulls US bank transactions
- Pluggy adapter pulls Brazilian bank transactions

### S04 — Categorization Engine
- Rules auto-categorize imported transactions
- Split transactions work
- Internal transfers detected and excluded from spending totals
- Reconciliation marks transactions as cleared

### S05 — Household + Per-Person Attribution
- `rtf spending --by-person` shows breakdown by household member
- Beneficiary tagging works across accounts

### S06 — Goals, Budgets, Debt & Health
- `rtf goals progress` shows savings goal tracking across currencies
- Debt payoff plan with snowball/avalanche comparison
- Financial health score computed
- Bill schedule shows upcoming due dates
- Recurring transactions auto-detected

### S07 — Agent-Ready Query Layer + Export
- All CLI subcommands return structured JSON
- `rtf forecast` projects next month based on historical patterns
- CSV/JSON data export works
- Net worth calculation across all accounts and currencies

## Open Questions

- Exact OFX format quirks per Brazilian bank — will need real sample files to test
- SimpleFIN API authentication flow — need to verify during S03
- Pluggy/Meu Pluggy credential management — how to store clientId/clientSecret securely
