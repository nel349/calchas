# Calchas Project Context & Guardrails

**Calchas** is a prediction market trading bot (Kalshi, Polymarket) built in Rust. Focus: momentum-based strategies on sports markets, not arbitrage. Personal tool, not SaaS.

---

## Key File References

### Core Documentation (SINGLE SOURCE OF TRUTH)

**Product Requirements & Design (STATIC):**
- `docs/Calchas_PRD_v1.md` - Business requirements, success metrics, strategy types
- `docs/TECHNICAL_ARCHITECTURE.md` - Pure design spec - components, data models, how things work (NO roadmaps, NO tracking)

**Build Progress (DYNAMIC):**
- `docs/PROJECT_STATUS.md` - **THE ONLY BUILD TRACKER** - Component checklist, what works, what's next, phases, roadmap

**Learning Progress (DYNAMIC):**
- `docs/RUST_SYLLABUS.md` - Rust concepts mapped to Calchas components (just a learning aid)

### Learning Resources
- **Rust Principles:** `references/aspiring_rust_engineer.md` - Proper learning order (ownership before lifetimes, etc.)
- **Rust Career Path:** `references/rust_engineer_path.md` - Beginner → Expert progression

### Related Project
- **Harbinger:** `/Users/norman/Development/harbinger` - Crypto intelligence platform (similar architecture)
  - Patterns to reuse: Signal confidence scoring, dual-model LLM, microservices, React UI

---

## Development Guardrails

### Project Tracking (CRITICAL - SINGLE SOURCE OF TRUTH)
1. **docs/PROJECT_STATUS.md is THE ONLY build tracker** - Never add roadmaps/phases to TECHNICAL_ARCHITECTURE.md
2. **UPDATE docs/PROJECT_STATUS.md after every component** - Mark ✅, update "What Works" section
3. **TodoWrite for current session tasks** - Track immediate work items only
4. **docs/RUST_SYLLABUS.md for learning progress** - Separate from product progress

### Architecture Principles
1. **No Mock Data** - Use real data or return "Not Implemented" (from Harbinger PRINCIPLES.md)
2. **No Premature Abstractions** - Build real things first, extract patterns later
3. **Simple Before Smart** - If-statements before ML models
4. **Honest Code** - Name things what they actually are
5. **Type Safety** - Use newtypes (MarketId, PositionId) to prevent ID mix-ups

### Rust Learning Philosophy
- **Syllabus-Driven:** Work through `RUST_SYLLABUS.md` phase by phase
- **Checkbox = Freedom:** Once user checks off a concept, assume they understand it (no more "academic mode")
- **Learn by Building:** Every concept is immediately applied to a Calchas component
- **Proper Order:** Follow `references/aspiring_rust_engineer.md` (e.g., ownership before lifetimes, iterators before async)

