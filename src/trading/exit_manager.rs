//! Exit condition evaluation
//!
//! This module provides stateless evaluation of position exit conditions.
//! It delegates to Position methods to check if exit triggers have been hit.
//!
//! # Exit Conditions
//!
//! 1. **Take Profit** - Current price hits target profit level
//! 2. **Stop Loss** - Current price hits loss limit
//! 3. **Trailing Stop** - Price drops from peak by trailing distance
//! 4. **Max Hold Time** - Position has been open too long
//!
//! # Example
//!
//! ```no_run
//! use calchas::trading::ExitManager;
//! # use calchas::models::Position;
//!
//! # fn example(position: &Position) {
//! let exit_mgr = ExitManager::new();
//!
//! if exit_mgr.should_exit(position) {
//!     let reason = exit_mgr.determine_exit_reason(position).unwrap();
//!     println!("Exit triggered: {:?}", reason);
//! }
//! # }
//! ```

use crate::models::{ExitReason, Position, Market, PositionSide};
use rust_decimal::Decimal;
use chrono::Utc;

// =============================================================================
// EXIT MANAGER
// =============================================================================

/// Stateless exit condition evaluator
///
/// Checks if positions should be exited based on their exit targets.
/// Delegates to Position methods for actual condition checking.
pub struct ExitManager;

impl ExitManager {
    /// Create a new exit manager
    pub fn new() -> Self {
        ExitManager
    }

    /// Check if position should be exited
    ///
    /// Returns true if ANY exit condition is met.
    ///
    /// # Arguments
    ///
    /// * `position` - Position to evaluate
    ///
    /// # Returns
    ///
    /// `true` if position should exit, `false` otherwise
    pub fn should_exit(&self, position: &Position) -> bool {
        // Check in same priority order as determine_exit_reason()
        position.hit_take_profit()
            || position.hit_trailing_stop()
            || position.hit_stop_loss()
            || position.is_expired()
    }

    /// Determine which exit condition was triggered
    ///
    /// Checks conditions in priority order:
    /// 1. Take Profit (highest priority - lock in gains)
    /// 2. Trailing Stop (protect profits)
    /// 3. Stop Loss (limit losses)
    /// 4. Max Hold Time (time-based exit)
    ///
    /// # Arguments
    ///
    /// * `position` - Position to evaluate
    ///
    /// # Returns
    ///
    /// `Some(ExitReason)` if any condition is met, `None` if should continue holding
    pub fn determine_exit_reason(&self, position: &Position) -> Option<ExitReason> {
        // Check in priority order
        if position.hit_take_profit() {
            Some(ExitReason::TakeProfit)
        } else if position.hit_trailing_stop() {
            Some(ExitReason::TrailingStop)
        } else if position.hit_stop_loss() {
            Some(ExitReason::StopLoss)
        } else if position.is_expired() {
            Some(ExitReason::MaxHoldTime)
        } else {
            None
        }
    }

