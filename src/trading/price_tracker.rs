//! Price tracking for momentum analysis
//!
//! Tracks market prices over time to calculate momentum (price change over a period).
//! Used by strategy evaluator to filter for markets showing movement.

use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use crate::models::MarketId;

/// Single price snapshot
#[derive(Debug, Clone)]
pub(crate) struct PriceSnapshot {
    pub(crate) yes_price: Decimal,
    #[allow(dead_code)]  // Stored for future NO-side momentum tracking
    pub(crate) no_price: Decimal,
    pub(crate) timestamp: DateTime<Utc>,
}

/// Tracks price history for momentum analysis
pub struct PriceTracker {
    /// Map of market_id -> list of price snapshots (newest first)
    pub(crate) snapshots: HashMap<MarketId, Vec<PriceSnapshot>>,

    /// How long to keep historical data (default: 2 hours)
    retention_period: Duration,
}

impl PriceTracker {
    /// Create a new price tracker
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            retention_period: Duration::hours(2),
        }
    }

    /// Record current prices for a market
    pub fn record_price(&mut self, market_id: &MarketId, yes_price: Decimal, no_price: Decimal) {
        let snapshot = PriceSnapshot {
            yes_price,
            no_price,
            timestamp: Utc::now(),
        };

        self.snapshots
            .entry(market_id.clone())
            .or_insert_with(Vec::new)
            .insert(0, snapshot); // Insert at front (newest first)

        // Clean old data for this market
        self.cleanup_market(market_id);
    }

    /// Calculate price change percentage over a time period
    ///
    /// Returns the percentage change for the YES side.
    /// Positive = price went up, Negative = price went down
    ///
    /// Returns None if insufficient data available (need at least 2 snapshots).
    ///
    /// **Cold-start behavior:** If we don't have the full lookback period yet,
    /// uses the oldest available snapshot. This allows detecting hot markets
    /// within seconds of startup (e.g., 2% move in 30 seconds is still valid momentum).
    pub fn calculate_momentum(
        &self,
        market_id: &MarketId,
        lookback_period: Duration,
    ) -> Option<Decimal> {
        let snapshots = self.snapshots.get(market_id)?;

        // Need at least 2 snapshots to calculate movement
        if snapshots.len() < 2 {
            return None;
        }

        // Current price (newest snapshot)
        let current = &snapshots[0];

        // Try to find snapshot from lookback_period ago
        let target_time = current.timestamp - lookback_period;

        let old_snapshot = snapshots.iter()
            .find(|s| s.timestamp <= target_time)
            .or_else(|| snapshots.last())?; // Use oldest available if no match (cold-start)

        // Calculate percentage change: ((new - old) / old) * 100
        if old_snapshot.yes_price == Decimal::ZERO {
            return None; // Avoid division by zero
        }

        let price_change = current.yes_price - old_snapshot.yes_price;
        let pct_change = (price_change / old_snapshot.yes_price) * Decimal::from(100);

        Some(pct_change)
    }

    /// Check if a market has moved at least X% in the lookback period
    ///
    /// Returns `true` if the market has moved >= min_pct_change in the lookback period.
    /// Returns `false` if no movement or insufficient data (< 2 snapshots).
    ///
    /// **Cold-start behavior:** Uses whatever data is available. If we only have
    /// 30 seconds of data, checks if it moved X% in those 30 seconds. This allows
    /// detecting hot markets within the first minute of startup.
    pub fn has_momentum(
        &self,
        market_id: &MarketId,
        min_pct_change: Decimal,
        lookback_period: Duration,
    ) -> bool {
        if let Some(momentum) = self.calculate_momentum(market_id, lookback_period) {
            momentum.abs() >= min_pct_change
        } else {
            // Not enough data (< 2 snapshots) - can't determine momentum
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

    /// Test helper: Insert a historical price snapshot
    ///
    /// **WARNING: This is only for testing** - allows creating price history with custom timestamps.
    /// Do not use in production code.
    pub fn insert_test_snapshot(
        &mut self,
        market_id: &MarketId,
        yes_price: Decimal,
        no_price: Decimal,
        timestamp: DateTime<Utc>,
    ) {
        let snapshot = PriceSnapshot {
            yes_price,
            no_price,
            timestamp,
        };

        self.snapshots
            .entry(market_id.clone())
            .or_insert_with(Vec::new)
            .push(snapshot);
    }
}

impl Default for PriceTracker {
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

    #[test]
    fn test_record_price() {
        let mut tracker = PriceTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Record a price
        tracker.record_price(&market_id, dec!(0.50), dec!(0.50));

        assert_eq!(tracker.market_count(), 1);

        // Record another price for same market
        tracker.record_price(&market_id, dec!(0.55), dec!(0.45));
        assert_eq!(tracker.market_count(), 1); // Still 1 market

        // Verify newest is first
        let snapshots = tracker.snapshots.get(&market_id).unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].yes_price, dec!(0.55)); // Newest
        assert_eq!(snapshots[1].yes_price, dec!(0.50)); // Older
    }

    #[test]
    fn test_calculate_momentum_with_manual_snapshots() {
        let mut tracker = PriceTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Manually create snapshots with different timestamps
        let now = Utc::now();
        let old_snapshot = PriceSnapshot {
            yes_price: dec!(0.50),
            no_price: dec!(0.50),
            timestamp: now - Duration::hours(1),
        };
        let new_snapshot = PriceSnapshot {
            yes_price: dec!(0.55),  // 10% increase
            no_price: dec!(0.45),
            timestamp: now,
        };

        tracker.snapshots.insert(
            market_id.clone(),
            vec![new_snapshot, old_snapshot]  // Newest first
        );

        // Calculate momentum over 1 hour
        let momentum = tracker.calculate_momentum(&market_id, Duration::hours(1));
        assert!(momentum.is_some());

        // 0.55 / 0.50 = 1.10 = 10% increase
        let pct_change = momentum.unwrap();
        assert_eq!(pct_change, dec!(10.0));
    }

    #[test]
    fn test_has_momentum_positive() {
        let mut tracker = PriceTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Manually create snapshots showing 5% gain
        let now = Utc::now();
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                PriceSnapshot {
                    yes_price: dec!(0.525),
                    no_price: dec!(0.475),
                    timestamp: now,
                },
                PriceSnapshot {
                    yes_price: dec!(0.50),
                    no_price: dec!(0.50),
                    timestamp: now - Duration::hours(1),
                },
            ]
        );

        // Should pass with 2% minimum (5% > 2%)
        assert!(tracker.has_momentum(&market_id, dec!(2.0), Duration::hours(1)));

        // Should fail with 10% minimum (5% < 10%)
        assert!(!tracker.has_momentum(&market_id, dec!(10.0), Duration::hours(1)));
    }

    #[test]
    fn test_has_momentum_negative() {
        let mut tracker = PriceTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Manually create snapshots showing 5% loss
        let now = Utc::now();
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                PriceSnapshot {
                    yes_price: dec!(0.475),  // Dropped from 0.50
                    no_price: dec!(0.525),
                    timestamp: now,
                },
                PriceSnapshot {
                    yes_price: dec!(0.50),
                    no_price: dec!(0.50),
                    timestamp: now - Duration::hours(1),
                },
            ]
        );

        // abs(momentum) check: |-5%| = 5% >= 2%
        assert!(tracker.has_momentum(&market_id, dec!(2.0), Duration::hours(1)));
    }

    #[test]
    fn test_has_momentum_with_no_data() {
        let tracker = PriceTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Should return false when no data available (can't determine momentum)
        let has_momentum = tracker.has_momentum(
            &market_id,
            dec!(2.0),
            Duration::hours(1),
        );

        assert!(!has_momentum);
    }

    #[test]
    fn test_has_momentum_cold_start_uses_oldest_available() {
        let mut tracker = PriceTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Simulate cold start: only 30 seconds of data, but strategy wants 60 minutes
        let now = Utc::now();
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                PriceSnapshot {
                    yes_price: dec!(0.51),  // 2% gain in 30 seconds
                    no_price: dec!(0.49),
                    timestamp: now,
                },
                PriceSnapshot {
                    yes_price: dec!(0.50),
                    no_price: dec!(0.50),
                    timestamp: now - Duration::seconds(30),  // Only 30 seconds ago
                },
            ]
        );

        // Strategy wants 2% over 60 minutes, but we only have 30 seconds of data
        // Should use the oldest available (30 seconds) and detect the 2% move
        let momentum = tracker.calculate_momentum(&market_id, Duration::hours(1));
        assert!(momentum.is_some());
        assert_eq!(momentum.unwrap(), dec!(2.0));  // 2% gain

        // Should pass momentum filter
        assert!(tracker.has_momentum(&market_id, dec!(2.0), Duration::hours(1)));
    }

    #[test]
    fn test_cleanup_all() {
        let mut tracker = PriceTracker::new();

        tracker.record_price(&MarketId::new("M1".to_string()), dec!(0.50), dec!(0.50));
        tracker.record_price(&MarketId::new("M2".to_string()), dec!(0.60), dec!(0.40));

        assert_eq!(tracker.market_count(), 2);

        tracker.cleanup_all();

        // Markets should still exist (they're recent)
        assert_eq!(tracker.market_count(), 2);
    }

    #[test]
    fn test_cleanup_removes_old_data() {
        let mut tracker = PriceTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Create snapshots with old timestamps
        let now = Utc::now();
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                PriceSnapshot {
                    yes_price: dec!(0.55),
                    no_price: dec!(0.45),
                    timestamp: now,  // Recent
                },
                PriceSnapshot {
                    yes_price: dec!(0.50),
                    no_price: dec!(0.50),
                    timestamp: now - Duration::hours(3),  // Old (> 2 hour retention)
                },
            ]
        );

        tracker.cleanup_all();

        // Should only keep the recent snapshot
        let snapshots = tracker.snapshots.get(&market_id).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].yes_price, dec!(0.55));
    }
}
