//! Integration test for Phase 3: Strategy Engine
//!
//! Tests the complete flow:
//! 1. Load strategy from JSON
//! 2. Create test markets
//! 3. Evaluate markets against strategy
//! 4. Verify signals generated correctly

use calchas::strategy::{StrategyLoader, StrategyEvaluator};
use calchas::models::{Market, MarketId, MarketCategory, MarketStatus, Orderbook, market::OrderbookLevel};
use calchas::trading::{PriceTracker, VolumeTracker, OrderFlowTracker};
use chrono::{Duration, Utc};
use rust_decimal_macros::dec;

fn create_test_markets() -> Vec<Market> {
    vec![
        // Market 1: Sports, cheap Yes side (should match underdog_hunter)
        Market {
            id: MarketId::new("SPORTS-001".to_string()),
            ticker: "NFL-CHIEFS-WIN".to_string(),
            title: "Will Kansas City Chiefs win Super Bowl?".to_string(),
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.15),  // Cheap - matches underdog_hunter filter
            no_price: dec!(0.85),
            yes_bid: dec!(0.14),
            yes_ask: dec!(0.16),
            no_bid: dec!(0.84),
            no_ask: dec!(0.86),
            volume: 5000,  // Above min_volume (1000)
            volume_24h: 0,
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
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.75),  // Too expensive - outside price range
            no_price: dec!(0.25),  // This is cheap but strategy looks at Yes side for UnderdogOnly
            yes_bid: dec!(0.74),
            yes_ask: dec!(0.76),
            no_bid: dec!(0.24),
            no_ask: dec!(0.26),
            volume: 3000,
            volume_24h: 0,
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
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Politics,
            sub_category: Some("Presidential".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.18),  // Would match price, but wrong category
            no_price: dec!(0.82),
            yes_bid: dec!(0.17),
            yes_ask: dec!(0.19),
            no_bid: dec!(0.81),
            no_ask: dec!(0.83),
            volume: 10000,
            volume_24h: 0,
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
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NHL".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.12),  // Good price
            no_price: dec!(0.88),
            yes_bid: dec!(0.11),
            yes_ask: dec!(0.13),
            no_bid: dec!(0.87),
            no_ask: dec!(0.89),
            volume: 500,  // Below min_volume (1000)
            volume_24h: 0,
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
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.14),
            no_price: dec!(0.86),
            yes_bid: dec!(0.13),
            yes_ask: dec!(0.15),
            no_bid: dec!(0.85),
            no_ask: dec!(0.87),
            volume: 2000,
            volume_24h: 0,
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
    let signals = StrategyEvaluator::evaluate(&markets, &strategy, None, None, None)
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

    // Verify timing (sports market uses event_time which is 24 hours in the test data)
    // After timing bug fix: crypto markets use close_time, sports markets use event_time
    assert!(signal.time_to_event_minutes >= 1434.0);  // 23.9 hours * 60
    assert!(signal.time_to_event_minutes <= 1446.0);  // 24.1 hours * 60

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
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Weather,  // Wrong category
            sub_category: None,
            status: MarketStatus::Active,
            yes_price: dec!(0.15),
            no_price: dec!(0.85),
            yes_bid: dec!(0.14),
            yes_ask: dec!(0.16),
            no_bid: dec!(0.84),
            no_ask: dec!(0.86),
            volume: 5000,
            volume_24h: 0,
            open_interest: 2000,
            event_time: Utc::now() + Duration::hours(24),
            close_time: Utc::now() + Duration::hours(23),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    let signals = StrategyEvaluator::evaluate(&markets, &strategy, None, None, None)
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
            event_ticker: "TEST-EVENT".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("NFL".to_string()),
            status: MarketStatus::Active,
            yes_price: dec!(0.48),  // In range 0.30-0.70
            no_price: dec!(0.52),   // Both sides in range
            yes_bid: dec!(0.47),
            yes_ask: dec!(0.49),
            no_bid: dec!(0.51),
            no_ask: dec!(0.53),
            volume: 10000,  // Above min_volume (5000)
            volume_24h: 0,
            open_interest: 5000,  // Above min_open_interest (2000)
            event_time: Utc::now() + Duration::hours(3),  // In time window (1-12 hours)
            close_time: Utc::now() + Duration::hours(2),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    let signals = StrategyEvaluator::evaluate(&markets, &strategy, None, None, None)
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

    let result = StrategyEvaluator::evaluate(&markets, &strategy, None, None, None);

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
    let signals = StrategyEvaluator::evaluate_all(&markets, &strategies, None, None, None)
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
            series_ticker: None,
            min_price: Some(dec!(0.10)),
            max_price: Some(dec!(0.90)),
            min_volume: Some(1000),
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: Some(dec!(5.0)),  // Require 5% movement
            momentum_lookback_minutes: Some(60),  // Over last hour
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            prioritize_live_games: None,
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
            event_ticker: "TEST-EVENT".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        yes_bid: dec!(0.49),
        yes_ask: dec!(0.51),
        no_bid: dec!(0.49),
        no_ask: dec!(0.51),
        volume: 5000,
        volume_24h: 0,
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
            event_ticker: "TEST-EVENT".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        yes_bid: dec!(0.49),
        yes_ask: dec!(0.51),
        no_bid: dec!(0.49),
        no_ask: dec!(0.51),
        volume: 5000,
        volume_24h: 0,
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
    let signals = StrategyEvaluator::evaluate(&markets, &strategy, Some(&tracker), None, None)
        .expect("Evaluation failed");

    // Should only match the market with sufficient momentum
    assert_eq!(signals.len(), 1, "Expected 1 signal (only market with >5% momentum)");
    assert_eq!(signals[0].market_ticker, "HAS-MOMENTUM");

    // Evaluate WITHOUT price tracker (should allow both - fallback behavior)
    let signals_no_tracker = StrategyEvaluator::evaluate(&markets, &strategy, None, None, None)
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

    // Best ask prices (LAST element, since Kalshi orderbook is ascending)
    assert_eq!(orderbook.yes_best_ask().unwrap(), dec!(0.56));
    assert_eq!(orderbook.no_best_ask().unwrap(), dec!(0.49));

    // Best quantities (LAST element)
    assert_eq!(orderbook.yes_best_ask_quantity(), 50);
    assert_eq!(orderbook.no_best_ask_quantity(), 25);

    // Spread calculation
    // YES ask = 0.56
    // NO ask = 0.49
    // Implied YES from NO = 1.00 - 0.49 = 0.51
    // Spread = 0.56 - 0.51 = 0.05
    let spread = orderbook.spread().unwrap();
    assert_eq!(spread, dec!(0.05));
}