    /// Check if position should be exited based on settlement timing (smart exit)
    ///
    /// **Settlement-Aware Logic:**
    /// - Within 30 minutes of settlement: Exit LOSING positions, hold WINNING positions
    /// - Rationale: If you're losing near settlement, you're wrong - cut it now
    /// - If you're winning near settlement, hold to 100% (free money)
    ///
    /// **Why this works:**
    /// - Sports games resolve at known times (event_time)
    /// - Within 30 min of settlement, outcome is usually clear
    /// - Losing positions won't recover - exit to save pennies
    /// - Winning positions will hit 100% at settlement - hold for full profit
    ///
    /// # Arguments
    ///
    /// * `position` - Position to evaluate
    /// * `market` - Market data (needed for event_time)
    /// * `current_price` - Current market price for the position's side
    ///
    /// # Returns
    ///
    /// `true` if should exit (losing position near settlement), `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::ExitManager;
    /// # use calchas::models::{Position, Market};
    /// # use rust_decimal_macros::dec;
    /// # fn example(position: &Position, market: &Market) {
    /// let exit_mgr = ExitManager::new();
    /// let current_price = dec!(0.15);  // Price dropped from 0.75 entry
    ///
    /// // Within 30 min of settlement, losing position → exit
    /// if exit_mgr.check_settlement_logic(position, market, current_price) {
    ///     println!("Cut losing position before settlement");
    /// }
    /// # }
    /// ```
    pub fn check_settlement_logic(
        &self,
        position: &Position,
        market: &Market,
        current_price: Decimal,
    ) -> bool {
        let now = Utc::now();

        // Calculate time to settlement
        let time_to_settlement = market.event_time.signed_duration_since(now);
        let minutes_to_settlement = time_to_settlement.num_minutes();

        // Only apply logic within 30 minutes of settlement
        // Window: 0 < minutes < 30 (1-29 minutes inclusive)
        // (and not after settlement has passed)
        if minutes_to_settlement <= 0 || minutes_to_settlement >= 30 {
            return false;
        }

        // Determine if position is winning or losing
        let is_winning = match position.side {
            PositionSide::Yes => current_price > position.entry_price,
            PositionSide::No => current_price > position.entry_price,
        };

        // Exit if LOSING near settlement (you're wrong, cut it)
        // Hold if WINNING near settlement (ride to 100%)
        !is_winning
    }
}

