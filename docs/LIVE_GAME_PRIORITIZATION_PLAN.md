# LIVE Game Prioritization Implementation Plan

## Summary
Implement 2-tier market evaluation that prioritizes LIVE games (expiring <2h with recent trading activity) before evaluating other markets. This ensures high-volatility, actively-playing games are captured first while maintaining backward compatibility.

## User-Confirmed Requirements
1. ✅ Add `volume_24h` to Market struct (store 24-hour volume from API)
2. ✅ Per-strategy configuration (`prioritize_live_games` field in strategy JSON)
3. ✅ 2-tier evaluation (LIVE first, fallback to non-LIVE if capacity remains)
4. ✅ LIVE criteria: Expiring <2h, >30% recent volume ratio, >1000 total volume (VALIDATED with real Kalshi data)
5. ✅ Use 54 official sports series tickers from Kalshi API

## LIVE Game Detection Criteria (✅ VALIDATED)

A market is "LIVE" if **ALL** conditions are met:

1. **Time to event < 2 hours (120 minutes)** ⚠️ VALIDATED (changed from 6h after real-world testing)
   - Rationale: Sports games last 2-3h. If ending in <2h, game is actively being played.
   - Crypto markets: use `close_time` (accurate)
   - Sports/Politics: use `event_time` (close_time is placeholder ~14 days)
   - Validated: Detected 12 LIVE games on Dec 30, 2025, matching Kalshi's website

2. **Recent volume ratio > 30%**
   - Formula: `volume_24h / volume > 0.30`
   - Indicates active trading in last 24 hours

3. **Total volume > 1000 contracts**
   - Minimum liquidity threshold

## Implementation Steps

### Step 1: Add `volume_24h` Field to Market Struct
**File:** `src/models/market.rs`

**Add field after `volume` (line ~82):**
```rust
pub volume: u64,         // Total contracts traded
pub volume_24h: u64,     // NEW: 24-hour volume (for LIVE detection)
pub open_interest: u64,  // Outstanding contracts
```

**Update test factory:** Add `volume_24h: 0` to `create_test_market()` (line ~239)

---

### Step 2: Preserve `volume_24h` from API Response
**File:** `src/kalshi/types.rs`

**Update conversion (line ~209):**
```rust
volume: km.volume.max(0) as u64,
volume_24h: km.volume_24h.max(0) as u64,  // NEW: Handle sentinel -1 → 0
open_interest: km.open_interest.max(0) as u64,
```

---

### Step 3: Add `is_live_game()` Method to Market
**File:** `src/models/market.rs`

**Add method after `is_crypto_market()` (line ~128):**
```rust
/// Determine if this market is a LIVE game (high-urgency entry opportunity).
///
/// LIVE criteria (ALL must be met):
/// - Time to event < 2 hours (uses close_time for crypto, event_time for sports)
/// - Recent volume ratio > 30% (volume_24h / volume > 0.30)
/// - Total volume > 1000 contracts
///
/// Rationale: Sports games last 2-3h. If ending in <2h, game is actively being played.
pub fn is_live_game(&self) -> bool {
    use chrono::Utc;

    // 1. Time to event check (<2 hours = 120 minutes)
    let now = Utc::now();
    let time_to_event = if self.is_crypto_market() {
        self.close_time.signed_duration_since(now)
    } else {
        self.event_time.signed_duration_since(now)
    };
    let minutes_to_event = time_to_event.num_minutes();

    if minutes_to_event < 0 || minutes_to_event >= 120 {
        return false;
    }

    // 2. Total volume check (>1000 contracts)
    if self.volume <= 1000 {
        return false;
    }

    // 3. Recent volume ratio check (>30%)
    if self.volume == 0 {
        return false;  // Avoid division by zero
    }

    let recent_volume_ratio = self.volume_24h as f64 / self.volume as f64;
    recent_volume_ratio > 0.30
}
```

**Add comprehensive unit tests for:**
- All criteria met → true
- Missing any criterion → false
- Edge cases: zero volume, negative time, boundary values (2h, 1000 volume, 30% ratio)
- Crypto vs sports timing logic

---

### Step 4: Add `prioritize_live_games` to Strategy Config
**File:** `src/models/strategy.rs`

**Add field after `min_order_flow_imbalance` (line ~115):**
```rust
/// Enable LIVE game prioritization (evaluate LIVE markets first)
/// LIVE = expiring <2h, >30% recent volume ratio, >1000 total volume
/// Default: false (backward compatible)
pub prioritize_live_games: Option<bool>,
```

**Update tests:** Add `prioritize_live_games: None` to test strategy construction

---

### Step 5: Update Strategy Template Documentation
**File:** `strategies/TEMPLATE_all_fields.json`

