# CALCHAS
## Product Requirements Document

**Prediction Market Trading Bot**

| Field | Value |
|-------|-------|
| Version | 1.0 |
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

```
┌─────────────────────────────────────────────────────┐
│                     CALCHAS ENGINE                   │
├─────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │   Kalshi    │  │ Polymarket  │  │  Future...  │  │
│  │   Client    │  │   Client    │  │   Client    │  │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  │
│         └────────────┬───────────────────┘          │
│                      ▼                               │
│         ┌─────────────────────────┐                 │
│         │   Market Aggregator     │                 │
│         │   (unified data model)  │                 │
│         └───────────┬─────────────┘                 │
│                     ▼                                │
│         ┌─────────────────────────┐                 │
│         │    Strategy Engine      │◄── strategies/  │
│         │    (loads & executes)   │                 │
│         └───────────┬─────────────┘                 │
│                     ▼                                │
│         ┌─────────────────────────┐                 │
│         │   Order Manager         │                 │
│         │   (execute, track, exit)│                 │
│         └─────────────────────────┘                 │
└─────────────────────────────────────────────────────┘
```

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

Example: `strategies/momentum_scalp.json`

```json
{
  "name": "momentum_scalp",
  "description": "Buy cheap underdogs, exit on momentum spike",
  "version": "1.0",
  
  "filters": {
    "categories": ["sports:american_football", "sports:nhl"],
    "platforms": ["kalshi", "polymarket"],
    "min_favorite_price": 0.80,
    "max_underdog_price": 0.20,
    "min_liquidity_usd": 1000,
    "game_status": ["pre_game", "live"]
  },
  
  "entry": {
    "side": "underdog_only",
    "amount_usd": 10,
    "order_type": "market"
  },
  
  "exit": {
    "take_profit_pct": 50,
    "stop_loss_pct": -60,
    "trailing_stop_pct": null,
    "max_hold_minutes": 180
  },
  
  "risk": {
    "max_concurrent_positions": 5,
    "max_daily_loss_usd": 50,
    "cooldown_after_loss_minutes": 15
  }
}
```

### 5.3 Strategy Directory Organization

```
strategies/
├── sports/
│   ├── nfl_underdog.json
│   ├── nhl_volatility.json
│   └── soccer_late_goal.json
├── politics/
│   ├── poll_swing.json
│   └── election_eve.json
├── crypto/
│   ├── btc_momentum.json
│   └── etf_approval.json
├── economics/
│   └── fed_rate_surprise.json
└── generic/
    ├── cheap_longshot.json
    └── high_volume_scalp.json
```

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
| Web UI | Axum + HTMX (lightweight) |
| Logging | tracing |

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

```
calchas/
├── Cargo.toml
├── config/
│   └── default.toml              # API keys, defaults
├── strategies/
│   ├── momentum_scalp.json
│   ├── volatility_hedge.json
│   └── examples/
├── src/
│   ├── main.rs                   # CLI entry point
│   ├── daemon.rs                 # Background service
│   ├── lib.rs
│   ├── config/
│   │   └── mod.rs                # Config loading
│   ├── platforms/
│   │   ├── mod.rs
│   │   ├── kalshi.rs             # Kalshi client
│   │   └── polymarket.rs         # Polymarket client (v1.5)
│   ├── markets/
│   │   ├── mod.rs
│   │   ├── aggregator.rs         # Unified market view
│   │   └── types.rs              # Market, Position, Order
│   ├── strategy/
│   │   ├── mod.rs
│   │   ├── loader.rs             # JSON strategy parser
│   │   └── engine.rs             # Strategy execution
│   ├── trading/
│   │   ├── mod.rs
│   │   ├── orders.rs             # Order management
│   │   ├── positions.rs          # Position tracking
│   │   └── simulator.rs          # Paper trading
│   ├── storage/
│   │   └── sqlite.rs             # Trade history, state
│   └── web/
│       ├── mod.rs
│       ├── server.rs             # Axum server
│       └── templates/            # HTMX templates
├── migrations/                   # SQLite migrations
└── tests/
```

---

## 10. Run Modes

```bash
# One-off CLI run (dry-run simulation)
calchas run --strategy momentum_scalp.json --dry-run

# Start daemon (background process + web UI)
calchas daemon --port 8420

# Then visit http://localhost:8420 for live dashboard
```

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

### 11.3 Related Project

**Harbinger:** Social events aggregator. Calchas should follow similar architectural patterns for consistency.

---

## Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | Dec 2025 | Initial PRD |