#[test]
fn test_volume_spike_detection_integration() {
    // Test that VolumeTracker works with StrategyEvaluator
    let mut volume_tracker = VolumeTracker::new();

    let market = Market {
        id: MarketId::new("NBA-GAME-001".to_string()),
        ticker: "LAKERS-WIN".to_string(),
        title: "Will Lakers win tonight?".to_string(),
        event_ticker: "KXNBAGAME-001".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.55),
        no_price: dec!(0.45),
        yes_bid: dec!(0.54),
        yes_ask: dec!(0.56),
        no_bid: dec!(0.44),
        no_ask: dec!(0.46),
        volume: 10000,
        volume_24h: 0,
        open_interest: 5000,
        event_time: Utc::now() + Duration::hours(2),
        close_time: Utc::now() + Duration::hours(2),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Simulate volume history: steady baseline then sudden spike
    let now = Utc::now();

    // Insert in chronological order (oldest to newest)
    // Baseline: 1000 contracts/hour for 1 hour
    volume_tracker.insert_test_snapshot(&market.id, 0, now - Duration::hours(1));
    volume_tracker.insert_test_snapshot(&market.id, 500, now - Duration::minutes(30));

    // Recent spike: 2000 contracts in last 10 minutes = 12000/hour rate
    // This is 12x the baseline (1000/hour), so 1100% spike
    volume_tracker.insert_test_snapshot(&market.id, 1000, now - Duration::minutes(10));
    volume_tracker.insert_test_snapshot(&market.id, 3000, now);

    // Test the volume spike calculation directly
    let spike_pct = volume_tracker
        .calculate_volume_spike(&market.id, Duration::minutes(10))
        .expect("Should have volume spike data");

    // Should show massive spike (2000 contracts in 10 min vs 1000 contracts in hour average)
    assert!(spike_pct > dec!(100.0), "Expected >100% volume spike, got {}", spike_pct);

    // Test with evaluator's matches_volume_spike function
    let matches = calchas::strategy::StrategyEvaluator::matches_volume_spike(
        &market,
        Some(dec!(75.0)),  // 75% spike threshold (from sharp-money-follower)
        Some(10),          // 10 minute lookback
        Some(&volume_tracker),
    );

    assert!(matches, "Market should match volume spike filter");

    // Test that it rejects when spike is too small
    let no_match = calchas::strategy::StrategyEvaluator::matches_volume_spike(
        &market,
        Some(dec!(2000.0)),  // Impossible 2000% threshold
        Some(10),
        Some(&volume_tracker),
    );

    assert!(!no_match, "Market should NOT match with very high threshold");
}

