//! Order execution
//!
//! This module handles converting signals and exit decisions into orders,
//! then simulating their execution. In Phase 4, all execution is simulated
//! using real market prices from Kalshi.
//!
//! # Entry Flow
//!
//! 1. Receive EntrySignal from strategy engine
//! 2. Convert signal to Order (signal_to_order)
//! 3. Simulate order fill using real prices
//! 4. Return filled Order
//!
//! # Exit Flow
//!
//! 1. Receive Position + ExitReason
//! 2. Create exit Order
//! 3. Simulate fill
//! 4. Return filled Order
//!
//! # Example
//!
//! ```no_run
//! use calchas::trading::OrderExecutor;
//! # use calchas::trading::OrderSimulator;
//! # use calchas::strategy::signals::EntrySignal;
//!
//! # async fn example(simulator: OrderSimulator, signal: EntrySignal) -> Result<(), Box<dyn std::error::Error>> {
//! let mut executor = OrderExecutor::new(simulator);
//!
//! let filled_order = executor.execute_entry(&signal).await?;
//! println!("Entry filled at ${:.2}", filled_order.average_fill_price.unwrap());
//! # Ok(())
//! # }
//! ```

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::models::{
    ExitReason, Order, OrderAction, OrderId, OrderSide, OrderType, Position,
};
use crate::strategy::signals::{EntrySignal, SignalSide};
use super::error::TradingError;
use super::simulator::OrderSimulator;

// =============================================================================
// ORDER EXECUTOR
// =============================================================================

/// Order executor (simulation mode)
///
/// Converts trading signals and exit decisions into orders, then simulates
/// their execution using real market prices.
pub struct OrderExecutor {
    simulator: OrderSimulator,
    order_history: Vec<Order>,
}

impl OrderExecutor {
    /// Create a new order executor
    ///
    /// # Arguments
    ///
    /// * `simulator` - Order simulator for executing fills
    pub fn new(simulator: OrderSimulator) -> Self {
        OrderExecutor {
            simulator,
            order_history: Vec::new(),
        }
    }

    /// Execute entry order from signal
    ///
    /// Converts an entry signal into an order, simulates execution,
    /// and returns the filled order.
    ///
    /// # Arguments
    ///
    /// * `signal` - Entry signal from strategy engine
    ///
    /// # Returns
    ///
    /// * `Ok(Order)` - Filled order ready to open position
    /// * `Err(TradingError)` - Failed to execute order
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::OrderExecutor;
    /// # use calchas::trading::OrderSimulator;
    /// # use calchas::strategy::signals::EntrySignal;
    /// # async fn example(mut executor: OrderExecutor, signal: EntrySignal) -> Result<(), Box<dyn std::error::Error>> {
    /// let filled_order = executor.execute_entry(&signal).await?;
    /// println!("Filled {} contracts at ${:.2}",
    ///     filled_order.filled_quantity,
    ///     filled_order.average_fill_price.unwrap()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_entry(&mut self, signal: &EntrySignal) -> Result<Order, TradingError> {
        // Convert signal to order
        let mut order = Self::signal_to_order(signal);

        // Simulate fill
        let fill = self.simulator.simulate_fill(&order).await?;

        // Update order with fill information
        order.update_fill(fill.filled_quantity, fill.fill_price);

        // Record in history
        self.order_history.push(order.clone());

