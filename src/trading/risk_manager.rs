//! Risk management for trading operations
//!
//! Enforces Strategy.RiskLimits to prevent over-trading and excessive losses.
//!
//! # Risk Checks
//!
//! 1. **duplicate_market** - Prevents multiple positions on the same market
//! 2. **loss_cooldown_minutes** - Enforces cooldown after daily loss limit hit
//! 3. **max_daily_loss_usd** - Stops trading if daily loss exceeds limit
//! 4. **max_concurrent_positions** - Limits number of open positions
//!
//! # Example
//!
//! ```no_run
//! use calchas::trading::RiskManager;
//! # use std::collections::HashMap;
//! # use calchas::strategy::EntrySignal;
//! # use calchas::models::Strategy;
//!
//! # fn example(signal: EntrySignal, strategy: Strategy) {
//! let mut risk_mgr = RiskManager::new();
//! let positions = HashMap::new();
//!
//! match risk_mgr.check_entry(&signal, &positions, &strategy) {
//!     calchas::trading::RiskDecision::Approved => {
//!         println!("Risk check passed - can open position");
//!     }
//!     calchas::trading::RiskDecision::Rejected(reason) => {
//!         println!("Risk check failed: {:?}", reason);
//!     }
//! }
//! # }
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::models::{Position, PositionId, Strategy, Trade};
use crate::strategy::signals::EntrySignal;

// =============================================================================
// DATA TYPES
// =============================================================================

/// Risk decision result
#[derive(Debug, Clone, PartialEq)]
pub enum RiskDecision {
    /// Entry is approved, safe to open position
    Approved,

    /// Entry is rejected due to risk limits
    Rejected(RejectionReason),
}

/// Reasons for rejecting a trade entry
#[derive(Debug, Clone, PartialEq)]
pub enum RejectionReason {
    /// Too many concurrent positions open
    MaxConcurrentPositions {
        current: usize,
        limit: usize,
    },

    /// Daily loss limit exceeded
    DailyLossExceeded {
        daily_pnl: Decimal,
        limit: Decimal,
    },

    /// In cooldown period after hitting daily loss limit
    InCooldown {
        minutes_remaining: i64,
    },

    /// Already have a position on this market
    DuplicateMarket {
        market_id: String,
    },
}

/// Daily trading statistics
#[derive(Debug, Clone)]
struct DailyStats {
    /// Net P&L for today (can be negative)
    daily_pnl: Decimal,

    /// When daily loss limit was hit (None if not hit today)
    cooldown_started_at: Option<DateTime<Utc>>,
}

impl Default for DailyStats {
    fn default() -> Self {
        DailyStats {
            daily_pnl: Decimal::ZERO,
            cooldown_started_at: None,
        }
    }
}

// =============================================================================
// RISK MANAGER
// =============================================================================

/// Risk manager for enforcing trading limits
///
/// Tracks daily P&L and enforces strategy risk limits before allowing
/// new positions to be opened.
pub struct RiskManager {
    /// Daily statistics (resets at midnight UTC)
    daily_stats: DailyStats,

    /// Last time daily stats were reset
    last_reset: DateTime<Utc>,
}

impl RiskManager {
    /// Create a new risk manager
    pub fn new() -> Self {
        RiskManager {
            daily_stats: DailyStats::default(),
            last_reset: Utc::now(),
        }
    }