#[test]
fn test_volume_spike_filter_with_no_data() {
    // Test that volume spike filter passes when no tracker provided
    let market = Market {
        id: MarketId::new("TEST-001".to_string()),
        ticker: "TEST".to_string(),
        title: "Test Market".to_string(),
        event_ticker: "TEST-EVENT".to_string(),
        category: MarketCategory::Sports,
        sub_category: None,
        status: MarketStatus::Active,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        yes_bid: dec!(0.49),
        yes_ask: dec!(0.51),
        no_bid: dec!(0.49),
        no_ask: dec!(0.51),
        volume: 1000,
        volume_24h: 0,
        open_interest: 500,
        event_time: Utc::now() + Duration::hours(1),
        close_time: Utc::now() + Duration::hours(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // When no tracker provided, filter should pass (allows trading to continue)
    let matches = calchas::strategy::StrategyEvaluator::matches_volume_spike(
        &market,
        Some(dec!(75.0)),
        Some(10),
        None,  // No tracker
    );

    assert!(matches, "Volume spike filter should pass when no tracker provided");
}

// =============================================================================
// PHASE 2: ORDER FLOW IMBALANCE TESTS
// =============================================================================

#[test]
fn test_order_flow_imbalance_bullish() {
    // Test that OrderFlowTracker detects bullish orderbook imbalance
    let mut tracker = OrderFlowTracker::new();

    let market = Market {
        id: MarketId::new("NBA-GAME-002".to_string()),
        ticker: "CELTICS-WIN".to_string(),
        title: "Will Celtics win tonight?".to_string(),
        event_ticker: "KXNBAGAME-002".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.60),
        no_price: dec!(0.40),
        yes_bid: dec!(0.59),
        yes_ask: dec!(0.61),
        no_bid: dec!(0.39),
        no_ask: dec!(0.41),
        volume: 15000,
        volume_24h: 0,
        open_interest: 8000,
        event_time: Utc::now() + Duration::hours(3),
        close_time: Utc::now() + Duration::hours(3),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Simulate bullish orderbook: 75% buy-side liquidity (3:1 ratio)
    // NO asks (bullish pressure): 750 contracts across top 3 levels
    // YES asks (bearish pressure): 250 contracts across top 3 levels
    // OFI = (750 - 250) / (750 + 250) = 0.5 (50% imbalance)
    let orderbook = Orderbook {
        market_id: market.id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.59), quantity: 50 },
            OrderbookLevel { price: dec!(0.60), quantity: 100 },
            OrderbookLevel { price: dec!(0.61), quantity: 100 },  // Best ask (last)
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.39), quantity: 200 },
            OrderbookLevel { price: dec!(0.40), quantity: 300 },
            OrderbookLevel { price: dec!(0.41), quantity: 250 },  // Best ask (last)
        ],
    };

    tracker.record_orderbook(&orderbook);

    // Test OFI calculation
    let ofi = tracker.calculate_ofi(&market.id).expect("Should have OFI data");
    assert_eq!(ofi, dec!(0.5), "OFI should be 0.5 (bullish)");

    // Test with evaluator's matches_order_flow function
    let matches = calchas::strategy::StrategyEvaluator::matches_order_flow(
        &market,
        Some(dec!(0.35)),  // 35% imbalance threshold (from strategy)
        Some(&tracker),
    );

    assert!(matches, "Market should match order flow imbalance filter (0.5 >= 0.35)");
}

#[test]
fn test_order_flow_imbalance_bearish() {
    // Test that OrderFlowTracker detects bearish orderbook imbalance
    let mut tracker = OrderFlowTracker::new();

    let market = Market {
        id: MarketId::new("NFL-GAME-001".to_string()),
        ticker: "CHIEFS-WIN".to_string(),
        title: "Will Chiefs win?".to_string(),
        event_ticker: "KXNFLGAME-001".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NFL".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.45),
        no_price: dec!(0.55),
        yes_bid: dec!(0.44),
        yes_ask: dec!(0.46),
        no_bid: dec!(0.54),
        no_ask: dec!(0.56),
        volume: 20000,
        volume_24h: 0,
        open_interest: 10000,
        event_time: Utc::now() + Duration::hours(4),
        close_time: Utc::now() + Duration::hours(4),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Simulate bearish orderbook: 25% buy-side liquidity (1:3 ratio)
    // NO asks (bullish pressure): 250 contracts
    // YES asks (bearish pressure): 750 contracts
    // OFI = (250 - 750) / (250 + 750) = -0.5 (50% bearish imbalance)
    let orderbook = Orderbook {
        market_id: market.id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.44), quantity: 200 },
            OrderbookLevel { price: dec!(0.45), quantity: 300 },
            OrderbookLevel { price: dec!(0.46), quantity: 250 },
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.54), quantity: 50 },
            OrderbookLevel { price: dec!(0.55), quantity: 100 },
            OrderbookLevel { price: dec!(0.56), quantity: 100 },
        ],
    };

    tracker.record_orderbook(&orderbook);

    // Test OFI calculation
    let ofi = tracker.calculate_ofi(&market.id).expect("Should have OFI data");
    assert_eq!(ofi, dec!(-0.5), "OFI should be -0.5 (bearish)");

    // Test with evaluator (checks absolute value)
    let matches = calchas::strategy::StrategyEvaluator::matches_order_flow(
        &market,
        Some(dec!(0.35)),  // Abs value check: |-0.5| = 0.5 >= 0.35
        Some(&tracker),
    );

    assert!(matches, "Market should match order flow imbalance filter (abs value)");
}

