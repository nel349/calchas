//! Order flow tracking for buy/sell pressure detection
//!
//! Tracks orderbook liquidity over time to detect institutional money flow.
//! Order Flow Imbalance (OFI) reveals where smart money is positioning:
//! - Positive OFI = More buy-side liquidity (bullish pressure)
//! - Negative OFI = More sell-side liquidity (bearish pressure)
//!
//! Used by HFT firms and professional traders for short-term price prediction.

use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use crate::models::{MarketId, Orderbook};

/// Single order flow snapshot
#[derive(Debug, Clone)]
pub(crate) struct OrderFlowSnapshot {
    /// Total liquidity on bid side (sum of top N levels)
    pub(crate) bid_liquidity: u64,

    /// Total liquidity on ask side (sum of top N levels)
    pub(crate) ask_liquidity: u64,

    pub(crate) timestamp: DateTime<Utc>,
}

/// Tracks order flow history for imbalance detection
pub struct OrderFlowTracker {
    /// Map of market_id -> list of order flow snapshots (newest first)
    pub(crate) snapshots: HashMap<MarketId, Vec<OrderFlowSnapshot>>,

    /// How many orderbook levels to sum (default: 3)
    /// Professional traders typically use top 3-5 levels
    depth_levels: usize,

    /// How long to keep historical data (default: 2 hours)
    retention_period: Duration,
}