    /// Check if entry signal passes risk limits
    ///
    /// # Arguments
    ///
    /// * `signal` - Entry signal to evaluate
    /// * `positions` - Currently open positions
    /// * `strategy` - Strategy with risk limits
    ///
    /// # Returns
    ///
    /// * `RiskDecision::Approved` - Safe to open position
    /// * `RiskDecision::Rejected(reason)` - Risk limit violated
    pub fn check_entry(
        &mut self,
        signal: &EntrySignal,
        positions: &HashMap<PositionId, Position>,
        strategy: &Strategy,
    ) -> RiskDecision {
        // Reset daily stats if new day
        self.reset_if_new_day();

        let risk_limits = &strategy.risk_limits;

        // Check 1: Duplicate market+side (allow same market with different side for "Both" strategy)
        for position in positions.values() {
            if position.market_id == signal.market_id {
                // Check if same side - only reject if both market AND side match
                let same_side = match (&position.side, &signal.side) {
                    (crate::models::PositionSide::Yes, crate::strategy::signals::SignalSide::Yes) => true,
                    (crate::models::PositionSide::No, crate::strategy::signals::SignalSide::No) => true,
                    _ => false,
                };

                if same_side {
                    return RiskDecision::Rejected(RejectionReason::DuplicateMarket {
                        market_id: signal.market_id.0.clone(),
                    });
                }
            }
        }

        // Check 2: Cooldown period
        // If user hit loss limit earlier and cooldown is active, reject immediately
        if let Some(cooldown_minutes) = risk_limits.loss_cooldown_minutes {
            if let Some(cooldown_start) = self.daily_stats.cooldown_started_at {
                let now = Utc::now();
                let elapsed = now.signed_duration_since(cooldown_start);
                let cooldown_duration = chrono::Duration::minutes(cooldown_minutes as i64);

                if elapsed < cooldown_duration {
                    let remaining = cooldown_duration - elapsed;
                    return RiskDecision::Rejected(RejectionReason::InCooldown {
                        minutes_remaining: remaining.num_minutes(),
                    });
                }
            }
        }

        // Check 3: Daily loss limit
        if let Some(max_loss) = risk_limits.max_daily_loss_usd {
            if self.daily_stats.daily_pnl < -max_loss {
                return RiskDecision::Rejected(RejectionReason::DailyLossExceeded {
                    daily_pnl: self.daily_stats.daily_pnl,
                    limit: max_loss,
                });
            }
        }

        // Check 4: Max concurrent positions (count only for this strategy)
        let max_positions = risk_limits.max_concurrent_positions;
        let current_positions = positions
            .values()
            .filter(|p| p.strategy_id == signal.strategy_id)
            .count();
        if current_positions >= max_positions as usize {
            return RiskDecision::Rejected(RejectionReason::MaxConcurrentPositions {
                current: current_positions,
                limit: max_positions as usize,
            });
        }

        RiskDecision::Approved
    }

    /// Record a completed trade and update daily stats
    ///
    /// Updates daily P&L and triggers cooldown if loss limit hit.
    ///
    /// # Arguments
    ///
    /// * `trade` - Completed trade to record
    /// * `strategy` - Strategy with risk limits to check
    pub fn record_trade(&mut self, trade: &Trade, strategy: &Strategy) {
        // Reset if new day
        self.reset_if_new_day();

        // Update daily P&L
        self.daily_stats.daily_pnl += trade.net_pnl;

        // Start cooldown if we just exceeded daily loss limit
        if let Some(max_loss) = strategy.risk_limits.max_daily_loss_usd {
            // Only start cooldown if:
            // 1. We're now below the loss limit
            // 2. Cooldown hasn't already started today
            if self.daily_stats.daily_pnl < -max_loss && self.daily_stats.cooldown_started_at.is_none() {
                self.daily_stats.cooldown_started_at = Some(Utc::now());
            }
        }
    }

    /// Reset daily stats if it's a new day
    ///
    /// Checks if current date differs from last reset date and resets
    /// statistics at midnight UTC boundary.
    fn reset_if_new_day(&mut self) {
        let now = Utc::now();
        let last_reset_date = self.last_reset.date_naive();
        let current_date = now.date_naive();

        if current_date > last_reset_date {
            self.daily_stats = DailyStats::default();
            self.last_reset = now;
        }
    }

    /// Manually reset daily statistics
    ///
    /// Used for testing or manual intervention.
    #[allow(dead_code)]
    pub fn reset_daily_stats(&mut self) {
        self.daily_stats = DailyStats::default();
        self.last_reset = Utc::now();
    }