#[test]
fn test_order_flow_imbalance_balanced() {
    // Test that balanced orderbook doesn't match high threshold
    let mut tracker = OrderFlowTracker::new();

    let market = Market {
        id: MarketId::new("TEST-BALANCED".to_string()),
        ticker: "BALANCED".to_string(),
        title: "Balanced Market".to_string(),
        event_ticker: "TEST-EVENT".to_string(),
        category: MarketCategory::Sports,
        sub_category: None,
        status: MarketStatus::Active,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        yes_bid: dec!(0.49),
        yes_ask: dec!(0.51),
        no_bid: dec!(0.49),
        no_ask: dec!(0.51),
        volume: 5000,
        volume_24h: 0,
        open_interest: 2000,
        event_time: Utc::now() + Duration::hours(2),
        close_time: Utc::now() + Duration::hours(2),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Perfectly balanced orderbook
    let orderbook = Orderbook {
        market_id: market.id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.49), quantity: 100 },
            OrderbookLevel { price: dec!(0.50), quantity: 200 },
            OrderbookLevel { price: dec!(0.51), quantity: 200 },
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.49), quantity: 100 },
            OrderbookLevel { price: dec!(0.50), quantity: 200 },
            OrderbookLevel { price: dec!(0.51), quantity: 200 },
        ],
    };

    tracker.record_orderbook(&orderbook);

    // Test OFI calculation
    let ofi = tracker.calculate_ofi(&market.id).expect("Should have OFI data");
    assert_eq!(ofi, dec!(0.0), "OFI should be 0.0 (balanced)");

    // Should not match with high threshold
    let matches = calchas::strategy::StrategyEvaluator::matches_order_flow(
        &market,
        Some(dec!(0.35)),  // Balanced (0.0) < 0.35
        Some(&tracker),
    );

    assert!(!matches, "Balanced market should not match high OFI threshold");
}

#[test]
fn test_order_flow_filter_with_no_tracker() {
    // Test that order flow filter passes when no tracker provided
    let market = Market {
        id: MarketId::new("TEST-NO-TRACKER".to_string()),
        ticker: "NO-TRACKER".to_string(),
        title: "Test Market".to_string(),
        event_ticker: "TEST-EVENT".to_string(),
        category: MarketCategory::Sports,
        sub_category: None,
        status: MarketStatus::Active,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        yes_bid: dec!(0.49),
        yes_ask: dec!(0.51),
        no_bid: dec!(0.49),
        no_ask: dec!(0.51),
        volume: 1000,
        volume_24h: 0,
        open_interest: 500,
        event_time: Utc::now() + Duration::hours(1),
        close_time: Utc::now() + Duration::hours(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // When no tracker provided, filter should pass
    let matches = calchas::strategy::StrategyEvaluator::matches_order_flow(
        &market,
        Some(dec!(0.35)),
        None,  // No tracker
    );

    assert!(matches, "Order flow filter should pass when no tracker provided");
}

#[test]
fn test_combined_volume_spike_and_order_flow() {
    // Test that both Phase 1 and Phase 2 filters work together
    let mut volume_tracker = VolumeTracker::new();
    let mut order_flow_tracker = OrderFlowTracker::new();

    let market = Market {
        id: MarketId::new("COMBINED-TEST".to_string()),
        ticker: "COMBINED".to_string(),
        title: "Combined Phase 1+2 Test".to_string(),
        event_ticker: "KXNBAGAME-COMBO".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.55),
        no_price: dec!(0.45),
        yes_bid: dec!(0.54),
        yes_ask: dec!(0.56),
        no_bid: dec!(0.44),
        no_ask: dec!(0.46),
        volume: 15000,
        volume_24h: 0,
        open_interest: 7500,
        event_time: Utc::now() + Duration::hours(2),
        close_time: Utc::now() + Duration::hours(2),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Simulate volume spike (Phase 1)
    let now = Utc::now();
    volume_tracker.insert_test_snapshot(&market.id, 0, now - Duration::hours(1));
    volume_tracker.insert_test_snapshot(&market.id, 1000, now - Duration::minutes(10));
    volume_tracker.insert_test_snapshot(&market.id, 3000, now);  // 2000 in 10 min = huge spike

    // Simulate bullish OFI (Phase 2)
    let orderbook = Orderbook {
        market_id: market.id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.54), quantity: 400 },
            OrderbookLevel { price: dec!(0.55), quantity: 500 },
            OrderbookLevel { price: dec!(0.56), quantity: 350 },  // Total: 1250
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.44), quantity: 100 },
            OrderbookLevel { price: dec!(0.45), quantity: 150 },
            OrderbookLevel { price: dec!(0.46), quantity: 100 },  // Total: 350
        ],
    };
    order_flow_tracker.record_orderbook(&orderbook);

    // Test Phase 1: Volume spike
    let volume_matches = calchas::strategy::StrategyEvaluator::matches_volume_spike(
        &market,
        Some(dec!(75.0)),  // 75% spike threshold
        Some(10),          // 10 minute lookback
        Some(&volume_tracker),
    );

    assert!(volume_matches, "Should detect volume spike");

    // Test Phase 2: Order flow imbalance
    let ofi_matches = calchas::strategy::StrategyEvaluator::matches_order_flow(
        &market,
        Some(dec!(0.35)),  // 35% imbalance threshold
        Some(&order_flow_tracker),
    );

    assert!(ofi_matches, "Should detect order flow imbalance");

    // Combined signal: Both indicators fire = HIGH CONVICTION
    assert!(volume_matches && ofi_matches, "Both Phase 1 and Phase 2 should fire together");
}


// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[test]
fn test_order_flow_empty_orderbook() {
    // Test OFI calculation with zero liquidity at all levels
    let mut tracker = OrderFlowTracker::new();

    let market_id = MarketId::new("EMPTY-OB".to_string());
    let orderbook = Orderbook {
        market_id: market_id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.50), quantity: 0 },  // Zero quantity
            OrderbookLevel { price: dec!(0.51), quantity: 0 },
            OrderbookLevel { price: dec!(0.52), quantity: 0 },
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.48), quantity: 0 },
            OrderbookLevel { price: dec!(0.49), quantity: 0 },
            OrderbookLevel { price: dec!(0.50), quantity: 0 },
        ],
    };

    tracker.record_orderbook(&orderbook);

    // Should return None (not 0) for empty orderbook
    let ofi = tracker.calculate_ofi(&market_id);
    assert!(ofi.is_none(), "Empty orderbook should return None, not 0");
}

#[test]
fn test_volume_spike_same_timestamp() {
    // Test volume spike calculation when snapshots have identical timestamps
    let mut tracker = VolumeTracker::new();
    let market_id = MarketId::new("SAME-TS".to_string());

    let now = Utc::now();

    // Insert snapshots with SAME timestamp (edge case: system clock didn't advance)
    tracker.insert_test_snapshot(&market_id, 1000, now);
    tracker.insert_test_snapshot(&market_id, 2000, now);  // Same timestamp!
    tracker.insert_test_snapshot(&market_id, 3000, now);

    // Should return None because time_elapsed = 0 (division by zero guard)
    let spike = tracker.calculate_volume_spike(&market_id, Duration::minutes(10));
    assert!(spike.is_none(), "Same timestamp should return None (avoid division by zero)");
}

#[test]
fn test_orderbook_fewer_levels_than_depth() {
    // Test OFI when orderbook has fewer levels than requested depth (3)
    let mut tracker = OrderFlowTracker::new();
    let market_id = MarketId::new("THIN-OB".to_string());

    let orderbook = Orderbook {
        market_id: market_id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.60), quantity: 100 },  // Only 1 level
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.40), quantity: 300 },  // Only 1 level
        ],
    };

    tracker.record_orderbook(&orderbook);

    // Should still calculate OFI correctly (sum what's available)
    let ofi = tracker.calculate_ofi(&market_id);
    assert!(ofi.is_some(), "Should calculate OFI even with thin orderbook");

    // OFI = (300 - 100) / (300 + 100) = 200 / 400 = 0.5
    assert_eq!(ofi.unwrap(), dec!(0.5), "OFI should be 0.5 for 300/100 split");
}

