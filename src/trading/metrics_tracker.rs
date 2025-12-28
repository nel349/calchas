//! Performance metrics tracking and exit-to-live validation
//!
//! This module tracks simulation performance to validate readiness for live trading.
//! It implements the exit-to-live criteria from PRD Section 2.4.
//!
//! # Exit-to-Live Criteria
//!
//! Before deploying with real capital, the bot must demonstrate:
//! 1. **7+ consecutive profitable days**
//! 2. **Net positive ROI** across all trades
//! 3. **No single-day loss > 15%** of starting capital
//!
//! # Example
//!
//! ```no_run
//! use calchas::trading::MetricsTracker;
//! # use calchas::models::Trade;
//! use rust_decimal_macros::dec;
//!
//! # fn example(trade: Trade) {
//! let mut tracker = MetricsTracker::new(dec!(10000.00));
//!
//! // Record trades as they close
//! tracker.record_trade(&trade);
//!
//! // Check metrics
//! let metrics = tracker.calculate_metrics();
//! println!("Consecutive profitable days: {}", metrics.consecutive_profitable_days);
//! println!("Net ROI: {:.2}%", metrics.net_roi);
//!
//! // Validate exit-to-live
//! match tracker.check_exit_to_live() {
//!     calchas::trading::ExitToLiveDecision::Approved => {
//!         println!("Ready for live trading!");
//!     }
//!     calchas::trading::ExitToLiveDecision::NotReady { unmet_criteria } => {
//!         println!("Not ready yet:");
//!         for criterion in unmet_criteria {
//!             println!("  - {}", criterion);
//!         }
//!     }
//! }
//! # }
//! ```

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::models::Trade;

// =============================================================================
// DATA TYPES
// =============================================================================

/// Daily performance record
#[derive(Debug, Clone)]
pub struct DailyRecord {
    /// Date for this record
    pub date: NaiveDate,

    /// Number of trades closed this day
    pub trade_count: u32,

    /// Total P&L for the day (net of fees)
    pub daily_pnl: Decimal,

    /// Was this day profitable?
    pub is_profitable: bool,

    /// Win count (profitable trades)
    pub wins: u32,

    /// Loss count (unprofitable trades)
    pub losses: u32,

    /// Total profit from winning trades
    pub total_win_amount: Decimal,

    /// Total loss from losing trades
    pub total_loss_amount: Decimal,
}

impl DailyRecord {
    /// Create a new empty daily record
    fn new(date: NaiveDate) -> Self {
        DailyRecord {
            date,
            trade_count: 0,
            daily_pnl: Decimal::ZERO,
            is_profitable: false,
            wins: 0,
            losses: 0,
            total_win_amount: Decimal::ZERO,
            total_loss_amount: Decimal::ZERO,
        }
    }

    /// Add a trade to this day's record
    fn add_trade(&mut self, trade: &Trade) {
        self.trade_count += 1;
        self.daily_pnl += trade.net_pnl;

        if trade.net_pnl > Decimal::ZERO {
            self.wins += 1;
            self.total_win_amount += trade.net_pnl;
        } else if trade.net_pnl < Decimal::ZERO {
            self.losses += 1;
            self.total_loss_amount += trade.net_pnl.abs();
        }

        // Update profitability
        self.is_profitable = self.daily_pnl > Decimal::ZERO;
    }
}

/// Aggregated simulation metrics
#[derive(Debug, Clone)]
pub struct SimulationMetrics {
    /// Number of consecutive profitable days (most recent streak)
    pub consecutive_profitable_days: u32,

    /// Net ROI across all trades (percentage)
    pub net_roi: Decimal,

    /// Win rate (percentage of profitable trades)
    pub win_rate: Decimal,

    /// Average profit per winning trade
    pub avg_profit_per_win: Decimal,

    /// Average loss per losing trade
    pub avg_loss_per_loss: Decimal,

    /// Total trades executed
    pub total_trades: u32,

    /// Total winning trades
    pub total_wins: u32,

    /// Total losing trades
    pub total_losses: u32,

    /// Net P&L (sum of all net_pnl)
    pub net_pnl: Decimal,

    /// Largest single-day loss (percentage)
    pub max_daily_loss_pct: Decimal,
}

/// Exit-to-live validation decision
#[derive(Debug, Clone, PartialEq)]
pub enum ExitToLiveDecision {
    /// All criteria met, ready for live trading
    Approved,

