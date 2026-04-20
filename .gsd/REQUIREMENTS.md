# Requirements

This file is the explicit capability and coverage contract for the project.

## Active

### R001 — Multi-account transaction import (OFX/CSV)
- Class: core-capability
- Status: active
- Description: Multi-account transaction import (OFX/CSV)
- Why it matters: Foundation for all financial tracking — without transaction data, nothing else works
- Source: user
- Primary owning slice: M001/S02
- Validation: unmapped

### R002 — Multi-currency normalization (USD↔BRL, historical PTAX rates)
- Class: core-capability
- Status: active
- Description: Multi-currency normalization (USD↔BRL, historical PTAX rates)
- Why it matters: Core differentiator — no existing tool handles BRL+USD as first-class. Every query must work across both currencies.
- Source: user
- Primary owning slice: M001/S03
- Validation: unmapped

### R003 — Unified SQLite storage with DDD domain model
- Class: core-capability
- Status: active
- Description: Unified SQLite storage with DDD domain model
- Why it matters: Single source of truth for all financial data. DDD ensures domain logic is clean and testable.
- Source: user
- Primary owning slice: M001/S01
- Validation: unmapped

### R004 — Category hierarchy with rules-based auto-categorization
- Class: primary-user-loop
- Status: active
- Description: Category hierarchy with rules-based auto-categorization
- Why it matters: Spending analysis is meaningless without categorization. Auto-rules reduce manual work; agent reviews uncategorized.
- Source: user
- Primary owning slice: M001/S04
- Validation: unmapped

### R005 — Household model (multi-person, owner + beneficiary)
- Class: core-capability
- Status: active
- Description: Household model (multi-person, owner + beneficiary)
- Why it matters: User needs to track spending per household member (husband, wife, baby) across accounts owned by different people.
- Source: user
- Primary owning slice: M001/S05
- Validation: unmapped

### R006 — Goal tracking with progress measurement
- Class: primary-user-loop
- Status: active
- Description: Goal tracking with progress measurement
- Why it matters: User wants to ask "how am I doing against my goal of saving $100k?" — goals are first-class entities with target, timeframe, progress.
- Source: user
- Primary owning slice: M001/S06
- Validation: unmapped

### R007 — Debt payoff modeling (snowball/avalanche)
- Class: primary-user-loop
- Status: active
- Description: Debt payoff modeling (snowball/avalanche)
- Why it matters: User has credit cards and student loans. Agent should answer "should I pay off Discover or Aidvantage first?" with modeled payoff timelines.
- Source: inferred
- Primary owning slice: M001/S06
- Validation: unmapped

### R008 — Recurring transaction detection
- Class: primary-user-loop
- Status: active
- Description: Recurring transaction detection
- Why it matters: Critical for forecasting and for agent to flag subscriptions. Auto-detect fixed bills, subscriptions, loan payments from transaction patterns.
- Source: inferred
- Primary owning slice: M001/S06
- Validation: unmapped

### R009 — Split transaction support
- Class: core-capability
- Status: active
- Description: Split transaction support
- Why it matters: One Costco trip may be part groceries, part baby supplies, part household. Needed for accurate per-person and per-category tracking.
- Source: inferred
- Primary owning slice: M001/S04
- Validation: unmapped

### R010 — Financial health scoring
- Class: primary-user-loop
- Status: active
- Description: Financial health scoring
- Why it matters: Composite number (debt-to-income, savings rate, expense coverage) gives quick "how am I doing?" answer. Simple to compute from existing data.
- Source: inferred
- Primary owning slice: M001/S06
- Validation: unmapped

### R011 — Spending forecasting from historical patterns
- Class: primary-user-loop
- Status: active
- Description: Spending forecasting from historical patterns
- Why it matters: Agent needs to answer "I'm planning on buying X, will it impact my goals?" — requires projecting forward based on monthly averages + actuals.
- Source: user
- Primary owning slice: M001/S07
- Validation: unmapped

