// Market data model
// Section 4.1 of TECHNICAL_ARCHITECTURE.md

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// =============================================================================
// NEWTYPE PATTERN - MarketId
// =============================================================================

/// Unique market identifier (newtype pattern prevents mixing with other IDs)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarketId(pub String);

impl MarketId {
    pub fn new(id: String) -> Self {
        MarketId(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// =============================================================================
// ENUMS
// =============================================================================

/// Market category classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketCategory {
    Sports,
    Politics,
    Economics,
    Weather,
    Entertainment,
    Other(String),  // For unknown categories from API
}

/// Market status lifecycle
/// These values match Kalshi API status field exactly
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketStatus {
    Initialized, // Market created, not trading yet (API: "initialized")
    Active,      // Actively trading (API: "active")
    Determined,  // Trading ended, outcome determined (API: "determined")
    Finalized,   // All payouts complete (API: "finalized")
}

// =============================================================================
// MAIN STRUCT
// =============================================================================

/// Represents a prediction market from Kalshi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    // Identification
    pub id: MarketId,
    pub ticker: String,
    pub title: String,
    pub event_ticker: String,  // Groups related markets (e.g., all markets for same game)

    // Classification
    pub category: MarketCategory,
    pub sub_category: Option<String>,

    // Status
    pub status: MarketStatus,

    // Pricing (using Decimal for precision)
    pub yes_price: Decimal,  // Midpoint price (average of bid/ask)
    pub no_price: Decimal,   // Midpoint price (average of bid/ask)

    // Bid/Ask spreads (for arbitrage and order execution)
    pub yes_bid: Decimal,    // Best bid (what buyers offer)
    pub yes_ask: Decimal,    // Best ask (what sellers want)
    pub no_bid: Decimal,     // Best bid for NO
    pub no_ask: Decimal,     // Best ask for NO

    // Liquidity
    pub volume: u64,         // Total contracts traded
    pub open_interest: u64,  // Outstanding contracts

    // Timing
    pub event_time: DateTime<Utc>,      // When event occurs
    pub close_time: DateTime<Utc>,      // When trading ends
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Market {
    /// Check if market is currently tradeable
    pub fn is_open(&self) -> bool {
        matches!(self.status, MarketStatus::Active)
    }

    /// Check if market has sufficient liquidity
    pub fn is_liquid(&self, min_volume: u64) -> bool {
        self.volume >= min_volume
    }

    /// Get the cheaper side price (for underdog hunting)
    pub fn cheaper_side_price(&self) -> Decimal {
        self.yes_price.min(self.no_price)
    }

    /// Get the expensive side price (for favorite hunting)
    pub fn expensive_side_price(&self) -> Decimal {
        self.yes_price.max(self.no_price)
    }

    /// Check if market is in a specific category
    pub fn is_category(&self, category: &MarketCategory) -> bool {
        self.category == *category
    }

    /// Determine if this is a crypto market based on event ticker pattern.
    ///
    /// Crypto markets (KXBTC, KXETH) have accurate close_time (to the minute).
    /// Sports/Politics markets have close_time as placeholder (~14 days out),
    /// and use event_time for actual settlement.
    ///
    /// This is used by strategy evaluation to determine which timestamp to use
    /// for time-based filtering.
    pub fn is_crypto_market(&self) -> bool {
        self.event_ticker.starts_with("KXBTC") || self.event_ticker.starts_with("KXETH")
    }
}

// =============================================================================
// ORDERBOOK STRUCTS
// =============================================================================

/// Single price level in the orderbook
#[derive(Debug, Clone)]
pub struct OrderbookLevel {
    pub price: Decimal,    // Price in cents (0-100)
    pub quantity: u64,     // Number of contracts available
}

/// Market orderbook showing liquidity at different price levels
#[derive(Debug, Clone)]
pub struct Orderbook {
    pub market_id: MarketId,

    /// YES side orders (sorted by price ascending - best ask LAST, stale orders first)
    pub yes_asks: Vec<OrderbookLevel>,

    /// NO side orders (sorted by price ascending - best ask LAST, stale orders first)
    pub no_asks: Vec<OrderbookLevel>,
}

impl Orderbook {
    /// Get best ask price for YES side
    ///
    /// Note: Kalshi orderbook is sorted ascending (1¢ → market price)
    /// The LAST element is the current market price, first elements are stale orders
    pub fn yes_best_ask(&self) -> Option<Decimal> {
        self.yes_asks.last().map(|level| level.price)
    }

    /// Get best ask price for NO side
    ///
    /// Note: Kalshi orderbook is sorted ascending (1¢ → market price)
    /// The LAST element is the current market price, first elements are stale orders
    pub fn no_best_ask(&self) -> Option<Decimal> {
        self.no_asks.last().map(|level| level.price)
    }

    /// Get quantity available at best YES ask
    pub fn yes_best_ask_quantity(&self) -> u64 {
        self.yes_asks.last().map(|level| level.quantity).unwrap_or(0)
    }

    /// Get quantity available at best NO ask
    pub fn no_best_ask_quantity(&self) -> u64 {
        self.no_asks.last().map(|level| level.quantity).unwrap_or(0)
    }

    /// Calculate spread (difference between YES and NO best asks)
    /// Note: YES + NO prices should sum to ~1.00 in efficient markets
    pub fn spread(&self) -> Option<Decimal> {
        let yes_ask = self.yes_best_ask()?;
        let no_ask = self.no_best_ask()?;

        // Spread = how much more you pay than the "fair" price
        // Fair price: YES = 1 - NO
        // Example: YES ask = 0.55, NO ask = 0.48
        // Implied YES from NO = 1 - 0.48 = 0.52
        // Spread = 0.55 - 0.52 = 0.03 (3 cents of slippage)
        let implied_yes = Decimal::ONE - no_ask;
        Some((yes_ask - implied_yes).abs())
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
    fn test_orderbook_ascending_sort_with_stale_orders() {
        // Verify that Kalshi orderbook is sorted ASCENDING (1¢ → market price)
        // and that we correctly get the LAST element (actual market price)
        // instead of the FIRST element (stale limit orders)
        let orderbook = Orderbook {
            market_id: MarketId::new("TEST-STALE".to_string()),
            yes_asks: vec![
                OrderbookLevel { price: dec!(0.01), quantity: 10 },   // Stale order at 1¢
                OrderbookLevel { price: dec!(0.02), quantity: 5 },    // Stale order at 2¢
                OrderbookLevel { price: dec!(0.45), quantity: 50 },   // Old order at 45¢
                OrderbookLevel { price: dec!(0.49), quantity: 100 },  // Current market price (LAST)
            ],
            no_asks: vec![
                OrderbookLevel { price: dec!(0.01), quantity: 10 },   // Stale order at 1¢
                OrderbookLevel { price: dec!(0.50), quantity: 75 },   // Current market price (LAST)
            ],
        };

        // Should get the LAST element (current market price), not FIRST (stale orders)
        assert_eq!(orderbook.yes_best_ask().unwrap(), dec!(0.49));
        assert_eq!(orderbook.yes_best_ask_quantity(), 100);
        assert_eq!(orderbook.no_best_ask().unwrap(), dec!(0.50));
        assert_eq!(orderbook.no_best_ask_quantity(), 75);

        // Verify spread calculation uses LAST elements
        let spread = orderbook.spread().unwrap();
        // YES = 49¢, NO = 50¢
        // Implied YES from NO = 1 - 0.50 = 0.50
        // Spread = |0.49 - 0.50| = 0.01 (1 cent)
        assert_eq!(spread, dec!(0.01));
    }

    fn create_test_market() -> Market {
        Market {
            id: MarketId::new("TEST-001".to_string()),
            ticker: "RAIN-NY-2024".to_string(),
            title: "Will it rain in NYC on Feb 11, 2024?".to_string(),
            event_ticker: "RAIN-NY-EVENT".to_string(),
            category: MarketCategory::Weather,
            sub_category: Some("Precipitation".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.24),
            no_price: dec!(0.76),
            yes_bid: dec!(0.23),
            yes_ask: dec!(0.25),
            no_bid: dec!(0.75),
            no_ask: dec!(0.77),
            volume: 5000,
            open_interest: 2000,
            event_time: Utc::now(),
            close_time: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_market_is_open() {
        let market = create_test_market();
        assert!(market.is_open());
    }

    #[test]
    fn test_market_status_lifecycle() {
        let mut market = create_test_market();

        // Initialized markets are NOT open (game hasn't started yet)
        market.status = MarketStatus::Initialized;
        assert!(!market.is_open());

        // Active markets ARE open
        market.status = MarketStatus::Active;
        assert!(market.is_open());

        // Determined markets are NOT open (outcome known but not settled)
        market.status = MarketStatus::Determined;
        assert!(!market.is_open());

        // Finalized markets are NOT open (fully settled)
        market.status = MarketStatus::Finalized;
        assert!(!market.is_open());
    }

    #[test]
    fn test_market_is_liquid() {
        let market = create_test_market();
        assert!(market.is_liquid(1000));
        assert!(!market.is_liquid(10000));
    }

    #[test]
    fn test_cheaper_side_price() {
        let market = create_test_market();
        assert_eq!(market.cheaper_side_price(), dec!(0.24));
    }

    #[test]
    fn test_expensive_side_price() {
        let market = create_test_market();
        assert_eq!(market.expensive_side_price(), dec!(0.76));
    }

    #[test]
    fn test_is_category() {
        let market = create_test_market();
        assert!(market.is_category(&MarketCategory::Weather));
        assert!(!market.is_category(&MarketCategory::Sports));
    }
}