    /// Not ready yet, with list of unmet criteria
    NotReady {
        unmet_criteria: Vec<String>,
    },
}

// =============================================================================
// METRICS TRACKER
// =============================================================================

/// Tracks performance metrics for exit-to-live validation
///
/// Records all trades, aggregates daily performance, and validates
/// against exit-to-live criteria.
pub struct MetricsTracker {
    /// Daily performance records (date -> record)
    daily_records: HashMap<NaiveDate, DailyRecord>,

    /// Starting capital for percentage calculations
    starting_capital: Decimal,
}

impl MetricsTracker {
    /// Create a new metrics tracker
    ///
    /// # Arguments
    ///
    /// * `starting_capital` - Initial capital for percentage calculations
    ///
    /// # Example
    ///
    /// ```
    /// use calchas::trading::MetricsTracker;
    /// use rust_decimal_macros::dec;
    ///
    /// let tracker = MetricsTracker::new(dec!(10000.00));
    /// ```
    pub fn new(starting_capital: Decimal) -> Self {
        MetricsTracker {
            daily_records: HashMap::new(),
            starting_capital,
        }
    }

    /// Record a closed trade
    ///
    /// Adds trade to appropriate daily record and updates metrics.
    ///
    /// # Arguments
    ///
    /// * `trade` - Closed trade to record
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::MetricsTracker;
    /// # use calchas::models::Trade;
    /// # use rust_decimal_macros::dec;
    /// # fn example(trade: Trade) {
    /// let mut tracker = MetricsTracker::new(dec!(10000.00));
    /// tracker.record_trade(&trade);
    /// # }
    /// ```
    pub fn record_trade(&mut self, trade: &Trade) {
        let date = trade.exit_timestamp.date_naive();

        let record = self.daily_records
            .entry(date)
            .or_insert_with(|| DailyRecord::new(date));

        record.add_trade(trade);
    }