### R012 — Structured JSON CLI output for agent orchestration
- Class: integration
- Status: active
- Description: Structured JSON CLI output for agent orchestration
- Why it matters: The agent (Ductor/Telegram/Claude) is the primary interface. CLI subcommands must return structured JSON that an agent can parse and compose.
- Source: user
- Primary owning slice: M001/S07
- Supporting slices: M001/S01, M001/S02
- Validation: unmapped

### R013 — Country-agnostic adapter architecture
- Class: constraint
- Status: active
- Description: Country-agnostic adapter architecture
- Why it matters: Product vision: someone should be able to swap Brazil for Germany without touching domain code. All country-specific logic behind adapters.
- Source: user
- Primary owning slice: M001/S01
- Supporting slices: M001/S02, M001/S03
- Validation: unmapped

### R014 — Per-person spending attribution and queries
- Class: primary-user-loop
- Status: active
- Description: Per-person spending attribution and queries
- Why it matters: User wants to ask "what have we spent per person — wife, baby, husband?" Beneficiary tagging on transactions enables this.
- Source: user
- Primary owning slice: M001/S05
- Validation: unmapped

### R015 — Transfer detection between own accounts
- Class: core-capability
- Status: active
- Description: Transfer detection between own accounts
- Why it matters: Without this, every internal transfer inflates both income and expense reports. Table stakes for accurate spending analysis.
- Source: research
- Primary owning slice: M001/S04
- Validation: unmapped

### R016 — Transaction reconciliation against statements
- Class: core-capability
- Status: active
- Description: Transaction reconciliation against statements
- Why it matters: Ability to mark transactions as cleared/reconciled against a statement balance. Without it, data can't be trusted after re-imports.
- Source: research
- Primary owning slice: M001/S04
- Validation: unmapped

### R017 — Data export (CSV/JSON)
- Class: operability
- Status: active
- Description: Data export (CSV/JSON)
- Why it matters: No vendor lock-in. User must be able to export all their financial data in standard formats.
- Source: research
- Primary owning slice: M001/S07
- Validation: unmapped

### R018 — TDD red/green + SOLID + DDD engineering principles
- Class: quality-attribute
- Status: active
- Description: TDD red/green + SOLID + DDD engineering principles
- Why it matters: User is building a potential product. Classical software engineering principles ensure maintainability, testability, and extensibility.
- Source: user
- Primary owning slice: M001/S01
- Supporting slices: all slices
- Validation: unmapped

### R019 — Net worth tracking (assets minus liabilities)
- Class: primary-user-loop
- Status: active
- Description: Net worth tracking (assets minus liabilities)
- Why it matters: Aggregates all account balances across currencies into a single net worth figure. Foundation for financial health scoring.
- Source: research
- Primary owning slice: M001/S06
- Validation: unmapped

### R020 — Automated US bank sync (SimpleFIN)
- Class: integration
- Status: active
- Description: Automated US bank sync (SimpleFIN)
- Why it matters: User wants minimal manual work. SimpleFIN at $15/yr covers US/Canada bank sync affordably.
- Source: user
- Primary owning slice: M001/S03
- Validation: unmapped

### R021 — Bill scheduling (due dates, reminders, upcoming bills)
- Class: primary-user-loop
- Status: active
- Description: Bill scheduling (due dates, reminders, upcoming bills)
- Why it matters: User wants to see upcoming bills and due dates. Agent can proactively warn about upcoming payments.
- Source: user
- Primary owning slice: M001/S06
- Validation: unmapped

### R022 — Automated Brazilian bank sync (Pluggy/Meu Pluggy)
- Class: integration
- Status: active
- Description: Automated Brazilian bank sync (Pluggy/Meu Pluggy)
- Why it matters: User wants minimal manual work for Brazilian accounts too. Meu Pluggy provides free developer access to 50+ Brazilian institutions.
- Source: user
- Primary owning slice: M001/S03
- Validation: unmapped

## Deferred

