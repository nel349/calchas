# CALCHAS
## Product Requirements Document

**Prediction Market Trading Bot**

| Field | Value |
|-------|-------|
| Version | 1.1 |
| Date | December 2025 |
| Status | Draft |

---

## 1. Overview

### 1.1 Product Name

**Calchas** — Named after the Greek seer who could read the signs of the future.

### 1.2 Problem Statement

- Manual monitoring of live prediction markets is tedious and error-prone
- Momentum swings happen fast — hard to catch manually in real-time
- Impossible to track hundreds of positions simultaneously across multiple games
- Opportunity discovery across platforms (Kalshi, Polymarket) requires constant attention
- Different market dynamics (football offense shifts, NHL puck possession, soccer set pieces) need tailored strategies

### 1.3 Vision

Personal trading tool that automates volatility-based strategies on sports prediction markets. Differentiated from crowded arbitrage bots by focusing on **in-game momentum capture** rather than cross-platform price discrepancies. Start with simulations to validate strategies before live capital. Foundation for more sophisticated approaches over time.

### 1.4 Key Differentiator

Not competing in saturated arbitrage space. Instead: directional volatility plays exploiting sport-specific dynamics (e.g., football offensive momentum, NHL possession swings) with automated position management and profit-threshold exits.

While arbitrage returns are typically 1-5% per trade in a crowded field, momentum-based strategies offer higher potential returns with managed risk.

---

## 2. Goals & Success Metrics

### 2.1 Simulation Phase Goals (Weeks 1-4)

| Metric | Minimum | Target | Exceptional |
|--------|---------|--------|-------------|
| Consecutive profitable days | 7 | 14 | 21+ |
| Net ROI | +5% | +20% | +50%+ |
| Win rate (trades) | 52% | 60% | 70%+ |
| Avg profit per winning trade | 2% | 5% | 10%+ |

### 2.2 Live Trading Goals (Monthly)

| Metric | Floor (Success) | Target (Great) | Exceptional (Moon) |
|--------|-----------------|----------------|-------------------|
| Monthly ROI | +20% | +50% | +100%+ |
| Max drawdown | <30% | <20% | <10% |
| Markets monitored | 50+ | 200+ | 500+ |

### 2.3 Operational Goals

- **Uptime:** 99%+ during game hours
- **Latency:** <500ms price updates (accuracy over speed)
- **Platforms:** Kalshi + Polymarket aggregated

### 2.4 Exit to Live Criteria

Simulation → Live when ALL of these are met:

1. 7+ consecutive profitable days (at least once)
2. Net positive over full simulation period
3. No single-day loss exceeding 15%
4. Strategy behaves as expected (momentum capture validated)

---

## 3. Target Users

**Primary User:** Personal trading tool for owner use

**Future Consideration:** SaaS potential (distant future)
- Architecture should be clean enough to support multi-user later
- No premature optimization — build for yourself first

---

## 4. Core Architecture

### 4.1 Design Philosophy

Calchas = Engine + Strategies (decoupled)

The engine handles platform connections, market aggregation, order execution, and monitoring. Strategies are pluggable JSON configuration files that define entry/exit rules, similar to FreqTrade's approach for crypto trading.

### 4.2 System Components

**CALCHAS ENGINE Layers:**
1. **Platform Clients** - Kalshi Client, Polymarket Client, Future Clients
2. **Market Aggregator** - Unified data model across platforms
3. **Strategy Engine** - Loads and executes strategy JSON files
4. **Order Manager** - Execute, track, and exit positions

### 4.3 Market Categories (Extensible)

While MVP focuses on sports, the architecture supports all prediction market categories:

| Category | Examples | Price Drivers |
|----------|----------|---------------|
| **Sports** | NFL, NBA, Soccer, NHL | In-game events, momentum |
| **Politics** | Elections, policy, appointments | Polls, news, debates |
| **Economics** | Fed rates, CPI, GDP | Data releases, speeches |
| **Crypto** | BTC price targets, ETF approvals | Price feeds, whale moves |
| **Entertainment** | Oscars, TV ratings | Reviews, buzz, leaks |
| **Weather** | Hurricane landfall, temperature | Forecasts, real-time data |

---

## 5. Strategy System

### 5.1 Strategy Types

#### Strategy A: Momentum Scalp (Underdog Only)

