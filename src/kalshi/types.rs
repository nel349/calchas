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

    /// Minimum close timestamp (Unix timestamp in seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_close_ts: Option<i64>,

    /// Maximum close timestamp (Unix timestamp in seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_close_ts: Option<i64>,
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

    /// When event expires/is resolved (fallback/max time)
    pub expiration_time: DateTime<Utc>,

    /// Expected expiration time (actual scheduled event time, e.g., game start)
    /// This is the real event time, while expiration_time is a fallback max
    #[serde(default)]
    pub expected_expiration_time: Option<DateTime<Utc>>,

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
        // Real API values: "initialized", "active", "finalized"
        // Note: "determined" exists in docs but rarely seen in practice
        let status = match km.status.as_str() {
            "initialized" => crate::models::MarketStatus::Initialized,
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
        // Note: API uses negative values (-1) as sentinel for "no quote available"
        let yes_bid = Decimal::new(km.yes_bid.max(0), 2);  // 2 decimal places, negatives → 0
        let yes_ask = Decimal::new(km.yes_ask.max(0), 2);
        let no_bid = Decimal::new(km.no_bid.max(0), 2);
        let no_ask = Decimal::new(km.no_ask.max(0), 2);

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
            yes_bid,
            yes_ask,
            no_bid,
            no_ask,
            // Convert negative sentinel values to 0
            volume: km.volume.max(0) as u64,
            open_interest: km.open_interest.max(0) as u64,
            // Use expected_expiration_time if available (actual game time), otherwise fallback to expiration_time
            event_time: km.expected_expiration_time.unwrap_or(km.expiration_time),
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
            expected_expiration_time: Some(Utc::now()),
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
    fn test_negative_prices_converted_to_zero() {
        // Test that negative sentinel values (-1) are converted to 0
        // API uses -1 to indicate "no quote available"
        let mut market = create_test_kalshi_market();
        market.yes_bid = -1;   // No quote
        market.yes_ask = -1;   // No quote
        market.no_bid = 50;    // Has quote: 50 cents
        market.no_ask = 52;    // Has quote: 52 cents

        let converted: crate::models::Market = market.into();

        // Negative values should be converted to 0
        assert_eq!(converted.yes_bid, Decimal::ZERO);
        assert_eq!(converted.yes_ask, Decimal::ZERO);

        // Positive values should work normally
        assert_eq!(converted.no_bid, Decimal::new(50, 2));  // $0.50
        assert_eq!(converted.no_ask, Decimal::new(52, 2));  // $0.52

        // yes_price should be 0 (avg of 0 and 0)
        assert_eq!(converted.yes_price, Decimal::ZERO);

        // no_price should be 0.51 (avg of 0.50 and 0.52)
        assert_eq!(converted.no_price, Decimal::new(51, 2));
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
            ("initialized", crate::models::MarketStatus::Initialized),  // Games not started yet
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
    fn test_market_status_transitions() {
        // Test the full lifecycle: initialized → active → determined → finalized
        let base_market = create_test_kalshi_market();

        // 1. INITIALIZED: Market created, game scheduled but not started
        let mut market = base_market.clone();
        market.status = "initialized".to_string();
        let converted: crate::models::Market = market.into();
        assert_eq!(converted.status, crate::models::MarketStatus::Initialized);
        assert!(!converted.is_open());  // Can't trade yet

        // 2. ACTIVE: Game started, trading live
        let mut market = base_market.clone();
        market.status = "active".to_string();
        let converted: crate::models::Market = market.into();
        assert_eq!(converted.status, crate::models::MarketStatus::Active);
        assert!(converted.is_open());  // Can trade

        // 3. DETERMINED: Game finished, outcome known but not settled
        let mut market = base_market.clone();
        market.status = "determined".to_string();
        let converted: crate::models::Market = market.into();
        assert_eq!(converted.status, crate::models::MarketStatus::Determined);
        assert!(!converted.is_open());  // Can't trade anymore

        // 4. FINALIZED: All payouts complete, market fully settled
        let mut market = base_market.clone();
        market.status = "finalized".to_string();
        market.result = Some("yes".to_string());  // Settlement result
        let converted: crate::models::Market = market.into();
        assert_eq!(converted.status, crate::models::MarketStatus::Finalized);
        assert!(!converted.is_open());  // Can't trade
    }

    #[test]
    fn test_finalized_market_with_result() {
        // Test that finalized markets can have settlement results
        let mut market = create_test_kalshi_market();
        market.status = "finalized".to_string();
        market.result = Some("yes".to_string());
        // When YES wins: both bid and ask settle at 100 cents ($1.00)
        market.yes_bid = 100;
        market.yes_ask = 100;
        market.no_bid = 0;
        market.no_ask = 0;

        let converted: crate::models::Market = market.into();
        assert_eq!(converted.status, crate::models::MarketStatus::Finalized);
        // Conversion: Decimal::new(100, 2) = 1.00, avg(1.00, 1.00) = 1.00
        assert_eq!(converted.yes_price, rust_decimal::Decimal::ONE);
        assert_eq!(converted.no_price, rust_decimal::Decimal::ZERO);
    }

    #[test]
    fn test_initialized_market_not_tradeable() {
        // Markets in initialized state should not be tradeable
        // (Game scheduled but hasn't started yet)
        let mut market = create_test_kalshi_market();
        market.status = "initialized".to_string();

        let converted: crate::models::Market = market.into();

        // Verify it's marked as initialized
        assert_eq!(converted.status, crate::models::MarketStatus::Initialized);

        // Verify it's NOT considered open for trading
        assert!(!converted.is_open());
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
            min_close_ts: None,
            max_close_ts: None,
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

    #[test]
    fn test_orderbook_response_deserialization() {
        let json = r#"{
            "orderbook": {
                "yes": [[45, 100], [46, 75], [47, 50]],
                "no": [[55, 100], [54, 75], [53, 50]],
                "yes_dollars": [["0.45", 100], ["0.46", 75], ["0.47", 50]],
                "no_dollars": [["0.55", 100], ["0.54", 75], ["0.53", 50]]
            }
        }"#;

        let response: OrderbookResponse = serde_json::from_str(json).unwrap();
        let orderbook = response.orderbook.unwrap();
        assert_eq!(orderbook.yes.len(), 3);
        assert_eq!(orderbook.yes[0], (45, 100));
        assert_eq!(orderbook.no[0], (55, 100));
    }

    #[test]
    fn test_orderbook_conversion_to_domain_model() {
        let response = OrderbookResponse {
            orderbook: Some(OrderbookData {
                yes: vec![(45, 100), (46, 75)],
                no: vec![(55, 100), (54, 75)],
                yes_dollars: vec![],
                no_dollars: vec![],
            }),
        };

        let orderbook: crate::models::Orderbook = response.try_into().unwrap();
        assert_eq!(orderbook.yes_asks.len(), 2);
        assert_eq!(orderbook.yes_asks[0].price, Decimal::new(45, 2)); // 0.45
        assert_eq!(orderbook.yes_asks[0].quantity, 100);
        assert_eq!(orderbook.no_asks[0].price, Decimal::new(55, 2)); // 0.55
        assert_eq!(orderbook.no_asks[0].quantity, 100);
    }
}

// =============================================================================
// ORDERBOOK TYPES
// =============================================================================

/// Response from GET /markets/{ticker}/orderbook endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookResponse {
    /// Orderbook data (can be null for markets with no liquidity)
    pub orderbook: Option<OrderbookData>,
}

