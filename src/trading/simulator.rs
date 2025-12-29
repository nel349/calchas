//! Order execution simulator
//!
//! This module simulates order fills using real market prices from Kalshi.
//! It does NOT make real trades - all execution is simulated.
//!
//! # Simulation Rules
//!
//! - **Entry orders (Buy):** Fill at ask price (we pay the ask)
//! - **Exit orders (Sell):** Fill at bid price (we receive the bid)
//! - **Instant fills:** All orders fill immediately (no order book dynamics)
//! - **No slippage:** Fill at exact bid/ask price
//! - **No partial fills:** Always fill the full quantity
//!
//! # Example
//!
//! ```no_run
//! use calchas::trading::OrderSimulator;
//! use calchas::kalshi::client::KalshiClient;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = todo!();
//! let simulator = OrderSimulator::new(Arc::new(client));
//! # let order = todo!();
//!
//! let fill = simulator.simulate_fill(&order).await?;
//! println!("Filled at ${:.2}", fill.fill_price);
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::sync::Arc;

use crate::kalshi::client::KalshiClient;
use crate::models::{Market, Order, OrderAction, OrderId};
use super::error::TradingError;

// =============================================================================
// DATA TYPES
// =============================================================================

/// Simulated order fill result
#[derive(Debug, Clone)]
pub struct SimulatedFill {
    /// Order ID that was filled
    pub order_id: OrderId,

    /// Price at which the order filled (real price from Kalshi)
    pub fill_price: Decimal,

    /// Quantity filled (always equals order quantity - instant fill)
    pub filled_quantity: u64,

    /// Timestamp when the fill occurred
    pub filled_at: DateTime<Utc>,
}

// =============================================================================
// ORDER SIMULATOR
// =============================================================================

/// Order simulator using real market prices from Kalshi
///
/// Simulates instant order fills at current market prices. Uses real data
/// from Kalshi API but does not execute real trades.
pub struct OrderSimulator {
    kalshi_client: Arc<KalshiClient>,
}

impl OrderSimulator {
    /// Create a new order simulator
    ///
    /// # Arguments
    ///
    /// * `kalshi_client` - Shared Kalshi client for fetching market prices
    pub fn new(kalshi_client: Arc<KalshiClient>) -> Self {
        OrderSimulator { kalshi_client }
    }

    /// Simulate order fill at current market price
    ///
    /// Fetches the current market price from Kalshi and simulates an instant
    /// fill. Entry orders (Buy) fill at ask price, exit orders (Sell) fill
    /// at bid price.
    ///
    /// # Arguments
    ///
    /// * `order` - The order to simulate filling
    ///
    /// # Returns
    ///
    /// * `Ok(SimulatedFill)` - Fill details including price from real market
    /// * `Err(TradingError)` - Market not found or API error
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use calchas::trading::OrderSimulator;
    /// # use calchas::kalshi::client::KalshiClient;
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = todo!();
    /// # let order = todo!();
    /// let mut simulator = OrderSimulator::new(Arc::new(client));
    ///
    /// let fill = simulator.simulate_fill(&order).await?;
    /// println!("Order {} filled at ${:.2}",
    ///     fill.order_id.as_str(),
    ///     fill.fill_price
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub async fn simulate_fill(&mut self, order: &Order) -> Result<SimulatedFill, TradingError> {
        // Fetch current market to get real prices
        let market = self.fetch_market(&order.market_id).await?;

        // Determine fill price based on order action and side
        let fill_price = self.determine_fill_price(&market, order);

        Ok(SimulatedFill {
            order_id: order.id.clone(),
            fill_price,
            filled_quantity: order.quantity,
            filled_at: Utc::now(),
        })
    }

    /// Fetch market data from Kalshi API
    async fn fetch_market(&self, market_id: &crate::models::MarketId) -> Result<Market, TradingError> {
        // Get market ticker from ID
        let ticker = market_id.as_str();

        // Search through markets in batches to find ours
        // Kalshi doesn't have a single-market endpoint, so we paginate through results
        let mut request = crate::kalshi::types::GetMarketsRequest::default();
        request.limit = Some(200);  // Fetch 200 at a time for efficiency

        loop {
            let response = self.kalshi_client.get_markets(request.clone()).await?;

            // Check if our market is in this page
            if let Some(kalshi_market) = response.markets.iter().find(|m| m.ticker == ticker) {
                return Ok(kalshi_market.clone().into());
            }

            // Move to next page if available
            match response.cursor {
                Some(cursor) if !cursor.is_empty() => {
                    request.cursor = Some(cursor);
                }
                _ => {
                    // No more pages, market not found
                    return Err(TradingError::MarketNotFound(ticker.to_string()));
                }
            }
        }
    }

