//! Integration test for Phase 3: Strategy Engine
//!
//! Tests the complete flow:
//! 1. Load strategy from JSON
//! 2. Create test markets
//! 3. Evaluate markets against strategy
//! 4. Verify signals generated correctly

use calchas::strategy::{StrategyLoader, StrategyEvaluator};
use calchas::models::{Market, MarketId, MarketCategory, MarketStatus};
use calchas::trading::PriceTracker;
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
            status: MarketStatus::Active,
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
            status: MarketStatus::Active,
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
            status: MarketStatus::Active,
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
            status: MarketStatus::Active,
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
            status: MarketStatus::Active,
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
    let strategy = StrategyLoader::load("tests/fixtures/strategies/underdog_hunter.json")
        .expect("Failed to load underdog_hunter.json");

    // Verify strategy loaded correctly
    assert_eq!(strategy.name, "Underdog Hunter");
    assert!(strategy.enabled);

    // Step 2: Create test markets
    let markets = create_test_markets();
    assert_eq!(markets.len(), 5);

    // Step 3: Evaluate markets against strategy
    let signals = StrategyEvaluator::evaluate(&markets, &strategy, None)
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

    // Verify timing (now using close_time which is 23 hours in the test data)
    assert!(signal.time_to_event_minutes >= 1374.0);  // 22.9 hours * 60
    assert!(signal.time_to_event_minutes <= 1386.0);  // 23.1 hours * 60

    // Verify market context
    assert_eq!(signal.market_volume, 5000);
    assert_eq!(signal.market_open_interest, 2000);
}