- Only buy underdog at <20¢
- Wait for momentum swing
- Exit at +X% gain OR stop-loss at -Y%
- **Higher risk, higher reward**

#### Strategy B: Volatility Hedge (Both Sides)

- Buy both sides with equal $
- Exit when combined position hits +X%
- **Lower risk, consistent smaller gains**

#### Strategy C: Hybrid

- Start with underdog-only
- If it drops significantly, buy favorite to hedge
- **Best of both worlds**

### 5.2 Strategy File Structure

**Example:** `strategies/momentum_scalp.json`

**JSON Structure:**
- name, description, version
- filters: categories, platforms, price thresholds, liquidity minimums, game status
- entry: side (underdog_only/both/favorite_only), amount_usd, order_type
- exit: take_profit_pct, stop_loss_pct, trailing_stop_pct, max_hold_minutes
- risk: max_concurrent_positions, max_daily_loss_usd, cooldown_after_loss_minutes

### 5.3 Strategy Directory Organization

**Directory Structure:**
- strategies/sports/ - NFL, NHL, soccer strategies
- strategies/politics/ - Election, poll-based strategies
- strategies/crypto/ - BTC, ETF strategies
- strategies/economics/ - Fed, economic indicator strategies
- strategies/generic/ - Platform-agnostic strategies

---

## 6. Features & Requirements

### 6.1 MVP (v1.0) — "Get Trading ASAP"

| Feature | Description | Priority |
|---------|-------------|----------|
| Kalshi Client | Connect to Kalshi API | 🔴 Must |
| Market Discovery | Fetch sports markets, filter by sport/odds | 🔴 Must |
| Strategy Loader | Read JSON strategy files | 🔴 Must |
| Position Entry | Open positions based on strategy rules | 🔴 Must |
| Real-time Monitoring | WebSocket price updates | 🔴 Must |
| Auto Exit | Take-profit/stop-loss execution | 🔴 Must |
| Position Tracker | Track open positions, P&L | 🔴 Must |
| Logging | Record all trades, decisions, errors | 🔴 Must |
| Simulation Mode | Paper trading (no real money) | 🔴 Must |
| CLI + Daemon + Web UI | Run modes with browser dashboard | 🔴 Must |

### 6.2 v1.5 — "Second Platform + Polish"

| Feature | Description | Priority |
|---------|-------------|----------|
| Polymarket Client | Add Polymarket integration | 🟡 High |
| Cross-platform Aggregation | Unified view of same event | 🟡 High |
| Alerts/Notifications | Telegram/Discord alerts on trades | 🟡 High |
| Multiple Strategies | Run 2+ strategies simultaneously | 🟡 High |

### 6.3 v2.0 — "Optimize & Scale"

| Feature | Description | Priority |
|---------|-------------|----------|
| Backtesting Engine | Test strategies on historical data | 🟢 Medium |
| Performance Analytics | Win rate, ROI, drawdown charts | 🟢 Medium |
| Strategy Editor UI | Create/edit strategies in browser | 🟢 Medium |
| Arbitrage Detection | Cross-platform price discrepancies | 🟢 Medium |
| ML Signal Integration | Plug in external prediction models | 🔵 Future |
| Multi-user / SaaS | Accounts, billing, etc. | 🔵 Future |

---

## 7. Technical Constraints

### 7.1 Kalshi API

| Aspect | Details |
|--------|---------|
| Base URL | `https://trading-api.kalshi.com/trade-api/v2` |
| Demo URL | `https://demo-api.kalshi.co/trade-api/v2` |
| Auth | Email/password → Bearer token |
| Rate Limits | ~10 req/sec (be conservative) |
| WebSocket | Yes, for live prices |
| Key Endpoints | `/login`, `/markets`, `/markets/{ticker}`, `/orders`, `/portfolio/positions` |

### 7.2 Polymarket API (v1.5)

| Aspect | Details |
|--------|---------|
| Data API | `https://gamma-api.polymarket.com` |
| Trading API | `https://clob.polymarket.com` |
| Auth | API key + EIP-712 signing (Ethereum wallet) |
| Chain | Polygon (USDC) |
| Complexity | Higher — crypto wallet management |

### 7.3 Technical Stack