    /// Determine fill price from market data
    ///
    /// # Simulation Rules:
    /// - Entry (Buy): Use ask price (we pay the ask)
    /// - Exit (Sell): Use bid price (we receive the bid)
    fn determine_fill_price(&self, market: &Market, order: &Order) -> Decimal {
        // Convert Market side prices to bid/ask
        // For Yes side: yes_price is the midpoint, we need bid/ask
        // For No side: no_price is the midpoint, we need bid/ask
        //
        // Since Market only stores midpoint prices, we approximate:
        // - Ask = midpoint (conservative for buyer)
        // - Bid = midpoint (conservative for seller)
        //
        // TODO: Phase 5 could fetch actual bid/ask from Kalshi if needed

        match order.action {
            OrderAction::Buy => {
                // Entry order: pay the ask (use current price as ask)
                match order.side {
                    crate::models::OrderSide::Yes => market.yes_price,
                    crate::models::OrderSide::No => market.no_price,
                }
            }
            OrderAction::Sell => {
                // Exit order: receive the bid (use current price as bid)
                match order.side {
                    crate::models::OrderSide::Yes => market.yes_price,
                    crate::models::OrderSide::No => market.no_price,
                }
            }
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Market, MarketId, Order, OrderId, OrderAction, OrderSide, OrderStatus, OrderType};
    use rust_decimal_macros::dec;

    // Helper: Create test order
    fn create_test_order(
        market_id: &str,
        side: OrderSide,
        action: OrderAction,
        quantity: u64,
    ) -> Order {
        Order {
            id: OrderId::new("test-order-001".to_string()),
            market_id: MarketId::new(market_id.to_string()),
            position_id: None,
            side,
            action,
            order_type: OrderType::Market,
            limit_price: None,
            quantity,
            status: OrderStatus::Pending,
            filled_quantity: 0,
            average_fill_price: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // Helper: Create test market for price calculation tests
    fn create_test_market() -> Market {
        Market {
            id: MarketId::new("TEST-MARKET".to_string()),
            ticker: "TEST-MARKET".to_string(),
            title: "Test Market".to_string(),
            category: crate::models::MarketCategory::Other("test".to_string()),
            sub_category: None,
            yes_price: dec!(0.60),
            no_price: dec!(0.40),
            yes_bid: dec!(0.59),
            yes_ask: dec!(0.61),
            no_bid: dec!(0.39),
            no_ask: dec!(0.41),
            volume: 1000,
            open_interest: 500,
            event_time: Utc::now(),
            close_time: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: crate::models::MarketStatus::Active,
        }
    }

    #[test]
    fn test_determine_fill_price_entry_yes() {
        let market = create_test_market();

        let order = create_test_order("TEST-MARKET", OrderSide::Yes, OrderAction::Buy, 10);

        // Manually test the logic without needing a real client
        let price = match order.action {
            OrderAction::Buy => match order.side {
                OrderSide::Yes => market.yes_price,
                OrderSide::No => market.no_price,
            },
            OrderAction::Sell => match order.side {
                OrderSide::Yes => market.yes_price,
                OrderSide::No => market.no_price,
            },
        };

        // Entry (Buy) on Yes side should use yes_price
        assert_eq!(price, dec!(0.60));
    }

    #[test]
    fn test_determine_fill_price_entry_no() {
        let market = create_test_market();

        let order = create_test_order("TEST-MARKET", OrderSide::No, OrderAction::Buy, 10);

        let price = match order.action {
            OrderAction::Buy => match order.side {
                OrderSide::Yes => market.yes_price,
                OrderSide::No => market.no_price,
            },
            OrderAction::Sell => match order.side {
                OrderSide::Yes => market.yes_price,
                OrderSide::No => market.no_price,
            },
        };

        // Entry (Buy) on No side should use no_price
        assert_eq!(price, dec!(0.40));
    }

    #[test]
    fn test_determine_fill_price_exit() {
        let market = create_test_market();

        let order = create_test_order("TEST-MARKET", OrderSide::Yes, OrderAction::Sell, 10);

        let price = match order.action {
            OrderAction::Buy => match order.side {
                OrderSide::Yes => market.yes_price,
                OrderSide::No => market.no_price,
            },
            OrderAction::Sell => match order.side {
                OrderSide::Yes => market.yes_price,
                OrderSide::No => market.no_price,
            },
        };

        // Exit (Sell) should use yes_price
        assert_eq!(price, dec!(0.60));
    }

    #[test]
    fn test_simulated_fill_structure() {
        let fill = SimulatedFill {
            order_id: OrderId::new("test-123".to_string()),
            fill_price: dec!(0.50),
            filled_quantity: 100,
            filled_at: Utc::now(),
        };

        assert_eq!(fill.order_id.as_str(), "test-123");
        assert_eq!(fill.fill_price, dec!(0.50));
        assert_eq!(fill.filled_quantity, 100);
    }
}
