//! Volume tracking for volume spike detection
//!
//! Tracks market volume over time to detect sharp money entering markets.
//! Volume spikes (sudden increases in trading activity) often indicate:
//! - Sharp bettors entering positions
//! - New information hitting the market
//! - Institutional money flow
//!
//! Used by strategy evaluator to filter for markets showing unusual trading activity.

use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use crate::models::MarketId;

/// Single volume snapshot
#[derive(Debug, Clone)]
pub(crate) struct VolumeSnapshot {
    /// Total contracts traded (cumulative)
    pub(crate) volume: u64,
    pub(crate) timestamp: DateTime<Utc>,
}

/// Tracks volume history for spike detection
pub struct VolumeTracker {
    /// Map of market_id -> list of volume snapshots (newest first)
    pub(crate) snapshots: HashMap<MarketId, Vec<VolumeSnapshot>>,

    /// How long to keep historical data (default: 2 hours)
    retention_period: Duration,
}

impl VolumeTracker {
    /// Create a new volume tracker
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            retention_period: Duration::hours(2),
        }
    }

    /// Record current volume for a market
    pub fn record_volume(&mut self, market_id: &MarketId, volume: u64) {
        let snapshot = VolumeSnapshot {
            volume,
            timestamp: Utc::now(),
        };

        self.snapshots
            .entry(market_id.clone())
            .or_insert_with(Vec::new)
            .insert(0, snapshot); // Insert at front (newest first)

        // Clean old data for this market
        self.cleanup_market(market_id);
    }

    /// Calculate volume spike percentage over a time period
    ///
    /// Returns the percentage increase in volume rate compared to the average rate.
    ///
    /// **How it works:**
    /// 1. Calculate recent volume rate (contracts/hour in lookback period)
    /// 2. Calculate average volume rate (contracts/hour across all available history)
    /// 3. Return: (recent_rate / avg_rate - 1.0) * 100
    ///
    /// **Examples:**
    /// - 50% spike = recent volume is 1.5x the average rate
    /// - 100% spike = recent volume is 2x the average rate
    /// - -50% = recent volume is 0.5x the average rate (slowing down)
    ///
    /// Returns None if insufficient data available (need at least 3 snapshots).
    ///
    /// **Cold-start behavior:** If we don't have the full lookback period yet,
    /// uses the oldest available snapshot. This allows detecting volume spikes
    /// within seconds of startup.
    pub fn calculate_volume_spike(
        &self,
        market_id: &MarketId,
        lookback_period: Duration,
    ) -> Option<Decimal> {
        let snapshots = self.snapshots.get(market_id)?;

        // Need at least 3 snapshots to calculate meaningful volume spike
        // (1 current, 1 old for recent rate, 1+ for average rate)
        if snapshots.len() < 3 {
            return None;
        }

        let current = &snapshots[0];
        let target_time = Utc::now() - lookback_period;

        // Find snapshot from lookback_period ago
        let old_snapshot = snapshots.iter()
            .find(|s| s.timestamp <= target_time)
            .or_else(|| snapshots.get(1))?; // Use 2nd oldest if no match (cold-start)

        // Calculate recent volume rate (contracts/hour)
        let volume_change = current.volume.saturating_sub(old_snapshot.volume) as f64;
        let time_elapsed_hours = current.timestamp
            .signed_duration_since(old_snapshot.timestamp)
            .num_seconds() as f64 / 3600.0;

        if time_elapsed_hours <= 0.0 {
            return None; // Avoid division by zero
        }

        let recent_rate = volume_change / time_elapsed_hours;

        // Calculate average volume rate across all available history
        let oldest = snapshots.last()?;
        let total_volume = current.volume.saturating_sub(oldest.volume) as f64;
        let total_time_hours = current.timestamp
            .signed_duration_since(oldest.timestamp)
            .num_seconds() as f64 / 3600.0;

        if total_time_hours <= 0.0 {
            return None;
        }

        let avg_rate = total_volume / total_time_hours;

        if avg_rate <= 0.0 {
            return None; // Avoid division by zero
        }

        // Calculate spike percentage: (recent / average - 1) * 100
        let spike_pct = ((recent_rate / avg_rate) - 1.0) * 100.0;

        Some(Decimal::from_f64_retain(spike_pct).unwrap_or(Decimal::ZERO))
    }

    /// Check if a market has a volume spike >= X% in the lookback period
    ///
    /// Returns `true` if the market has a volume spike >= min_spike_pct in the lookback period.
    /// Returns `false` if no spike or insufficient data (< 3 snapshots).
    ///
    /// **Cold-start behavior:** Uses whatever data is available. If we only have
    /// 30 seconds of data, checks if volume spiked in those 30 seconds.
    pub fn has_volume_spike(
        &self,
        market_id: &MarketId,
        min_spike_pct: Decimal,
        lookback_period: Duration,
    ) -> bool {
        if let Some(spike_pct) = self.calculate_volume_spike(market_id, lookback_period) {
            spike_pct >= min_spike_pct
        } else {
            // Not enough data (< 3 snapshots) - can't determine spike
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

    /// Test helper: Insert a historical volume snapshot
    ///
    /// **WARNING: This is only for testing** - allows creating volume history with custom timestamps.
    /// Do not use in production code.
    ///
    /// **Note:** Insert snapshots in chronological order (oldest to newest).
    /// This function will insert them at the correct position to maintain newest-first ordering.
    #[allow(dead_code)]
    pub fn insert_test_snapshot(
        &mut self,
        market_id: &MarketId,
        volume: u64,
        timestamp: DateTime<Utc>,
    ) {
        let snapshot = VolumeSnapshot {
            volume,
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

impl Default for VolumeTracker {
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
    fn test_record_volume() {
        let mut tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Record volume
        tracker.record_volume(&market_id, 1000);
        assert_eq!(tracker.market_count(), 1);

        // Record another volume for same market
        tracker.record_volume(&market_id, 1500);
        assert_eq!(tracker.market_count(), 1); // Still 1 market

        // Verify newest is first
        let snapshots = tracker.snapshots.get(&market_id).unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].volume, 1500); // Newest
        assert_eq!(snapshots[1].volume, 1000); // Older
    }

    #[test]
    fn test_calculate_volume_spike_with_manual_snapshots() {
        let mut tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Simulate: 1000 contracts traded in first hour (avg rate = 1000/hr)
        // Then 2000 contracts in next 10 minutes (recent rate = 12000/hr = 12x spike)
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                VolumeSnapshot {
                    volume: 3000,  // Current total
                    timestamp: now,
                },
                VolumeSnapshot {
                    volume: 1000,  // 10 minutes ago (2000 contracts in 10 min)
                    timestamp: now - Duration::minutes(10),
                },
                VolumeSnapshot {
                    volume: 0,  // 1 hour ago (for average calculation)
                    timestamp: now - Duration::hours(1),
                },
            ]
        );

        // Calculate spike over last 10 minutes
        let spike = tracker.calculate_volume_spike(&market_id, Duration::minutes(10));
        assert!(spike.is_some());

        let spike_pct = spike.unwrap();

        // Recent rate: 2000 contracts / (10 min / 60) = 12000/hr
        // Avg rate: 3000 contracts / 1 hr = 3000/hr
        // Spike: (12000 / 3000 - 1) * 100 = 300% spike
        assert!(spike_pct >= dec!(290.0) && spike_pct <= dec!(310.0)); // Allow some rounding
    }

    #[test]
    fn test_has_volume_spike_positive() {
        let mut tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Simulate 100% volume spike (2x average rate)
        // Recent rate (last 30 min): 3000 contracts / 0.5 hr = 6000/hr
        // Avg rate (overall): 3000 contracts / 1 hr = 3000/hr
        // Spike: (6000 / 3000 - 1) * 100 = 100%
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                VolumeSnapshot {
                    volume: 3000,
                    timestamp: now,
                },
                VolumeSnapshot {
                    volume: 0,  // 3000 in last 30 min = 6000/hr rate
                    timestamp: now - Duration::minutes(30),
                },
                VolumeSnapshot {
                    volume: 0,  // 3000 in 1 hour total = 3000/hr avg
                    timestamp: now - Duration::hours(1),
                },
            ]
        );

        // Should pass with 50% minimum (100% > 50%)
        assert!(tracker.has_volume_spike(&market_id, dec!(50.0), Duration::minutes(30)));

        // Should fail with 150% minimum (100% < 150%)
        assert!(!tracker.has_volume_spike(&market_id, dec!(150.0), Duration::minutes(30)));
    }

    #[test]
    fn test_has_volume_spike_with_no_data() {
        let tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Should return false when no data available
        let has_spike = tracker.has_volume_spike(
            &market_id,
            dec!(50.0),
            Duration::minutes(10),
        );

        assert!(!has_spike);
    }

    #[test]
    fn test_has_volume_spike_insufficient_data() {
        let mut tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        // Only 2 snapshots (need 3 minimum)
        let now = Utc::now();
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                VolumeSnapshot {
                    volume: 2000,
                    timestamp: now,
                },
                VolumeSnapshot {
                    volume: 1000,
                    timestamp: now - Duration::minutes(10),
                },
            ]
        );

        // Should return false (not enough data)
        assert!(!tracker.has_volume_spike(&market_id, dec!(50.0), Duration::minutes(10)));
    }

    #[test]
    fn test_volume_spike_cold_start_uses_available_data() {
        let mut tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Simulate cold start: only 1 minute of data, but strategy wants 10 minutes
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                VolumeSnapshot {
                    volume: 1000,
                    timestamp: now,
                },
                VolumeSnapshot {
                    volume: 500,  // 500 contracts in 30 sec
                    timestamp: now - Duration::seconds(30),
                },
                VolumeSnapshot {
                    volume: 0,  // Start
                    timestamp: now - Duration::minutes(1),
                },
            ]
        );

        // Should calculate spike using available data (not wait for 10 minutes)
        let spike = tracker.calculate_volume_spike(&market_id, Duration::minutes(10));
        assert!(spike.is_some());
    }

    #[test]
    fn test_cleanup_all() {
        let mut tracker = VolumeTracker::new();

        tracker.record_volume(&MarketId::new("M1".to_string()), 1000);
        tracker.record_volume(&MarketId::new("M2".to_string()), 2000);

        assert_eq!(tracker.market_count(), 2);

        tracker.cleanup_all();

        // Markets should still exist (they're recent)
        assert_eq!(tracker.market_count(), 2);
    }

    #[test]
    fn test_cleanup_removes_old_data() {
        let mut tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                VolumeSnapshot {
                    volume: 2000,
                    timestamp: now,  // Recent
                },
                VolumeSnapshot {
                    volume: 1000,
                    timestamp: now - Duration::hours(3),  // Old (> 2 hour retention)
                },
            ]
        );

        tracker.cleanup_all();

        // Should only keep the recent snapshot
        let snapshots = tracker.snapshots.get(&market_id).unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].volume, 2000);
    }

    #[test]
    fn test_no_spike_when_volume_same() {
        let mut tracker = VolumeTracker::new();
        let market_id = MarketId::new("TEST-001".to_string());

        let now = Utc::now();

        // Simulate steady volume (1000 contracts/hour consistently)
        tracker.snapshots.insert(
            market_id.clone(),
            vec![
                VolumeSnapshot {
                    volume: 2000,
                    timestamp: now,
                },
                VolumeSnapshot {
                    volume: 1500,  // 500 in last 30 min
                    timestamp: now - Duration::minutes(30),
                },
                VolumeSnapshot {
                    volume: 0,  // 2000 in 2 hours = 1000/hr
                    timestamp: now - Duration::hours(2),
                },
            ]
        );

        // Recent rate: 500 / 0.5hr = 1000/hr
        // Avg rate: 2000 / 2hr = 1000/hr
        // Spike: (1000 / 1000 - 1) * 100 = 0%
        let spike = tracker.calculate_volume_spike(&market_id, Duration::minutes(30));
        assert!(spike.is_some());

        let spike_pct = spike.unwrap();
        assert!(spike_pct >= dec!(-5.0) && spike_pct <= dec!(5.0)); // ~0% (allow rounding)
    }
}