    /// Get current daily P&L
    ///
    /// For monitoring and reporting purposes.
    #[allow(dead_code)]
    pub fn daily_pnl(&self) -> Decimal {
        self.daily_stats.daily_pnl
    }
}

impl Default for RiskManager {
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
    use crate::models::{
        EntrySide, ExitRules, ExitTarget, MarketId, OrderId, Position, PositionId,
        PositionSide, PositionStatus, RiskLimits, Strategy, StrategyFilters, StrategyId, Trade,
        TradeId,
    };
    use crate::strategy::signals::{EntrySignal, SignalSide};
    use rust_decimal_macros::dec;
    use chrono::Duration;

    // Helper: Create test strategy with specific risk limits
    fn create_test_strategy(
        max_positions: u32,
        max_daily_loss: Option<Decimal>,
        cooldown_minutes: Option<u32>,
    ) -> Strategy {
        Strategy {
            id: StrategyId::new("test-strategy".to_string()),
            name: "Test Strategy".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            filters: StrategyFilters {
                categories: None,
                exclude_categories: None,
                min_price: None,
                max_price: None,
                min_volume: None,
                min_open_interest: None,
                min_time_to_event_minutes: None,
                max_time_to_event_minutes: None,
                min_momentum_pct: None,
                momentum_lookback_minutes: None,
                max_spread_cents: None,
                min_best_price_quantity: None,
            },
            entry_rules: crate::models::EntryRules {
                side: EntrySide::CheaperSide,
                position_size: 10,
                position_size_unit: crate::models::strategy::PositionSizeUnit::Contracts,
                order_type: crate::models::strategy::OrderType::Market,
                limit_price_offset: None,
            },
            exit_rules: ExitRules {
                take_profit_pct: Some(dec!(50.0)),
                stop_loss_pct: Some(dec!(50.0)),
                trailing_stop_pct: None,
                trailing_stop_activation_pct: None,
                max_hold_time_minutes: None,
                exit_order_type: crate::models::strategy::OrderType::Market,
            },
            risk_limits: RiskLimits {
                max_concurrent_positions: max_positions,
                max_daily_loss_usd: max_daily_loss,
                max_position_loss_usd: None,
                loss_cooldown_minutes: cooldown_minutes,
            },
        }
    }

    // Helper: Create test entry signal
    fn create_test_signal() -> EntrySignal {
        EntrySignal {
            market_id: MarketId::new("TEST-MARKET".to_string()),
            market_ticker: "TEST-MARKET".to_string(),
            market_title: "Test Market".to_string(),
            strategy_id: StrategyId::new("test-strategy".to_string()),
            strategy_name: "Test Strategy".to_string(),
            side: SignalSide::Yes,
            position_size: 10,
            order_type: crate::models::strategy::OrderType::Market,
            limit_price_offset: None,
            recommended_price: dec!(0.50),
            generated_at: Utc::now(),
            time_to_event_minutes: 1440.0,
            market_volume: 1000,
            market_open_interest: 500,
        }
    }

    // Helper: Create test position
    fn create_test_position(id_str: &str) -> Position {
        Position {
            id: PositionId::new(),
            strategy_id: StrategyId::new("test-strategy".to_string()),
            market_id: MarketId::new(format!("MARKET-{}", id_str)),
            entry_order_id: OrderId::new("test-order-1".to_string()),
            exit_order_id: None,
            side: PositionSide::Yes,
            quantity: 10,
            entry_price: dec!(0.50),
            current_price: dec!(0.50),
            exit_target: ExitTarget {
                take_profit_price: Some(dec!(0.75)),
                stop_loss_price: Some(dec!(0.25)),
                trailing_stop_distance: None,
                expiry_time: None,
            },
            unrealized_pnl: dec!(0.00),
            status: PositionStatus::Active,
            entry_timestamp: Utc::now(),
            peak_pnl: dec!(0.00),
            updated_at: Utc::now(),
        }
    }