    /// Calculate aggregated metrics
    ///
    /// Returns current performance metrics across all recorded trades.
    ///
    /// # Returns
    ///
    /// `SimulationMetrics` with all calculated metrics
    ///
    /// # Example
    ///
    /// ```
    /// # use calchas::trading::MetricsTracker;
    /// # use rust_decimal_macros::dec;
    /// let tracker = MetricsTracker::new(dec!(10000.00));
    /// let metrics = tracker.calculate_metrics();
    /// println!("ROI: {:.2}%", metrics.net_roi);
    /// ```
    pub fn calculate_metrics(&self) -> SimulationMetrics {
        let mut total_trades = 0u32;
        let mut total_wins = 0u32;
        let mut total_losses = 0u32;
        let mut net_pnl = Decimal::ZERO;
        let mut total_win_amount = Decimal::ZERO;
        let mut total_loss_amount = Decimal::ZERO;
        let mut max_daily_loss_pct = Decimal::ZERO;

        // Aggregate across all days
        for record in self.daily_records.values() {
            total_trades += record.trade_count;
            total_wins += record.wins;
            total_losses += record.losses;
            net_pnl += record.daily_pnl;
            total_win_amount += record.total_win_amount;
            total_loss_amount += record.total_loss_amount;

            // Track max daily loss percentage
            if record.daily_pnl < Decimal::ZERO {
                let loss_pct = (record.daily_pnl.abs() / self.starting_capital) * Decimal::from(100);
                if loss_pct > max_daily_loss_pct {
                    max_daily_loss_pct = loss_pct;
                }
            }
        }

        // Calculate win rate
        let win_rate = if total_trades > 0 {
            (Decimal::from(total_wins) / Decimal::from(total_trades)) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // Calculate average profit per win
        let avg_profit_per_win = if total_wins > 0 {
            total_win_amount / Decimal::from(total_wins)
        } else {
            Decimal::ZERO
        };

        // Calculate average loss per loss
        let avg_loss_per_loss = if total_losses > 0 {
            total_loss_amount / Decimal::from(total_losses)
        } else {
            Decimal::ZERO
        };

        // Calculate ROI
        let net_roi = if self.starting_capital > Decimal::ZERO {
            (net_pnl / self.starting_capital) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // Calculate consecutive profitable days
        let consecutive_profitable_days = self.calculate_consecutive_profitable_days();

        SimulationMetrics {
            consecutive_profitable_days,
            net_roi,
            win_rate,
            avg_profit_per_win,
            avg_loss_per_loss,
            total_trades,
            total_wins,
            total_losses,
            net_pnl,
            max_daily_loss_pct,
        }
    }

    /// Check if exit-to-live criteria are met
    ///
    /// Validates against PRD Section 2.4 criteria:
    /// 1. 7+ consecutive profitable days
    /// 2. Net positive ROI
    /// 3. No single-day loss > 15%
    ///
    /// # Returns
    ///
    /// `ExitToLiveDecision` indicating readiness for live trading
    ///
    /// # Example
    ///
    /// ```
    /// # use calchas::trading::{MetricsTracker, ExitToLiveDecision};
    /// # use rust_decimal_macros::dec;
    /// let tracker = MetricsTracker::new(dec!(10000.00));
    ///
    /// match tracker.check_exit_to_live() {
    ///     ExitToLiveDecision::Approved => println!("Ready!"),
    ///     ExitToLiveDecision::NotReady { unmet_criteria } => {
    ///         for c in unmet_criteria {
    ///             println!("Missing: {}", c);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn check_exit_to_live(&self) -> ExitToLiveDecision {
        let metrics = self.calculate_metrics();
        let mut unmet = Vec::new();

        // Criterion 1: 7+ consecutive profitable days
        if metrics.consecutive_profitable_days < 7 {
            unmet.push(format!(
                "Need 7+ consecutive profitable days (current: {})",
                metrics.consecutive_profitable_days
            ));
        }

        // Criterion 2: Net positive ROI
        if metrics.net_roi <= Decimal::ZERO {
            unmet.push(format!(
                "Need positive ROI (current: {:.2}%)",
                metrics.net_roi
            ));
        }

        // Criterion 3: No single-day loss > 15%
        let max_allowed_loss = Decimal::from(15);
        if metrics.max_daily_loss_pct > max_allowed_loss {
            unmet.push(format!(
                "Single-day loss too high (max: 15%, actual: {:.2}%)",
                metrics.max_daily_loss_pct
            ));
        }

        if unmet.is_empty() {
            ExitToLiveDecision::Approved
        } else {
            ExitToLiveDecision::NotReady {
                unmet_criteria: unmet,
            }
        }
    }

    /// Get daily records sorted by date
    ///
    /// Returns all daily records in chronological order.
    pub fn get_daily_records(&self) -> Vec<&DailyRecord> {
        let mut records: Vec<_> = self.daily_records.values().collect();
        records.sort_by_key(|r| r.date);
        records
    }

    /// Get record for specific date
    pub fn get_record(&self, date: NaiveDate) -> Option<&DailyRecord> {
        self.daily_records.get(&date)
    }

    /// Get total number of trading days
    pub fn trading_days(&self) -> usize {
        self.daily_records.len()
    }

    // =========================================================================
    // PRIVATE HELPERS
    // =========================================================================

    /// Calculate consecutive profitable days (most recent streak)
    ///
    /// Counts backwards from most recent day to find longest streak.
    fn calculate_consecutive_profitable_days(&self) -> u32 {
        if self.daily_records.is_empty() {
            return 0;
        }

        // Get all dates sorted (most recent first)
        let mut dates: Vec<_> = self.daily_records.keys().copied().collect();
        dates.sort_by(|a, b| b.cmp(a)); // Reverse sort (newest first)

        let mut streak = 0u32;

        for date in dates {
            if let Some(record) = self.daily_records.get(&date) {
                if record.is_profitable {
                    streak += 1;
                } else {
                    // Streak broken
                    break;
                }
            }
        }

        streak
    }
}

impl Default for MetricsTracker {
    fn default() -> Self {
        // Default to $10,000 starting capital
        Self::new(Decimal::from(10000))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ExitReason, MarketId, OrderId, PositionId, StrategyId,
    };
    use chrono::{Duration as ChronoDuration, NaiveDate, TimeZone, Utc};
    use rust_decimal_macros::dec;

    // Helper: Create test trade
    fn create_test_trade(
        date: NaiveDate,
        entry_price: Decimal,
        exit_price: Decimal,
        quantity: u64,
        fees: Decimal,
    ) -> Trade {
        let entry_time = Utc.from_utc_datetime(&date.and_hms_opt(9, 0, 0).unwrap());
        let exit_time = entry_time + ChronoDuration::hours(2);

        Trade::new(
            PositionId::new(),
            MarketId::new("TEST-MARKET".to_string()),
            StrategyId::new("test-strategy".to_string()),
            OrderId::new("entry-123".to_string()),
            entry_price,
            quantity,
            entry_time,
            OrderId::new("exit-456".to_string()),
            exit_price,
            quantity,
            exit_time,
            ExitReason::TakeProfit,
            fees,
        )
    }