impl OrderFlowTracker {
    /// Create a new order flow tracker
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            depth_levels: 3,  // Top 3 levels (industry standard)
            retention_period: Duration::hours(2),
        }
    }

    /// Record current orderbook state for a market
    ///
    /// # Arguments
    ///
    /// * `orderbook` - The orderbook to record
    ///
    /// **Note:** This sums the top N levels (default 3) from yes_asks and no_asks
    pub fn record_orderbook(&mut self, orderbook: &Orderbook) {
        // Sum top N levels from YES side (best prices are LAST in Kalshi orderbook)
        let yes_liquidity = Self::sum_top_levels(&orderbook.yes_asks, self.depth_levels);

        // Sum top N levels from NO side
        let no_liquidity = Self::sum_top_levels(&orderbook.no_asks, self.depth_levels);

        let snapshot = OrderFlowSnapshot {
            bid_liquidity: no_liquidity,   // NO asks = bullish pressure (people selling NO = bullish on YES)
            ask_liquidity: yes_liquidity,  // YES asks = bearish pressure (people selling YES = bearish on YES)
            timestamp: Utc::now(),
        };

        self.snapshots
            .entry(orderbook.market_id.clone())
            .or_insert_with(Vec::new)
            .insert(0, snapshot); // Insert at front (newest first)

        // Clean old data for this market
        self.cleanup_market(&orderbook.market_id);
    }

    /// Sum liquidity across top N orderbook levels
    ///
    /// **Note:** Kalshi orderbook is sorted ascending, so best prices are LAST
    fn sum_top_levels(levels: &[crate::models::market::OrderbookLevel], count: usize) -> u64 {
        levels
            .iter()
            .rev()  // Reverse to get best prices first
            .take(count)
            .map(|level| level.quantity)
            .sum()
    }

    /// Calculate Order Flow Imbalance (OFI) for a market
    ///
    /// # Formula
    ///
    /// ```text
    /// OFI = (bid_liquidity - ask_liquidity) / (bid_liquidity + ask_liquidity)
    /// ```
    ///
    /// # Returns
    ///
    /// * `Some(Decimal)` - OFI value ranging from -1.0 to +1.0
    ///   - **+1.0** = All buy-side liquidity (maximum bullish pressure)
    ///   - **0.0** = Balanced orderbook
    ///   - **-1.0** = All sell-side liquidity (maximum bearish pressure)
    /// * `None` - No data available
    ///
    /// # Examples
    ///
    /// * **OFI = +0.5** → 3:1 bid/ask ratio (75% bids, 25% asks) = bullish
    /// * **OFI = -0.5** → 1:3 bid/ask ratio (25% bids, 75% asks) = bearish
    /// * **OFI = +0.3** → 65% bids, 35% asks = slightly bullish
    pub fn calculate_ofi(&self, market_id: &MarketId) -> Option<Decimal> {
        let snapshots = self.snapshots.get(market_id)?;

        if snapshots.is_empty() {
            return None;
        }

        // Get latest snapshot
        let snapshot = &snapshots[0];

        let total = snapshot.bid_liquidity + snapshot.ask_liquidity;

        if total == 0 {
            return None;  // No liquidity = can't calculate OFI
        }

        // OFI = (Bids - Asks) / (Bids + Asks)
        let bid_signed = snapshot.bid_liquidity as i128;
        let ask_signed = snapshot.ask_liquidity as i128;
        let total_signed = total as i128;

        let ofi_value = (bid_signed - ask_signed) as f64 / total_signed as f64;

        Decimal::from_f64_retain(ofi_value)
    }

    /// Check if a market has order flow imbalance >= threshold
    ///
    /// # Arguments
    ///
    /// * `market_id` - The market to check
    /// * `min_imbalance` - Minimum OFI required (e.g., 0.3 = 65% buy-side liquidity)
    ///
    /// # Returns
    ///
    /// `true` if OFI >= min_imbalance, `false` otherwise
    ///
    /// **Note:** This checks absolute value, so both +0.3 and -0.3 pass a 0.3 threshold
    pub fn has_order_flow_imbalance(
        &self,
        market_id: &MarketId,
        min_imbalance: Decimal,
    ) -> bool {
        if let Some(ofi) = self.calculate_ofi(market_id) {
            ofi.abs() >= min_imbalance
        } else {
            false
        }
    }

    /// Calculate OFI trend (change in OFI over time)
    ///
    /// Detects if order flow pressure is GROWING (momentum building) or FADING (momentum dying).
    ///
    /// # Arguments
    ///
    /// * `market_id` - The market to check
    /// * `lookback` - How far back to compare (e.g., Duration::seconds(30))
    ///
    /// # Returns
    ///
    /// * `Some(Decimal)` - Change in OFI value
    ///   - **Positive** = OFI is growing (pressure intensifying)
    ///   - **Negative** = OFI is shrinking (pressure fading)
    ///   - **Example:** +0.3 means OFI increased from 0.2 to 0.5 in lookback period
    /// * `None` - Insufficient data (need at least 2 snapshots spanning lookback period)
    ///
    /// # Examples
    ///
    /// * **OFI: 0.2 → 0.6 in 30 sec** → Trend = +0.4 (strong momentum building) ✅
    /// * **OFI: 0.6 → 0.6 in 30 sec** → Trend = 0.0 (stale signal) ⚠️
    /// * **OFI: 0.6 → 0.3 in 30 sec** → Trend = -0.3 (momentum fading) ❌
    pub fn calculate_ofi_trend(
        &self,
        market_id: &MarketId,
        lookback: Duration,
    ) -> Option<Decimal> {
        let snapshots = self.snapshots.get(market_id)?;

        // Need at least 2 snapshots to calculate trend
        if snapshots.len() < 2 {
            return None;
        }

        // Calculate current OFI from most recent snapshot
        let current_snapshot = &snapshots[0];
        let current_total = current_snapshot.bid_liquidity + current_snapshot.ask_liquidity;
        if current_total == 0 {
            return None;
        }

        let current_ofi = {
            let bid_signed = current_snapshot.bid_liquidity as i128;
            let ask_signed = current_snapshot.ask_liquidity as i128;
            let total_signed = current_total as i128;
            let ofi_value = (bid_signed - ask_signed) as f64 / total_signed as f64;
            Decimal::from_f64_retain(ofi_value)?
        };

        // Find snapshot from lookback period ago
        let target_time = current_snapshot.timestamp - lookback;
        let old_snapshot = snapshots
            .iter()
            .find(|s| s.timestamp <= target_time)
            .or_else(|| snapshots.get(1))?; // Fallback to 2nd newest if no exact match

        // Calculate old OFI
        let old_total = old_snapshot.bid_liquidity + old_snapshot.ask_liquidity;
        if old_total == 0 {
            return None;
        }

        let old_ofi = {
            let bid_signed = old_snapshot.bid_liquidity as i128;
            let ask_signed = old_snapshot.ask_liquidity as i128;
            let total_signed = old_total as i128;
            let ofi_value = (bid_signed - ask_signed) as f64 / total_signed as f64;
            Decimal::from_f64_retain(ofi_value)?
        };

        // Return change in OFI (positive = growing pressure)
        Some(current_ofi - old_ofi)
    }

    /// Check if OFI is trending strongly (momentum is accelerating)
    ///
    /// # Arguments
    ///
    /// * `market_id` - The market to check
    /// * `min_trend` - Minimum OFI change required (e.g., 0.2 = OFI must grow by 0.2+ in lookback period)
    /// * `lookback` - How far back to measure trend
    ///
    /// # Returns
    ///
    /// `true` if OFI trend (absolute value) >= min_trend, `false` otherwise
    ///
    /// **Example:** min_trend = 0.2 means OFI must have changed by at least 0.2 (e.g., 0.3→0.5 or 0.7→0.5)
    pub fn has_ofi_trend(
        &self,
        market_id: &MarketId,
        min_trend: Decimal,
        lookback: Duration,
    ) -> bool {
        if let Some(trend) = self.calculate_ofi_trend(market_id, lookback) {
            trend.abs() >= min_trend
        } else {
            false
        }
    }

    /// Remove snapshots older than retention period for a specific market
    fn cleanup_market(&mut self, market_id: &MarketId) {
        if let Some(snapshots) = self.snapshots.get_mut(market_id) {
            let cutoff = Utc::now() - self.retention_period;
            snapshots.retain(|s| s.timestamp >= cutoff);
        }
    }

    /// Remove all old snapshots (call periodically to free memory)
    pub fn cleanup_all(&mut self) {
        let cutoff = Utc::now() - self.retention_period;

        // Remove old snapshots
        for snapshots in self.snapshots.values_mut() {
            snapshots.retain(|s| s.timestamp >= cutoff);
        }

        // Remove markets with no snapshots
        self.snapshots.retain(|_, snapshots| !snapshots.is_empty());
    }

    /// Get number of markets being tracked
    pub fn market_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Test helper: Insert a historical order flow snapshot
    ///
    /// **WARNING: This is only for testing** - allows creating order flow history with custom timestamps.
    /// Do not use in production code.
    #[allow(dead_code)]
    pub fn insert_test_snapshot(
        &mut self,
        market_id: &MarketId,
        bid_liquidity: u64,
        ask_liquidity: u64,
        timestamp: DateTime<Utc>,
    ) {
        let snapshot = OrderFlowSnapshot {
            bid_liquidity,
            ask_liquidity,
            timestamp,
        };

        let snapshots = self.snapshots
            .entry(market_id.clone())
            .or_insert_with(Vec::new);

        // Insert at correct chronological position (newest first)
        let insert_pos = snapshots
            .iter()
            .position(|s| s.timestamp < timestamp)
            .unwrap_or(snapshots.len());

        snapshots.insert(insert_pos, snapshot);
    }
}