#[test]
fn test_all_three_trackers_in_evaluate() {
    // Test that all three trackers (price, volume, order_flow) work together in evaluate()
    let mut price_tracker = PriceTracker::new();
    let mut volume_tracker = VolumeTracker::new();
    let mut order_flow_tracker = OrderFlowTracker::new();

    // Create a market that should match ALL filters
    let market = Market {
        id: MarketId::new("ALL-TRACKERS".to_string()),
        ticker: "ALL3".to_string(),
        title: "Test All 3 Trackers".to_string(),
        event_ticker: "KXNBAGAME-ALL3".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.55),
        no_price: dec!(0.45),
        yes_bid: dec!(0.54),
        yes_ask: dec!(0.56),
        no_bid: dec!(0.44),
        no_ask: dec!(0.46),
        volume: 20000,
        volume_24h: 0,
        open_interest: 10000,
        event_time: Utc::now() + Duration::hours(3),
        close_time: Utc::now() + Duration::hours(3),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let now = Utc::now();

    // Phase 0: Price momentum (2% move in 60 min)
    // NOTE: insert_test_snapshot appends, so insert NEWEST FIRST to maintain newest-first ordering
    price_tracker.insert_test_snapshot(&market.id, dec!(0.51), dec!(0.49), now);  // 2% gain (newest)
    price_tracker.insert_test_snapshot(&market.id, dec!(0.50), dec!(0.50), now - Duration::hours(1));  // older

    // Phase 1: Volume spike (100% spike)
    volume_tracker.insert_test_snapshot(&market.id, 0, now - Duration::hours(1));
    volume_tracker.insert_test_snapshot(&market.id, 5000, now - Duration::minutes(10));
    volume_tracker.insert_test_snapshot(&market.id, 20000, now);  // 15000 in 10 min = big spike

    // Phase 2: Order flow imbalance (+0.5 OFI)
    let orderbook = Orderbook {
        market_id: market.id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.54), quantity: 100 },
            OrderbookLevel { price: dec!(0.55), quantity: 100 },
            OrderbookLevel { price: dec!(0.56), quantity: 50 },  // Total: 250 (bearish)
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.44), quantity: 300 },
            OrderbookLevel { price: dec!(0.45), quantity: 300 },
            OrderbookLevel { price: dec!(0.46), quantity: 150 },  // Total: 750 (bullish)
        ],
    };
    order_flow_tracker.record_orderbook(&orderbook);

    // Create strategy that requires ALL THREE filters
    let strategy = calchas::models::Strategy {
        id: calchas::models::StrategyId::new("all-three-trackers".to_string()),
        name: "All Three Trackers".to_string(),
        description: "Requires momentum + volume spike + OFI".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        filters: calchas::models::StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: Some(10000),
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: Some(dec!(2.0)),        // Momentum filter
            momentum_lookback_minutes: Some(60),
            min_volume_spike_pct: Some(dec!(50.0)),   // Volume spike filter
            volume_spike_lookback_minutes: Some(10),
            min_order_flow_imbalance: Some(dec!(0.3)), // OFI filter
            prioritize_live_games: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        },
        entry_rules: calchas::models::EntryRules {
            side: calchas::models::strategy::EntrySide::CheaperSide,
            position_size: 100,
            position_size_unit: calchas::models::strategy::PositionSizeUnit::Dollars,
            order_type: calchas::models::strategy::OrderType::Market,
            limit_price_offset: None,
        },
        exit_rules: calchas::models::ExitRules {
            take_profit_pct: Some(dec!(5.0)),
            stop_loss_pct: Some(dec!(2.0)),
            trailing_stop_pct: None,
            trailing_stop_activation_pct: None,
            max_hold_time_minutes: Some(120),
            exit_order_type: calchas::models::strategy::OrderType::Market,
        },
        risk_limits: calchas::models::RiskLimits {
            max_concurrent_positions: 3,
            max_daily_loss_usd: Some(dec!(100.0)),
            max_position_loss_usd: Some(dec!(25.0)),
            loss_cooldown_minutes: Some(30),
        },
    };

    // Evaluate with ALL THREE trackers
    let signals = StrategyEvaluator::evaluate(
        &[market],
        &strategy,
        Some(&price_tracker),
        Some(&volume_tracker),
        Some(&order_flow_tracker),
    ).expect("Evaluation should succeed");

    // Should generate 1 signal (market passes all 3 filters)
    assert_eq!(signals.len(), 1, "Should generate signal when all 3 trackers match");
    assert_eq!(signals[0].market_ticker, "ALL3");
}