/// Orderbook data from Kalshi API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookData {
    /// YES side asks: array of [price_in_cents, contract_count]
    #[serde(default)]
    pub yes: Vec<(i64, u64)>,

    /// NO side asks: array of [price_in_cents, contract_count]
    #[serde(default)]
    pub no: Vec<(i64, u64)>,

    /// YES side asks in dollar strings (we ignore this)
    #[serde(default, skip_serializing)]
    pub yes_dollars: Vec<(String, u64)>,

    /// NO side asks in dollar strings (we ignore this)
    #[serde(default, skip_serializing)]
    pub no_dollars: Vec<(String, u64)>,
}

/// Convert Kalshi orderbook response to domain model
impl TryFrom<OrderbookResponse> for crate::models::Orderbook {
    type Error = String;

    fn try_from(response: OrderbookResponse) -> Result<Self, Self::Error> {
        // Check if orderbook data exists (API returns null for markets with no liquidity)
        let data = response
            .orderbook
            .ok_or_else(|| "Orderbook is null (market has no liquidity)".to_string())?;

        // Extract market ID from context (need to pass this separately)
        // For now, create placeholder - will be set by caller
        let market_id = crate::models::MarketId::new("PLACEHOLDER".to_string());

        // Convert YES asks
        let yes_asks: Vec<crate::models::OrderbookLevel> = data
            .yes
            .iter()
            .map(|(price_cents, quantity)| crate::models::OrderbookLevel {
                price: Decimal::new(*price_cents, 2), // Convert cents to decimal
                quantity: *quantity,
            })
            .collect();

        // Convert NO asks
        let no_asks: Vec<crate::models::OrderbookLevel> = data
            .no
            .iter()
            .map(|(price_cents, quantity)| crate::models::OrderbookLevel {
                price: Decimal::new(*price_cents, 2),
                quantity: *quantity,
            })
            .collect();

        Ok(crate::models::Orderbook {
            market_id,
            yes_asks,
            no_asks,
        })
    }
}

// =============================================================================
// SEARCH / DISCOVERY TYPES
// =============================================================================

/// Response from GET /search/tags_by_categories endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsByCategoriesResponse {
    /// Mapping of series categories to their associated tags (can be null)
    pub tags_by_categories: std::collections::HashMap<String, Option<Vec<String>>>,
}

/// Response from GET /search/filters_by_sport endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiltersBySportResponse {
    /// Mapping of sports to their filter details
    pub filters_by_sports: std::collections::HashMap<String, SportFilters>,
    /// Ordered list of sports for display (NOTE: This field doesn't exist in actual API response)
    #[serde(default)]
    pub sport_ordering: Vec<String>,
}

/// Filter details for a specific sport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SportFilters {
    /// Available scopes for this sport
    pub scopes: Vec<String>,
    /// Mapping of competitions to their details
    pub competitions: std::collections::HashMap<String, CompetitionDetails>,
}

/// Details for a specific competition within a sport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionDetails {
    /// Available scopes for this competition
    pub scopes: Vec<String>,
}
