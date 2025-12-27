# Market Inefficiencies & Trading Opportunities

**Purpose:** Document market inefficiencies, arbitrage opportunities, and anomalies discovered during development.

**Last Updated:** December 26, 2024

---

## 1. Wide Bid-Ask Spreads

**Discovered:** 2024-12-26 during API investigation

**Market:** KXEARTHQUAKECALIFORNIA-35 (California 8.0+ magnitude earthquake before 2035)

**Anomaly:**
- Yes: 10 bid / 80 ask (70 cent spread!)
- No: 20 bid / 90 ask (70 cent spread!)
- Combined spread width: 140 cents total

**Implications:**
- Market makers are uncertain → wide spread
- Low liquidity → hard to get fills
- Potential opportunity: If you have conviction, you could provide liquidity at better prices
- Risk: Wide spread means high slippage for market orders

**Trading Strategy:**
- Use limit orders only
- Never use market orders on wide-spread markets
- Consider market-making: place bids/asks inside the current spread
- Filter criterion: `max_spread_width: 10 cents` to avoid these markets

---

## 2. Zero Bids (18 markets found)

**Discovered:** 2024-12-26 during edge case analysis

**Examples:**
- KXNEXTISRAELPM-45JAN01-YLEV
- KXNEXTISRAELPM-45JAN01-YGOL
- KXNEXTISRAELPM-45JAN01-IKAT
- ... (15 more)

**Pattern:** Israeli PM prediction markets with yes_bid = 0 cents

**Anomaly:**
- Yes bid = 0 cents (no one willing to buy)
- Yes ask = 3-7 cents (sellers asking for payment)
- This means market makers have NO bid for these outcomes

**Implications:**
- These are considered extremely unlikely events
- If you own contracts, you can't sell them (no bid!)
- Potential trap: Easy to buy, impossible to sell
- Could be a value opportunity if you have unique information

**Risk Considerations:**
- Illiquid → can't exit position
- Zero bid = zero liquidity on sell side
- Only trade if holding to expiration

**Filter Criteria:**
- Add `min_bid_price: 0.01` to avoid these markets
- Or track `has_bid: true` flag

---

## 3. Last Price vs Midpoint Discrepancy

**Discovered:** 2024-12-26 during price calculation comparison

**Observation:**
- In 8 out of 10 markets sampled, `last_price == yes_ask`
- This suggests most recent trades were market buy orders
- Midpoint is typically 1-4 cents lower than last price

**Example:** KXNEWPOPE-70-PPIZ
- Yes: 1 bid / 7 ask
- Midpoint: $0.04
- Last price: $0.07 (= ask)

**Implications:**
- Consistent buy pressure (market orders hitting asks)
- Midpoint underestimates current execution price
- If using midpoint for filtering, actual entry will be 1-4 cents worse

**Trading Strategy:**
- When signal uses midpoint ($0.04), actual entry will be at ask ($0.07)
- Account for this slippage in position sizing
- Consider using `ask` price for signal generation instead

---

## 4. Category Data Missing

**Discovered:** 2024-12-26 during API investigation

**Finding:** 100 out of 100 markets have `category: ""`

**Implication:**
- Cannot filter by category using Kalshi data
- Strategies targeting "Sports" or "Politics" won't work
- Must use alternative filtering:
  - Ticker patterns (e.g., "NFL", "ELECTION")
  - Title keyword parsing
  - Manual categorization

**Opportunity:**
- Build proprietary category mapping
- Could be competitive advantage if accurate
- Use NLP to auto-categorize from titles

---

## 5. Systematic Price Biases (TO BE INVESTIGATED)

**Hypothesis:** Markets might systematically misprice certain types of events

**To Research:**
- Long-dated vs short-dated markets
- High volume vs low volume bias
- Category-specific biases (if we can categorize)
- Time-of-day effects
- Day-of-week effects

**Method:**
- Collect historical data
- Compare predicted probabilities to actual outcomes
- Look for consistent over/under-pricing patterns

**Status:** Not yet investigated (requires historical data collection)

---

## 6. Arbitrage Between Platforms (FUTURE)

**Potential Opportunity:** Phase 7 will integrate Polymarket

**To Check:**
- Same events listed on both platforms at different prices
- Cross-platform arbitrage opportunities
- Transfer costs and timing

**Status:** Cannot investigate until Polymarket integration complete

---

## Notes for Developers

**When to update this file:**
- Discovering new price anomalies
- Finding systematic biases
- Identifying arbitrage opportunities
- Observing unusual market behavior
- After backtesting (compare predictions vs outcomes)

**Security:**
- DO NOT commit specific trade ideas to git
- This file documents patterns, not proprietary strategies
- Keep alpha-generating insights in private notes

---

## Action Items

- [ ] Add `min_bid_price` filter to strategies
- [ ] Add `max_spread_width` filter to strategies
- [ ] Build category inference from titles (NLP)
- [ ] Collect historical data for bias analysis
- [ ] Track slippage: midpoint vs actual execution
- [ ] Monitor zero-bid markets for changes (liquidity improving?)