#[test]
fn test_all_trackers_with_one_failing() {
    // Test that if ANY filter fails, no signal is generated
    let mut price_tracker = PriceTracker::new();
    let mut volume_tracker = VolumeTracker::new();
    let mut order_flow_tracker = OrderFlowTracker::new();

    let market = Market {
        id: MarketId::new("ONE-FAILS".to_string()),
        ticker: "FAILS".to_string(),
        title: "Test One Filter Fails".to_string(),
        event_ticker: "KXNBAGAME-FAILS".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.55),
        no_price: dec!(0.45),
        yes_bid: dec!(0.54),
        yes_ask: dec!(0.56),
        no_bid: dec!(0.44),
        no_ask: dec!(0.46),
        volume: 20000,
        volume_24h: 0,
        open_interest: 10000,
        event_time: Utc::now() + Duration::hours(3),
        close_time: Utc::now() + Duration::hours(3),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let now = Utc::now();

    // ✅ Phase 0: Has momentum (2% gain)
    price_tracker.insert_test_snapshot(&market.id, dec!(0.50), dec!(0.50), now - Duration::hours(1));
    price_tracker.insert_test_snapshot(&market.id, dec!(0.51), dec!(0.49), now);

    // ❌ Phase 1: NO volume spike (only 10% spike, need 50%)
    volume_tracker.insert_test_snapshot(&market.id, 10000, now - Duration::hours(1));
    volume_tracker.insert_test_snapshot(&market.id, 18000, now - Duration::minutes(10));
    volume_tracker.insert_test_snapshot(&market.id, 20000, now);  // Only 2000 in 10 min = small spike

    // ✅ Phase 2: Has OFI (+0.5)
    let orderbook = Orderbook {
        market_id: market.id.clone(),
        yes_asks: vec![OrderbookLevel { price: dec!(0.55), quantity: 250 }],
        no_asks: vec![OrderbookLevel { price: dec!(0.45), quantity: 750 }],
    };
    order_flow_tracker.record_orderbook(&orderbook);

    let strategy = calchas::models::Strategy {
        id: calchas::models::StrategyId::new("strict-filters".to_string()),
        name: "Strict Filters".to_string(),
        description: "All filters must pass".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        filters: calchas::models::StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: Some(10000),
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: Some(dec!(2.0)),
            momentum_lookback_minutes: Some(60),
            min_volume_spike_pct: Some(dec!(50.0)),  // ❌ Market only has 10% spike
            volume_spike_lookback_minutes: Some(10),
            min_order_flow_imbalance: Some(dec!(0.3)),
            prioritize_live_games: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        },
        entry_rules: calchas::models::EntryRules {
            side: calchas::models::strategy::EntrySide::CheaperSide,
            position_size: 100,
            position_size_unit: calchas::models::strategy::PositionSizeUnit::Dollars,
            order_type: calchas::models::strategy::OrderType::Market,
            limit_price_offset: None,
        },
        exit_rules: calchas::models::ExitRules {
            take_profit_pct: Some(dec!(5.0)),
            stop_loss_pct: Some(dec!(2.0)),
            trailing_stop_pct: None,
            trailing_stop_activation_pct: None,
            max_hold_time_minutes: Some(120),
            exit_order_type: calchas::models::strategy::OrderType::Market,
        },
        risk_limits: calchas::models::RiskLimits {
            max_concurrent_positions: 3,
            max_daily_loss_usd: Some(dec!(100.0)),
            max_position_loss_usd: Some(dec!(25.0)),
            loss_cooldown_minutes: Some(30),
        },
    };

    let signals = StrategyEvaluator::evaluate(
        &[market],
        &strategy,
        Some(&price_tracker),
        Some(&volume_tracker),
        Some(&order_flow_tracker),
    ).expect("Evaluation should succeed");

    // Should generate NO signals (volume spike filter failed)
    assert_eq!(signals.len(), 0, "Should generate NO signal when any filter fails");
}

#[test]
fn test_volume_spike_calculation_debug() {
    // Debug test to verify volume spike calculation
    let mut volume_tracker = VolumeTracker::new();
    let market_id = MarketId::new("DEBUG".to_string());

    let now = Utc::now();

    // Same setup as test_all_three_trackers_in_evaluate
    volume_tracker.insert_test_snapshot(&market_id, 0, now - Duration::hours(1));
    volume_tracker.insert_test_snapshot(&market_id, 5000, now - Duration::minutes(10));
    volume_tracker.insert_test_snapshot(&market_id, 20000, now);

    // Calculate volume spike
    let spike = volume_tracker.calculate_volume_spike(&market_id, Duration::minutes(10));

    println!("DEBUG: Volume spike = {:?}", spike);
    assert!(spike.is_some(), "Should calculate volume spike");

    let spike_pct = spike.unwrap();
    println!("DEBUG: Volume spike percentage = {}", spike_pct);

    // Should be a huge spike (15000 contracts in 10 min vs 20000 in 60 min)
    assert!(spike_pct >= dec!(50.0), "Spike should be >= 50%");
}

