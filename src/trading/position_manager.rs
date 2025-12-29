//! Position lifecycle management
//!
//! This module provides centralized management of trading positions, coordinating
//! between entry signals, exit conditions, and trade history.
//!
//! # Responsibilities
//!
//! 1. **Open positions** from filled entry orders
//! 2. **Track positions** in memory (HashMap)
//! 3. **Update prices** from Kalshi API
//! 4. **Monitor exits** via ExitManager
//! 5. **Close positions** and create Trade records
//!
//! # Architecture
//!
//! PositionManager is the central coordinator:
//! - Uses ExitManager to check exit conditions
//! - Uses OrderExecutor to place exit orders
//! - Uses KalshiClient to fetch current prices
//! - Creates Trade records when positions close
//!
//! # Example
//!
//! ```no_run
//! use calchas::trading::PositionManager;
//! # use calchas::trading::{ExitManager, OrderExecutor};
//! # use calchas::kalshi::client::KalshiClient;
//! # use std::sync::{Arc, Mutex};
//!
//! # async fn example(
//! #     kalshi_client: Arc<KalshiClient>,
//! #     exit_manager: ExitManager,
//! #     order_executor: Arc<Mutex<OrderExecutor>>,
//! # ) -> Result<(), Box<dyn std::error::Error>> {
//! let mut pos_mgr = PositionManager::new(
//!     kalshi_client,
//!     exit_manager,
//!     order_executor,
//! );
//!
//! // Position lifecycle:
//! // 1. Open position from filled order
//! # let filled_order = todo!();
//! # let strategy = todo!();
//! let position_id = pos_mgr.open_position(filled_order, &strategy)?;
//!
//! // 2. Update prices (returns positions that need to exit)
//! let to_exit = pos_mgr.update_prices().await?;
//!
//! // 3. Close positions
//! for pos_id in to_exit {
//!     let trade = pos_mgr.close_position(&pos_id).await?;
//!     println!("Trade closed: ${:.2} P&L", trade.net_pnl);
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;

use crate::kalshi::client::KalshiClient;
use crate::kalshi::types::GetMarketsRequest;
use crate::models::{
    ExitReason, ExitTarget, Market, Order, Position, PositionId, PositionSide,
    Strategy, Trade,
};
use crate::kalshi::fees::{calculate_kalshi_taker_fee};
use super::error::TradingError;
use super::exit_manager::ExitManager;
use super::order_executor::OrderExecutor;

// =============================================================================
// POSITION MANAGER
// =============================================================================

/// Central coordinator for position lifecycle
///
/// Manages all open positions, coordinates price updates, monitors exit
/// conditions, and creates trade records when positions close.
pub struct PositionManager {
    /// All positions (active and closed)
    positions: HashMap<PositionId, Position>,

    /// Kalshi client for fetching prices
    kalshi_client: Arc<KalshiClient>,

    /// Exit condition checker
    exit_manager: ExitManager,

    /// Order executor (shared, needs mutex for async execution)
    order_executor: Arc<Mutex<OrderExecutor>>,
}

impl PositionManager {
    /// Create a new position manager
    ///
    /// # Arguments
    ///
    /// * `kalshi_client` - Kalshi API client for price updates
    /// * `exit_manager` - Exit condition evaluator
    /// * `order_executor` - Order executor for exit orders
    pub fn new(
        kalshi_client: Arc<KalshiClient>,
        exit_manager: ExitManager,
        order_executor: Arc<Mutex<OrderExecutor>>,
    ) -> Self {
        PositionManager {
            positions: HashMap::new(),
            kalshi_client,
            exit_manager,
            order_executor,
        }
    }