### Implementation Rules
- **Reference Architecture:** Every component must match `TECHNICAL_ARCHITECTURE.md` design
- **Phase Discipline:** Complete Phase 1 before Phase 2 (don't skip ahead)
- **No Skipping:** Don't skip tests, don't use mocks (unless simulation mode), don't remove complexity to pass
- **Security Aware:** No command injection, XSS, SQL injection (OWASP Top 10)
- **Git Operations:** Only with explicit approval from user

---

## Project Structure Reference

**Top-Level:**
- Calchas_PRD_v1.md - Product requirements
- TECHNICAL_ARCHITECTURE.md - System design
- RUST_SYLLABUS.md - Learning roadmap
- CLAUDE.md - This file
- references/ - Rust learning resources
- strategies/ - Strategy JSON files
- config/ - TOML configuration

**Source Code (src/):**
- models/ - Market, Position, Order, Strategy, Trade
- platforms/ - Kalshi client (REST + WebSocket)
- strategy/ - JSON loader, evaluator, engine
- trading/ - Position manager, order executor, risk manager
- storage/ - SQLite integration
- runtime/ - Supervisor, channels, shutdown
- web/ - Axum server + WebSocket
- utils/ - Logging, decimal helpers

**Other Directories:**
- frontend/ - React + TypeScript + Vite
- migrations/ - SQLite schema migrations
- tests/ - Integration tests

---

## Core Philosophy: Engine vs Strategy

**Critical Insight:** Calchas is **trading infrastructure**, not a specific strategy.

**CALCHAS ENGINE (What We're Building):**
- Reusable infrastructure
- 12 weeks to build
- Valuable regardless of strategy
- Primary goal: Learn Rust

**STRATEGY JSONs (What Makes Money):**
- Disposable configuration
- Hot-swappable (no code changes)
- Iterate rapidly
- Validate AFTER engine is built

**Why This Matters:**
- Don't worry about strategy profitability while building
- Engine works for ANY strategy (momentum, arbitrage, market-making, ML-based)
- Strategy validation happens in **Phase 0** (post-engine, using simulation mode)
- Focus on learning Rust and building quality infrastructure

**See:** `docs/TECHNICAL_ARCHITECTURE.md` Section 16.4 for simulation mode and `docs/PROJECT_STATUS.md` for Phase 0 validation roadmap

---

## Current Status

**See `docs/PROJECT_STATUS.md` for all build progress and component status.**

---

## Tech Stack Summary

| Component | Technology |
|-----------|------------|
| Language | Rust (learning from beginner → expert) |
| Async Runtime | Tokio |
| HTTP Client | reqwest |
| WebSocket | tokio-tungstenite |
| JSON | serde + serde_json |
| Database | SQLite (rusqlite) |
| Web Backend | Axum |
| Web Frontend | React 18 + TypeScript + Vite |
| CLI | clap |
| Logging | tracing |
| Decimal Math | rust_decimal (no floats for money!) |

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Rust + Tokio | Memory safety, async I/O, no GC pauses |
| Message passing (channels) | Avoid shared mutable state, prevent data races |
| SQLite (not Postgres) | Simple, embedded, sufficient for single user |
| JSON strategies (not code) | Hot-reload without restart, non-programmers can edit |
| React frontend | Reuse Harbinger components, real-time WebSocket |
| Decimal for money | No floating-point precision errors |
| Newtype IDs | Prevent mixing up MarketId, PositionId, OrderId |

---

## PRD Success Metrics (Simulation → Live)

**Exit to Live Criteria (ALL must be met):**
1. 7+ consecutive profitable days
2. Net positive over full simulation period
3. No single-day loss exceeding 15%
4. Strategy behaves as expected (momentum capture validated)

**Live Trading Goals (Monthly):**
- Floor: +20% ROI, <30% max drawdown
- Target: +50% ROI, <20% max drawdown
- Exceptional: +100% ROI, <10% max drawdown

---

## Interaction Guidelines

### When User Asks Questions
1. Check if concept is already covered in checked-off syllabus items (assume they know it)
2. Reference specific sections: "See docs/TECHNICAL_ARCHITECTURE.md Section 6.3" or "See docs/RUST_SYLLABUS.md Phase 3.4"
3. For new concepts not yet covered, explain clearly with Calchas-specific examples

### When User is Coding
1. Verify implementation matches `docs/TECHNICAL_ARCHITECTURE.md` design
2. Point out if they're skipping ahead in `docs/RUST_SYLLABUS.md` (suggest proper order)
3. Remind about principles: no mocks, no premature abstractions, type safety
4. Test everything before marking syllabus checkbox as complete

### When User is Stuck
1. Check which phase they're on in syllabus
2. Provide specific file/line references from architecture
3. Offer Rust learning resources: The Book chapter, Tokio tutorial, etc.
4. Give working code examples from docs/TECHNICAL_ARCHITECTURE.md

### When to Reference Harbinger
- UI patterns (React components, WebSocket updates)
- Signal confidence scoring methodology
- Dual-model LLM patterns (if adding AI features later)
- Service architecture patterns (microservices → modular monolith)

---

## Important Reminders

- **No cheating:** No mocks (except simulation mode), no skipping complexity, no TODOs without approval
- **Read context:** Always check TECHNICAL_ARCHITECTURE.md before suggesting changes
- **Proper learning order:** Ownership before lifetimes, iterators before async, threads before async (see references/)
- **Git discipline:** No git operations without explicit user approval
- **Security first:** Validate inputs, prevent injection attacks, handle errors gracefully

---

**Version:** 1.0
**Last Updated:** December 2025
**Next Review:** After Phase 1 completion