| Component | Technology |
|-----------|------------|
| Language | Rust |
| Async Runtime | Tokio |
| HTTP Client | reqwest |
| WebSocket | tokio-tungstenite |
| Serialization | serde + serde_json |
| Config | Strategy JSON files |
| Storage | SQLite (trades, positions) |
| Web Backend | Axum (REST API + WebSocket) |
| Web Frontend | React 18 + TypeScript + Vite |
| Logging | tracing |

**Note:** UI stack matches Harbinger for code reusability and proven WebSocket real-time update patterns.

### 7.4 Constraints & Limitations

| Constraint | Impact | Mitigation |
|------------|--------|------------|
| Kalshi rate limits | Can't poll too fast | Use WebSocket for prices |
| US regulations | Kalshi = compliant | Start with Kalshi |
| No shorting on Polymarket | Can only buy YES/NO | Strategy design accounts for this |
| Liquidity on small markets | Slippage risk | Filter by min volume |
| API downtime | Missed trades | Retry logic, graceful degradation |

### 7.5 Compliance

| Platform | Status | Obligations |
|----------|--------|-------------|
| Kalshi | CFTC regulated, legal in US | KYC, report taxes on gains |
| Polymarket | Crypto-based, US access evolving | Use at own risk |

---

## 8. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Strategy loses money | Medium | High | Simulation first, stop-losses, position limits |
| API changes break bot | Medium | Medium | Abstracted client layer, version pinning |
| Rate limited/banned | Low | High | Respect limits, exponential backoff |
| Missed exit (connection drop) | Low | High | Persistent position tracking, auto-reconnect |
| Flash crash / manipulation | Low | Medium | Max position sizes, sanity checks on prices |
| Regulatory changes | Low | High | Start with Kalshi (compliant) |

---

## 9. Project Structure

**Top-Level:**
- Cargo.toml - Rust project configuration
- config/ - TOML configuration files (API keys, defaults)
- strategies/ - JSON strategy files (momentum_scalp.json, volatility_hedge.json, examples/)

**Source Code (src/):**
- main.rs, daemon.rs, lib.rs - Entry points
- config/ - Configuration loading
- platforms/ - Kalshi and Polymarket clients
- markets/ - Market aggregator, unified types
- strategy/ - JSON loader, strategy execution engine
- trading/ - Order management, position tracking, simulator
- storage/ - SQLite trade history
- web/ - Axum server (REST + WebSocket)

**Frontend:**
- React + TypeScript + Vite setup
- Components: PositionTracker, StrategyMonitor, MarketScanner, TradeHistory, PerformanceCharts

**Other:**
- migrations/ - SQLite schema migrations
- tests/ - Test files

---

## 10. Run Modes

**CLI Commands:**
- `calchas run --strategy momentum_scalp.json --dry-run` - One-off simulation
- `calchas daemon --port 8420` - Start background service with web UI
- Visit http://localhost:8420 for live dashboard

---

## 11. Appendix

### 11.1 Proven Concept

**Manual trade validation:**
- Bought Team B (underdog) at 11¢
- Sold at 24¢
- **Result: 118% return**

This validates the core strategy of buying cheap underdog contracts and exiting on momentum swings.

### 11.2 Market Context

- Prediction market total 2025 volume: ~$38B (Polymarket $21.5B, Kalshi $17.1B)
- Sports: 39% of Polymarket volume, 85% of Kalshi volume
- Arbitrage is crowded (1-5% returns, bot competition in milliseconds)
- Momentum/volatility strategies are less saturated
- Top arbitrageur: $2.01M across 4,049 transactions

### 11.3 Related Project: Harbinger

**Harbinger** is a production-grade crypto market intelligence platform that detects market-moving events and generates trading signals. It shares architectural goals with Calchas and provides patterns/infrastructure that can be leveraged.

#### Architecture Similarities

| Component | Harbinger | Calchas |
|-----------|-----------|---------|
| Language | Python 3.13 | Rust |
| Architecture | Microservices (9+ services) | Modular monolith → services |
| Data Layer | PostgreSQL + Redis | SQLite (MVP) |
| Web Backend | FastAPI + WebSocket | Axum + WebSocket |
| Web Frontend | React 18 + TypeScript + Vite | React 18 + TypeScript + Vite |
| Real-time Updates | WebSocket broadcasting | WebSocket broadcasting |
| Service Discovery | Centralized service registry (8080-8095) | Single daemon (port 8420) |

#### Key Patterns to Adopt

