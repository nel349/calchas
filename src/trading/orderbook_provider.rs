//! Orderbook provider abstraction
//!
//! Provides orderbook data for order execution decisions.
//! Has two implementations:
//! - Simulated: generates synthetic orderbook based on market data
//! - Real: fetches actual orderbook from Kalshi API (Phase 6)

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;

use crate::kalshi::KalshiClient;
use crate::models::{Orderbook, OrderbookLevel, MarketId};

/// Error types for orderbook operations
#[derive(Debug)]
pub enum OrderbookError {
    /// API error when fetching orderbook
    ApiFailed(String),

    /// Market not found
    MarketNotFound(MarketId),

    /// Orderbook is empty (no liquidity)
    EmptyOrderbook,
}

impl std::fmt::Display for OrderbookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderbookError::ApiFailed(msg) => write!(f, "Orderbook API failed: {}", msg),
            OrderbookError::MarketNotFound(id) => write!(f, "Market not found: {}", id.as_str()),
            OrderbookError::EmptyOrderbook => write!(f, "Orderbook is empty"),
        }
    }
}

impl std::error::Error for OrderbookError {}

/// Trait for providing orderbook data
///
/// Allows different implementations for simulation vs live trading
#[async_trait]
pub trait OrderbookProvider: Send + Sync {
    /// Get orderbook for a market
    ///
    /// # Arguments
    ///
    /// * `market_id` - Market to fetch orderbook for
    ///
    /// # Returns
    ///
    /// Orderbook with bid/ask levels, or None if not available
    async fn get_orderbook(&self, market_id: &MarketId) -> Result<Option<Orderbook>, OrderbookError>;
}

// =============================================================================
// SIMULATED ORDERBOOK PROVIDER (Phase 4)
// =============================================================================

/// Simulated orderbook provider for testing
///
/// Generates synthetic orderbook data based on default assumptions.
/// Used in Phase 4 simulation mode.
///
/// Note: This generates orderbooks with reasonable defaults but doesn't
/// fetch real market data. For Phase 6 live trading, use RealOrderbookProvider.
pub struct SimulatedOrderbookProvider {
    #[allow(dead_code)]  // Reserved for Phase 6 real API implementation
    kalshi_client: Arc<KalshiClient>,
}

impl SimulatedOrderbookProvider {
    pub fn new(kalshi_client: Arc<KalshiClient>) -> Self {
        Self { kalshi_client }
    }

    /// Generate synthetic orderbook with default assumptions
    ///
    /// For simulation, we assume:
    /// - Moderate spread (2% = 200 bps for YES side, implied for NO)
    /// - Decent liquidity (75 contracts at best price)
    /// - Market price estimated from base_price (YES side)
    ///
    /// This is intentionally simple for Phase 4. Phase 6 will use real orderbook API.
    fn generate_default_orderbook(&self, market_id: &MarketId, yes_base_price: Decimal) -> Orderbook {
        // Assume 2% spread (reasonable for markets we're targeting)
        let spread = yes_base_price * Decimal::from(200) / Decimal::from(10000);

        // YES side: ask at base price + half spread
        let yes_ask = yes_base_price + (spread / Decimal::from(2));

        // NO side: calculate from YES (should sum to ~1.00)
        let no_base_price = Decimal::ONE - yes_base_price;
        let no_ask = no_base_price + (spread / Decimal::from(2));

        // Decent liquidity (enough to pass most min_quantity filters)
        let liquidity = 75;

        Orderbook {
            market_id: market_id.clone(),
            yes_asks: vec![
                OrderbookLevel {
                    price: yes_ask,
                    quantity: liquidity,
                },
                OrderbookLevel {
                    price: yes_ask + Decimal::from_str("0.01").unwrap(),
                    quantity: liquidity / 2,
                },
            ],
            no_asks: vec![
                OrderbookLevel {
                    price: no_ask,
                    quantity: liquidity,
                },
                OrderbookLevel {
                    price: no_ask + Decimal::from_str("0.01").unwrap(),
                    quantity: liquidity / 2,
                },
            ],
        }
    }
}

