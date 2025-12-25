# Kalshi Fee Structure

**Last Updated:** October 1, 2025
**Source:** [Kalshi Help Center - Trading Fees](https://help.kalshi.com/trading/fees)

## Overview

Kalshi charges fees based on a formula that scales with market uncertainty. The fee is lowest when trading highly certain outcomes (prices near $0 or $1) and highest when trading maximum uncertainty (prices near $0.50).

## Fee Formula

### Taker Fee (Market Orders)
```
Fee = 0.07 × Contracts × Price × (1 - Price)
```

**When it applies:** You're taking liquidity from the order book (immediate execution)

### Maker Fee (Limit Orders)
```
Fee = 0.0175 × Contracts × Price × (1 - Price)
```

**When it applies:** You're adding liquidity to the order book (your order rests on the book)

## Fee Constants

| Type | Rate | Description |
|------|------|-------------|
| Taker | 7.0% | Market orders (immediate execution) |
| Maker | 1.75% | Limit orders (resting on book) |
| **Savings** | **75%** | **Using maker vs taker orders** |

### Fee Cap
- **$1.75 per 100 contracts** (taker orders only)
- Prevents excessive fees on trades near $0.50
- Applies per side of the trade (entry and exit are separate)

## Mathematical Properties

The formula `P × (1 - P)` creates a parabolic fee structure:

### Fee Peaks at 50¢ (Maximum Uncertainty)
- At P = $0.50: `0.50 × 0.50 = 0.25` (maximum value)
- This is where the fee cap kicks in

### Fee Approaches $0 at Extremes
- At P = $0.01: `0.01 × 0.99 = 0.0099` (very small)
- At P = $0.99: `0.99 × 0.01 = 0.0099` (same as $0.01)
- High-conviction trades (very cheap or very expensive) cost less

## Fee Behavior Examples

Based on 100 contracts:

| Price | Taker Fee (Single Side) | Notes |
|-------|------------------------|-------|
| $0.01 | $0.0693 | Very cheap - high conviction |
| $0.11 | $0.6853 | Proven entry price |
| $0.24 | $1.2768 | Proven exit price |
| **$0.50** | **$1.7500** | **Maximum - hits fee cap** |
| $0.75 | $1.3125 | Expensive but certain |
| $0.99 | $0.0693 | Nearly certain - mirrors $0.01 |

### Round-Trip Trade Example

**Proven trade: Buy at $0.11, sell at $0.24 (100 contracts)**

#### Using Market Orders (Taker)
- Entry fee: $0.6853
- Exit fee: $1.2768
- **Total fees: $1.9621**
- Gross profit: $13.00
- Net profit: $11.04
- Net return: 100.34%

#### Using Limit Orders (Maker)
- Entry fee: $0.1713
- Exit fee: $0.3192
- **Total fees: $0.4905**
- Gross profit: $13.00
- Net profit: $12.51
- Net return: 113.72%

#### Savings
- **$1.47 saved (75% less fees)**
- **+13.38% higher return**

## Strategic Implications

### 1. Default to Limit Orders
- 75% fee savings is massive
- Only use market orders when speed is critical
- Consider: "Is immediate execution worth $1.47 on this trade?"

### 2. Exit Thresholds Must Be Fee-Aware
- Don't exit at 10% profit if fees eat 8% of it
- Calculate minimum profitable exit price including fees
- Example: If entry was $0.11, exit must clear $0.13+ after fees for 10% real return

### 3. Trading Near 50¢ is Expensive
- Fee cap means less penalty than formula suggests
- But still $1.75 per side = $3.50 round trip per 100 contracts
- Need larger price moves to overcome fees

### 4. High-Conviction Trades Get Better Economics
- Trading at $0.05 or $0.95 has minimal fees
- Underdog hunting (buying < $0.10) has excellent fee economics
- Extreme mispricing opportunities are fee-efficient

### 5. Position Sizing Matters
- Fees scale linearly with quantity
- 1000 contracts at $0.50 = $17.50 per side
- Large positions need proportionally larger profits

## Fee Rounding

- Fees are **rounded up to the nearest cent**
- If total rounding adds more than $10/month, Kalshi refunds the excess
- This is negligible for most trading strategies

## Additional Notes

### No Minimum Fee
- Fees can be fractions of a cent (before rounding)
- Very small trades still pay at least 1¢ after rounding

### Fee Changes
- Kalshi can change fee structure with notice
- Always check: https://help.kalshi.com/trading/fees
- Update constants in `src/kalshi/fees.rs` if changed

### API Rate Limits
- Separate from fees
- Check Kalshi API docs for current limits

## Code Reference

All fee calculations are implemented in:
- **Constants:** `src/kalshi/fees.rs` (TAKER_FEE_RATE, MAKER_FEE_RATE, FEE_CAP_PER_100_CONTRACTS)
- **Functions:** `src/kalshi/fees.rs` (calculate_kalshi_taker_fee, calculate_kalshi_maker_fee, etc.)
- **Usage:** See `src/main.rs` for examples

## Sources

1. [Kalshi Trading Fees Help Center](https://help.kalshi.com/trading/fees)
2. [Kalshi Fee Schedule PDF](https://kalshi.com/docs/kalshi-fee-schedule.pdf)
3. [Kalshi API Documentation](https://trading-api.readme.io/reference/getting-started)

---

**Last verified:** December 24, 2025
**Fee structure effective:** October 1, 2025