**1. Signal Confidence Scoring**
- Harbinger uses: `Combined_Confidence = ML_Confidence × Market_Multiplier × Source_Trust`
- Urgency tiers: immediate (≥80%), hours (≥70%), days (≥60%), watch (<60%)
- **Calchas Application:** Use similar confidence scoring for bet sizing and entry timing

**2. Dual-Model Strategy (LLM)**
- Fast model (Qwen2.5-3B, ~500ms) handles 80% of cases
- Accurate model (Qwen3-4B, ~2000ms) for high-value decisions
- Semantic similarity pre-filtering to skip irrelevant events
- **Calchas Application:** Evaluate prediction market opportunities (significant vs noise)

**3. Multi-Tier Aggregation**
- Tier 1: Temporal clustering (10-min windows)
- Tier 2: Event threading (24-hour narratives)
- Tier 3: Macro event alerts (market-wide impacts)
- **Calchas Application:** Cluster prediction market opportunities, detect correlated events

**4. UI Component Patterns**
- Real-time signal feeds with WebSocket updates
- Multi-panel dashboards (metrics, charts, feeds)
- Cluster/timeline visualizations
- Sentiment analysis displays
- **Calchas Application:** Position tracker, P&L charts, active strategies, market scanner

**5. Development Principles** (from `/harbinger/docs/PRINCIPLES.md`)
- ✅ No mock data - use real data or return "Not Implemented"
- ✅ No premature abstractions - build real things first
- ✅ Simple before smart - if-statements before ML
- ✅ Honest code - name things what they actually are
- **Calchas Adoption:** Follow same pragmatic, anti-abstraction approach

#### Reusable Components

**From Harbinger Frontend:**
- `MacroEventFeed.tsx` → Adapt for high-value prediction market opportunities
- `SignalClusterView.tsx` → Adapt for position timeline/clustering
- `MarketSentiment.tsx` → Adapt for prediction market momentum indicators
- `TradingSignals.tsx` → Adapt for strategy entry/exit signals

**Backend Patterns:**
- FastAPI service structure → Map to Axum handlers
- WebSocket broadcast architecture → Real-time position updates
- Service health monitoring → Daemon status endpoints
- Structured logging patterns → tracing implementation

#### Tech Stack Decisions Informed by Harbinger

| Decision | Rationale |
|----------|-----------|
| React instead of HTMX | Code reuse, proven WebSocket patterns, component library |
| WebSocket for real-time | Harbinger validates this scales well (9 services, real-time feeds) |
| Modular architecture | Harbinger's microservices show clear service boundaries work |
| Structured logging | tracing (Rust) equivalent to structlog (Python) patterns |

---

## 12. Harbinger Integration (Future Phases)

**Status:** 🔵 Future - Pending approval

**Vision:** Leverage Harbinger's event detection and ML infrastructure to inform Calchas prediction market strategies.

### 12.1 Integration Architecture (Option A: Loose Coupling)

**HARBINGER (Event Detection & Market Intelligence):**
- Market Context API (Port 8095): Volatility multipliers, Fear & Greed Index, trading session indicators
- Signal Aggregator API (Port 8087): Macro event detection, cross-market correlations, WebSocket macro events
- Sentiment Analysis: Daily/rolling sentiment scores, LLM-generated narratives

**Communication:** HTTP/WebSocket

**CALCHAS (Prediction Market Trading Bot):**
- Strategy Engine: Consumes Harbinger signals (optional)
- Bet Sizing Module: Applies volatility multipliers from Harbinger
- Market Filter: Avoids bets during high-volatility periods

### 12.2 Integration Phases

#### Phase 1: Market Context Integration (v2.0+)
**Goal:** Use Harbinger's volatility indicators to adjust Calchas bet sizing

**Implementation:**
- Calchas queries Harbinger's Market Context API (GET http://localhost:8095/context/multiplier)
- Response includes: multiplier, volatility level, fear_greed_index, trading session
- Adjust bet sizing: base_bet_size / volatility_multiplier (bet smaller during chaos)

**Benefits:**
- Reduce bet sizes during market chaos (avoid overexposure)
- Increase bet sizes during calm periods (capitalize on stable conditions)
- No code changes to Harbinger (consumes existing API)

**Risks:**
- Crypto volatility ≠ Sports prediction market volatility (needs validation)
- Requires both systems running simultaneously

---

#### Phase 2: Macro Event Awareness (v2.5+)
**Goal:** Pause or adjust Calchas strategies during major market events