#[async_trait]
impl OrderbookProvider for SimulatedOrderbookProvider {
    async fn get_orderbook(&self, market_id: &MarketId) -> Result<Option<Orderbook>, OrderbookError> {
        // For Phase 4 simulation, generate synthetic orderbook with reasonable defaults
        // Assume mid-range price (0.50) since we don't have context here
        // This is fine for testing spread/liquidity filters in simulation mode
        //
        // TODO Phase 6: Replace with real orderbook API call:
        //   GET /markets/{ticker}/orderbook
        //
        // Note: The current approach always passes orderbook checks in simulation
        // because we generate "good" orderbooks. This is intentional - we want to
        // test the trading logic, not orderbook quality in Phase 4.

        let orderbook = self.generate_default_orderbook(market_id, Decimal::from_str("0.50").unwrap());
        Ok(Some(orderbook))
    }
}

// =============================================================================
// REAL ORDERBOOK PROVIDER (Phase 4+)
// =============================================================================

/// Real orderbook provider using Kalshi API
///
/// Fetches live orderbook data from Kalshi's orderbook endpoint.
/// Used for realistic simulation and live trading.
pub struct RealOrderbookProvider {
    kalshi_client: Arc<KalshiClient>,
}

impl RealOrderbookProvider {
    /// Create a new real orderbook provider
    ///
    /// # Arguments
    ///
    /// * `kalshi_client` - Shared Kalshi client for API calls
    pub fn new(kalshi_client: Arc<KalshiClient>) -> Self {
        Self { kalshi_client }
    }
}

