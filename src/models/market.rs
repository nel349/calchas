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

    // Classification
    pub category: MarketCategory,
    pub sub_category: Option<String>,

    // Status
    pub status: MarketStatus,

    // Pricing (using Decimal for precision)
    pub yes_price: Decimal,  // Price in cents (0-100)
    pub no_price: Decimal,   // Price in cents (0-100)

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
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_market() -> Market {
        Market {
            id: MarketId::new("TEST-001".to_string()),
            ticker: "RAIN-NY-2024".to_string(),
            title: "Will it rain in NYC on Feb 11, 2024?".to_string(),
            category: MarketCategory::Weather,
            sub_category: Some("Precipitation".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.24),
            no_price: dec!(0.76),
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