    #[test]
    fn test_new_tracker_empty() {
        let tracker = MetricsTracker::new(dec!(10000.00));

        assert_eq!(tracker.trading_days(), 0);
        assert_eq!(tracker.starting_capital, dec!(10000.00));
    }

    #[test]
    fn test_record_single_trade() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // Profitable trade: entry=$0.50, exit=$0.75, quantity=100, fees=$2
        // Gross P&L = (0.75 - 0.50) * 100 = $25
        // Net P&L = $25 - $2 = $23
        let trade = create_test_trade(date, dec!(0.50), dec!(0.75), 100, dec!(2.00));
        tracker.record_trade(&trade);

        assert_eq!(tracker.trading_days(), 1);

        let record = tracker.get_record(date).unwrap();
        assert_eq!(record.trade_count, 1);
        assert_eq!(record.daily_pnl, dec!(23.00));
        assert!(record.is_profitable);
        assert_eq!(record.wins, 1);
        assert_eq!(record.losses, 0);
    }

    #[test]
    fn test_record_multiple_trades_same_day() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // Trade 1: +$23
        let trade1 = create_test_trade(date, dec!(0.50), dec!(0.75), 100, dec!(2.00));
        tracker.record_trade(&trade1);

        // Trade 2: +$13
        let trade2 = create_test_trade(date, dec!(0.30), dec!(0.45), 100, dec!(2.00));
        tracker.record_trade(&trade2);