#[async_trait]
impl OrderbookProvider for RealOrderbookProvider {
    async fn get_orderbook(&self, market_id: &MarketId) -> Result<Option<Orderbook>, OrderbookError> {
        // Extract ticker from MarketId (they're the same in Kalshi)
        let ticker = market_id.as_str();

        tracing::debug!("Fetching real orderbook for {}", ticker);

        // Fetch orderbook from API (depth=None means all levels)
        let response = self
            .kalshi_client
            .get_orderbook(ticker, None)
            .await
            .map_err(|e| OrderbookError::ApiFailed(e.to_string()))?;

        // Check if orderbook data exists
        if let Some(ref data) = response.orderbook {
            tracing::debug!("Received orderbook: YES={} levels, NO={} levels",
                data.yes.len(),
                data.no.len()
            );
        } else {
            tracing::debug!("Received null orderbook (no liquidity)");
        }

        // Convert to domain model
        let mut orderbook: Orderbook = response
            .try_into()
            .map_err(|e: String| OrderbookError::ApiFailed(e))?;

        // Fix the market_id (conversion sets it to PLACEHOLDER)
        orderbook.market_id = market_id.clone();

        // Check if orderbook is empty
        if orderbook.yes_asks.is_empty() && orderbook.no_asks.is_empty() {
            tracing::warn!("Empty orderbook received for {}", ticker);
            return Err(OrderbookError::EmptyOrderbook);
        }

        Ok(Some(orderbook))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // Test orderbook spread calculation
    #[test]
    fn test_orderbook_spread() {
        use crate::models::MarketId;

        let orderbook = Orderbook {
            market_id: MarketId::new("TEST".to_string()),
            yes_asks: vec![
                OrderbookLevel { price: dec!(0.55), quantity: 100 },
            ],
            no_asks: vec![
                OrderbookLevel { price: dec!(0.48), quantity: 100 },
            ],
        };

        let spread = orderbook.spread().unwrap();
        // YES ask = 0.55
        // NO ask = 0.48
        // Implied YES from NO = 1.00 - 0.48 = 0.52
        // Spread = 0.55 - 0.52 = 0.03
        assert_eq!(spread, dec!(0.03));
    }

    #[test]
    fn test_orderbook_best_prices() {
        use crate::models::MarketId;

        let orderbook = Orderbook {
            market_id: MarketId::new("TEST".to_string()),
            yes_asks: vec![
                OrderbookLevel { price: dec!(0.25), quantity: 50 },
                OrderbookLevel { price: dec!(0.26), quantity: 100 },  // ← LAST = best ask
            ],
            no_asks: vec![
                OrderbookLevel { price: dec!(0.75), quantity: 75 },
                OrderbookLevel { price: dec!(0.76), quantity: 150 },  // ← LAST = best ask
            ],
        };

        // Note: Kalshi orderbook is ascending, so LAST element is the best ask (current market price)
        assert_eq!(orderbook.yes_best_ask().unwrap(), dec!(0.26));
        assert_eq!(orderbook.yes_best_ask_quantity(), 100);
        assert_eq!(orderbook.no_best_ask().unwrap(), dec!(0.76));
        assert_eq!(orderbook.no_best_ask_quantity(), 150);
    }

    #[test]
    fn test_simulated_orderbook_generation() {
        // Test the orderbook generation logic directly without needing a client
        let market_id = MarketId::new("TEST-MARKET".to_string());

        // Manually create an orderbook with the same logic as generate_default_orderbook
        let yes_base_price = dec!(0.40);
        let spread = yes_base_price * Decimal::from(200) / Decimal::from(10000);
        let yes_ask = yes_base_price + (spread / Decimal::from(2));
        let no_base_price = Decimal::ONE - yes_base_price;
        let no_ask = no_base_price + (spread / Decimal::from(2));
        let liquidity = 75;

        let orderbook = Orderbook {
            market_id: market_id.clone(),
            yes_asks: vec![
                OrderbookLevel {
                    price: yes_ask,
                    quantity: liquidity,
                },
                OrderbookLevel {
                    price: yes_ask + Decimal::from_str("0.01").unwrap(),
                    quantity: liquidity / 2,
                },
            ],
            no_asks: vec![
                OrderbookLevel {
                    price: no_ask,
                    quantity: liquidity,
                },
                OrderbookLevel {
                    price: no_ask + Decimal::from_str("0.01").unwrap(),
                    quantity: liquidity / 2,
                },
            ],
        };

        // Verify structure
        assert_eq!(orderbook.market_id.as_str(), "TEST-MARKET");
        assert!(!orderbook.yes_asks.is_empty());
        assert!(!orderbook.no_asks.is_empty());

        // Verify reasonable spread (2% on 0.40 = 0.008)
        let yes_ask_price = orderbook.yes_best_ask().unwrap();
        assert!(yes_ask_price > dec!(0.40)); // Should be above base price
        assert!(yes_ask_price < dec!(0.45)); // But not too much

        // Verify decent liquidity (last level has liquidity/2 = 37)
        assert!(orderbook.yes_best_ask_quantity() >= 30);

        // Verify YES + NO approximately equals 1.00 (within spread)
        // Note: Using .last() means we get the higher prices (+0.01 each), so sum is ~1.028
        let no_ask_price = orderbook.no_best_ask().unwrap();
        let sum = yes_ask_price + no_ask_price;
        assert!(sum > dec!(1.00));  // Should be above 1.00 (both sides at higher price level)
        assert!(sum < dec!(1.04));  // But not too much (spread + both +0.01)
    }

    #[test]
    fn test_orderbook_empty_lists() {
        use crate::models::MarketId;

        let orderbook = Orderbook {
            market_id: MarketId::new("EMPTY".to_string()),
            yes_asks: vec![],
            no_asks: vec![],
        };

        assert!(orderbook.yes_best_ask().is_none());
        assert_eq!(orderbook.yes_best_ask_quantity(), 0);
        assert!(orderbook.no_best_ask().is_none());
        assert_eq!(orderbook.no_best_ask_quantity(), 0);
        assert!(orderbook.spread().is_none());
    }
}
