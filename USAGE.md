# Calchas Usage Guide

## Running the Bot

Calchas supports two trading modes via CLI argument:

### Arbitrage Mode (Recommended for $500 capital)

**What it does:**
- Scans all Kalshi markets for cross-market arbitrage opportunities
- Displays opportunities where YES + NO < $0.98
- Guaranteed profit (hedged position)
- Currently: Detection only (Week 1 - displays opportunities)

**Run command:**
```bash
cargo run --release -- --mode arbitrage
```

**Expected output:**
```
🔮 Calchas - Prediction Market Trading Bot
Mode: Arbitrage (SIMULATION - paper trading)

🎯 ARBITRAGE MODE: Scanning for cross-market opportunities
    Strategy: Buy YES + NO when total < $0.98
    Risk: Hedged (guaranteed profit at settlement)

=== ARBITRAGE SCAN 1 ===
🔍 Starting arbitrage scan...
Scanning 347 active markets
✅ Found 5 arbitrage opportunities

🎯 ARBITRAGE OPPORTUNITIES DETECTED
═══════════════════════════════════════════════════════════════
[1] Will Bitcoin hit $110K by Dec 31, 2025? (45d)
    YES: $0.48 | NO: $0.47 | Total: $0.95 | Profit: 5.3% | Annualized ROI: 64%
    Qty: 100 contracts | Capital: $95.00 | Expected profit: $5.00
    Market ID: KXBTC-110K-2025

[2] Will it rain in NYC tomorrow? (1d)
    YES: $0.51 | NO: $0.46 | Total: $0.97 | Profit: 3.1% | Annualized ROI: 1131%
    Qty: 75 contracts | Capital: $72.75 | Expected profit: $2.25
    Market ID: KXRAIN-NYC-TMR

📊 SUMMARY:
  Total opportunities: 5
  Average profit: 4.2%
  Capital to deploy all: $427.50
═══════════════════════════════════════════════════════════════
```

---

### Strategy Mode (For testing custom strategies)

**What it does:**
- Loads strategies from `strategies/*.json` files
- Executes custom trading logic defined in strategy configs
- Records prices, evaluates signals, executes trades

**Run command:**
```bash
cargo run --release -- --mode strategy
```

**Note:** Validate any strategy in paper trading mode before deploying real capital. Arbitrage mode recommended for proven edge.

---

## Quick Reference

```bash
# Show help
cargo run -- --help

# Run arbitrage scanner (recommended)
cargo run --release -- --mode arbitrage

# Run custom strategies from JSON files
cargo run --release -- --mode strategy

# Development mode (faster compile, slower runtime)
cargo run -- --mode arbitrage
```

---

## Capital Configuration

The bot auto-configures based on your starting capital:

**Small capital (<$1000):**
- Min profit: 4% (selective, high quality)
- Max per trade: $75
- Min time to settlement: 12 hours

**Medium capital ($1000-$3000):**
- Min profit: 3%
- Max per trade: $150
- Min time to settlement: 24 hours

**Large capital (>$3000):**
- Min profit: 2.5%
- Max per trade: $300
- Min time to settlement: 48 hours

Starting capital is read from your strategy JSON files (field: `risk_limits.max_daily_loss_usd`).

---

## Configuration Files

**Environment variables:** `.env`
```bash
# Kalshi API credentials
KALSHI_EMAIL=your_email@example.com
KALSHI_PASSWORD=your_password
KALSHI_PRIVATE_KEY_PATH=/path/to/private_key.pem

# Use demo API (recommended for testing)
KALSHI_USE_DEMO=true
```

**Bot config:** `config/config.toml`
```toml
[runtime]
default_min_time_minutes = 60
default_max_time_minutes = 10080  # 7 days
```

---

## Current Status

**Week 1: Detection (COMPLETE)**
- ✅ Arbitrage scanner built
- ✅ CLI mode selection
- ✅ Capital-aware configuration
- ✅ Real-time opportunity display
- ⬜ Auto-execution (Week 2)

**Week 2: Execution (NEXT)**
- Build arbitrage executor
- Parallel order execution (YES + NO)
- Position tracking
- P&L measurement

---

## Getting Help

```bash
# View available commands
cargo run -- --help

# Check project status
cat docs/PROJECT_STATUS.md

# View arbitrage strategy details
cat docs/ARBITRAGE_STRATEGY.md

# View test coverage
cat docs/TEST_STRATEGY.md
```

---

## Pro Tips

1. **Always use `--release` for production:** 10x faster execution
2. **Start with demo API:** Set `KALSHI_USE_DEMO=true` in `.env`
3. **Use arbitrage mode for $500:** Proven edge, guaranteed profit
4. **Monitor for 1-2 days:** Verify opportunities exist before deploying capital
5. **Week 2 adds execution:** Currently detection-only (no real trades)