**Implementation:**
- Calchas subscribes to Harbinger WebSocket (ws://localhost:8087/ws/macro-events)
- Receives alerts with: severity, title, affected_markets, confidence, narrative
- On critical events affecting relevant markets, pause new positions until clarity

**Benefits:**
- Avoid betting during black swan events
- Detect cross-market spillover (stocks crash → sports betting affected?)
- Real-time alerts without polling

**Risks:**
- False positives could freeze profitable trades
- Crypto events may not affect sports prediction markets

---

#### Phase 3: LLM-Powered Event Scoring (v3.0+)
**Goal:** Use Harbinger's dual-model LLM pipeline to evaluate prediction market events

**Implementation:**
- Calchas sends event to Harbinger (POST http://localhost:8090/events)
- Event includes: source_type, event_type, content (e.g., "NFL: Chiefs down 14-0 in Q2")
- Harbinger processes via dual-model: Fast model (Qwen2.5-3B) for initial eval, escalates to accurate model (Qwen3-4B) if uncertain
- Returns: is_significant, confidence, reasoning
- Calchas adjusts bet sizing based on confidence level

**Benefits:**
- Reuse Harbinger's proven LLM infrastructure (no new model servers)
- Dual-model strategy balances speed vs accuracy
- Semantic similarity pre-filtering reduces unnecessary LLM calls

**Risks:**
- Harbinger models trained on crypto/financial events (may need fine-tuning for sports)
- Adds latency to Calchas decision loop (~500-2000ms per LLM call)
- Requires Harbinger running (dependency)

---

#### Phase 4: Unified Intelligence Platform (v3.5+)
**Goal:** Expand Harbinger to become the "intelligence layer" for all prediction markets

**New Harbinger Service:** Prediction Market Analyzer (Port 8096)
- Monitors Kalshi/Polymarket event streams
- Classifies events using existing LLM pipeline
- Generates prediction market-specific signals
- Feeds into Harbinger's macro event detection

**Architecture:**
- Harbinger Signal Aggregator with new service: Prediction Market Analyzer
- Collects from: Twitter, RSS, Stock Tracker, Prediction Markets (Kalshi, Polymarket WebSockets)
- Feeds events to Event Processor (port 8090)
- Calchas consumes signals from Harbinger Signal Aggregator

**Benefits:**
- Single intelligence platform for all trading activities
- Prediction market events visible in Harbinger dashboard
- Cross-domain correlation detection (crypto news → sports betting impact?)

**Risks:**
- Tight coupling between Harbinger and Calchas
- Harbinger becomes critical path for Calchas (must be running)
- Scope creep (Harbinger was designed for crypto, not prediction markets)

---

### 12.3 Decision Matrix: When to Integrate?

| Phase | Trigger | Effort | Value |
|-------|---------|--------|-------|
| Phase 1: Market Context | Calchas v2.0 stable, profitable | Low (API calls only) | Medium (better risk management) |
| Phase 2: Macro Events | Frequent false entries during market chaos | Low (WebSocket sub) | Medium (avoid losses) |
| Phase 3: LLM Scoring | Manual event evaluation too slow | Medium (API integration + testing) | High (better entry signals) |
| Phase 4: Unified Platform | Running 5+ strategies across markets | High (new service) | High (single intelligence hub) |

**Recommendation:** Start with Phase 1 once Calchas is profitable. Evaluate Phase 2-4 based on actual pain points, not speculation.

---

### 12.4 Alternative: Keep Separate

**Calchas operates independently, no Harbinger integration.**

**Pros:**
- ✅ No dependencies (Calchas runs standalone)
- ✅ Simpler architecture (fewer moving parts)
- ✅ Faster iteration (no coordination between projects)

**Cons:**
- ❌ Duplicate infrastructure (both build confidence scoring, event processing)
- ❌ Missed opportunities (Harbinger detects market-wide events Calchas ignores)
- ❌ No code reuse (UI components, LLM patterns built separately)

**When to choose this:**
- Calchas remains a small, personal tool (<5 strategies)
- Prediction markets don't correlate with crypto/stock markets
- Prefer simplicity over optimization

---

## Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | Dec 2025 | Initial PRD |
| 1.1 | Dec 2025 | Updated tech stack (React instead of HTMX), expanded Harbinger integration details (section 11.3), added Harbinger Integration roadmap (section 12) |