    /// Open a new position from filled entry order
    ///
    /// Converts a filled order into an active position with calculated exit targets.
    ///
    /// # Arguments
    ///
    /// * `filled_order` - Order that was filled (must be filled)
    /// * `strategy` - Strategy that generated this position
    ///
    /// # Returns
    ///
    /// * `Ok(PositionId)` - ID of newly opened position
    /// * `Err(TradingError)` - Order not filled or invalid
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::PositionManager;
    /// # fn example(mut pos_mgr: PositionManager, order: calchas::models::Order, strategy: &calchas::models::Strategy) -> Result<(), Box<dyn std::error::Error>> {
    /// let position_id = pos_mgr.open_position(order, strategy)?;
    /// println!("Opened position: {:?}", position_id);
    /// # Ok(())
    /// # }
    /// ```
    pub fn open_position(
        &mut self,
        filled_order: Order,
        strategy: &Strategy,
    ) -> Result<PositionId, TradingError> {
        // Validate order is filled
        if !filled_order.is_filled() {
            return Err(TradingError::OrderNotFilled(filled_order.id.as_str().to_string()));
        }

        let fill_price = filled_order.average_fill_price
            .ok_or_else(|| TradingError::OrderNotFilled(filled_order.id.as_str().to_string()))?;

        // Calculate exit target from strategy rules
        let exit_target = Self::calculate_exit_target(
            fill_price,
            &strategy.exit_rules,
            filled_order.created_at,
        );

        // Convert OrderSide to PositionSide
        let side = match filled_order.side {
            crate::models::OrderSide::Yes => PositionSide::Yes,
            crate::models::OrderSide::No => PositionSide::No,
        };

        // Create position
        let position = Position::new(
            filled_order.market_id.clone(),
            strategy.id.clone(),
            side,
            fill_price,
            filled_order.filled_quantity,
            filled_order.id.clone(),
            exit_target,
        );

        let position_id = position.id.clone();
        self.positions.insert(position_id.clone(), position);

        Ok(position_id)
    }

    /// Update prices for all active positions
    ///
    /// Fetches current market prices from Kalshi, updates all active positions,
    /// and returns list of positions that should be exited.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<PositionId>)` - Positions that hit exit conditions
    /// * `Err(TradingError)` - Failed to fetch prices
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::PositionManager;
    /// # async fn example(mut pos_mgr: PositionManager) -> Result<(), Box<dyn std::error::Error>> {
    /// let to_exit = pos_mgr.update_prices().await?;
    /// println!("Positions to exit: {}", to_exit.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_prices(&mut self) -> Result<Vec<PositionId>, TradingError> {
        let mut positions_to_exit = Vec::new();

        // Get unique market IDs from active positions
        let market_ids: Vec<_> = self.positions
            .values()
            .filter(|p| p.is_open())
            .map(|p| p.market_id.clone())
            .collect();

        if market_ids.is_empty() {
            return Ok(positions_to_exit);
        }

        // Fetch current prices for all markets
        // Build map of market_id -> current_price
        let mut market_prices = HashMap::new();

        for market_id in market_ids.iter() {
            let market = self.fetch_market_price(market_id).await?;
            market_prices.insert(market_id.clone(), market);
        }

        // Update each active position
        for (position_id, position) in self.positions.iter_mut() {
            if !position.is_open() {
                continue;
            }

            if let Some(market) = market_prices.get(&position.market_id) {
                // Get price for position's side
                let current_price = match position.side {
                    PositionSide::Yes => market.yes_price,
                    PositionSide::No => market.no_price,
                };

                // Update position price (recalculates P&L)
                position.update_price(current_price);

                // Check if should exit
                if self.exit_manager.should_exit(position) {
                    positions_to_exit.push(position_id.clone());
                }
            }
        }