    // Helper: Create test trade
    fn create_test_trade(net_pnl: Decimal) -> Trade {
        Trade {
            id: TradeId::new(),
            position_id: PositionId::new(),
            market_id: MarketId::new("TEST-MARKET".to_string()),
            strategy_id: StrategyId::new("test-strategy".to_string()),
            entry_order_id: OrderId::new("test-entry-order".to_string()),
            entry_price: dec!(0.50),
            entry_quantity: 10,
            entry_timestamp: Utc::now(),
            exit_order_id: OrderId::new("test-exit-order".to_string()),
            exit_price: dec!(0.60),
            exit_quantity: 10,
            exit_timestamp: Utc::now(),
            exit_reason: crate::models::ExitReason::TakeProfit,
            gross_pnl: net_pnl,
            fees: dec!(0.00),
            net_pnl,
            return_pct: dec!(20.0),
            hold_duration: Duration::minutes(10),
            notes: None,
        }
    }

    #[test]
    fn test_approved_when_under_limits() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let positions = HashMap::new();
        let strategy = create_test_strategy(10, None, None);

        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        assert_eq!(decision, RiskDecision::Approved);
    }

    #[test]
    fn test_approved_under_position_limit() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let mut positions = HashMap::new();
        let pos = create_test_position("pos-1");
        positions.insert(pos.id.clone(), pos);

        let strategy = create_test_strategy(3, None, None);

        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        assert_eq!(decision, RiskDecision::Approved);
    }

    #[test]
    fn test_rejected_max_concurrent_positions() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let mut positions = HashMap::new();
        let pos1 = create_test_position("pos-1");
        let pos2 = create_test_position("pos-2");
        positions.insert(pos1.id.clone(), pos1);
        positions.insert(pos2.id.clone(), pos2);

        let strategy = create_test_strategy(2, None, None);

        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        assert_eq!(
            decision,
            RiskDecision::Rejected(RejectionReason::MaxConcurrentPositions {
                current: 2,
                limit: 2
            })
        );
    }

    #[test]
    fn test_multi_strategy_position_limit() {
        let mut risk_mgr = RiskManager::new();

        // Signal for strategy A
        let mut signal_a = create_test_signal();
        signal_a.strategy_id = StrategyId::new("strategy-a".to_string());

        // Strategy A has limit of 2
        let strategy_a = create_test_strategy(2, None, None);

        // Positions: 2 from strategy A, 3 from strategy B
        let mut positions = HashMap::new();

        // Strategy A positions
        let mut pos_a1 = create_test_position("a1");
        pos_a1.strategy_id = StrategyId::new("strategy-a".to_string());
        let mut pos_a2 = create_test_position("a2");
        pos_a2.strategy_id = StrategyId::new("strategy-a".to_string());

        // Strategy B positions (should not affect strategy A's count)
        let mut pos_b1 = create_test_position("b1");
        pos_b1.strategy_id = StrategyId::new("strategy-b".to_string());
        let mut pos_b2 = create_test_position("b2");
        pos_b2.strategy_id = StrategyId::new("strategy-b".to_string());
        let mut pos_b3 = create_test_position("b3");
        pos_b3.strategy_id = StrategyId::new("strategy-b".to_string());

        positions.insert(pos_a1.id.clone(), pos_a1);
        positions.insert(pos_a2.id.clone(), pos_a2);
        positions.insert(pos_b1.id.clone(), pos_b1);
        positions.insert(pos_b2.id.clone(), pos_b2);
        positions.insert(pos_b3.id.clone(), pos_b3);

        // Total positions: 5
        // Strategy A positions: 2 (at limit)
        // Strategy B positions: 3

        let decision = risk_mgr.check_entry(&signal_a, &positions, &strategy_a);

        // Should be REJECTED because strategy A has 2 positions and limit is 2
        assert_eq!(
            decision,
            RiskDecision::Rejected(RejectionReason::MaxConcurrentPositions {
                current: 2,
                limit: 2
            })
        );

        // Now test signal from strategy B with higher limit
        let mut signal_b = create_test_signal();
        signal_b.strategy_id = StrategyId::new("strategy-b".to_string());
        let strategy_b = create_test_strategy(5, None, None);

        let decision = risk_mgr.check_entry(&signal_b, &positions, &strategy_b);

        // Should be APPROVED because strategy B has 3 positions and limit is 5
        assert_eq!(decision, RiskDecision::Approved);
    }

    #[test]
    fn test_rejected_duplicate_market() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let mut positions = HashMap::new();

        // Create a position on the SAME market as the signal
        let mut pos = create_test_position("pos-1");
        pos.market_id = MarketId::new("TEST-MARKET".to_string()); // Same as signal
        positions.insert(pos.id.clone(), pos);

        let strategy = create_test_strategy(10, None, None);

        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        assert_eq!(
            decision,
            RiskDecision::Rejected(RejectionReason::DuplicateMarket {
                market_id: "TEST-MARKET".to_string()
            })
        );
    }

    #[test]
    fn test_approved_under_daily_loss_limit() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let positions = HashMap::new();
        let strategy = create_test_strategy(10, Some(dec!(100.00)), None);

        // Record a small loss
        let trade = create_test_trade(dec!(-10.00));
        risk_mgr.record_trade(&trade, &strategy);

        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        assert_eq!(decision, RiskDecision::Approved);
    }

    #[test]
    fn test_rejected_daily_loss_exceeded() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let positions = HashMap::new();
        let strategy = create_test_strategy(10, Some(dec!(50.00)), None);

        // Record a large loss
        let trade = create_test_trade(dec!(-75.00));
        risk_mgr.record_trade(&trade, &strategy);

        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        assert_eq!(
            decision,
            RiskDecision::Rejected(RejectionReason::DailyLossExceeded {
                daily_pnl: dec!(-75.00),
                limit: dec!(50.00)
            })
        );
    }

    #[test]
    fn test_daily_stats_reset_on_new_day() {
        let mut risk_mgr = RiskManager::new();
        let strategy = create_test_strategy(10, Some(dec!(100.00)), None);

        // Record a loss
        let trade = create_test_trade(dec!(-30.00));
        risk_mgr.record_trade(&trade, &strategy);
        assert_eq!(risk_mgr.daily_pnl(), dec!(-30.00));

        // Simulate new day by manually resetting
        risk_mgr.reset_daily_stats();

        // Stats should be reset
        assert_eq!(risk_mgr.daily_pnl(), dec!(0.00));
    }

    #[test]
    fn test_accumulates_multiple_trades() {
        let mut risk_mgr = RiskManager::new();
        let strategy = create_test_strategy(10, Some(dec!(100.00)), None);

        // Record multiple trades
        risk_mgr.record_trade(&create_test_trade(dec!(10.00)), &strategy);
        risk_mgr.record_trade(&create_test_trade(dec!(-5.00)), &strategy);
        risk_mgr.record_trade(&create_test_trade(dec!(3.00)), &strategy);

        assert_eq!(risk_mgr.daily_pnl(), dec!(8.00));
    }

    #[test]
    fn test_positive_pnl_not_rejected() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let positions = HashMap::new();
        let strategy = create_test_strategy(10, Some(dec!(50.00)), None);

        // Record profits
        let trade = create_test_trade(dec!(100.00));
        risk_mgr.record_trade(&trade, &strategy);

        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        // Should be approved (daily P&L is positive)
        assert_eq!(decision, RiskDecision::Approved);
    }

    #[test]
    fn test_cooldown_triggered_on_loss_limit() {
        let mut risk_mgr = RiskManager::new();
        let strategy = create_test_strategy(10, Some(dec!(50.00)), Some(60));

        // Record a large loss that exceeds the limit
        let trade = create_test_trade(dec!(-75.00));
        risk_mgr.record_trade(&trade, &strategy);

        // Cooldown should be started
        assert!(risk_mgr.daily_stats.cooldown_started_at.is_some());
    }

    #[test]
    fn test_cooldown_not_triggered_under_limit() {
        let mut risk_mgr = RiskManager::new();
        let strategy = create_test_strategy(10, Some(dec!(100.00)), Some(60));

        // Record a small loss under the limit
        let trade = create_test_trade(dec!(-30.00));
        risk_mgr.record_trade(&trade, &strategy);

        // Cooldown should NOT be started
        assert!(risk_mgr.daily_stats.cooldown_started_at.is_none());
    }

    #[test]
    fn test_rejected_during_cooldown() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let positions = HashMap::new();
        let strategy = create_test_strategy(10, Some(dec!(50.00)), Some(60));

        // Record a large loss that triggers cooldown
        let trade = create_test_trade(dec!(-75.00));
        risk_mgr.record_trade(&trade, &strategy);

        // Try to enter a new position - should be rejected due to cooldown
        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        match decision {
            RiskDecision::Rejected(RejectionReason::InCooldown { minutes_remaining }) => {
                // Should have approximately 60 minutes remaining
                assert!(minutes_remaining >= 59);
                assert!(minutes_remaining <= 60);
            }
            _ => panic!("Expected InCooldown rejection, got: {:?}", decision),
        }
    }

    #[test]
    fn test_cooldown_expires_after_duration() {
        let mut risk_mgr = RiskManager::new();
        let signal = create_test_signal();
        let positions = HashMap::new();
        let strategy = create_test_strategy(10, Some(dec!(50.00)), Some(0)); // 0 minute cooldown

        // Record a large loss
        let trade = create_test_trade(dec!(-75.00));
        risk_mgr.record_trade(&trade, &strategy);

        // Cooldown duration is 0, so should be expired immediately
        let decision = risk_mgr.check_entry(&signal, &positions, &strategy);

        // Should still be rejected for DailyLossExceeded, but NOT for cooldown
        match decision {
            RiskDecision::Rejected(RejectionReason::DailyLossExceeded { .. }) => {
                // This is expected - cooldown expired but loss still exceeds limit
            }
            RiskDecision::Rejected(RejectionReason::InCooldown { .. }) => {
                panic!("Cooldown should have expired");
            }
            _ => panic!("Expected DailyLossExceeded, got: {:?}", decision),
        }
    }

    #[test]
    fn test_cooldown_only_starts_once() {
        let mut risk_mgr = RiskManager::new();
        let strategy = create_test_strategy(10, Some(dec!(50.00)), Some(60));

        // Record first loss that triggers cooldown
        let trade1 = create_test_trade(dec!(-60.00));
        risk_mgr.record_trade(&trade1, &strategy);
        let first_cooldown_time = risk_mgr.daily_stats.cooldown_started_at.unwrap();

        // Record second loss (cooldown already active)
        let trade2 = create_test_trade(dec!(-20.00));
        risk_mgr.record_trade(&trade2, &strategy);
        let second_cooldown_time = risk_mgr.daily_stats.cooldown_started_at.unwrap();

        // Cooldown time should not change
        assert_eq!(first_cooldown_time, second_cooldown_time);
    }

    #[test]
    fn test_cooldown_resets_on_new_day() {
        let mut risk_mgr = RiskManager::new();
        let strategy = create_test_strategy(10, Some(dec!(50.00)), Some(60));

        // Record loss that triggers cooldown
        let trade = create_test_trade(dec!(-75.00));
        risk_mgr.record_trade(&trade, &strategy);
        assert!(risk_mgr.daily_stats.cooldown_started_at.is_some());

        // Simulate new day
        risk_mgr.reset_daily_stats();

        // Cooldown should be cleared
        assert!(risk_mgr.daily_stats.cooldown_started_at.is_none());
    }
}
