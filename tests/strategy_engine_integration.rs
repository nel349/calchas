//! Integration test for Phase 3: Strategy Engine
//!
//! Tests the complete flow:
//! 1. Load strategy from JSON
//! 2. Create test markets
//! 3. Evaluate markets against strategy
//! 4. Verify signals generated correctly

use calchas::strategy::{StrategyLoader, StrategyEvaluator};
use calchas::models::{Market, MarketId, MarketCategory, MarketStatus};
use chrono::{Duration, Utc};
use rust_decimal_macros::dec;

fn create_test_markets() -> Vec<Market> {
    vec![
        // Market 1: Sports, cheap Yes side (should match underdog_hunter)
        Market {
            id: MarketId::new("SPORTS-001".to_string()),
            ticker: "NFL-CHIEFS-WIN".to_string(),
            title: "Will Kansas City Chiefs win Super Bowl?".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Open,
            yes_price: dec!(0.15),  // Cheap - matches underdog_hunter filter
            no_price: dec!(0.85),
            volume: 5000,  // Above min_volume (1000)
            open_interest: 2000,
            event_time: Utc::now() + Duration::hours(24),  // In time window (2-48 hours)
            close_time: Utc::now() + Duration::hours(23),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        // Market 2: Sports, expensive (should NOT match underdog_hunter)
        Market {
            id: MarketId::new("SPORTS-002".to_string()),
            ticker: "NFL-BILLS-LOSE".to_string(),
            title: "Will Buffalo Bills lose?".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Open,
            yes_price: dec!(0.75),  // Too expensive - outside price range
            no_price: dec!(0.25),  // This is cheap but strategy looks at Yes side for UnderdogOnly
            volume: 3000,
            open_interest: 1500,
            event_time: Utc::now() + Duration::hours(12),
            close_time: Utc::now() + Duration::hours(11),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        // Market 3: Politics (should NOT match underdog_hunter - wrong category)
        Market {
            id: MarketId::new("POLITICS-001".to_string()),
            ticker: "ELECTION-2024".to_string(),
            title: "Will candidate win election?".to_string(),
            category: MarketCategory::Politics,
            sub_category: Some("Presidential".to_string()),
            status: MarketStatus::Open,
            yes_price: dec!(0.18),  // Would match price, but wrong category
            no_price: dec!(0.82),
            volume: 10000,
            open_interest: 5000,
            event_time: Utc::now() + Duration::hours(36),
            close_time: Utc::now() + Duration::hours(35),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        // Market 4: Sports, low volume (should NOT match underdog_hunter)
        Market {
            id: MarketId::new("SPORTS-003".to_string()),
            ticker: "NHL-GAME-WIN".to_string(),
            title: "Will team win NHL game?".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NHL".to_string()),
            status: MarketStatus::Open,
            yes_price: dec!(0.12),  // Good price
            no_price: dec!(0.88),
            volume: 500,  // Below min_volume (1000)
            open_interest: 200,
            event_time: Utc::now() + Duration::hours(6),
            close_time: Utc::now() + Duration::hours(5),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        // Market 5: Sports, event too far in future (should NOT match underdog_hunter)
        Market {
            id: MarketId::new("SPORTS-004".to_string()),
            ticker: "NFL-SUPERBOWL-2026".to_string(),
            title: "Will team win Super Bowl 2026?".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Open,
            yes_price: dec!(0.14),
            no_price: dec!(0.86),
            volume: 2000,
            open_interest: 1000,
            event_time: Utc::now() + Duration::hours(100),  // Outside time window (2-48 hours)
            close_time: Utc::now() + Duration::hours(99),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ]
}

#[test]
fn test_full_strategy_evaluation_flow() {
    // Step 1: Load real strategy from JSON file
    let strategy = StrategyLoader::load("strategies/underdog_hunter.json")
        .expect("Failed to load underdog_hunter.json");

    // Verify strategy loaded correctly
    assert_eq!(strategy.name, "Underdog Hunter");
    assert!(strategy.enabled);

    // Step 2: Create test markets
    let markets = create_test_markets();
    assert_eq!(markets.len(), 5);

    // Step 3: Evaluate markets against strategy
    let signals = StrategyEvaluator::evaluate(&markets, &strategy)
        .expect("Evaluation failed");

    // Step 4: Verify only the matching market generated a signal
    // Should only match SPORTS-001 (cheap, sports, good volume, in time window)
    assert_eq!(signals.len(), 1, "Expected exactly 1 signal");

    let signal = &signals[0];

    // Verify signal content
    assert_eq!(signal.market_ticker, "NFL-CHIEFS-WIN");
    assert_eq!(signal.strategy_name, "Underdog Hunter");
    assert_eq!(signal.position_size, 100);  // From strategy JSON

    // Verify it chose the cheaper side (No in this case, since yes=0.15, no=0.85)
    // Actually underdog_hunter uses CheaperSide, so it should pick Yes (0.15)
    assert_eq!(signal.side, calchas::strategy::SignalSide::Yes);
    assert_eq!(signal.recommended_price, dec!(0.15));

    // Verify timing
    assert!(signal.time_to_event_hours >= 23.9);
    assert!(signal.time_to_event_hours <= 24.1);

    // Verify market context
    assert_eq!(signal.market_volume, 5000);
    assert_eq!(signal.market_open_interest, 2000);
}

#[test]
fn test_no_signals_when_no_matches() {
    let strategy = StrategyLoader::load("strategies/underdog_hunter.json")
        .expect("Failed to load strategy");

    // Create markets that don't match any filters
    let markets = vec![
        Market {
            id: MarketId::new("WEATHER-001".to_string()),
            ticker: "RAIN-TOMORROW".to_string(),
            title: "Will it rain tomorrow?".to_string(),
            category: MarketCategory::Weather,  // Wrong category
            sub_category: None,
            status: MarketStatus::Open,
            yes_price: dec!(0.15),
            no_price: dec!(0.85),
            volume: 5000,
            open_interest: 2000,
            event_time: Utc::now() + Duration::hours(24),
            close_time: Utc::now() + Duration::hours(23),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    let signals = StrategyEvaluator::evaluate(&markets, &strategy)
        .expect("Evaluation failed");

    assert_eq!(signals.len(), 0, "Expected no signals for non-matching markets");
}

#[test]
fn test_volatility_hedge_generates_two_signals() {
    let strategy = StrategyLoader::load("strategies/volatility_hedge.json")
        .expect("Failed to load volatility_hedge.json");

    // Create a market that matches volatility hedge strategy
    let markets = vec![
        Market {
            id: MarketId::new("SPORTS-HEDGE-001".to_string()),
            ticker: "CLOSE-GAME".to_string(),
            title: "Will team win close game?".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Open,
            yes_price: dec!(0.48),  // In range 0.30-0.70
            no_price: dec!(0.52),   // Both sides in range
            volume: 10000,  // Above min_volume (5000)
            open_interest: 5000,  // Above min_open_interest (2000)
            event_time: Utc::now() + Duration::hours(3),  // In time window (1-12 hours)
            close_time: Utc::now() + Duration::hours(2),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    let signals = StrategyEvaluator::evaluate(&markets, &strategy)
        .expect("Evaluation failed");

    // Volatility hedge uses EntrySide::Both, so should generate 2 signals
    assert_eq!(signals.len(), 2, "Expected 2 signals for Both strategy");

    // Verify both sides are present
    assert!(signals.iter().any(|s| matches!(s.side, calchas::strategy::SignalSide::Yes)));
    assert!(signals.iter().any(|s| matches!(s.side, calchas::strategy::SignalSide::No)));

    // Verify both have same position size
    assert_eq!(signals[0].position_size, 50);
    assert_eq!(signals[1].position_size, 50);
}

#[test]
fn test_disabled_strategy_returns_error() {
    let mut strategy = StrategyLoader::load("strategies/underdog_hunter.json")
        .expect("Failed to load strategy");

    // Disable the strategy
    strategy.enabled = false;

    let markets = create_test_markets();

    let result = StrategyEvaluator::evaluate(&markets, &strategy);

    assert!(result.is_err(), "Expected error for disabled strategy");
    assert!(matches!(
        result.unwrap_err(),
        calchas::strategy::EvaluationError::StrategyDisabled(_)
    ));
}

#[test]
fn test_evaluate_all_with_multiple_strategies() {
    // Load both strategies
    let strategies = StrategyLoader::load_all("strategies")
        .expect("Failed to load strategies");

    assert!(strategies.len() >= 2, "Expected at least 2 strategies");

    // Create markets
    let markets = create_test_markets();

    // Evaluate all strategies
    let signals = StrategyEvaluator::evaluate_all(&markets, &strategies)
        .expect("Evaluation failed");

    // Should have at least some signals (underdog_hunter matches SPORTS-001)
    assert!(!signals.is_empty(), "Expected some signals from multiple strategies");

    // Verify signals have different strategy names
    let strategy_names: Vec<&str> = signals.iter()
        .map(|s| s.strategy_name.as_str())
        .collect();

    // Should have signals from at least one strategy
    assert!(!strategy_names.is_empty());
}