### R023 — Investment/crypto account tracking
- Class: core-capability
- Status: deferred
- Description: Investment/crypto account tracking
- Why it matters: User has Fidelity stocks/crypto and Kraken crypto. Full portfolio tracking planned for immediately after MVP.
- Source: user
- Primary owning slice: M002
- Validation: unmapped
- Notes: Explicitly deferred to M002 — built right after MVP. Do not forget.

### R024 — Web UI
- Class: launchability
- Status: deferred
- Description: Web UI
- Why it matters: Visual dashboard for financial data. Not needed for MVP — agent interface is primary.
- Source: user
- Primary owning slice: none
- Validation: unmapped
- Notes: Deferred to future milestone. CLI-first, agent-first.

### R025 — Multi-tenant managed service
- Class: operability
- Status: deferred
- Description: Multi-tenant managed service
- Why it matters: Future monetization path — cloud-hosted version for other users.
- Source: user
- Primary owning slice: none
- Validation: unmapped
- Notes: Deferred to future. Adapter architecture supports this transition.

## Out of Scope

### R026 — Tax reporting
- Class: anti-feature
- Status: out-of-scope
- Description: Tax reporting
- Why it matters: Out of scope — different problem domain, different regulatory requirements per country.
- Source: inferred
- Primary owning slice: none
- Validation: n/a

### R027 — Bill payment execution
- Class: anti-feature
- Status: out-of-scope
- Description: Bill payment execution
- Why it matters: User explicitly wants scheduling, not payment. Payment execution carries liability and security risk.
- Source: user
- Primary owning slice: none
- Validation: n/a

### R028 — Multi-user authentication
- Class: anti-feature
- Status: out-of-scope
- Description: Multi-user authentication
- Why it matters: Personal tool on user's machine. Household model uses tags, not user accounts. Auth adds complexity without value for MVP.
- Source: inferred
- Primary owning slice: none
- Validation: n/a

## Traceability

| ID | Class | Status | Primary owner | Supporting | Proof |
|---|---|---|---|---|---|
| R001 | core-capability | active | M001/S02 | none | unmapped |
| R002 | core-capability | active | M001/S03 | none | unmapped |
| R003 | core-capability | active | M001/S01 | none | unmapped |
| R004 | primary-user-loop | active | M001/S04 | none | unmapped |
| R005 | core-capability | active | M001/S05 | none | unmapped |
| R006 | primary-user-loop | active | M001/S06 | none | unmapped |
| R007 | primary-user-loop | active | M001/S06 | none | unmapped |
| R008 | primary-user-loop | active | M001/S06 | none | unmapped |
| R009 | core-capability | active | M001/S04 | none | unmapped |
| R010 | primary-user-loop | active | M001/S06 | none | unmapped |
| R011 | primary-user-loop | active | M001/S07 | none | unmapped |
| R012 | integration | active | M001/S07 | M001/S01, M001/S02 | unmapped |
| R013 | constraint | active | M001/S01 | M001/S02, M001/S03 | unmapped |
| R014 | primary-user-loop | active | M001/S05 | none | unmapped |
| R015 | core-capability | active | M001/S04 | none | unmapped |
| R016 | core-capability | active | M001/S04 | none | unmapped |
| R017 | operability | active | M001/S07 | none | unmapped |
| R018 | quality-attribute | active | M001/S01 | all slices | unmapped |
| R019 | primary-user-loop | active | M001/S06 | none | unmapped |
| R020 | integration | active | M001/S03 | none | unmapped |
| R021 | primary-user-loop | active | M001/S06 | none | unmapped |
| R022 | integration | active | M001/S03 | none | unmapped |
| R023 | core-capability | deferred | M002 | none | unmapped |
| R024 | launchability | deferred | none | none | unmapped |
| R025 | operability | deferred | none | none | unmapped |
| R026 | anti-feature | out-of-scope | none | none | n/a |
| R027 | anti-feature | out-of-scope | none | none | n/a |
| R028 | anti-feature | out-of-scope | none | none | n/a |

## Coverage Summary

- Active requirements: 22
- Mapped to slices: 22
- Validated: 0
- Unmapped active requirements: 0