impl Default for ExitManager {
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
        ExitTarget, MarketId, OrderId, PositionId, PositionSide, PositionStatus, StrategyId,
    };
    use chrono::{Duration, Utc};
    use rust_decimal_macros::dec;

    // Helper: Create test position with specific exit target
    fn create_test_position(exit_target: ExitTarget) -> Position {
        Position {
            id: PositionId::new(),
            strategy_id: StrategyId::new("test-strategy".to_string()),
            market_id: MarketId::new("TEST-MARKET".to_string()),
            entry_order_id: OrderId::new("test-order-1".to_string()),
            exit_order_id: None,
            side: PositionSide::Yes,
            quantity: 10,
            entry_price: dec!(0.50),
            current_price: dec!(0.50),
            exit_target,
            unrealized_pnl: dec!(0.00),
            status: PositionStatus::Active,
            entry_timestamp: Utc::now(),
            peak_pnl: dec!(0.00),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_should_exit_take_profit_hit() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.current_price = dec!(0.76); // Above TP

        let exit_mgr = ExitManager::new();

        assert!(exit_mgr.should_exit(&position));
    }

    #[test]
    fn test_should_exit_stop_loss_hit() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.current_price = dec!(0.24); // Below SL

        let exit_mgr = ExitManager::new();

        assert!(exit_mgr.should_exit(&position));
    }

    #[test]
    fn test_should_exit_trailing_stop_hit() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: Some(dec!(0.10)),
            expiry_time: None,
        });
        position.peak_pnl = dec!(5.00); // +$5.00
        position.current_price = dec!(0.60); // Entry was 0.50, peak at 1.00, now 0.60

        let exit_mgr = ExitManager::new();

        assert!(exit_mgr.should_exit(&position));
    }

    #[test]
    fn test_should_exit_expired() {
        let position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: Some(Utc::now() - Duration::seconds(1)), // Expired 1 second ago
        });

        let exit_mgr = ExitManager::new();

        assert!(exit_mgr.should_exit(&position));
    }

    #[test]
    fn test_should_not_exit_no_conditions_met() {
        let position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: Some(Utc::now() + Duration::hours(1)), // Not expired
        });

        let exit_mgr = ExitManager::new();

        assert!(!exit_mgr.should_exit(&position));
    }

    #[test]
    fn test_determine_exit_reason_take_profit() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.current_price = dec!(0.76);

        let exit_mgr = ExitManager::new();

        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::TakeProfit)
        );
    }

    #[test]
    fn test_determine_exit_reason_stop_loss() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.current_price = dec!(0.24);

        let exit_mgr = ExitManager::new();

        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::StopLoss)
        );
    }

    #[test]
    fn test_determine_exit_reason_max_hold_time() {
        let position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: Some(Utc::now() - Duration::seconds(1)),
        });

        let exit_mgr = ExitManager::new();

        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::MaxHoldTime)
        );
    }

    #[test]
    fn test_determine_exit_reason_none() {
        let position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: Some(Utc::now() + Duration::hours(1)),
        });

        let exit_mgr = ExitManager::new();

        assert_eq!(exit_mgr.determine_exit_reason(&position), None);
    }

    #[test]
    fn test_determine_exit_reason_trailing_stop() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: Some(dec!(0.10)),
            expiry_time: None,
        });
        // Set up trailing stop trigger
        position.peak_pnl = dec!(5.00);
        position.current_price = dec!(0.60);

        let exit_mgr = ExitManager::new();

        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::TrailingStop)
        );
    }

    #[test]
    fn test_priority_take_profit_over_trailing_stop() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: Some(dec!(0.10)),
            expiry_time: None,
        });
        // Set price that triggers BOTH take profit and trailing stop
        position.peak_pnl = dec!(5.00);
        position.current_price = dec!(0.76); // Above TP and below trailing

        let exit_mgr = ExitManager::new();

        // Take profit should win (higher priority)
        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::TakeProfit)
        );
    }

    #[test]
    fn test_priority_trailing_stop_over_stop_loss() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: Some(dec!(0.10)),
            expiry_time: None,
        });
        // Set price that triggers BOTH trailing stop and stop loss
        position.peak_pnl = dec!(2.00);
        position.current_price = dec!(0.20); // Below SL and triggers trailing

        let exit_mgr = ExitManager::new();

        // Trailing stop should win (higher priority)
        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::TrailingStop)
        );
    }

    #[test]
    fn test_priority_stop_loss_over_max_hold_time() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: Some(Utc::now() - Duration::seconds(1)), // Expired
        });
        // Trigger both stop loss and expiry
        position.current_price = dec!(0.20); // Below SL

        let exit_mgr = ExitManager::new();

        // Stop loss should win (higher priority)
        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::StopLoss)
        );
    }

    #[test]
    fn test_no_exit_targets_defined() {
        let position = create_test_position(ExitTarget {
            take_profit_price: None,
            stop_loss_price: None,
            trailing_stop_distance: None,
            expiry_time: None,
        });

        let exit_mgr = ExitManager::new();

        // With no exit targets, should never exit
        assert!(!exit_mgr.should_exit(&position));
        assert_eq!(exit_mgr.determine_exit_reason(&position), None);
    }

    #[test]
    fn test_exact_boundary_take_profit() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        // Exact boundary: current_price == take_profit_price
        position.current_price = dec!(0.75);

        let exit_mgr = ExitManager::new();

        // Should trigger (>= comparison)
        assert!(exit_mgr.should_exit(&position));
        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::TakeProfit)
        );
    }

    #[test]
    fn test_exact_boundary_stop_loss() {
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        // Exact boundary: current_price == stop_loss_price
        position.current_price = dec!(0.25);

        let exit_mgr = ExitManager::new();

        // Should trigger (<= comparison)
        assert!(exit_mgr.should_exit(&position));
        assert_eq!(
            exit_mgr.determine_exit_reason(&position),
            Some(ExitReason::StopLoss)
        );
    }

    // =============================================================================
    // SETTLEMENT LOGIC TESTS
    // =============================================================================

    // Helper: Create test market with specific settlement time
    fn create_test_market(minutes_to_settlement: i64) -> Market {
        use crate::models::{MarketStatus, MarketCategory};

        Market {
            id: MarketId::new("TEST-MARKET".to_string()),
            ticker: "TEST-001".to_string(),
            title: "Test Market".to_string(),
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Sports,
            sub_category: None,
            status: MarketStatus::Active,
            yes_price: dec!(0.50),
            yes_bid: dec!(0.49),
            yes_ask: dec!(0.51),
            no_price: dec!(0.50),
            no_bid: dec!(0.49),
            no_ask: dec!(0.51),
            volume: 10000,
            volume_24h: 5000,
            open_interest: 5000,
            event_time: Utc::now() + Duration::minutes(minutes_to_settlement),
            close_time: Utc::now() + Duration::days(1), // Placeholder
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_settlement_logic_losing_yes_position_within_30_min() {
        // YES position losing (current price < entry price)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.75); // Entered at 75 cents
        position.current_price = dec!(0.15); // Now at 15 cents (LOSING)

        let market = create_test_market(15); // 15 minutes to settlement
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should exit losing position near settlement
        assert!(exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_winning_yes_position_within_30_min() {
        // YES position winning (current price > entry price)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.25); // Entered at 25 cents
        position.current_price = dec!(0.85); // Now at 85 cents (WINNING)

        let market = create_test_market(15); // 15 minutes to settlement
        let current_price = dec!(0.85);

        let exit_mgr = ExitManager::new();

        // Should NOT exit winning position (hold to 100%)
        assert!(!exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_losing_no_position_within_30_min() {
        // NO position losing (current price < entry price)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::No;
        position.entry_price = dec!(0.75); // Entered at 75 cents
        position.current_price = dec!(0.15); // Now at 15 cents (LOSING)

        let market = create_test_market(20); // 20 minutes to settlement
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should exit losing position near settlement
        assert!(exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_winning_no_position_within_30_min() {
        // NO position winning (current price > entry price)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::No;
        position.entry_price = dec!(0.25); // Entered at 25 cents
        position.current_price = dec!(0.85); // Now at 85 cents (WINNING)

        let market = create_test_market(10); // 10 minutes to settlement
        let current_price = dec!(0.85);

        let exit_mgr = ExitManager::new();

        // Should NOT exit winning position (hold to 100%)
        assert!(!exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_too_far_from_settlement() {
        // Position losing but more than 30 minutes to settlement
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.75);
        position.current_price = dec!(0.15); // LOSING

        let market = create_test_market(60); // 60 minutes to settlement (too far)
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should NOT apply settlement logic (too far from settlement)
        assert!(!exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_past_settlement() {
        // Settlement already passed
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.75);
        position.current_price = dec!(0.15); // LOSING

        let market = create_test_market(-5); // Settlement was 5 minutes ago
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should NOT apply settlement logic (already settled)
        assert!(!exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_outside_window_31_minutes() {
        // 31 minutes to settlement (outside window - should NOT apply)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.75);
        position.current_price = dec!(0.15); // LOSING

        let market = create_test_market(31); // 31 minutes (outside window)
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should NOT apply (logic is: minutes >= 30, so 31 is excluded)
        assert!(!exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_exactly_0_minutes() {
        // Exactly at settlement time (boundary - should NOT apply)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.75);
        position.current_price = dec!(0.15); // LOSING

        let market = create_test_market(0); // Exactly at settlement
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should NOT apply (logic is: minutes <= 0, so 0 is excluded)
        assert!(!exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_within_window_5_minutes() {
        // 5 minutes to settlement (well within window)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.75);
        position.current_price = dec!(0.15); // LOSING

        let market = create_test_market(5); // 5 minutes to settlement
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should apply (within 0 < minutes < 30 window)
        assert!(exit_mgr.check_settlement_logic(&position, &market, current_price));
    }

    #[test]
    fn test_settlement_logic_29_minutes() {
        // 29 minutes to settlement (just inside upper boundary)
        let mut position = create_test_position(ExitTarget {
            take_profit_price: Some(dec!(0.75)),
            stop_loss_price: Some(dec!(0.25)),
            trailing_stop_distance: None,
            expiry_time: None,
        });
        position.side = PositionSide::Yes;
        position.entry_price = dec!(0.75);
        position.current_price = dec!(0.15); // LOSING

        let market = create_test_market(29); // 29 minutes to settlement
        let current_price = dec!(0.15);

        let exit_mgr = ExitManager::new();

        // Should apply (within 0 < minutes <= 30 window)
        assert!(exit_mgr.check_settlement_logic(&position, &market, current_price));
    }
}