#[test]
fn test_no_signals_when_no_matches() {
    let strategy = StrategyLoader::load("tests/fixtures/strategies/underdog_hunter.json")
        .expect("Failed to load strategy");

    // Create markets that don't match any filters
    let markets = vec![
        Market {
            id: MarketId::new("WEATHER-001".to_string()),
            ticker: "RAIN-TOMORROW".to_string(),
            title: "Will it rain tomorrow?".to_string(),
            category: MarketCategory::Weather,  // Wrong category
            sub_category: None,
            status: MarketStatus::Active,
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

    let signals = StrategyEvaluator::evaluate(&markets, &strategy, None)
        .expect("Evaluation failed");

    assert_eq!(signals.len(), 0, "Expected no signals for non-matching markets");
}

#[test]
fn test_volatility_hedge_generates_two_signals() {
    let strategy = StrategyLoader::load("tests/fixtures/strategies/volatility_hedge.json")
        .expect("Failed to load volatility_hedge.json");

    // Create a market that matches volatility hedge strategy
    let markets = vec![
        Market {
            id: MarketId::new("SPORTS-HEDGE-001".to_string()),
            ticker: "CLOSE-GAME".to_string(),
            title: "Will team win close game?".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Active,
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

    let signals = StrategyEvaluator::evaluate(&markets, &strategy, None)
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

    let result = StrategyEvaluator::evaluate(&markets, &strategy, None);

    assert!(result.is_err(), "Expected error for disabled strategy");
    assert!(matches!(
        result.unwrap_err(),
        calchas::strategy::EvaluationError::StrategyDisabled(_)
    ));
}

#[test]
fn test_evaluate_all_with_multiple_strategies() {
    // Load both strategies
    let strategies = StrategyLoader::load_all("tests/fixtures/strategies")
        .expect("Failed to load strategies");

    assert!(strategies.len() >= 2, "Expected at least 2 strategies");

    // Create markets
    let markets = create_test_markets();

    // Evaluate all strategies
    let signals = StrategyEvaluator::evaluate_all(&markets, &strategies, None)
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

#[test]
fn test_momentum_filter_integration() {
    use calchas::models::strategy::{
        Strategy, StrategyId, StrategyFilters, EntryRules, EntrySide, ExitRules,
        RiskLimits, PositionSizeUnit, OrderType
    };

    // Create a strategy with momentum filters
    let strategy = Strategy {
        id: StrategyId::new("momentum-test".to_string()),
        name: "Momentum Test".to_string(),
        description: "Test strategy for momentum filtering".to_string(),
        version: "1.0".to_string(),
        enabled: true,
        filters: StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            min_price: Some(dec!(0.10)),
            max_price: Some(dec!(0.90)),
            min_volume: Some(1000),
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: Some(dec!(5.0)),  // Require 5% movement
            momentum_lookback_minutes: Some(60),  // Over last hour
            max_spread_cents: None,
            min_best_price_quantity: None,
        },
        entry_rules: EntryRules {
            side: EntrySide::Yes,
            position_size: 10,
            position_size_unit: PositionSizeUnit::Contracts,
            order_type: OrderType::Market,
            limit_price_offset: None,
        },
        exit_rules: ExitRules {
            take_profit_pct: Some(dec!(10.0)),
            stop_loss_pct: Some(dec!(5.0)),
            trailing_stop_pct: None,
            trailing_stop_activation_pct: None,
            max_hold_time_minutes: None,
            exit_order_type: OrderType::Market,
        },
        risk_limits: RiskLimits {
            max_concurrent_positions: 5,
            max_daily_loss_usd: Some(dec!(100.0)),
            max_position_loss_usd: None,
            loss_cooldown_minutes: None,
        },
    };

    // Create test markets
    let market_with_momentum = Market {
        id: MarketId::new("MOMENTUM-MARKET".to_string()),
        ticker: "HAS-MOMENTUM".to_string(),
        title: "Market with momentum".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        volume: 5000,
        open_interest: 2000,
        event_time: Utc::now() + Duration::hours(24),
        close_time: Utc::now() + Duration::hours(23),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let market_without_momentum = Market {
        id: MarketId::new("STALE-MARKET".to_string()),
        ticker: "NO-MOMENTUM".to_string(),
        title: "Stale market".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        volume: 5000,
        open_interest: 2000,
        event_time: Utc::now() + Duration::hours(24),
        close_time: Utc::now() + Duration::hours(23),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Create price tracker with historical data
    let mut tracker = PriceTracker::new();
    let now = Utc::now();

    // Market 1: 10% gain (0.50 -> 0.55) - should pass 5% filter
    // Record old price first (1 hour ago)
    tracker.insert_test_snapshot(
        &market_with_momentum.id,
        dec!(0.50),
        dec!(0.50),
        now - Duration::hours(1),
    );
    // Record current price
    tracker.record_price(&market_with_momentum.id, dec!(0.55), dec!(0.45));

    // Market 2: Only 2% gain (0.50 -> 0.51) - should fail 5% filter
    // Record old price first (1 hour ago)
    tracker.insert_test_snapshot(
        &market_without_momentum.id,
        dec!(0.50),
        dec!(0.50),
        now - Duration::hours(1),
    );
    // Record current price
    tracker.record_price(&market_without_momentum.id, dec!(0.51), dec!(0.49));

    let markets = vec![market_with_momentum.clone(), market_without_momentum.clone()];

    // Evaluate WITH price tracker
    let signals = StrategyEvaluator::evaluate(&markets, &strategy, Some(&tracker))
        .expect("Evaluation failed");

    // Should only match the market with sufficient momentum
    assert_eq!(signals.len(), 1, "Expected 1 signal (only market with >5% momentum)");
    assert_eq!(signals[0].market_ticker, "HAS-MOMENTUM");

    // Evaluate WITHOUT price tracker (should allow both - fallback behavior)
    let signals_no_tracker = StrategyEvaluator::evaluate(&markets, &strategy, None)
        .expect("Evaluation failed");

    assert_eq!(signals_no_tracker.len(), 2, "Without tracker, should allow both markets (fallback)");
}

#[test]
fn test_orderbook_structure() {
    use calchas::models::{Orderbook, OrderbookLevel};

    // Test orderbook spread calculation
    let orderbook = Orderbook {
        market_id: MarketId::new("TEST-MARKET".to_string()),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.55), quantity: 100 },
            OrderbookLevel { price: dec!(0.56), quantity: 50 },
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.48), quantity: 75 },
            OrderbookLevel { price: dec!(0.49), quantity: 25 },
        ],
    };

    // Best ask prices
    assert_eq!(orderbook.yes_best_ask().unwrap(), dec!(0.55));
    assert_eq!(orderbook.no_best_ask().unwrap(), dec!(0.48));

    // Best quantities
    assert_eq!(orderbook.yes_best_ask_quantity(), 100);
    assert_eq!(orderbook.no_best_ask_quantity(), 75);

    // Spread calculation
    // YES ask = 0.55
    // NO ask = 0.48
    // Implied YES from NO = 1.00 - 0.48 = 0.52
    // Spread = 0.55 - 0.52 = 0.03
    let spread = orderbook.spread().unwrap();
    assert_eq!(spread, dec!(0.03));
}
