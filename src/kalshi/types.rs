// Kalshi API data types
// Based on https://docs.kalshi.com/api-reference/market/get-markets

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// =============================================================================
// REQUEST TYPES
// =============================================================================

/// Request parameters for GET /markets endpoint
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GetMarketsRequest {
    /// Maximum number of markets to return (1-1000)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,

    /// Cursor for pagination (from previous response)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,

    /// Filter by market status ("open", "closed", "settled", etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// Filter by series ticker
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_ticker: Option<String>,
}

// =============================================================================
// RESPONSE TYPES
// =============================================================================

/// Response from GET /markets endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketsResponse {
    /// Cursor for fetching next page (None if no more pages)
    pub cursor: Option<String>,

    /// List of markets
    pub markets: Vec<KalshiMarket>,
}

/// Kalshi market data (raw API format)
/// Maps to Kalshi's market response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalshiMarket {
    /// Unique market ticker (e.g., "INXD-24FEB11-T5000")
    pub ticker: String,

    /// Event ticker this market belongs to
    pub event_ticker: String,

    /// Market type ("binary", etc.)
    pub market_type: String,

    /// Full market title
    pub title: String,

    /// Market subtitle
    pub subtitle: String,

    /// "Yes" outcome subtitle
    pub yes_sub_title: String,

    /// "No" outcome subtitle
    pub no_sub_title: String,

    /// When market was created
    pub created_time: DateTime<Utc>,

    /// When market opens for trading
    pub open_time: DateTime<Utc>,

    /// When market closes for trading
    pub close_time: DateTime<Utc>,

    /// When event expires/is resolved
    pub expiration_time: DateTime<Utc>,

    /// Market status
    pub status: String,

    /// Price units used in response ("usd_cent")
    pub response_price_units: String,

    /// Current best bid for Yes (cents)
    #[serde(default)]
    pub yes_bid: i64,

    /// Current best ask for Yes (cents)
    #[serde(default)]
    pub yes_ask: i64,

    /// Current best bid for No (cents)
    #[serde(default)]
    pub no_bid: i64,

    /// Current best ask for No (cents)
    #[serde(default)]
    pub no_ask: i64,

    /// Last traded price (cents)
    #[serde(default)]
    pub last_price: i64,

    /// Total volume traded (can be negative as sentinel value)
    #[serde(default)]
    pub volume: i64,

    /// 24-hour volume (can be negative as sentinel value)
    #[serde(default)]
    pub volume_24h: i64,

    /// Open interest (outstanding contracts, can be negative as sentinel value)
    #[serde(default)]
    pub open_interest: i64,

    /// Liquidity measure (can be negative as sentinel value)
    #[serde(default)]
    pub liquidity: i64,

    /// Market result if settled ("yes", "no", or None)
    #[serde(default)]
    pub result: Option<String>,

    /// Whether market can close early
    #[serde(default)]
    pub can_close_early: bool,

    /// Market category
    pub category: String,

    /// Notional value (cents)
    #[serde(default)]
    pub notional_value: i64,
}

// =============================================================================
// CONVERSION TO GENERIC MARKET MODEL
// =============================================================================