#[test]
fn test_individual_filters_all_pass() {
    // Test each filter individually to find which one is failing
    let mut price_tracker = PriceTracker::new();
    let mut volume_tracker = VolumeTracker::new();
    let mut order_flow_tracker = OrderFlowTracker::new();

    let market = Market {
        id: MarketId::new("FILTER-TEST".to_string()),
        ticker: "FTEST".to_string(),
        title: "Filter Test".to_string(),
        event_ticker: "KXNBAGAME-TEST".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("NBA".to_string()),
        status: MarketStatus::Active,
        yes_price: dec!(0.55),
        no_price: dec!(0.45),
        yes_bid: dec!(0.54),
        yes_ask: dec!(0.56),
        no_bid: dec!(0.44),
        no_ask: dec!(0.46),
        volume: 20000,
        volume_24h: 0,
        open_interest: 10000,
        event_time: Utc::now() + Duration::hours(3),
        close_time: Utc::now() + Duration::hours(3),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let now = Utc::now();

    // Set up price momentum (insert NEWEST FIRST)
    price_tracker.insert_test_snapshot(&market.id, dec!(0.51), dec!(0.49), now);
    price_tracker.insert_test_snapshot(&market.id, dec!(0.50), dec!(0.50), now - Duration::hours(1));

    // Set up volume spike
    volume_tracker.insert_test_snapshot(&market.id, 0, now - Duration::hours(1));
    volume_tracker.insert_test_snapshot(&market.id, 5000, now - Duration::minutes(10));
    volume_tracker.insert_test_snapshot(&market.id, 20000, now);

    // Set up OFI
    let orderbook = Orderbook {
        market_id: market.id.clone(),
        yes_asks: vec![
            OrderbookLevel { price: dec!(0.54), quantity: 100 },
            OrderbookLevel { price: dec!(0.55), quantity: 100 },
            OrderbookLevel { price: dec!(0.56), quantity: 50 },
        ],
        no_asks: vec![
            OrderbookLevel { price: dec!(0.44), quantity: 300 },
            OrderbookLevel { price: dec!(0.45), quantity: 300 },
            OrderbookLevel { price: dec!(0.46), quantity: 150 },
        ],
    };
    order_flow_tracker.record_orderbook(&orderbook);

    // Test each filter individually
    let cat_pass = StrategyEvaluator::matches_category(&market, &Some(vec![MarketCategory::Sports]), &None);
    println!("Category filter: {}", cat_pass);
    assert!(cat_pass, "Category filter should pass");

    let vol_pass = StrategyEvaluator::matches_volume(&market, Some(10000));
    println!("Volume filter: {}", vol_pass);
    assert!(vol_pass, "Volume filter should pass");

    let momentum_pass = StrategyEvaluator::matches_momentum(&market, Some(dec!(2.0)), Some(60), Some(&price_tracker));
    println!("Momentum filter: {}", momentum_pass);
    assert!(momentum_pass, "Momentum filter should pass");

    let vol_spike_pass = StrategyEvaluator::matches_volume_spike(&market, Some(dec!(50.0)), Some(10), Some(&volume_tracker));
    println!("Volume spike filter: {}", vol_spike_pass);
    assert!(vol_spike_pass, "Volume spike filter should pass");

    let ofi_pass = StrategyEvaluator::matches_order_flow(&market, Some(dec!(0.3)), Some(&order_flow_tracker));
    println!("OFI filter: {}", ofi_pass);
    assert!(ofi_pass, "OFI filter should pass");

    println!("All individual filters pass!");
}

#[test]
fn test_momentum_calculation_debug() {
    // Debug test to verify momentum calculation
    let mut price_tracker = PriceTracker::new();
    let market_id = MarketId::new("MOM-DEBUG".to_string());

    let now = Utc::now();

    // Same setup as the failing tests
    // NOTE: insert NEWEST FIRST
    price_tracker.insert_test_snapshot(&market_id, dec!(0.51), dec!(0.49), now);
    price_tracker.insert_test_snapshot(&market_id, dec!(0.50), dec!(0.50), now - Duration::hours(1));

    // Calculate momentum
    let momentum = price_tracker.calculate_momentum(&market_id, Duration::minutes(60));

    println!("DEBUG: Momentum = {:?}", momentum);
    assert!(momentum.is_some(), "Should calculate momentum");

    let momentum_pct = momentum.unwrap();
    println!("DEBUG: Momentum percentage = {}", momentum_pct);

    // Expected: (0.51 - 0.50) / 0.50 * 100 = 2.0%
    assert_eq!(momentum_pct, dec!(2.0), "Should be exactly 2%");
}