        Ok(positions_to_exit)
    }

    /// Close a position
    ///
    /// Executes exit order, marks position as closed, and creates trade record.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position to close
    ///
    /// # Returns
    ///
    /// * `Ok(Trade)` - Trade record for closed position
    /// * `Err(TradingError)` - Position not found or exit failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::PositionManager;
    /// # use calchas::models::PositionId;
    /// # async fn example(mut pos_mgr: PositionManager, position_id: PositionId) -> Result<(), Box<dyn std::error::Error>> {
    /// let trade = pos_mgr.close_position(&position_id).await?;
    /// println!("Closed with ${:.2} P&L", trade.net_pnl);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn close_position(&mut self, position_id: &PositionId) -> Result<Trade, TradingError> {
        // Get position
        let position = self.positions.get_mut(position_id)
            .ok_or_else(|| TradingError::PositionNotFound(format!("{:?}", position_id)))?;

        // Determine exit reason
        let exit_reason = self.exit_manager.determine_exit_reason(position)
            .ok_or_else(|| TradingError::NoExitCondition)?;

        // Execute exit order
        let exit_order = {
            let mut executor = self.order_executor.lock()
                .map_err(|_| TradingError::LockError)?;
            executor.execute_exit(position, exit_reason.clone()).await?
        };

        // Get exit price
        let exit_price = exit_order.average_fill_price
            .ok_or_else(|| TradingError::OrderNotFilled(exit_order.id.as_str().to_string()))?;

        // Mark position as closed
        position.mark_exit_pending(exit_order.id.clone());
        position.mark_closed();

        // Create trade record
        let trade = Self::create_trade_record(
            position,
            &exit_order,
            exit_price,
            exit_reason,
        );

        Ok(trade)
    }

    /// Get all active positions
    ///
    /// Returns references to all positions that are currently open.
    pub fn get_active_positions(&self) -> Vec<&Position> {
        self.positions.values()
            .filter(|p| p.is_open())
            .collect()
    }

    /// Get position by ID
    ///
    /// Returns reference to position if it exists.
    pub fn get_position(&self, position_id: &PositionId) -> Option<&Position> {
        self.positions.get(position_id)
    }

    /// Get mutable position by ID
    ///
    /// Returns mutable reference to position if it exists.
    pub fn get_position_mut(&mut self, position_id: &PositionId) -> Option<&mut Position> {
        self.positions.get_mut(position_id)
    }

    /// Get total count of positions
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    /// Get count of active positions
    pub fn active_position_count(&self) -> usize {
        self.get_active_positions().len()
    }

    // =========================================================================
    // PRIVATE HELPERS
    // =========================================================================

    /// Calculate exit target from strategy exit rules
    ///
    /// Converts percentage-based exit rules into absolute price levels.
    ///
    /// # Arguments
    ///
    /// * `entry_price` - Price position was entered at
    /// * `exit_rules` - Strategy's exit configuration
    /// * `entry_timestamp` - When position was opened
    ///
    /// # Returns
    ///
    /// ExitTarget with calculated price levels and expiry time
    fn calculate_exit_target(
        entry_price: Decimal,
        exit_rules: &crate::models::strategy::ExitRules,
        entry_timestamp: DateTime<Utc>,
    ) -> ExitTarget {
        // Calculate take profit price
        let take_profit_price = exit_rules.take_profit_pct.map(|pct| {
            let pct_decimal = pct / Decimal::from(100);
            entry_price + (entry_price * pct_decimal)
        });

        // Calculate stop loss price
        let stop_loss_price = exit_rules.stop_loss_pct.map(|pct| {
            let pct_decimal = pct / Decimal::from(100);
            entry_price - (entry_price * pct_decimal)
        });

        // Calculate trailing stop distance
        let trailing_stop_distance = exit_rules.trailing_stop_pct.map(|pct| {
            let pct_decimal = pct / Decimal::from(100);
            entry_price * pct_decimal
        });

        // Calculate expiry time
        let expiry_time = exit_rules.max_hold_time_minutes.map(|minutes| {
            entry_timestamp + Duration::minutes(minutes as i64)
        });

        ExitTarget {
            take_profit_price,
            stop_loss_price,
            trailing_stop_distance,
            expiry_time,
        }
    }

    /// Fetch current price for a market
    async fn fetch_market_price(&self, market_id: &crate::models::MarketId) -> Result<Market, TradingError> {
        let ticker = market_id.as_str();

        // Paginate through markets to find ours
        let mut request = GetMarketsRequest::default();
        request.limit = Some(200);

        loop {
            let response = self.kalshi_client.get_markets(request.clone()).await?;

            // Find our market
            if let Some(kalshi_market) = response.markets.iter().find(|m| m.ticker == ticker) {
                return Ok(kalshi_market.clone().into());
            }

            // Move to next page
            match response.cursor {
                Some(cursor) if !cursor.is_empty() => {
                    request.cursor = Some(cursor);
                }
                _ => {
                    return Err(TradingError::MarketNotFound(ticker.to_string()));
                }
            }
        }
    }

    /// Create trade record from closed position
    fn create_trade_record(
        position: &Position,
        exit_order: &Order,
        exit_price: Decimal,
        exit_reason: ExitReason,
    ) -> Trade {
        // Calculate fees (we assume taker fees for simplicity in simulation)
        let entry_fees = calculate_kalshi_taker_fee(position.entry_price, position.quantity);
        let exit_fees = calculate_kalshi_taker_fee(exit_price, position.quantity);
        let total_fees = entry_fees + exit_fees;

        // Trade::new calculates gross_pnl, net_pnl, return_pct, hold_duration internally
        Trade::new(
            position.id.clone(),
            position.market_id.clone(),
            position.strategy_id.clone(),
            position.entry_order_id.clone(),
            position.entry_price,
            position.quantity,
            position.entry_timestamp,
            exit_order.id.clone(),
            exit_price,
            exit_order.filled_quantity,
            exit_order.updated_at,
            exit_reason,
            total_fees,
        )
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::strategy::{EntryRules, EntrySide, ExitRules, OrderType, PositionSizeUnit, RiskLimits, StrategyFilters};
    use crate::models::{MarketId, OrderAction, OrderId, OrderSide, StrategyId};
    use rust_decimal_macros::dec;

    // Helper: Create test strategy
    fn create_test_strategy() -> Strategy {
        Strategy {
            id: StrategyId::new("test-strategy".to_string()),
            name: "Test Strategy".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            filters: StrategyFilters {
                categories: None,
                exclude_categories: None,
                series_ticker: None,
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
            entry_rules: EntryRules {
                side: EntrySide::CheaperSide,
                position_size: 100,
                position_size_unit: PositionSizeUnit::Contracts,
                order_type: OrderType::Market,
                limit_price_offset: None,
            },
            exit_rules: ExitRules {
                take_profit_pct: Some(dec!(50.0)),  // 50%
                stop_loss_pct: Some(dec!(30.0)),     // 30%
                trailing_stop_pct: Some(dec!(20.0)), // 20%
                trailing_stop_activation_pct: None,
                max_hold_time_minutes: Some(1440),   // 24 hours
                exit_order_type: OrderType::Market,
            },
            risk_limits: RiskLimits {
                max_concurrent_positions: 5,
                max_daily_loss_usd: Some(dec!(100.00)),
                max_position_loss_usd: None,
                loss_cooldown_minutes: None,
            },
        }
    }

    // Helper: Create filled order
    fn create_filled_order(
        order_id: &str,
        market_id: &str,
        side: OrderSide,
        price: Decimal,
        quantity: u64,
    ) -> Order {
        let mut order = Order::new(
            OrderId::new(order_id.to_string()),
            MarketId::new(market_id.to_string()),
            None,
            side,
            OrderAction::Buy,
            crate::models::OrderType::Market,
            None,
            quantity,
        );

        // Mark as filled
        order.update_fill(quantity, price);

        order
    }

    #[test]
    fn test_calculate_exit_target_take_profit() {
        let entry_price = dec!(0.50);
        let entry_time = Utc::now();
        let strategy = create_test_strategy();

        let exit_target = PositionManager::calculate_exit_target(
            entry_price,
            &strategy.exit_rules,
            entry_time,
        );

        // 50% above 0.50 = 0.75
        assert_eq!(exit_target.take_profit_price, Some(dec!(0.75)));
    }

    #[test]
    fn test_calculate_exit_target_stop_loss() {
        let entry_price = dec!(0.50);
        let entry_time = Utc::now();
        let strategy = create_test_strategy();

        let exit_target = PositionManager::calculate_exit_target(
            entry_price,
            &strategy.exit_rules,
            entry_time,
        );

        // 30% below 0.50 = 0.35
        assert_eq!(exit_target.stop_loss_price, Some(dec!(0.35)));
    }

    #[test]
    fn test_calculate_exit_target_trailing_stop() {
        let entry_price = dec!(0.50);
        let entry_time = Utc::now();
        let strategy = create_test_strategy();

        let exit_target = PositionManager::calculate_exit_target(
            entry_price,
            &strategy.exit_rules,
            entry_time,
        );

        // 20% of 0.50 = 0.10
        assert_eq!(exit_target.trailing_stop_distance, Some(dec!(0.10)));
    }

    #[test]
    fn test_calculate_exit_target_expiry() {
        let entry_time = Utc::now();
        let strategy = create_test_strategy();

        let exit_target = PositionManager::calculate_exit_target(
            dec!(0.50),
            &strategy.exit_rules,
            entry_time,
        );

        let expected_expiry = entry_time + Duration::hours(24);
        assert_eq!(exit_target.expiry_time, Some(expected_expiry));
    }

    #[test]
    fn test_calculate_exit_target_no_rules() {
        let mut strategy = create_test_strategy();
        strategy.exit_rules.take_profit_pct = None;
        strategy.exit_rules.stop_loss_pct = None;
        strategy.exit_rules.trailing_stop_pct = None;
        strategy.exit_rules.max_hold_time_minutes = None;

        let exit_target = PositionManager::calculate_exit_target(
            dec!(0.50),
            &strategy.exit_rules,
            Utc::now(),
        );

        assert_eq!(exit_target.take_profit_price, None);
        assert_eq!(exit_target.stop_loss_price, None);
        assert_eq!(exit_target.trailing_stop_distance, None);
        assert_eq!(exit_target.expiry_time, None);
    }

    #[test]
    fn test_calculate_exit_target_different_prices() {
        let entry_price = dec!(0.30);
        let entry_time = Utc::now();
        let strategy = create_test_strategy();

        let exit_target = PositionManager::calculate_exit_target(
            entry_price,
            &strategy.exit_rules,
            entry_time,
        );

        // 50% above 0.30 = 0.45
        assert_eq!(exit_target.take_profit_price, Some(dec!(0.45)));
        // 30% below 0.30 = 0.21
        assert_eq!(exit_target.stop_loss_price, Some(dec!(0.21)));
        // 20% of 0.30 = 0.06
        assert_eq!(exit_target.trailing_stop_distance, Some(dec!(0.06)));
    }

    #[test]
    fn test_calculate_exit_target_high_price() {
        let entry_price = dec!(0.90);
        let entry_time = Utc::now();
        let mut strategy = create_test_strategy();
        strategy.exit_rules.take_profit_pct = Some(dec!(10.0));  // 10%
        strategy.exit_rules.stop_loss_pct = Some(dec!(10.0));     // 10%

        let exit_target = PositionManager::calculate_exit_target(
            entry_price,
            &strategy.exit_rules,
            entry_time,
        );

        // 10% above 0.90 = 0.99
        assert_eq!(exit_target.take_profit_price, Some(dec!(0.99)));
        // 10% below 0.90 = 0.81
        assert_eq!(exit_target.stop_loss_price, Some(dec!(0.81)));
    }

    // Note: Integration tests for open_position, update_prices, and close_position
    // require async setup with KalshiClient and OrderExecutor mocks.
    // These will be added in Phase 4 integration demo.
    // The calculation logic above is thoroughly tested.
}