impl From<KalshiMarket> for crate::models::Market {
    fn from(km: KalshiMarket) -> Self {
        // Convert Kalshi status to MarketStatus enum
        // Real API values: "active", "determined", "finalized"
        let status = match km.status.as_str() {
            "active" => crate::models::MarketStatus::Active,
            "determined" => crate::models::MarketStatus::Determined,
            "finalized" => crate::models::MarketStatus::Finalized,
            // Unknown statuses default to Determined (conservative - not tradeable)
            _ => crate::models::MarketStatus::Determined,
        };

        // Convert Kalshi category to MarketCategory enum
        let category = match km.category.as_str() {
            "Sports" => crate::models::MarketCategory::Sports,
            "Politics" => crate::models::MarketCategory::Politics,
            "Economics" => crate::models::MarketCategory::Economics,
            "Weather" => crate::models::MarketCategory::Weather,
            "Entertainment" => crate::models::MarketCategory::Entertainment,
            other => crate::models::MarketCategory::Other(other.to_string()),
        };

        // Convert prices from cents (i64) to Decimal
        // Kalshi prices are in cents, so divide by 100 for dollar amount
        let yes_bid = Decimal::new(km.yes_bid, 2);  // 2 decimal places
        let yes_ask = Decimal::new(km.yes_ask, 2);
        let no_bid = Decimal::new(km.no_bid, 2);
        let no_ask = Decimal::new(km.no_ask, 2);

        // Use average of bid/ask as "price" for generic model
        let yes_price = (yes_bid + yes_ask) / Decimal::from(2);
        let no_price = (no_bid + no_ask) / Decimal::from(2);

        crate::models::Market {
            id: crate::models::MarketId::new(km.ticker.clone()),
            ticker: km.ticker,
            title: km.title,
            category,
            sub_category: Some(km.subtitle),
            status,
            yes_price,
            no_price,
            // Convert negative sentinel values to 0
            volume: km.volume.max(0) as u64,
            open_interest: km.open_interest.max(0) as u64,
            event_time: km.expiration_time,
            close_time: km.close_time,
            created_at: km.created_time,
            updated_at: Utc::now(),  // Kalshi doesn't provide this, use current time
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_kalshi_market() -> KalshiMarket {
        KalshiMarket {
            ticker: "TEST-MARKET-001".to_string(),
            event_ticker: "TEST-EVENT-001".to_string(),
            market_type: "binary".to_string(),
            title: "Will it rain tomorrow?".to_string(),
            subtitle: "Weather prediction".to_string(),
            yes_sub_title: "Yes, it will rain".to_string(),
            no_sub_title: "No, it will not rain".to_string(),
            created_time: Utc::now(),
            open_time: Utc::now(),
            close_time: Utc::now(),
            expiration_time: Utc::now(),
            status: "active".to_string(),  // Real API value
            response_price_units: "usd_cent".to_string(),
            yes_bid: 45,   // 45 cents = $0.45
            yes_ask: 47,   // 47 cents = $0.47
            no_bid: 53,    // 53 cents = $0.53
            no_ask: 55,    // 55 cents = $0.55
            last_price: 46,
            volume: 10000,
            volume_24h: 5000,
            open_interest: 3000,
            liquidity: 2000,
            result: None,
            can_close_early: false,
            category: "Weather".to_string(),
            notional_value: 100000,
        }
    }

    #[test]
    fn test_kalshi_market_creation() {
        let market = create_test_kalshi_market();
        assert_eq!(market.ticker, "TEST-MARKET-001");
        assert_eq!(market.status, "active");  // Real API value
        assert_eq!(market.yes_bid, 45);
        assert_eq!(market.volume, 10000);
    }

    #[test]
    fn test_kalshi_market_deserialization() {
        let json = r#"{
            "ticker": "INXD-24FEB11-T5000",
            "event_ticker": "INXD-24FEB11",
            "market_type": "binary",
            "title": "Will S&P 500 close above 5000?",
            "subtitle": "S&P 500 Index",
            "yes_sub_title": "Above 5000",
            "no_sub_title": "Below 5000",
            "created_time": "2024-01-01T00:00:00Z",
            "open_time": "2024-01-01T09:00:00Z",
            "close_time": "2024-02-11T21:00:00Z",
            "expiration_time": "2024-02-11T21:00:00Z",
            "status": "open",
            "response_price_units": "usd_cent",
            "yes_bid": 67,
            "yes_ask": 68,
            "no_bid": 32,
            "no_ask": 33,
            "last_price": 67,
            "volume": 12450,
            "volume_24h": 2300,
            "open_interest": 5600,
            "liquidity": 1200,
            "result": null,
            "can_close_early": false,
            "category": "Economics",
            "notional_value": 124500
        }"#;

        let market: KalshiMarket = serde_json::from_str(json).unwrap();
        assert_eq!(market.ticker, "INXD-24FEB11-T5000");
        assert_eq!(market.yes_bid, 67);
        assert_eq!(market.category, "Economics");
    }

    #[test]
    fn test_conversion_to_generic_market() {
        let kalshi_market = create_test_kalshi_market();
        let generic_market: crate::models::Market = kalshi_market.into();

        assert_eq!(generic_market.ticker, "TEST-MARKET-001");
        assert_eq!(generic_market.title, "Will it rain tomorrow?");

        // Check price conversion (cents to decimal)
        // yes_bid=45, yes_ask=47 -> avg = 46 cents = $0.46
        let expected_yes = Decimal::new(46, 2);  // 0.46
        assert_eq!(generic_market.yes_price, expected_yes);

        // Check category mapping
        assert_eq!(generic_market.category, crate::models::MarketCategory::Weather);

        // Check status mapping
        assert_eq!(generic_market.status, crate::models::MarketStatus::Active);
    }

    #[test]
    fn test_category_conversion_sports() {
        let mut market = create_test_kalshi_market();
        market.category = "Sports".to_string();

        let generic: crate::models::Market = market.into();
        assert_eq!(generic.category, crate::models::MarketCategory::Sports);
    }

    #[test]
    fn test_category_conversion_unknown() {
        let mut market = create_test_kalshi_market();
        market.category = "Crypto".to_string();

        let generic: crate::models::Market = market.into();
        assert_eq!(generic.category, crate::models::MarketCategory::Other("Crypto".to_string()));
    }

    #[test]
    fn test_status_conversion() {
        // Test REAL API values found in production
        let test_cases = vec![
            ("active", crate::models::MarketStatus::Active),
            ("determined", crate::models::MarketStatus::Determined),
            ("finalized", crate::models::MarketStatus::Finalized),
            ("unknown", crate::models::MarketStatus::Determined),  // Default (conservative)
        ];

        for (kalshi_status, expected_status) in test_cases {
            let mut market = create_test_kalshi_market();
            market.status = kalshi_status.to_string();

            let generic: crate::models::Market = market.into();
            assert_eq!(generic.status, expected_status);
        }
    }

    #[test]
    fn test_get_markets_request_default() {
        let request = GetMarketsRequest::default();
        assert!(request.limit.is_none());
        assert!(request.cursor.is_none());
        assert!(request.status.is_none());
    }

    #[test]
    fn test_get_markets_request_serialization() {
        let request = GetMarketsRequest {
            limit: Some(50),
            cursor: None,
            status: Some("open".to_string()),
            series_ticker: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"limit\":50"));
        assert!(json.contains("\"status\":\"open\""));
        // None fields should be skipped
        assert!(!json.contains("cursor"));
    }

    #[test]
    fn test_markets_response_deserialization() {
        let json = r#"{
            "cursor": "next-page-token",
            "markets": []
        }"#;

        let response: MarketsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.cursor, Some("next-page-token".to_string()));
        assert_eq!(response.markets.len(), 0);
    }

    #[test]
    fn test_markets_response_no_cursor() {
        let json = r#"{
            "cursor": null,
            "markets": []
        }"#;

        let response: MarketsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.cursor, None);
    }
}