impl Default for OrderFlowTracker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use crate::models::market::OrderbookLevel;

    fn create_test_orderbook(market_id: &str, yes_levels: Vec<(Decimal, u64)>, no_levels: Vec<(Decimal, u64)>) -> Orderbook {
        Orderbook {
            market_id: MarketId::new(market_id.to_string()),
            yes_asks: yes_levels.into_iter().map(|(price, qty)| OrderbookLevel { price, quantity: qty }).collect(),
            no_asks: no_levels.into_iter().map(|(price, qty)| OrderbookLevel { price, quantity: qty }).collect(),
        }
    }

    #[test]
    fn test_record_orderbook() {
        let mut tracker = OrderFlowTracker::new();

        let orderbook = create_test_orderbook(
            "TEST-001",
            vec![(dec!(0.50), 100), (dec!(0.51), 200), (dec!(0.52), 300)],  // YES: 600 total
            vec![(dec!(0.48), 50), (dec!(0.49), 100), (dec!(0.50), 150)],   // NO: 300 total
        );

        tracker.record_orderbook(&orderbook);

        assert_eq!(tracker.market_count(), 1);

        let snapshots = tracker.snapshots.get(&orderbook.market_id).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].bid_liquidity, 300);  // NO asks (bullish pressure)
        assert_eq!(snapshots[0].ask_liquidity, 600);  // YES asks (bearish pressure)
    }

    #[test]
    fn test_calculate_ofi_bullish() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // 75% buy-side liquidity (3:1 ratio)
        tracker.insert_test_snapshot(&market_id, 750, 250, Utc::now());

        let ofi = tracker.calculate_ofi(&market_id).unwrap();

        // OFI = (750 - 250) / (750 + 250) = 500 / 1000 = 0.5
        assert_eq!(ofi, dec!(0.5));
    }

    #[test]
    fn test_calculate_ofi_bearish() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // 25% buy-side liquidity (1:3 ratio)
        tracker.insert_test_snapshot(&market_id, 250, 750, Utc::now());

        let ofi = tracker.calculate_ofi(&market_id).unwrap();

        // OFI = (250 - 750) / (250 + 750) = -500 / 1000 = -0.5
        assert_eq!(ofi, dec!(-0.5));
    }

    #[test]
    fn test_calculate_ofi_balanced() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Perfectly balanced
        tracker.insert_test_snapshot(&market_id, 500, 500, Utc::now());

        let ofi = tracker.calculate_ofi(&market_id).unwrap();

        // OFI = (500 - 500) / (500 + 500) = 0 / 1000 = 0.0
        assert_eq!(ofi, dec!(0.0));
    }

    #[test]
    fn test_calculate_ofi_no_data() {
        let tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let ofi = tracker.calculate_ofi(&market_id);
        assert!(ofi.is_none());
    }

    #[test]
    fn test_has_order_flow_imbalance_bullish() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // OFI = +0.5 (bullish)
        tracker.insert_test_snapshot(&market_id, 750, 250, Utc::now());

        // Should pass with 0.3 threshold
        assert!(tracker.has_order_flow_imbalance(&market_id, dec!(0.3)));

        // Should fail with 0.6 threshold
        assert!(!tracker.has_order_flow_imbalance(&market_id, dec!(0.6)));
    }

    #[test]
    fn test_has_order_flow_imbalance_bearish() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // OFI = -0.5 (bearish)
        tracker.insert_test_snapshot(&market_id, 250, 750, Utc::now());

        // Should pass with 0.3 threshold (abs value)
        assert!(tracker.has_order_flow_imbalance(&market_id, dec!(0.3)));

        // Should fail with 0.6 threshold
        assert!(!tracker.has_order_flow_imbalance(&market_id, dec!(0.6)));
    }

    #[test]
    fn test_sum_top_levels() {
        let levels = vec![
            OrderbookLevel { price: dec!(0.45), quantity: 50 },   // Stale
            OrderbookLevel { price: dec!(0.48), quantity: 100 },  // Level 3
            OrderbookLevel { price: dec!(0.49), quantity: 200 },  // Level 2
            OrderbookLevel { price: dec!(0.50), quantity: 300 },  // Level 1 (best)
        ];

        // Sum top 3 levels: 300 + 200 + 100 = 600
        let sum = OrderFlowTracker::sum_top_levels(&levels, 3);
        assert_eq!(sum, 600);
    }

    #[test]
    fn test_sum_top_levels_fewer_than_requested() {
        let levels = vec![
            OrderbookLevel { price: dec!(0.49), quantity: 100 },
            OrderbookLevel { price: dec!(0.50), quantity: 200 },
        ];

        // Only 2 levels available, sum both
        let sum = OrderFlowTracker::sum_top_levels(&levels, 3);
        assert_eq!(sum, 300);
    }

    #[test]
    fn test_cleanup_all() {
        let mut tracker = OrderFlowTracker::new();

        let ob1 = create_test_orderbook("M1", vec![(dec!(0.50), 100)], vec![(dec!(0.50), 100)]);
        let ob2 = create_test_orderbook("M2", vec![(dec!(0.50), 100)], vec![(dec!(0.50), 100)]);

        tracker.record_orderbook(&ob1);
        tracker.record_orderbook(&ob2);

        assert_eq!(tracker.market_count(), 2);

        tracker.cleanup_all();

        // Markets should still exist (they're recent)
        assert_eq!(tracker.market_count(), 2);
    }

    #[test]
    fn test_cleanup_removes_old_data() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Recent snapshot
        tracker.insert_test_snapshot(&market_id, 500, 500, now);

        // Old snapshot (>2 hours)
        tracker.insert_test_snapshot(&market_id, 300, 700, now - Duration::hours(3));

        tracker.cleanup_all();

        // Should only keep recent snapshot
        let snapshots = tracker.snapshots.get(&market_id).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].bid_liquidity, 500);
    }

    #[test]
    fn test_calculate_ofi_trend_growing() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Old snapshot: OFI = (200 - 800) / 1000 = -0.6 (bearish)
        tracker.insert_test_snapshot(&market_id, 200, 800, now - Duration::seconds(30));

        // Current snapshot: OFI = (700 - 300) / 1000 = +0.4 (bullish)
        tracker.insert_test_snapshot(&market_id, 700, 300, now);

        // Trend: +0.4 - (-0.6) = +1.0 (massive bullish reversal)
        let trend = tracker.calculate_ofi_trend(&market_id, Duration::seconds(30));
        assert!(trend.is_some());

        let trend_value = trend.unwrap();
        assert!(trend_value >= dec!(0.9) && trend_value <= dec!(1.1)); // Allow rounding
    }

    #[test]
    fn test_calculate_ofi_trend_fading() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Old snapshot: OFI = (800 - 200) / 1000 = +0.6 (bullish)
        tracker.insert_test_snapshot(&market_id, 800, 200, now - Duration::seconds(30));

        // Current snapshot: OFI = (300 - 700) / 1000 = -0.4 (bearish)
        tracker.insert_test_snapshot(&market_id, 300, 700, now);

        // Trend: -0.4 - (+0.6) = -1.0 (momentum fading/reversing)
        let trend = tracker.calculate_ofi_trend(&market_id, Duration::seconds(30));
        assert!(trend.is_some());

        let trend_value = trend.unwrap();
        assert!(trend_value <= dec!(-0.9) && trend_value >= dec!(-1.1));
    }

    #[test]
    fn test_calculate_ofi_trend_stable() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Old snapshot: OFI = (600 - 400) / 1000 = +0.2
        tracker.insert_test_snapshot(&market_id, 600, 400, now - Duration::seconds(30));

        // Current snapshot: OFI = (600 - 400) / 1000 = +0.2 (unchanged)
        tracker.insert_test_snapshot(&market_id, 600, 400, now);

        // Trend: +0.2 - (+0.2) = 0.0 (stale signal)
        let trend = tracker.calculate_ofi_trend(&market_id, Duration::seconds(30));
        assert!(trend.is_some());

        let trend_value = trend.unwrap();
        assert!(trend_value >= dec!(-0.05) && trend_value <= dec!(0.05)); // ~0
    }

    #[test]
    fn test_calculate_ofi_trend_insufficient_data() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Only 1 snapshot
        tracker.insert_test_snapshot(&market_id, 500, 500, Utc::now());

        // Should return None (need at least 2 snapshots)
        let trend = tracker.calculate_ofi_trend(&market_id, Duration::seconds(30));
        assert!(trend.is_none());
    }

    #[test]
    fn test_has_ofi_trend_positive() {
        let mut tracker = OrderFlowTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Trend = +0.5 (OFI increased from 0.0 to 0.5)
        tracker.insert_test_snapshot(&market_id, 500, 500, now - Duration::seconds(30));
        tracker.insert_test_snapshot(&market_id, 750, 250, now);

        // Should pass with 0.3 threshold (0.5 > 0.3)
        assert!(tracker.has_ofi_trend(&market_id, dec!(0.3), Duration::seconds(30)));

        // Should fail with 0.6 threshold (0.5 < 0.6)
        assert!(!tracker.has_ofi_trend(&market_id, dec!(0.6), Duration::seconds(30)));
    }
}
