# rtf

## What This Is

A personal finance CLI tool in Rust that ingests transactions from US and Brazilian bank accounts, credit cards, and loans into a unified SQLite database with multi-currency support (USD/BRL). Designed for agent orchestration — an external AI agent (via Ductor/Telegram/Claude) calls CLI subcommands to help the user understand spending, track goals, and plan finances. Built with DDD, TDD, SOLID, and adapter patterns so it can serve as a product for users in any country configuration.

## Core Value

Multi-currency financial awareness through an agent-driven conversational interface. The user asks questions about their money in natural language; the agent orchestrates CLI commands against a unified transaction database that spans multiple countries, currencies, and household members.

## Current State

New project. No code written yet.

## Architecture / Key Patterns

- **Language:** Rust
- **CLI:** clap + clap_derive
- **Storage:** rusqlite (SQLite)
- **Money:** rust_decimal (never floating point)
- **OFX parsing:** quick-xml
- **HTTP:** reqwest (for Pluggy, SimpleFIN, BCB PTAX APIs)
- **Serialization:** serde + serde_json
- **Async:** tokio (minimal, API calls only)
- **Testing:** cargo test (TDD red/green)
- **DDD:** domain modules + traits for adapter boundaries
- **Adapters:** bank importers, currency providers, storage backends are all trait-based and swappable

## Capability Contract

See `.gsd/REQUIREMENTS.md` for the explicit capability contract, requirement status, and coverage mapping.

## Milestone Sequence

- [ ] M001: Core Financial Engine — Transaction ingestion, multi-currency normalization, categorization, goals, household, and agent-ready CLI
- [ ] M002: Investment & Crypto Tracking — Portfolio tracking for stock and crypto accounts (Fidelity, Kraken)
