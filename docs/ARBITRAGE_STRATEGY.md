# Arbitrage Strategy - Design & Implementation Plan

**Goal:** Build the best arbitrage scanner for Kalshi prediction markets
**Philosophy:** Math-based edge, not prediction-based gambling
**Target ROI:** 180%+ annually (conservative estimate)

---

## 📋 Table of Contents

1. [Strategy Overview](#strategy-overview)
2. [Why Arbitrage Works](#why-arbitrage-works)
3. [Phase 1: Cross-Market Arbitrage](#phase-1-cross-market-arbitrage)
4. [Phase 2: Correlated Market Arbitrage](#phase-2-correlated-market-arbitrage)
5. [Implementation Roadmap](#implementation-roadmap)
6. [Status Tracker](#status-tracker)

---

## Strategy Overview

### What is Arbitrage?

**Arbitrage = Risk-free profit from price inefficiencies**

Unlike momentum trading (betting on price direction), arbitrage exploits **mathematical certainties**:

```
Momentum Trading:
  "BTC is going up, so I'll buy YES" → GAMBLING (could go down)

Cross-Market Arbitrage:
  "YES + NO costs $0.95, settlement pays $1.00" → GUARANTEED $0.05 profit
```

### The Three Types We'll Build

| Type | Risk | Profit/Trade | Complexity | Phase |
|------|------|--------------|------------|-------|
| **Cross-Market** | None (hedged) | 2-4% | Easy | 1 |
| **Correlated Markets** | Low (statistical) | 5-10% | Medium | 2 |
| **Temporal** | None (logic-based) | 3-6% | Medium | 2 |

---

## Why Arbitrage Works

### Market Inefficiencies on Kalshi

**1. Separate Market Makers**
```
Market: "Will it rain tomorrow?"
Market Maker A sets: YES ask = $0.52
Market Maker B sets: NO ask = $0.49

They don't coordinate → YES + NO = $1.01 (impossible at settlement)

If you can buy YES at $0.48 and NO at $0.47:
  Total cost: $0.95
  Settlement: $1.00 (one side wins)
  Profit: $0.05 (5.3%)
```

**2. Low Liquidity**
- Many markets have <$10K volume
- Wide spreads (5-15%)
- Slow price discovery
- Manual traders dominate (not bots)

**3. Fee Structure Creates Gaps**
- Takers pay ~3% round-trip fees
- Market makers pay ~0.5%
- Creates pricing inefficiencies

**4. Slow Information Propagation**
- Related markets don't update together
- Correlated events misprice vs each other
- 10-30 second lag for human traders

---

## Phase 1: Cross-Market Arbitrage

### The Math

**Core Principle:**
```
YES price + NO price must equal $1.00 at settlement

If current: YES ask + NO ask < $0.98 (after fees)
  → Buy BOTH sides
  → Guaranteed profit when market settles
```

**Example Trade:**
```
Market: "Will Bitcoin hit $110K by Dec 31, 2025?"

Orderbook:
  YES ask: $0.48 (100 contracts available)
  NO ask: $0.47 (150 contracts available)

Trade:
  Buy 100 YES @ $0.48 = $48
  Buy 100 NO @ $0.47 = $47
  Total cost: $95

Settlement (Jan 1, 2025):
  Scenario A: BTC hits $110K
    → YES pays $100, NO pays $0
    → Total: $100

  Scenario B: BTC doesn't hit $110K
    → YES pays $0, NO pays $100
    → Total: $100

Profit: $100 - $95 = $5 (5.3% return)
Time to settlement: ~30 days
Annualized: 5.3% × 12 = 63.6%
```

### Detection Algorithm

**Step 1: Fetch All Markets**
```rust
// Every 10 seconds
let markets = kalshi_client.get_markets().await?;
```

**Step 2: Get Orderbooks**
```rust
for market in markets {
    let orderbook = kalshi_client.get_orderbook(&market.id).await?;

    let yes_ask = orderbook.yes_best_ask()?;  // Price to BUY YES
    let no_ask = orderbook.no_best_ask()?;    // Price to BUY NO

    // Check for arbitrage
    if yes_ask + no_ask < Decimal::from_str("0.98")? {
        opportunities.push(ArbitrageOpportunity {
            market_id: market.id,
            yes_ask,
            no_ask,
            profit_pct: (Decimal::ONE - (yes_ask + no_ask)) / (yes_ask + no_ask),
        });
    }
}
```

**Step 3: Rank by Profit**
```rust
opportunities.sort_by(|a, b| b.profit_pct.cmp(&a.profit_pct));
```

**Step 4: Execute (Simultaneously)**
```rust
// MUST execute both sides at same time
let yes_order = Order::market_buy(market_id, OrderSide::Yes, quantity);
let no_order = Order::market_buy(market_id, OrderSide::No, quantity);

// Execute in parallel
tokio::join!(
    executor.execute(yes_order),
    executor.execute(no_order),
);
```

### Risk Management

**Execution Risk:**
- ⚠️ What if only ONE side fills?
  - Solution: Use limit orders at calculated price
  - Cancel unfilled side immediately

**Liquidity Risk:**
- ⚠️ What if orderbook depth is insufficient?
  - Solution: Only trade up to min(yes_quantity, no_quantity)

**Fees Impact:**
- ⚠️ Fees reduce profit
  - Solution: Only execute if profit > 3% (covers fees + buffer)

### Expected Performance

**Conservative Estimate:**
```
Markets scanned: 500 (Kalshi has ~300-500 active)
Arbitrage opportunities per day: 5-10
Average profit per trade: 3%
Average capital per trade: $100
Trades per day: 5

Daily profit: 5 × $100 × 3% = $15
Monthly profit: $450
Annual profit: $5,400
Capital required: $3,000
Annual ROI: 180%
```

**Why conservative:**
- Assumes only 1-2% of markets have arbitrage
- Assumes small position sizes ($100)
- Assumes we miss 50% of opportunities (execution failures)

**Realistic upside:**
- With $10K capital: $18,000/year profit
- With better execution: 250%+ ROI
- With WebSocket (faster): 300%+ ROI

---

## Phase 2: Correlated Market Arbitrage

**NOTE: Phase 2 requires historical data collection from Phase 1**

### The Logic

**Markets don't exist in isolation. They're related:**

**Example: NBA Game**
```
Market A: "Will Lakers win tonight?" → 60% YES
Market B: "Will LeBron score 25+ points?" → 35% YES

Historical correlation (from 100 games):
  P(Lakers win | LeBron 25+) = 85%
  P(LeBron 25+ | Lakers win) = 50%

Current market pricing implies:
  P(Lakers win | LeBron 25+) = 60%

Mispricing: 85% (historical) vs 60% (market) = 25% edge
```

**Trade:**
```
Buy YES on Market B (LeBron 25+) at 35%
Hedge with NO on Market A (Lakers lose) at 40%

If LeBron scores 25+:
  - Market B pays: 100 contracts × $1.00 = $100
  - Market A loses: 70 contracts × $0 = $0
  - Net: +$100 - $35 (cost B) - $28 (cost A) = +$37

If LeBron doesn't score 25+:
  - Market B loses: $0
  - Market A outcome uncertain
  - Partial hedge limits downside
```

### Types of Correlations

**1. Same-Game Props (High Correlation)**
```
Game: Warriors vs Lakers

Market 1: "Will Warriors win?" → 65%
Market 2: "Will Steph Curry score 30+?" → 40%
Market 3: "Will Warriors score 115+ points?" → 55%

Correlation analysis:
  P(Warriors win ∩ Steph 30+) = ?
  P(Warriors win ∩ Team 115+) = ?

Historical data reveals true probabilities
Compare to market prices → find edge
```

**2. Series-Level Bets (Logical Constraints)**
```
Market 1: "Will Chiefs make playoffs?" → 95%
Market 2: "Will Chiefs win division?" → 97%

Logic: P(playoffs) ≥ P(win division) (always true)

If Market 2 > Market 1:
  → IMPOSSIBLE
  → Arbitrage: Sell Market 2, Buy Market 1
```

**3. Temporal Constraints**
```
Event: "Will it rain in NYC tomorrow?"

Market 1: "Rain by 12 PM" → 30%
Market 2: "Rain by 6 PM" → 25%

Logic: P(rain by 6pm) ≥ P(rain by 12pm)

Current: 25% < 30% → IMPOSSIBLE
→ Arbitrage opportunity
```

### Data Collection Strategy

**During Phase 1, collect:**
1. All market prices (every 10 seconds)
2. All market outcomes (when settled)
3. Market metadata (teams, players, events)
4. Orderbook snapshots

**Store in SQLite:**
```sql
CREATE TABLE market_prices (
    id INTEGER PRIMARY KEY,
    market_id TEXT,
    timestamp DATETIME,
    yes_price DECIMAL,
    no_price DECIMAL
);

CREATE TABLE market_outcomes (
    id INTEGER PRIMARY KEY,
    market_id TEXT,
    settled_at DATETIME,
    outcome TEXT,  -- 'yes' or 'no'
    final_price DECIMAL
);

CREATE TABLE market_metadata (
    market_id TEXT PRIMARY KEY,
    title TEXT,
    category TEXT,
    series_ticker TEXT,
    event_date DATETIME,
    related_markets TEXT  -- JSON array of related market IDs
);
```

**After 30 days:**
- Calculate correlation matrix
- Identify high-correlation pairs
- Build statistical models
- Deploy Phase 2

### Expected Performance (Phase 2)

**Conservative Estimate:**
```
Opportunities per day: 10-20 (more markets qualify)
Average profit per trade: 6%
Capital per trade: $200 (hedged, can deploy more)
Trades per day: 10

Daily profit: 10 × $200 × 6% = $120
Monthly profit: $3,600
Annual profit: $43,200
Capital required: $10,000
Annual ROI: 432%
```

---

## Implementation Roadmap

### Week 1: Cross-Market Arbitrage Scanner (Detection Only)

**Goal:** Identify and display arbitrage opportunities in real-time

**Deliverables:**
- [ ] New module: `src/arbitrage/mod.rs`
- [ ] Cross-market detector: `src/arbitrage/cross_market.rs`
- [ ] Opportunity model: `src/arbitrage/opportunity.rs`
- [ ] Calculator: `src/arbitrage/calculator.rs`
- [ ] Integration with main loop
- [ ] Console output showing opportunities

**Output Example:**
```
=== ARBITRAGE OPPORTUNITIES ===
[1] KXBTC-110K-2025 | YES: $0.48 NO: $0.47 | Total: $0.95 | Profit: 5.3% | Qty: 100
[2] KXRAIN-NYC-TMR | YES: $0.51 NO: $0.46 | Total: $0.97 | Profit: 3.1% | Qty: 75
[3] KXLAKERS-WIN | YES: $0.49 NO: $0.49 | Total: $0.98 | Profit: 2.0% | Qty: 50
```

**Success Criteria:**
- ✅ Scans all markets every 10 seconds
- ✅ Correctly identifies arbitrage (YES + NO < 0.98)
- ✅ Calculates profit % accurately
- ✅ Displays opportunities in real-time

---

### Week 2: Auto-Execution Engine

**Goal:** Automatically execute arbitrage trades

**Deliverables:**
- [ ] Arbitrage executor: `src/arbitrage/executor.rs`
- [ ] Simultaneous order execution (both sides)
- [ ] Fill monitoring
- [ ] Position tracking
- [ ] P&L measurement

**Features:**
- Execute both YES and NO orders in parallel
- Cancel unfilled orders after 5 seconds
- Track actual profit vs expected
- Alert on execution failures

**Success Criteria:**
- ✅ Both sides execute within 1 second
- ✅ Actual profit matches expected (within 0.5%)
- ✅ Zero execution failures over 10 trades

---

### Week 3: WebSocket Integration (Speed Upgrade)

**Goal:** Real-time price updates for faster execution

**Deliverables:**
- [ ] WebSocket client for Kalshi
- [ ] Real-time orderbook updates
- [ ] Sub-second opportunity detection
- [ ] Faster execution (reduce slippage)

**Why:**
- 10-second polling → miss fast-moving opportunities
- WebSocket → detect opportunities in <100ms
- Competitive edge over other bots

**Success Criteria:**
- ✅ Price updates in <500ms
- ✅ 2x more opportunities detected
- ✅ Higher execution success rate

---

### Week 4: Data Collection Infrastructure (Prep for Phase 2)

**Goal:** Start collecting historical data for correlation analysis

**Deliverables:**
- [ ] SQLite schema for historical data
- [ ] Background task: Record all prices every 10 seconds
- [ ] Settlement outcome recording
- [ ] Market metadata extraction
- [ ] Query tools for analysis

**Data to Collect:**
- Market prices (time series)
- Orderbook snapshots
- Market outcomes (win/loss)
- Related market mappings

**Success Criteria:**
- ✅ Collecting 500+ markets per scan
- ✅ 30 days of historical data
- ✅ Can query: "What's correlation between Market A and B?"

---

### Month 2: Phase 2 Implementation

**Prerequisites:**
- ✅ Phase 1 profitable for 30+ days
- ✅ Historical data collected
- ✅ WebSocket deployed

**Deliverables:**
- [ ] Correlation calculator
- [ ] Market pair detector
- [ ] Mispricing detector
- [ ] Hedge calculator
- [ ] Correlated arbitrage executor

**Goal:** 400%+ annual ROI from combined Phase 1 + Phase 2

---

## Status Tracker

### Current Status: **WEEK 1 DETECTION - READY FOR TESTING** ✅

### Phase 1: Cross-Market Arbitrage

| Task | Status | Started | Completed | Notes |
|------|--------|---------|-----------|-------|
| **Week 1: Detection** | | | | |
| Create arbitrage module structure | ✅ Complete | 2024-12-28 | 2024-12-28 | src/arbitrage/mod.rs created |
| Build cross-market detector | ✅ Complete | 2024-12-28 | 2024-12-28 | src/arbitrage/cross_market.rs with scan() method |
| Create opportunity model | ✅ Complete | 2024-12-28 | 2024-12-28 | ArbitrageOpportunity with profit calculations |
| Build profit calculator | ✅ Complete | 2024-12-28 | 2024-12-28 | ArbitrageCalculator with filtering and ranking |
| Integrate with main loop | ✅ Complete | 2024-12-28 | 2024-12-28 | Added to AppState, scan function in loop_handlers |
| Add console output | ✅ Complete | 2024-12-28 | 2024-12-28 | display_arbitrage_opportunities() with formatted table |
| Test with live data | ⬜ Pending | | | Ready to test with `cargo run` |
| **Week 2: Execution** | | | | |
| Build arbitrage executor | ⬜ Not Started | | | |
| Implement parallel order execution | ⬜ Not Started | | | |
| Add fill monitoring | ⬜ Not Started | | | |
| Create position tracker | ⬜ Not Started | | | |
| Add P&L measurement | ⬜ Not Started | | | |
| Execute first live trade | ⬜ Not Started | | | |
| Validate profit calculation | ⬜ Not Started | | | |
| **Week 3: Speed** | | | | |
| Design WebSocket client | ⬜ Not Started | | | |
| Implement real-time updates | ⬜ Not Started | | | |
| Optimize detection speed | ⬜ Not Started | | | |
| Deploy and test | ⬜ Not Started | | | |
| Measure performance improvement | ⬜ Not Started | | | |
| **Week 4: Data** | | | | |
| Design SQLite schema | ⬜ Not Started | | | |
| Build price recorder | ⬜ Not Started | | | |
| Add outcome tracking | ⬜ Not Started | | | |
| Extract market metadata | ⬜ Not Started | | | |
| Build query interface | ⬜ Not Started | | | |
| Verify data quality | ⬜ Not Started | | | |

### Phase 2: Correlated Market Arbitrage

| Task | Status | Started | Completed | Notes |
|------|--------|---------|-----------|-------|
| Collect 30 days historical data | ⬜ Not Started | | | Prerequisite |
| Build correlation calculator | ⬜ Not Started | | | |
| Detect market pairs | ⬜ Not Started | | | |
| Build mispricing detector | ⬜ Not Started | | | |
| Create hedge calculator | ⬜ Not Started | | | |
| Implement correlated executor | ⬜ Not Started | | | |
| Backtest strategy | ⬜ Not Started | | | |
| Deploy live | ⬜ Not Started | | | |

---

## Key Metrics to Track

### Phase 1 Metrics

**Detection:**
- Markets scanned per minute
- Arbitrage opportunities found per day
- Average profit % per opportunity
- False positive rate

**Execution:**
- Trades executed per day
- Execution success rate (%)
- Average slippage (actual vs expected)
- Fill rate (both sides filled)

**Profitability:**
- Total profit (USD)
- ROI (%)
- Win rate (should be 100% for arbitrage)
- Average hold time to settlement

**Target Metrics (Month 1):**
```
Opportunities detected: 5-10/day
Execution success: >80%
Average profit/trade: 3%+
Monthly ROI: 15%+ (180% annualized)
```

### Phase 2 Metrics

**Correlation Quality:**
- Market pairs identified
- Average correlation strength
- Prediction accuracy (%)

**Profitability:**
- Total profit (USD)
- ROI (%)
- Win rate (target: 70%+)
- Average hold time

**Target Metrics (Month 2):**
```
Correlated trades: 10-20/day
Win rate: 70%+
Average profit/trade: 6%+
Monthly ROI: 35%+ (432% annualized)
```

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2024-12-29 | Start with Phase 1 (Cross-Market) | Easiest to validate, guaranteed profit, build foundation |
| 2024-12-29 | Data collection in Phase 2 | Phase 1 doesn't need historical data, can start immediately |
| 2024-12-29 | Use 10-second polling initially | Good enough for Phase 1, upgrade to WebSocket in Week 3 |
| 2024-12-29 | SQLite for data storage | Simple, embedded, sufficient for correlation analysis |
| 2024-12-29 | Minimum 3% profit threshold | Covers fees (3%) + buffer for slippage |

---

## Risk Assessment

### Phase 1 Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Only one side fills | Medium | High | Use limit orders, cancel unfilled immediately |
| Slippage eats profit | Low | Medium | Only execute if profit > 3% |
| Market settles before both fills | Low | High | Check settlement time, avoid markets <1 hour to close |
| Kalshi API rate limits | Low | Medium | Respect rate limits, use exponential backoff |
| Insufficient liquidity | Medium | Low | Filter by min quantity (50+ contracts) |

### Phase 2 Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Correlations change over time | Medium | Medium | Re-calculate monthly, use rolling windows |
| Overfitting to historical data | Medium | High | Out-of-sample testing, conservative thresholds |
| Related markets don't settle together | Low | Medium | Track separately, don't assume coupling |
| Fees eat smaller edges | Medium | Medium | Only trade when edge > 5% |

---

## Next Steps

1. ✅ **Document created** - This file
2. ✅ **Review and approve plan** - User approved
3. ✅ **Create arbitrage module structure** - Complete
4. ✅ **Build cross-market detector** - Complete
5. ✅ **Integrate with existing codebase** - Complete
6. ⬜ **Test with live Kalshi API** - Run bot and verify opportunities detected
7. ⬜ **Start Week 2: Auto-execution** - Build arbitrage executor

**Week 1 Complete! Ready to test detection with live data. 🚀**

---

## How to Run

**Arbitrage mode (detection only - Week 1):**
```bash
cargo run --release -- --mode arbitrage
```

**Strategy mode (custom strategies from JSON):**
```bash
cargo run --release -- --mode strategy
```

**Show help:**
```bash
cargo run -- --help
```

See `USAGE.md` for full documentation.
