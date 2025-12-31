# Profit Optimization Roadmap
**Goal:** Increase daily profit from +$0.65 to +$15-20 through systematic optimization

**Current Status (Dec 31, 2024):**
- Win Rate: 100% (fake - losers don't exit)
- Avg Win: +$0.20
- Avg Loss: -$2.50 (uncontrolled)
- Net Daily: +$0.65
- **Just Added:** 15% SL + 4-hour max hold

---

## Phase 0: Data Collection (Week 1) ✅ IN PROGRESS

**Goal:** Collect 200+ trades with basic protection to establish baseline

**Status:** Bot running with:
- ✅ Stop loss: 15%
- ✅ Max hold time: 240 minutes
- ✅ Take profit: 5%

**What to Track:**
```
For each trade, record:
- Entry price, exit price
- Entry time, exit time (hold duration)
- Exit reason (TP, SL, MaxHold)
- Market type (NBA, NFL, Soccer, etc.)
- Time to settlement when entered
- P&L in dollars and percentage
```

**Success Criteria:**
- [ ] Collect 200+ trades
- [ ] Record REAL win rate (not 100%)
- [ ] Measure actual avg win/loss
- [ ] Calculate Sharpe ratio baseline

**Expected Baseline Metrics (after cleanup):**
- Win Rate: 85-90%
- Avg Win: +$0.20
- Avg Loss: -$0.75
- Net per 100 trades: +$2-5
- Daily trades: ~30-50
- **Net Daily: +$1-3** (baseline)

**Timeline:** Run for 7 days (collect 200-350 trades)

---

## Phase 1: Settlement-Aware Exits (Week 2) 🎯 HIGH ROI

**Goal:** Exit losers early, hold winners to settlement → Win Rate 90% → 95%

**Why First:** Prediction markets are unique - we KNOW when they settle. Use this!

### **Implementation Plan:**

#### **Step 1.1: Add Settlement Logic to Exit Manager**

**File:** `src/trading/exit_manager.rs`

**Add new exit reason:**
```rust
// In src/models/trade.rs (around line 15)
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    TrailingStop,
    MaxHoldTime,
    StrategyDisabled,
    ManualExit,
    MarketClosed,
    SettlementCutLoss,  // ✅ NEW: Cut losing position near settlement
}
```

**Add new check method:**
```rust
// In src/trading/exit_manager.rs (after check_max_hold_time)
pub fn check_settlement_logic(
    position: &Position,
    market: &Market,
    current_price: Decimal,
) -> bool {
    use chrono::Utc;

    // Calculate time to settlement
    let now = Utc::now();
    let time_to_settlement = market.event_time.signed_duration_since(now);
    let minutes_to_settlement = time_to_settlement.num_minutes();

    // Only apply within 30 minutes of settlement
    if minutes_to_settlement <= 0 || minutes_to_settlement > 30 {
        return false;
    }

    // Determine if position is winning or losing
    let is_winning = match position.side {
        OrderSide::Yes => current_price > position.entry_price,
        OrderSide::No => current_price > position.entry_price,
    };

    // If losing near settlement, exit NOW (you're wrong, cut it)
    // If winning, hold to settlement (free money)
    !is_winning
}
```

**Update main check_exits loop:**
```rust
// In src/trading/exit_manager.rs check_exits() method
// Add after max_hold_time check:

// 5. Settlement logic: Cut losers near settlement
if self.check_settlement_logic(position, &market, current_price) {
    exits_triggered.push((position.id, ExitReason::SettlementCutLoss));
    continue;
}
```

**Files to Modify:**
1. `src/models/trade.rs` - Add `SettlementCutLoss` enum variant
2. `src/trading/exit_manager.rs` - Add settlement check method
3. `src/trading/exit_manager.rs` - Update check_exits loop

**Testing:**
- [ ] Unit test: Position losing with 25 min to settlement → exit
- [ ] Unit test: Position winning with 25 min to settlement → hold
- [ ] Unit test: Position losing with 35 min to settlement → no exit
- [ ] Integration test: Verify losers exit before settlement

---

#### **Step 1.2: Add Settlement-Aware Strategy Flag**

**File:** `src/models/strategy.rs`

**Add field to StrategyFilters (line ~120):**
```rust
/// Enable settlement-aware exit logic (cut losers near settlement)
/// Default: false (backward compatible)
pub settlement_aware_exit: Option<bool>,
```

**Update strategy JSON:**
```json
{
  "filters": {
    "settlement_aware_exit": true  // ✅ Enable smart settlement exits
  }
}
```

**Files to Modify:**
1. `src/models/strategy.rs` - Add field
2. `strategies/order-flow-imbalance.json` - Enable flag
3. `strategies/TEMPLATE_all_fields.json` - Document feature

---

### **Expected Results (Phase 1):**

**Before Settlement Logic:**
- Win Rate: 88%
- Avg Win: +$0.20
- Avg Loss: -$0.75
- Net per 100 trades: +$2-5

**After Settlement Logic:**
- Win Rate: 93-95% ✅ (losers exit early)
- Avg Win: +$0.20 (unchanged)
- Avg Loss: -$0.30 ✅ (exit before full loss)
- Net per 100 trades: +$15-18
- **Net Daily: +$8-12** (3-4x improvement!)

**Timeline:** 3-4 days to implement + 3-4 days to validate

---

## Phase 2: Backtest-Driven Optimization (Week 3) 📊 DATA-DRIVEN

**Goal:** Find OPTIMAL parameters using YOUR data (not guesses)

### **Step 2.1: Build Backtesting Framework**

**File:** `src/backtest/mod.rs` (NEW)

**What It Does:**
- Replays historical trades with different parameters
- Tests SL: [5%, 10%, 15%, 20%, 25%]
- Tests MaxHold: [60, 120, 180, 240, 300 min]
- Tests TP: [3%, 5%, 7%, 10%]
- Calculates Sharpe ratio for each combination

**Implementation:**
```rust
// src/backtest/mod.rs
pub struct BacktestConfig {
    pub stop_loss_pct: Vec<Decimal>,     // [5, 10, 15, 20, 25]
    pub take_profit_pct: Vec<Decimal>,   // [3, 5, 7, 10]
    pub max_hold_minutes: Vec<i64>,      // [60, 120, 180, 240]
}

pub struct BacktestResult {
    pub params: (Decimal, Decimal, i64),  // (SL, TP, MaxHold)
    pub win_rate: f64,
    pub avg_win: Decimal,
    pub avg_loss: Decimal,
    pub sharpe_ratio: f64,
    pub max_drawdown: Decimal,
    pub total_return: Decimal,
}

pub fn run_backtest(
    trades: Vec<Trade>,
    config: BacktestConfig
) -> Vec<BacktestResult> {
    // For each parameter combination:
    //   - Replay trades
    //   - Apply exit rules
    //   - Calculate metrics
    //   - Store results
}
```

**Files to Create:**
1. `src/backtest/mod.rs` - Backtesting engine
2. `src/backtest/replay.rs` - Trade replay logic
3. `src/backtest/metrics.rs` - Sharpe, Sortino, max drawdown
4. `examples/run_backtest.rs` - CLI tool to run backtests

---

### **Step 2.2: Export Trade History**

**File:** `examples/export_trades.rs` (NEW)

**What It Does:**
- Exports all trades to CSV
- Includes: entry/exit prices, timestamps, hold duration, P&L, exit reason

**Usage:**
```bash
cargo run --example export_trades -- --output trades.csv
```

**CSV Format:**
```csv
trade_id,market_id,strategy_id,side,entry_price,exit_price,entry_time,exit_time,hold_minutes,pnl_usd,pnl_pct,exit_reason,time_to_settlement_at_entry
uuid-123,KXNBA...,order-flow,YES,0.75,0.79,2024-12-31T10:00,2024-12-31T11:00,60,0.20,5.3,TakeProfit,180
```

---

### **Step 2.3: Run Grid Search**

**Command:**
```bash
cargo run --example run_backtest -- \
  --trades trades.csv \
  --stop-loss 5,10,15,20,25 \
  --take-profit 3,5,7,10 \
  --max-hold 60,120,180,240
```

**Output:**
```
=== BACKTEST RESULTS ===
Testing 5 × 4 × 4 = 80 parameter combinations...

TOP 5 CONFIGURATIONS (by Sharpe Ratio):

1. SL=10%, TP=5%, MaxHold=180min
   Win Rate: 91.2%
   Avg Win: +$0.22
   Avg Loss: -$0.42
   Sharpe: 2.14 ⭐
   Max DD: -8.3%
   Total Return: +$127 (200 trades)

2. SL=15%, TP=7%, MaxHold=180min
   Win Rate: 88.5%
   Avg Win: +$0.28
   Avg Loss: -$0.58
   Sharpe: 2.01
   Max DD: -11.2%
   Total Return: +$118

3. SL=10%, TP=5%, MaxHold=120min
   Win Rate: 92.1%
   Avg Win: +$0.21
   Avg Loss: -$0.38
   Sharpe: 1.97
   Max DD: -7.1%
   Total Return: +$115

RECOMMENDATION: Use configuration #1
```

**Update strategy with optimal values:**
```json
{
  "exit_rules": {
    "take_profit_pct": "5.0",
    "stop_loss_pct": "10.0",      // ✅ Optimized (was 15%)
    "max_hold_time_minutes": 180  // ✅ Optimized (was 240)
  }
}
```

---

### **Expected Results (Phase 2):**

**Before Optimization:**
- Net per 100 trades: +$15-18
- Sharpe Ratio: 1.2-1.5

**After Optimization:**
- Net per 100 trades: +$20-25 ✅ (found optimal params)
- Sharpe Ratio: 1.8-2.2 ✅ (better risk-adjusted returns)
- **Net Daily: +$12-16**

**Timeline:** 4-5 days to build + 2 days to run + 1 day to validate

---

## Phase 3: Kelly Criterion Position Sizing (Week 4) 💰 MAXIMIZE GROWTH

**Goal:** Size positions optimally based on REAL edge

### **Step 3.1: Calculate True Edge**

**After 200+ trades with optimized params, you'll know:**
```
Win Rate (p) = 92%
Avg Win = +$0.22 per $5 position = +4.4% per trade
Avg Loss = -$0.42 per $5 position = -8.4% per trade

Expectancy = (0.92 × 4.4%) - (0.08 × 8.4%) = +3.4% per trade
```

**Kelly Formula:**
```
f* = (bp - q) / b
Where:
  b = avg_win / avg_loss = 4.4 / 8.4 = 0.524
  p = win_rate = 0.92
  q = 1 - p = 0.08

f* = (0.524 × 0.92 - 0.08) / 0.524
   = (0.482 - 0.08) / 0.524
   = 0.402 / 0.524
   = 0.767 = 76.7% of capital per trade
```

**But Kelly is AGGRESSIVE.** Use **Half Kelly** (safer):
```
Half Kelly = 76.7% / 2 = 38.4% per trade
```

---

### **Step 3.2: Implement Dynamic Position Sizing**

**File:** `src/trading/position_sizing.rs` (NEW)

**Add to strategy:**
```rust
pub enum PositionSizeUnit {
    Dollars,        // Fixed $5 per trade
    Contracts,      // Fixed 10 contracts
    KellyFraction,  // ✅ NEW: Kelly-based sizing
}

pub struct KellyConfig {
    pub win_rate: Decimal,
    pub avg_win_pct: Decimal,
    pub avg_loss_pct: Decimal,
    pub use_half_kelly: bool,  // Recommended: true
    pub max_position_pct: Decimal,  // Cap at 25% of capital
}

pub fn calculate_kelly_size(
    capital: Decimal,
    config: &KellyConfig
) -> Decimal {
    let b = config.avg_win_pct / config.avg_loss_pct;
    let p = config.win_rate;
    let q = Decimal::ONE - p;

    let kelly = (b * p - q) / b;
    let size = if config.use_half_kelly {
        kelly / Decimal::TWO
    } else {
        kelly
    };

    // Cap at max position size
    size.min(config.max_position_pct) * capital
}
```

**Update strategy JSON:**
```json
{
  "entry_rules": {
    "side": "ExpensiveSide",
    "position_size_unit": "KellyFraction",  // ✅ NEW
    "kelly_config": {
      "use_half_kelly": true,
      "max_position_pct": "0.25"  // Cap at 25% of capital per trade
    }
  }
}
```

---

### **Expected Results (Phase 3):**

**Before Kelly Sizing (Fixed $5):**
- Position size: $5 per trade
- Net per 100 trades: +$20-25
- **Net Daily (50 trades): +$10-12**

**After Kelly Sizing (with $500 capital):**
- Position size: $500 × 38.4% = $192 per trade (but capped at $125 = 25%)
- Net per 100 trades: +$20-25 × (125/5) = +$500-625
- **Net Daily (50 trades): +$250-312** 🚀

**⚠️ WARNING:** Kelly requires ACCURATE edge calculation. Use Half Kelly + cap to reduce risk.

**Timeline:** 3 days to implement + 5 days to validate carefully

---

## Phase 4: Market-Specific Optimization (Week 5) 🎯 SPECIALIZE

**Goal:** Different strategies for different markets (NBA ≠ Soccer ≠ Crypto)

### **Step 4.1: Analyze Performance by Market Type**

**Query trades database:**
```sql
SELECT
  market_type,
  COUNT(*) as trades,
  AVG(CASE WHEN net_pnl > 0 THEN 1 ELSE 0 END) as win_rate,
  AVG(net_pnl) as avg_pnl,
  AVG(hold_duration_minutes) as avg_hold
FROM trades
GROUP BY market_type;
```

**Example Results:**
```
Market Type    | Trades | Win Rate | Avg P&L | Avg Hold
---------------|--------|----------|---------|----------
NBA            | 80     | 94%      | +$0.25  | 90 min
NFL            | 40     | 88%      | +$0.18  | 180 min
Soccer (EPL)   | 35     | 85%      | +$0.12  | 120 min
College FB     | 45     | 82%      | +$0.08  | 150 min
```

**Insight:** NBA has highest win rate + profit → increase allocation!

---

### **Step 4.2: Create Market-Specific Strategies**

**Strategy 1: NBA High-Frequency**
```json
{
  "id": "nba-high-freq",
  "filters": {
    "series_ticker": ["KXNBAGAME"],
    "prioritize_live_games": true,
    "settlement_aware_exit": true
  },
  "entry_rules": {
    "position_size_unit": "KellyFraction",
    "kelly_config": { "max_position_pct": "0.30" }  // Higher allocation
  },
  "exit_rules": {
    "take_profit_pct": "4.0",  // Tighter (games are fast)
    "stop_loss_pct": "8.0",
    "max_hold_time_minutes": 90
  },
  "risk_limits": {
    "max_concurrent_positions": 15  // More NBA positions
  }
}
```

**Strategy 2: NFL Conservative**
```json
{
  "id": "nfl-conservative",
  "filters": {
    "series_ticker": ["KXNFLGAME"],
    "settlement_aware_exit": true
  },
  "exit_rules": {
    "take_profit_pct": "6.0",  // Wider (games are slower)
    "stop_loss_pct": "12.0",
    "max_hold_time_minutes": 180
  }
}
```

---

### **Expected Results (Phase 4):**

**Before Specialization:**
- Net Daily: +$12-16 (one strategy for all markets)

**After Specialization:**
- NBA strategy: +$8-10 daily
- NFL strategy: +$4-6 daily
- Soccer strategy: +$2-3 daily
- **Net Daily: +$14-19** ✅

**Timeline:** 3 days to analyze + 2 days to create strategies + 5 days to validate

---

## Phase 5: Advanced Features (Week 6+) 🚀 PROFESSIONAL-GRADE

### **5.1: Correlation-Aware Risk Management**

**Problem:** Holding 15 NBA positions in same game time slot = correlated risk

**Solution:**
```rust
pub fn check_correlation_risk(
    existing_positions: &[Position],
    new_signal: &EntrySignal
) -> bool {
    // Count positions in same game window (±30 min)
    let correlated = existing_positions.iter()
        .filter(|p| {
            let time_diff = (p.market.event_time - new_signal.market.event_time).abs();
            time_diff.num_minutes() < 30
        })
        .count();

    correlated < 10  // Max 10 positions in same game window
}
```

---

### **5.2: Volatility-Based Position Sizing**

**Concept:** Reduce size in high-volatility markets

```rust
pub fn adjust_for_volatility(
    base_size: Decimal,
    market: &Market,
    price_tracker: &PriceTracker
) -> Decimal {
    let volatility = price_tracker.calculate_std_dev(market.id, 60)?;

    // If volatility is 2x average, reduce size by 50%
    let vol_adjustment = Decimal::ONE / (Decimal::ONE + volatility);
    base_size * vol_adjustment
}
```

---

### **5.3: Machine Learning Price Prediction** (Optional)

**Use XGBoost/LightGBM to predict settlement probability:**

**Features:**
- Current price
- Volume in last hour
- Order flow imbalance
- Time to settlement
- Market type
- Day of week, hour of day

**Output:** Predicted settlement probability → adjust confidence

---

## 📊 Complete Roadmap Timeline

| Phase | Duration | Goal | Expected Daily Profit |
|-------|----------|------|----------------------|
| **Phase 0** | Week 1 | Data collection | +$1-3 (baseline) |
| **Phase 1** | Week 2 | Settlement-aware exits | +$8-12 |
| **Phase 2** | Week 3 | Backtest optimization | +$12-16 |
| **Phase 3** | Week 4 | Kelly sizing | +$250-312 🚀 |
| **Phase 4** | Week 5 | Market specialization | +$280-350 |
| **Phase 5** | Week 6+ | Advanced features | +$350-500+ |

---

## 🎯 Success Metrics by Phase

### **Phase 1 (Settlement-Aware):**
- [ ] Win Rate: 93-95%
- [ ] Avg Loss: <$0.40
- [ ] Max Drawdown: <12%
- [ ] Sharpe Ratio: >1.5

### **Phase 2 (Optimized Params):**
- [ ] Sharpe Ratio: >2.0
- [ ] Avg Win/Loss Ratio: >2.0
- [ ] Net per 100 trades: >$20

### **Phase 3 (Kelly Sizing):**
- [ ] Capital growth: >5% per week
- [ ] Max position size: <30% of capital
- [ ] No single-trade loss >5% of capital

### **Phase 4 (Specialization):**
- [ ] 3+ market-specific strategies
- [ ] Each strategy profitable independently
- [ ] Combined Sharpe >2.5

---

## 🚦 Risk Gates (DON'T SKIP PHASES)

**Gate 1 (before Phase 2):**
- ✅ 200+ trades collected
- ✅ Real win rate measured (not 100%)
- ✅ Settlement logic implemented and tested

**Gate 2 (before Phase 3):**
- ✅ Backtest shows improvement
- ✅ Win rate stable >90%
- ✅ Max drawdown <15%

**Gate 3 (before Phase 4):**
- ✅ Kelly sizing validated in small size
- ✅ No catastrophic losses
- ✅ Capital preserved

---

## 📝 Implementation Priority

### **MUST DO (High ROI, Low Complexity):**
1. ✅ Phase 0: Basic protection (DONE - SL + MaxHold)
2. 🎯 Phase 1: Settlement-aware exits (NEXT - 3-4 days)
3. 📊 Phase 2: Backtest framework (CRITICAL - find optimal params)

### **SHOULD DO (High ROI, Medium Complexity):**
4. 💰 Phase 3: Kelly sizing (BIG profit boost)
5. 🎯 Phase 4: Market specialization (Incremental gains)

### **NICE TO HAVE (Lower ROI, High Complexity):**
6. 🚀 Phase 5: Advanced features (Professional polish)

---

## 🎉 Final Target

**6 Weeks from Now:**
- Starting capital: $500
- Daily profit: +$280-350
- Win rate: 93-96%
- Sharpe ratio: 2.5-3.0
- Max drawdown: <15%
- **Monthly profit: +$6,000-7,500** (1200-1500% monthly return!) 🚀

**This is aggressive but achievable if:**
- ✅ Your edge is real (we'll validate in Phase 0)
- ✅ You don't skip phases (collect data first!)
- ✅ You use proper risk management (Half Kelly + caps)

---

**Ready to start Phase 1 (Settlement-Aware Exits)?** This is the highest ROI next step!