**Add after `min_order_flow_imbalance` (line ~78):**
```json
"prioritize_live_games": false,
"_prioritize_live_note": "Enable LIVE game prioritization. If true, markets expiring <2h with >30% recent volume are evaluated FIRST.",
"_prioritize_criteria": "LIVE = time_to_event < 2h AND volume_24h/volume > 0.30 AND volume > 1000",
"_prioritize_example": "NBA game in 4th quarter (ending in 1h) = LIVE, game starting in 3 hours = non-LIVE",
"_prioritize_rationale": "Sports games last 2-3h. If ending in <2h, game is actively being played.",
"_prioritize_default": "false (backward compatible - no sorting)",
```

---

### Step 6: Implement 2-Tier Market Evaluation
**File:** `src/loop_handlers.rs`

**Replace `evaluate_strategies()` function (lines 285-324):**

**Key Logic:**
1. Check if strategy has `prioritize_live_games: true`
2. If yes:
   - Split markets into LIVE and non-LIVE using `.partition(|m| m.is_live_game())`
   - Evaluate LIVE markets first
   - Calculate remaining capacity: `max_concurrent - current_positions - live_signals`
   - Evaluate non-LIVE markets only if capacity remains
   - Log stats: LIVE count, non-LIVE count, signal counts
3. If no:
   - Evaluate all markets as before (no sorting)

**Important Details:**
- Use `.partition()` for O(n) split (efficient)
- Check `remaining_capacity > 0` before evaluating non-LIVE
- Chain signals: `live_signals.into_iter().chain(non_live_signals.into_iter())`
- Preserve signal order: LIVE first, non-LIVE second

**Logging Points:**
- "Prioritizing {live_count} LIVE games, {non_live_count} non-LIVE games"
- "Generated {count} signals from LIVE games"
- "Remaining capacity: {slots}"
- "Generated {count} signals from non-LIVE games" (or "Skipping non-LIVE games (no capacity)")

---

## Edge Cases Handled

1. **Division by zero:** `is_live_game()` checks `volume == 0` before calculating ratio
2. **Sentinel value (-1):** Conversion uses `.max(0)` to convert `-1 → 0`
3. **Past events:** `is_live_game()` checks `minutes_to_event < 0`
4. **Crypto vs sports timing:** Uses `is_crypto_market()` to select correct timestamp
5. **No LIVE markets:** `if !live_markets.is_empty()` before evaluation
6. **Zero remaining capacity:** `if remaining_capacity > 0` before evaluating non-LIVE
7. **"Both" strategy:** Risk manager handles duplicate market+side checks

---

## Backward Compatibility

**Existing strategies (no changes required):**
- `prioritize_live_games` is `Option<bool>` defaulting to `None`
- `None.unwrap_or(false)` → no prioritization
- Evaluation order unchanged
- **Zero breaking changes**

**New strategies (opt-in):**
- Add `"prioritize_live_games": true` to JSON
- LIVE games prioritized automatically

---

## Testing Strategy

**Unit Tests:**
1. `is_live_game()` method (all criteria combinations)
2. Strategy JSON deserialization (`prioritize_live_games` field)

**Integration Tests:**
1. No prioritization (existing behavior preserved)
2. LIVE prioritization enabled (split + order verification)
3. Edge cases: all LIVE, all non-LIVE, empty markets

**Manual Validation:**
1. Run bot with `prioritize_live_games: true`
2. Verify logs show LIVE/non-LIVE split counts
3. Check signals from USC vs TCU, NBA games appear first
4. Measure performance (should be <5ms overhead for 10,000 markets)

---

## Risks & Mitigation

| Risk | Mitigation |
|------|------------|
| **volume_24h data quality** | Sentinel value handling (`-1 → 0`), safe default |
| **False LIVE detection** | Conservative thresholds (30%, 1000 vol, 2h), validated with real data |
| **Performance degradation** | O(n) partition is fast, prioritization is optional |
| **Breaking existing strategies** | `Option<bool>` with backward-compatible default |

---

## Files Modified

1. ✅ `src/models/market.rs` - Add `volume_24h` field + `is_live_game()` method
2. ✅ `src/kalshi/types.rs` - Preserve `volume_24h` in conversion
3. ✅ `src/loop_handlers.rs` - Implement 2-tier evaluation in `evaluate_strategies()`
4. ✅ `src/models/strategy.rs` - Add `prioritize_live_games` to StrategyFilters
5. ✅ `strategies/TEMPLATE_all_fields.json` - Document new field

---

## Success Criteria

- ✅ Bot catches USC vs TCU, NBA, NHL LIVE games
- ✅ LIVE games generate signals before non-LIVE games
- ✅ Existing strategies work without modification
- ✅ No performance regression (<5ms overhead)
- ✅ All unit tests pass
- ✅ Logs show clear LIVE/non-LIVE split statistics