        Ok(order)
    }

    /// Execute exit order for position
    ///
    /// Creates an exit order for a position, simulates execution,
    /// and returns the filled order.
    ///
    /// # Arguments
    ///
    /// * `position` - Position to exit
    /// * `exit_reason` - Why we're exiting (for logging/tracking)
    ///
    /// # Returns
    ///
    /// * `Ok(Order)` - Filled exit order
    /// * `Err(TradingError)` - Failed to execute order
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::OrderExecutor;
    /// # use calchas::models::{Position, ExitReason};
    /// # async fn example(mut executor: OrderExecutor, position: Position) -> Result<(), Box<dyn std::error::Error>> {
    /// let exit_order = executor.execute_exit(&position, ExitReason::TakeProfit).await?;
    /// println!("Exit filled at ${:.2}", exit_order.average_fill_price.unwrap());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_exit(
        &mut self,
        position: &Position,
        _exit_reason: ExitReason,
    ) -> Result<Order, TradingError> {
        // Create exit order
        let mut order = Self::position_to_exit_order(position);

        // Simulate fill
        let fill = self.simulator.simulate_fill(&order).await?;

        // Update order with fill information
        order.update_fill(fill.filled_quantity, fill.fill_price);

        // Record in history
        self.order_history.push(order.clone());

        Ok(order)
    }

    /// Get order history
    ///
    /// Returns all orders executed by this executor (for testing/logging).
    pub fn order_history(&self) -> &[Order] {
        &self.order_history
    }

    /// Convert entry signal to order
    ///
    /// This is the critical signal→order conversion logic.
    ///
    /// # Signal → Order Mapping
    ///
    /// - `SignalSide::Yes` → `OrderSide::Yes`
    /// - `SignalSide::No` → `OrderSide::No`
    /// - `action` = `OrderAction::Buy` (always Buy for entry)
    /// - `order_type` from signal (Market or Limit)
    /// - `limit_price` = recommended_price + offset (if Limit order)
    /// - `position_id` = None (entry orders don't have positions yet)
    fn signal_to_order(signal: &EntrySignal) -> Order {
        // Convert SignalSide to OrderSide
        let side = match signal.side {
            SignalSide::Yes => OrderSide::Yes,
            SignalSide::No => OrderSide::No,
        };

        // Convert strategy OrderType to model OrderType
        let order_type = match signal.order_type {
            crate::models::strategy::OrderType::Market => OrderType::Market,
            crate::models::strategy::OrderType::Limit => OrderType::Limit,
        };

        // Calculate limit price if needed
        let limit_price = if order_type == OrderType::Limit {
            let offset = signal.limit_price_offset.unwrap_or(Decimal::ZERO);
            Some(signal.recommended_price + offset)
        } else {
            None
        };

        // Generate unique order ID
        let order_id = OrderId::new(format!("sim-entry-{}", Uuid::new_v4()));

        Order::new(
            order_id,
            signal.market_id.clone(),
            None, // No position yet (entry order)
            side,
            OrderAction::Buy,
            order_type,
            limit_price,
            signal.position_size,
        )
    }

    /// Convert position to exit order
    ///
    /// Creates a Market order to sell/close the position.
    ///
    /// # Exit Order Details
    ///
    /// - `side` = same as position (we're selling what we bought)
    /// - `action` = `OrderAction::Sell` (always Sell for exit)
    /// - `order_type` = `OrderType::Market` (exit ASAP)
    /// - `position_id` = Some(position.id) (links to position)
    fn position_to_exit_order(position: &Position) -> Order {
        // Convert PositionSide to OrderSide
        let side = match position.side {
            crate::models::PositionSide::Yes => OrderSide::Yes,
            crate::models::PositionSide::No => OrderSide::No,
        };

        // Generate unique order ID
        let order_id = OrderId::new(format!("sim-exit-{}", Uuid::new_v4()));

        Order::new(
            order_id,
            position.market_id.clone(),
            Some(position.id.clone()),
            side,
            OrderAction::Sell,
            OrderType::Market, // Always market order for exits
            None,              // No limit price for market orders
            position.quantity,
        )
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        strategy::{OrderType as StrategyOrderType},
        ExitTarget, MarketId, PositionId, PositionSide, PositionStatus,
        StrategyId,
    };
    use chrono::Utc;
    use rust_decimal_macros::dec;

    // Helper: Create test signal
    fn create_test_signal(
        side: SignalSide,
        order_type: StrategyOrderType,
        limit_offset: Option<Decimal>,
    ) -> EntrySignal {
        EntrySignal {
            market_id: MarketId::new("TEST-MARKET".to_string()),
            market_ticker: "TEST-MARKET".to_string(),
            market_title: "Test Market".to_string(),
            strategy_id: StrategyId::new("test-strategy".to_string()),
            strategy_name: "Test Strategy".to_string(),
            side,
            recommended_price: dec!(0.50),
            position_size: 100,
            order_type,
            limit_price_offset: limit_offset,
            generated_at: Utc::now(),
            time_to_event_minutes: 1440.0,
            market_volume: 1000,
            market_open_interest: 500,
        }
    }

    // Helper: Create test position
    fn create_test_position(side: PositionSide, quantity: u64) -> Position {
        Position {
            id: PositionId::new(),
            market_id: MarketId::new("TEST-MARKET".to_string()),
            strategy_id: StrategyId::new("test-strategy".to_string()),
            side,
            entry_price: dec!(0.50),
            quantity,
            entry_timestamp: Utc::now(),
            entry_order_id: OrderId::new("entry-123".to_string()),
            current_price: dec!(0.60),
            unrealized_pnl: dec!(10.00),
            peak_pnl: dec!(10.00),
            exit_target: ExitTarget {
                take_profit_price: Some(dec!(0.75)),
                stop_loss_price: Some(dec!(0.25)),
                trailing_stop_distance: None,
                expiry_time: None,
            },
            exit_order_id: None,
            status: PositionStatus::Active,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_signal_to_order_market_yes() {
        let signal = create_test_signal(SignalSide::Yes, StrategyOrderType::Market, None);

        let order = OrderExecutor::signal_to_order(&signal);

        assert_eq!(order.side, OrderSide::Yes);
        assert_eq!(order.action, OrderAction::Buy);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.limit_price, None);
        assert_eq!(order.quantity, 100);
        assert_eq!(order.position_id, None);
    }

    #[test]
    fn test_signal_to_order_market_no() {
        let signal = create_test_signal(SignalSide::No, StrategyOrderType::Market, None);

        let order = OrderExecutor::signal_to_order(&signal);

        assert_eq!(order.side, OrderSide::No);
        assert_eq!(order.action, OrderAction::Buy);
        assert_eq!(order.order_type, OrderType::Market);
    }

    #[test]
    fn test_signal_to_order_limit_with_offset() {
        let signal = create_test_signal(
            SignalSide::Yes,
            StrategyOrderType::Limit,
            Some(dec!(-0.01)), // 1 cent below
        );

        let order = OrderExecutor::signal_to_order(&signal);

        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.limit_price, Some(dec!(0.49))); // 0.50 - 0.01
    }

    #[test]
    fn test_signal_to_order_limit_no_offset() {
        let signal = create_test_signal(SignalSide::Yes, StrategyOrderType::Limit, None);

        let order = OrderExecutor::signal_to_order(&signal);

        assert_eq!(order.order_type, OrderType::Limit);
        assert_eq!(order.limit_price, Some(dec!(0.50))); // recommended_price + 0
    }

    #[test]
    fn test_signal_to_order_limit_positive_offset() {
        let signal = create_test_signal(
            SignalSide::No,
            StrategyOrderType::Limit,
            Some(dec!(0.02)), // 2 cents above
        );

        let order = OrderExecutor::signal_to_order(&signal);

        assert_eq!(order.limit_price, Some(dec!(0.52))); // 0.50 + 0.02
    }

    #[test]
    fn test_signal_to_order_generates_unique_ids() {
        let signal = create_test_signal(SignalSide::Yes, StrategyOrderType::Market, None);

        let order1 = OrderExecutor::signal_to_order(&signal);
        let order2 = OrderExecutor::signal_to_order(&signal);

        // IDs should be different (UUIDs)
        assert_ne!(order1.id.as_str(), order2.id.as_str());
    }

    #[test]
    fn test_position_to_exit_order_yes() {
        let position = create_test_position(PositionSide::Yes, 100);

        let order = OrderExecutor::position_to_exit_order(&position);

        assert_eq!(order.side, OrderSide::Yes);
        assert_eq!(order.action, OrderAction::Sell);
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.quantity, 100);
        assert_eq!(order.position_id, Some(position.id.clone()));
    }

    #[test]
    fn test_position_to_exit_order_no() {
        let position = create_test_position(PositionSide::No, 50);

        let order = OrderExecutor::position_to_exit_order(&position);

        assert_eq!(order.side, OrderSide::No);
        assert_eq!(order.action, OrderAction::Sell);
        assert_eq!(order.quantity, 50);
    }

    #[test]
    fn test_position_to_exit_order_generates_unique_ids() {
        let position = create_test_position(PositionSide::Yes, 100);

        let order1 = OrderExecutor::position_to_exit_order(&position);
        let order2 = OrderExecutor::position_to_exit_order(&position);

        assert_ne!(order1.id.as_str(), order2.id.as_str());
    }

    #[test]
    fn test_signal_to_order_preserves_market_id() {
        let signal = create_test_signal(SignalSide::Yes, StrategyOrderType::Market, None);

        let order = OrderExecutor::signal_to_order(&signal);

        assert_eq!(order.market_id.as_str(), "TEST-MARKET");
    }

    #[test]
    fn test_signal_to_order_sets_initial_status() {
        let signal = create_test_signal(SignalSide::Yes, StrategyOrderType::Market, None);

        let order = OrderExecutor::signal_to_order(&signal);

        assert_eq!(order.status, crate::models::OrderStatus::Pending);
        assert_eq!(order.filled_quantity, 0);
        assert_eq!(order.average_fill_price, None);
    }

    #[test]
    fn test_signal_to_order_sets_timestamps() {
        let before = Utc::now();
        let signal = create_test_signal(SignalSide::Yes, StrategyOrderType::Market, None);

        let order = OrderExecutor::signal_to_order(&signal);
        let after = Utc::now();

        // Timestamps should be between before and after
        assert!(order.created_at >= before);
        assert!(order.created_at <= after);
        assert!(order.updated_at >= before);
        assert!(order.updated_at <= after);
    }

    #[test]
    fn test_position_to_exit_order_preserves_market_id() {
        let position = create_test_position(PositionSide::Yes, 100);

        let order = OrderExecutor::position_to_exit_order(&position);

        assert_eq!(order.market_id, position.market_id);
    }

    #[test]
    fn test_position_to_exit_order_links_to_position() {
        let position = create_test_position(PositionSide::Yes, 100);

        let order = OrderExecutor::position_to_exit_order(&position);

        assert_eq!(order.position_id, Some(position.id.clone()));
    }

    #[test]
    fn test_position_to_exit_order_always_market() {
        let position = create_test_position(PositionSide::Yes, 100);

        let order = OrderExecutor::position_to_exit_order(&position);

        // Exit orders should always be Market orders (exit ASAP)
        assert_eq!(order.order_type, OrderType::Market);
        assert_eq!(order.limit_price, None);
    }

    // =========================================================================
    // INTEGRATION TESTS
    // =========================================================================
    //
    // These test the full execute_entry() and execute_exit() flows.
    // We use a mock OrderSimulator to avoid needing a real KalshiClient.

    mod integration {
        use super::*;
        use std::sync::Arc;
        use crate::kalshi::client::KalshiClient;
        use crate::trading::simulator::SimulatedFill;

        // Mock: Create a fake KalshiClient for testing
        // This doesn't actually make network calls
        fn create_mock_kalshi_client() -> Arc<KalshiClient> {
            // Note: This will fail to construct without real credentials
            // For now, we'll skip these tests until we have proper mocking
            // infrastructure. The synchronous tests above provide good coverage
            // of the conversion logic, which is the critical part.
            unimplemented!("Mock KalshiClient not implemented yet - covered in Phase 4 integration demo")
        }

        // When we add proper mocking, tests would look like:
        //
        // #[tokio::test]
        // async fn test_execute_entry_fills_order() {
        //     let client = create_mock_kalshi_client();
        //     let simulator = OrderSimulator::new(client);
        //     let mut executor = OrderExecutor::new(simulator);
        //
        //     let signal = create_test_signal(...);
        //     let order = executor.execute_entry(&signal).await.unwrap();
        //
        //     assert!(order.is_filled());
        //     assert!(order.average_fill_price.is_some());
        // }
    }
}
