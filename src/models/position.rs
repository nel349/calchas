// Position data model
// Section 4.3 of TECHNICAL_ARCHITECTURE.md

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::market::MarketId;
use super::strategy::StrategyId;
use super::order::OrderId;

// =============================================================================
// NEWTYPE PATTERN - PositionId
// =============================================================================

/// Unique position identifier (UUID-based newtype)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PositionId(pub Uuid);

impl PositionId {
    pub fn new() -> Self {
        PositionId(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        PositionId(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for PositionId {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ENUMS
// =============================================================================

/// Which side of the market we're on
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Yes,  // Betting the event will happen
    No,   // Betting the event won't happen
}

/// Position status lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionStatus {
    Active,       // Position is open
    ExitPending,  // Exit order placed, waiting for fill
    Closed,       // Position closed successfully
    Error,        // Something went wrong
}

// =============================================================================
// EXIT TARGET STRUCT
// =============================================================================

/// Exit criteria for the position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitTarget {
    // Price-based exits
    pub take_profit_price: Option<Decimal>,
    pub stop_loss_price: Option<Decimal>,

    // Trailing stop (distance from peak in cents)
    pub trailing_stop_distance: Option<Decimal>,

    // Time-based exit
    pub expiry_time: Option<DateTime<Utc>>,
}

// =============================================================================
// MAIN STRUCT
// =============================================================================

/// Represents an open trading position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    // Identification
    pub id: PositionId,
    pub market_id: MarketId,
    pub strategy_id: StrategyId,

    // Entry details
    pub side: PositionSide,
    pub entry_price: Decimal,
    pub quantity: u64,
    pub entry_timestamp: DateTime<Utc>,
    pub entry_order_id: OrderId,

    // Current state
    pub current_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub peak_pnl: Decimal,  // For trailing stops

    // Exit tracking
    pub exit_target: ExitTarget,
    pub exit_order_id: Option<OrderId>,
    pub status: PositionStatus,

    // Timestamps
    pub updated_at: DateTime<Utc>,
}

impl Position {
    /// Create a new position
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        market_id: MarketId,
        strategy_id: StrategyId,
        side: PositionSide,
        entry_price: Decimal,
        quantity: u64,
        entry_order_id: OrderId,
        exit_target: ExitTarget,
    ) -> Self {
        let now = Utc::now();

        Position {
            id: PositionId::new(),
            market_id,
            strategy_id,
            side,
            entry_price,
            quantity,
            entry_timestamp: now,
            entry_order_id,
            current_price: entry_price,
            unrealized_pnl: Decimal::ZERO,
            peak_pnl: Decimal::ZERO,
            exit_target,
            exit_order_id: None,
            status: PositionStatus::Active,
            updated_at: now,
        }
    }

    /// Update current price and recalculate P&L
    pub fn update_price(&mut self, new_price: Decimal) {
        self.current_price = new_price;

        // Calculate gross P&L (price difference only)
        let price_diff = new_price - self.entry_price;
        let gross_pnl = price_diff * Decimal::from(self.quantity);

        // Calculate fees (use Kalshi's actual taker fee formula)
        let entry_fees = crate::kalshi::fees::calculate_kalshi_taker_fee(self.entry_price, self.quantity);
        let exit_fees = crate::kalshi::fees::calculate_kalshi_taker_fee(new_price, self.quantity);
        let total_fees = entry_fees + exit_fees;

        // Unrealized P&L = gross P&L - fees (what you'd actually get if you closed now)
        self.unrealized_pnl = gross_pnl - total_fees;

        // Update peak P&L for trailing stops
        if self.unrealized_pnl > self.peak_pnl {
            self.peak_pnl = self.unrealized_pnl;
        }

        self.updated_at = Utc::now();
    }

    /// Check if position is currently open
    pub fn is_open(&self) -> bool {
        matches!(self.status, PositionStatus::Active | PositionStatus::ExitPending)
    }