        let record = tracker.get_record(date).unwrap();
        assert_eq!(record.trade_count, 2);
        assert_eq!(record.daily_pnl, dec!(36.00)); // 23 + 13
        assert_eq!(record.wins, 2);
    }

    #[test]
    fn test_record_losing_trade() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // Losing trade: entry=$0.75, exit=$0.50, quantity=100, fees=$2
        // Gross P&L = (0.50 - 0.75) * 100 = -$25
        // Net P&L = -$25 - $2 = -$27
        let trade = create_test_trade(date, dec!(0.75), dec!(0.50), 100, dec!(2.00));
        tracker.record_trade(&trade);

        let record = tracker.get_record(date).unwrap();
        assert_eq!(record.daily_pnl, dec!(-27.00));
        assert!(!record.is_profitable);
        assert_eq!(record.wins, 0);
        assert_eq!(record.losses, 1);
        assert_eq!(record.total_loss_amount, dec!(27.00));
    }

    #[test]
    fn test_calculate_metrics_single_trade() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        let trade = create_test_trade(date, dec!(0.50), dec!(0.75), 100, dec!(2.00));
        tracker.record_trade(&trade);

        let metrics = tracker.calculate_metrics();

        assert_eq!(metrics.total_trades, 1);
        assert_eq!(metrics.total_wins, 1);
        assert_eq!(metrics.total_losses, 0);
        assert_eq!(metrics.net_pnl, dec!(23.00));
        assert_eq!(metrics.win_rate, dec!(100.00)); // 100%
        assert_eq!(metrics.net_roi, dec!(0.23)); // 23/10000 * 100 = 0.23%
        assert_eq!(metrics.consecutive_profitable_days, 1);
    }

    #[test]
    fn test_calculate_metrics_mixed_trades() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // Day 1: Win +$23
        let trade1 = create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            dec!(0.50),
            dec!(0.75),
            100,
            dec!(2.00),
        );
        tracker.record_trade(&trade1);

        // Day 1: Loss -$27
        let trade2 = create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            dec!(0.75),
            dec!(0.50),
            100,
            dec!(2.00),
        );
        tracker.record_trade(&trade2);

        let metrics = tracker.calculate_metrics();

        assert_eq!(metrics.total_trades, 2);
        assert_eq!(metrics.total_wins, 1);
        assert_eq!(metrics.total_losses, 1);
        assert_eq!(metrics.net_pnl, dec!(-4.00)); // 23 - 27
        assert_eq!(metrics.win_rate, dec!(50.00)); // 1/2 * 100
        assert_eq!(metrics.avg_profit_per_win, dec!(23.00));
        assert_eq!(metrics.avg_loss_per_loss, dec!(27.00));
    }

    #[test]
    fn test_consecutive_profitable_days() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // Day 1: Profitable (+$23)
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        // Day 2: Profitable (+$23)
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        // Day 3: Profitable (+$23)
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        let metrics = tracker.calculate_metrics();
        assert_eq!(metrics.consecutive_profitable_days, 3);
    }

    #[test]
    fn test_consecutive_days_broken_by_loss() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // Day 1: Profitable
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        // Day 2: Profitable
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        // Day 3: LOSS
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
            dec!(0.75), dec!(0.50), 100, dec!(2.00),
        ));

        // Day 4: Profitable
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 18).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        let metrics = tracker.calculate_metrics();
        // Only counts most recent streak (day 4 only)
        assert_eq!(metrics.consecutive_profitable_days, 1);
    }

    #[test]
    fn test_exit_to_live_not_ready_days() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // Only 3 consecutive profitable days
        for i in 0..3 {
            tracker.record_trade(&create_test_trade(
                NaiveDate::from_ymd_opt(2024, 1, 15 + i).unwrap(),
                dec!(0.50), dec!(0.75), 100, dec!(2.00),
            ));
        }

        let decision = tracker.check_exit_to_live();
        match decision {
            ExitToLiveDecision::NotReady { unmet_criteria } => {
                assert!(unmet_criteria.iter().any(|c| c.contains("7+ consecutive")));
            }
            _ => panic!("Should not be approved"),
        }
    }

    #[test]
    fn test_exit_to_live_not_ready_negative_roi() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // 7 days but net negative
        for i in 0..7 {
            // Each day loses $5
            tracker.record_trade(&create_test_trade(
                NaiveDate::from_ymd_opt(2024, 1, 15 + i).unwrap(),
                dec!(0.60), dec!(0.50), 100, dec!(15.00), // Loss: -$10 - $15 fees = -$25
            ));
        }

        let decision = tracker.check_exit_to_live();
        match decision {
            ExitToLiveDecision::NotReady { unmet_criteria } => {
                assert!(unmet_criteria.iter().any(|c| c.contains("positive ROI")));
            }
            _ => panic!("Should not be approved"),
        }
    }

    #[test]
    fn test_exit_to_live_not_ready_large_loss_day() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // 7 profitable days
        for i in 0..7 {
            tracker.record_trade(&create_test_trade(
                NaiveDate::from_ymd_opt(2024, 1, 15 + i).unwrap(),
                dec!(0.50), dec!(0.75), 100, dec!(2.00),
            ));
        }

        // But one day with >15% loss
        // 15% of $10,000 = $1,500
        // Need loss > $1,500
        // entry=$0.90, exit=$0.10, quantity=2000, fees=$100
        // Gross P&L = (0.10 - 0.90) * 2000 = -0.80 * 2000 = -$1,600
        // Net P&L = -1,600 - 100 = -$1,700 (17% of $10,000)
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 22).unwrap(),
            dec!(0.90), dec!(0.10), 2000, dec!(100.00),
        ));

        let decision = tracker.check_exit_to_live();
        match decision {
            ExitToLiveDecision::NotReady { unmet_criteria } => {
                assert!(unmet_criteria.iter().any(|c| c.contains("Single-day loss")));
            }
            _ => panic!("Should not be approved"),
        }
    }

    #[test]
    fn test_exit_to_live_approved() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // 7 consecutive profitable days, positive ROI, no large losses
        for i in 0..7 {
            tracker.record_trade(&create_test_trade(
                NaiveDate::from_ymd_opt(2024, 1, 15 + i).unwrap(),
                dec!(0.50), dec!(0.75), 100, dec!(2.00), // +$23 each day
            ));
        }

        let decision = tracker.check_exit_to_live();
        assert_eq!(decision, ExitToLiveDecision::Approved);
    }

    #[test]
    fn test_get_daily_records_sorted() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // Add trades in non-chronological order
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        let records = tracker.get_daily_records();
        assert_eq!(records.len(), 3);

        // Should be sorted chronologically
        assert_eq!(records[0].date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert_eq!(records[1].date, NaiveDate::from_ymd_opt(2024, 1, 16).unwrap());
        assert_eq!(records[2].date, NaiveDate::from_ymd_opt(2024, 1, 17).unwrap());
    }

    #[test]
    fn test_break_even_trade() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        // Break-even trade: entry=$0.50, exit=$0.52, quantity=100, fees=$2
        // Gross P&L = (0.52 - 0.50) * 100 = $2
        // Net P&L = $2 - $2 = $0
        let trade = create_test_trade(date, dec!(0.50), dec!(0.52), 100, dec!(2.00));
        tracker.record_trade(&trade);

        let record = tracker.get_record(date).unwrap();
        assert_eq!(record.daily_pnl, Decimal::ZERO);
        assert!(!record.is_profitable); // Zero is not profitable
        assert_eq!(record.wins, 0);
        assert_eq!(record.losses, 0);

        let metrics = tracker.calculate_metrics();
        assert_eq!(metrics.total_wins, 0);
        assert_eq!(metrics.total_losses, 0);
        assert_eq!(metrics.win_rate, Decimal::ZERO); // 0/1 trades won
    }

    #[test]
    fn test_exit_to_live_exactly_7_days() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // Exactly 7 consecutive profitable days
        for i in 0..7 {
            tracker.record_trade(&create_test_trade(
                NaiveDate::from_ymd_opt(2024, 1, 15 + i).unwrap(),
                dec!(0.50), dec!(0.75), 100, dec!(2.00),
            ));
        }

        let decision = tracker.check_exit_to_live();
        assert_eq!(decision, ExitToLiveDecision::Approved);
    }

    #[test]
    fn test_exit_to_live_exactly_15_percent_loss() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // 7 profitable days
        for i in 0..7 {
            tracker.record_trade(&create_test_trade(
                NaiveDate::from_ymd_opt(2024, 1, 15 + i).unwrap(),
                dec!(0.50), dec!(0.75), 100, dec!(2.00),
            ));
        }

        // Day with exactly 15% loss
        // 15% of $10,000 = $1,500
        // entry=$0.90, exit=$0.15, quantity=2000, fees=$0
        // Gross P&L = (0.15 - 0.90) * 2000 = -0.75 * 2000 = -$1,500
        // Net P&L = -1,500 - 0 = -$1,500 (exactly 15%)
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 22).unwrap(),
            dec!(0.90), dec!(0.15), 2000, dec!(0.00),
        ));

        let decision = tracker.check_exit_to_live();
        // Exactly 15% should FAIL (criterion is > 15%, not >= 15%)
        // Wait, the check is `>` not `>=`, so exactly 15% should PASS
        match decision {
            ExitToLiveDecision::NotReady { unmet_criteria } => {
                // Should NOT fail on loss (exactly 15% is ok)
                assert!(!unmet_criteria.iter().any(|c| c.contains("Single-day loss")));
            }
            ExitToLiveDecision::Approved => {
                // This is correct - exactly 15% is allowed
            }
        }
    }

    #[test]
    fn test_exit_to_live_multiple_criteria_fail() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        // Only 3 days (not 7)
        // All losing (negative ROI)
        // Large loss on one day
        for i in 0..3 {
            tracker.record_trade(&create_test_trade(
                NaiveDate::from_ymd_opt(2024, 1, 15 + i).unwrap(),
                dec!(0.90), dec!(0.10), 2000, dec!(100.00), // -$1,700 each day
            ));
        }

        let decision = tracker.check_exit_to_live();
        match decision {
            ExitToLiveDecision::NotReady { unmet_criteria } => {
                assert_eq!(unmet_criteria.len(), 3); // All 3 criteria unmet
                assert!(unmet_criteria.iter().any(|c| c.contains("7+ consecutive")));
                assert!(unmet_criteria.iter().any(|c| c.contains("positive ROI")));
                assert!(unmet_criteria.iter().any(|c| c.contains("Single-day loss")));
            }
            _ => panic!("Should not be approved"),
        }
    }

    #[test]
    fn test_get_record_nonexistent() {
        let tracker = MetricsTracker::new(dec!(10000.00));

        let record = tracker.get_record(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert!(record.is_none());
    }

    #[test]
    fn test_trading_days_count() {
        let mut tracker = MetricsTracker::new(dec!(10000.00));

        assert_eq!(tracker.trading_days(), 0);

        // Add trades on 3 different days
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        assert_eq!(tracker.trading_days(), 3);

        // Add another trade on existing day (should still be 3)
        tracker.record_trade(&create_test_trade(
            NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
            dec!(0.50), dec!(0.75), 100, dec!(2.00),
        ));

        assert_eq!(tracker.trading_days(), 3);
    }
}
