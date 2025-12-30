// Trade data model (historical record)
// Section 4.5 of TECHNICAL_ARCHITECTURE.md

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;

use super::market::MarketId;
use super::strategy::StrategyId;
use super::position::PositionId;
use super::order::OrderId;

// =============================================================================
// NEWTYPE PATTERN - TradeId
// =============================================================================

/// Unique trade identifier (UUID-based newtype)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradeId(pub Uuid);

impl TradeId {
    pub fn new() -> Self {
        TradeId(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        TradeId(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for TradeId {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// ENUMS
// =============================================================================

/// Reason for exiting the position
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    TakeProfit,        // Hit profit target
    StopLoss,          // Hit stop loss
    TrailingStop,      // Hit trailing stop
    MaxHoldTime,       // Time-based exit
    ManualExit,        // User manually closed
    StrategyDisabled,  // Strategy was turned off
    MarketClosed,      // Market settled/closed
}

// =============================================================================
// MAIN STRUCT
// =============================================================================

/// Immutable record of a completed trade (for analytics)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    // Identification
    pub id: TradeId,
    pub position_id: PositionId,
    pub market_id: MarketId,
    pub strategy_id: StrategyId,

    // Entry details
    pub entry_order_id: OrderId,
    pub entry_price: Decimal,
    pub entry_quantity: u64,
    pub entry_timestamp: DateTime<Utc>,

    // Exit details
    pub exit_order_id: OrderId,
    pub exit_price: Decimal,
    pub exit_quantity: u64,
    pub exit_timestamp: DateTime<Utc>,
    pub exit_reason: ExitReason,

    // Performance metrics
    pub gross_pnl: Decimal,
    pub fees: Decimal,
    pub net_pnl: Decimal,
    pub return_pct: Decimal,
    pub hold_duration: Duration,

    // Optional metadata
    pub notes: Option<String>,
}

impl Trade {
    /// Create a new trade record
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position_id: PositionId,
        market_id: MarketId,
        strategy_id: StrategyId,
        entry_order_id: OrderId,
        entry_price: Decimal,
        entry_quantity: u64,
        entry_timestamp: DateTime<Utc>,
        exit_order_id: OrderId,
        exit_price: Decimal,
        exit_quantity: u64,
        exit_timestamp: DateTime<Utc>,
        exit_reason: ExitReason,
        fees: Decimal,
    ) -> Self {
        // Calculate gross P&L
        let price_diff = exit_price - entry_price;
        let gross_pnl = price_diff * Decimal::from(exit_quantity);

        // Calculate net P&L (gross profit minus all fees)
        let net_pnl = gross_pnl - fees;

        // Calculate return percentage: net_pnl / total_capital_invested
        // Total capital = contract cost + entry fees
        // Note: fees param is total (entry + exit), so recalculate entry fees separately
        let entry_cost = entry_price * Decimal::from(entry_quantity);
        let entry_fees = crate::kalshi::fees::calculate_kalshi_taker_fee(entry_price, entry_quantity);
        let total_invested = entry_cost + entry_fees;
        let return_pct = if total_invested > Decimal::ZERO {
            (net_pnl / total_invested) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // Calculate hold duration
        let hold_duration = exit_timestamp - entry_timestamp;

        Trade {
            id: TradeId::new(),
            position_id,
            market_id,
            strategy_id,
            entry_order_id,
            entry_price,
            entry_quantity,
            entry_timestamp,
            exit_order_id,
            exit_price,
            exit_quantity,
            exit_timestamp,
            exit_reason,
            gross_pnl,
            fees,
            net_pnl,
            return_pct,
            hold_duration,
            notes: None,
        }
    }

    /// Check if trade was profitable
    pub fn is_profitable(&self) -> bool {
        self.net_pnl > Decimal::ZERO
    }

    /// Check if trade was a loss
    pub fn is_loss(&self) -> bool {
        self.net_pnl < Decimal::ZERO
    }

    /// Get hold duration in hours
    pub fn hold_duration_hours(&self) -> i64 {
        self.hold_duration.num_hours()
    }

    /// Get hold duration in minutes
    pub fn hold_duration_minutes(&self) -> i64 {
        self.hold_duration.num_minutes()
    }

    /// Add notes to the trade
    pub fn add_notes(&mut self, notes: String) {
        self.notes = Some(notes);
    }

    /// Check if exit was due to profit target
    pub fn hit_take_profit(&self) -> bool {
        self.exit_reason == ExitReason::TakeProfit
    }

    /// Check if exit was due to stop loss
    pub fn hit_stop_loss(&self) -> bool {
        self.exit_reason == ExitReason::StopLoss
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_trade() -> Trade {
        let entry_time = Utc::now();
        let exit_time = entry_time + Duration::hours(2);

        Trade::new(
            PositionId::new(),
            MarketId::new("TEST-001".to_string()),
            StrategyId::new("strat-1".to_string()),
            OrderId::new("order-entry".to_string()),
            dec!(0.11),  // entry price
            100,         // entry quantity
            entry_time,
            OrderId::new("order-exit".to_string()),
            dec!(0.24),  // exit price
            100,         // exit quantity
            exit_time,
            ExitReason::TakeProfit,
            dec!(1.96),  // fees
        )
    }

    #[test]
    fn test_trade_creation() {
        let trade = create_test_trade();
        assert_eq!(trade.entry_price, dec!(0.11));
        assert_eq!(trade.exit_price, dec!(0.24));
        assert_eq!(trade.entry_quantity, 100);
    }

    #[test]
    fn test_gross_pnl_calculation() {
        let trade = create_test_trade();
        // (0.24 - 0.11) * 100 = 13.00
        assert_eq!(trade.gross_pnl, dec!(13.00));
    }

    #[test]
    fn test_net_pnl_calculation() {
        let trade = create_test_trade();
        // 13.00 - 1.96 = 11.04
        assert_eq!(trade.net_pnl, dec!(11.04));
    }

    #[test]
    fn test_return_pct_calculation() {
        let trade = create_test_trade();
        // Return % = (net_pnl / total_invested) * 100
        // Total invested = contract cost + entry fees
        // Entry fees calculated using kalshi::fees (single source of truth)
        let entry_fees = crate::kalshi::fees::calculate_kalshi_taker_fee(dec!(0.11), 100);
        let total_invested = dec!(11.00) + entry_fees;
        let expected = (dec!(11.04) / total_invested) * Decimal::from(100);
        assert_eq!(trade.return_pct, expected);
    }

    #[test]
    fn test_is_profitable() {
        let trade = create_test_trade();
        assert!(trade.is_profitable());
        assert!(!trade.is_loss());
    }

    #[test]
    fn test_hold_duration() {
        let trade = create_test_trade();
        assert_eq!(trade.hold_duration_hours(), 2);
        assert_eq!(trade.hold_duration_minutes(), 120);
    }

    #[test]
    fn test_exit_reason_checks() {
        let trade = create_test_trade();
        assert!(trade.hit_take_profit());
        assert!(!trade.hit_stop_loss());
    }

    #[test]
    fn test_add_notes() {
        let mut trade = create_test_trade();
        trade.add_notes("Great momentum swing!".to_string());
        assert_eq!(trade.notes, Some("Great momentum swing!".to_string()));
    }
}