    /// Check if take profit target hit
    pub fn hit_take_profit(&self) -> bool {
        if let Some(tp_price) = self.exit_target.take_profit_price {
            self.current_price >= tp_price
        } else {
            false
        }
    }

    /// Check if stop loss hit
    pub fn hit_stop_loss(&self) -> bool {
        if let Some(sl_price) = self.exit_target.stop_loss_price {
            self.current_price <= sl_price
        } else {
            false
        }
    }

    /// Check if trailing stop hit
    pub fn hit_trailing_stop(&self) -> bool {
        if let Some(trailing_distance) = self.exit_target.trailing_stop_distance {
            let trailing_price = self.peak_pnl / Decimal::from(self.quantity) + self.entry_price - trailing_distance;
            self.current_price <= trailing_price
        } else {
            false
        }
    }

    /// Check if position expired
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.exit_target.expiry_time {
            Utc::now() >= expiry
        } else {
            false
        }
    }

    /// Check if any exit condition is met
    pub fn should_exit(&self) -> bool {
        self.hit_take_profit()
            || self.hit_stop_loss()
            || self.hit_trailing_stop()
            || self.is_expired()
    }

    /// Mark position as exit pending
    pub fn mark_exit_pending(&mut self, exit_order_id: OrderId) {
        self.exit_order_id = Some(exit_order_id);
        self.status = PositionStatus::ExitPending;
        self.updated_at = Utc::now();
    }

    /// Mark position as closed
    pub fn mark_closed(&mut self) {
        self.status = PositionStatus::Closed;
        self.updated_at = Utc::now();
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_position() -> Position {
        let exit_target = ExitTarget {
            take_profit_price: Some(dec!(0.24)),
            stop_loss_price: Some(dec!(0.08)),
            trailing_stop_distance: None,
            expiry_time: None,
        };

        Position::new(
            MarketId::new("TEST-001".to_string()),
            StrategyId::new("strat-1".to_string()),
            PositionSide::Yes,
            dec!(0.11),
            100,
            OrderId::new("order-123".to_string()),
            exit_target,
        )
    }

    #[test]
    fn test_position_creation() {
        let position = create_test_position();
        assert_eq!(position.entry_price, dec!(0.11));
        assert_eq!(position.quantity, 100);
        assert!(position.is_open());
    }

    #[test]
    fn test_update_price() {
        let mut position = create_test_position();
        position.update_price(dec!(0.15));

        assert_eq!(position.current_price, dec!(0.15));

        // Unrealized P&L now includes fees:
        // Gross P&L: (0.15 - 0.11) * 100 = $4.00
        // Entry fees: 0.07 * 100 * 0.11 * (1 - 0.11) = 0.6853
        // Exit fees: 0.07 * 100 * 0.15 * (1 - 0.15) = 0.8925
        // Net P&L: 4.00 - 0.6853 - 0.8925 = 2.4222
        assert_eq!(position.unrealized_pnl, dec!(2.4222));
        assert_eq!(position.peak_pnl, dec!(2.4222));
    }

    #[test]
    fn test_hit_take_profit() {
        let mut position = create_test_position();
        assert!(!position.hit_take_profit());

        position.update_price(dec!(0.25));
        assert!(position.hit_take_profit());
    }

    #[test]
    fn test_hit_stop_loss() {
        let mut position = create_test_position();
        assert!(!position.hit_stop_loss());

        position.update_price(dec!(0.07));
        assert!(position.hit_stop_loss());
    }

    #[test]
    fn test_should_exit() {
        let mut position = create_test_position();
        assert!(!position.should_exit());

        // Hit take profit
        position.update_price(dec!(0.25));
        assert!(position.should_exit());
    }

    #[test]
    fn test_mark_closed() {
        let mut position = create_test_position();
        position.mark_closed();

        assert_eq!(position.status, PositionStatus::Closed);
        assert!(!position.is_open());
    }
}
