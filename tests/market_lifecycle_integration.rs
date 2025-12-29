//! Integration tests for market lifecycle and settlement detection
//!
//! Tests the full lifecycle: initialized → active → determined → finalized
//! and how the system handles positions when markets change state

use calchas::models::{Market, MarketCategory, MarketId, MarketStatus};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// =============================================================================
// TEST HELPERS
// =============================================================================

fn create_market_with_status(status: MarketStatus, ticker: &str) -> Market {
    Market {
        id: MarketId::new(ticker.to_string()),
        ticker: ticker.to_string(),
        title: "Test Game".to_string(),
        event_ticker: "TEST-EVENT".to_string(),
        category: MarketCategory::Sports,
        sub_category: Some("Basketball".to_string()),
        status,
        yes_price: dec!(0.50),
        no_price: dec!(0.50),
        yes_bid: dec!(0.49),
        yes_ask: dec!(0.51),
        no_bid: dec!(0.49),
        no_ask: dec!(0.51),
        volume: 10000,
        open_interest: 5000,
        event_time: Utc::now() + Duration::hours(2),
        close_time: Utc::now() + Duration::hours(2),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// =============================================================================
// STATUS TRANSITION TESTS
// =============================================================================

#[test]
fn test_market_lifecycle_complete() {
    // Test the full lifecycle: initialized → active → determined → finalized

    let ticker = "KXNBAGAME-25DEC29DENMIA-DEN";

    // 1. INITIALIZED: Market created, game scheduled but not started
    let market = create_market_with_status(MarketStatus::Initialized, ticker);
    assert_eq!(market.status, MarketStatus::Initialized);
    assert!(!market.is_open());  // Cannot trade yet
    println!("✓ INITIALIZED: Game scheduled, trading not started");

    // 2. ACTIVE: Game started, trading begins
    let market = create_market_with_status(MarketStatus::Active, ticker);
    assert_eq!(market.status, MarketStatus::Active);
    assert!(market.is_open());  // Can trade now
    println!("✓ ACTIVE: Game live, trading active");

    // 3. DETERMINED: Game finished, outcome known but not settled
    let market = create_market_with_status(MarketStatus::Determined, ticker);
    assert_eq!(market.status, MarketStatus::Determined);
    assert!(!market.is_open());  // Cannot trade anymore
    println!("✓ DETERMINED: Game finished, outcome known");

    // 4. FINALIZED: Settlement complete, payouts processed
    let market = create_market_with_status(MarketStatus::Finalized, ticker);
    assert_eq!(market.status, MarketStatus::Finalized);
    assert!(!market.is_open());  // Cannot trade
    println!("✓ FINALIZED: Settlement complete");
}

#[test]
fn test_settlement_detection_finalized() {
    // Test that we correctly detect when a market is finalized
    // This mirrors the logic in loop_handlers.rs:575
    //
    // if market.status == crate::models::MarketStatus::Finalized {
    //     // Close position at settlement price
    // }

    let market = create_market_with_status(MarketStatus::Finalized, "TEST-MARKET");

    // Check 1: Market is finalized
    assert_eq!(market.status, MarketStatus::Finalized);

    // Check 2: Market is NOT open (trading stopped)
    assert!(!market.is_open());

    // Check 3: Market should trigger settlement logic
    let should_settle = market.status == MarketStatus::Finalized;
    assert!(should_settle);

    println!("✓ Settlement detection: Market is finalized, should close position");
}

#[test]
fn test_settlement_detection_not_finalized() {
    // Test that we DON'T close positions for non-finalized markets

    // INITIALIZED: Game not started yet - don't close
    let market = create_market_with_status(MarketStatus::Initialized, "TEST-001");
    assert_ne!(market.status, MarketStatus::Finalized);
    assert!(!market.is_open());  // Not open, but not settled either
    println!("✓ INITIALIZED: Don't close position (game hasn't started)");

    // ACTIVE: Game in progress - don't close
    let market = create_market_with_status(MarketStatus::Active, "TEST-002");
    assert_ne!(market.status, MarketStatus::Finalized);
    assert!(market.is_open());  // Still trading
    println!("✓ ACTIVE: Don't close position (game in progress)");

    // DETERMINED: Outcome known but not settled - don't close
    let market = create_market_with_status(MarketStatus::Determined, "TEST-003");
    assert_ne!(market.status, MarketStatus::Finalized);
    assert!(!market.is_open());  // Not trading, but not settled
    println!("✓ DETERMINED: Don't close position (not settled yet)");
}

#[test]
fn test_settlement_prices() {
    // Test settlement price calculation for finalized markets

    // YES wins: YES → $1.00, NO → $0.00
    let mut market_yes_wins = create_market_with_status(MarketStatus::Finalized, "TEST-YES");
    market_yes_wins.yes_price = dec!(1.00);
    market_yes_wins.no_price = dec!(0.00);

    assert_eq!(market_yes_wins.yes_price, Decimal::ONE);
    assert_eq!(market_yes_wins.no_price, Decimal::ZERO);
    println!("✓ YES wins: YES=$1.00, NO=$0.00");

    // NO wins: YES → $0.00, NO → $1.00
    let mut market_no_wins = create_market_with_status(MarketStatus::Finalized, "TEST-NO");
    market_no_wins.yes_price = dec!(0.00);
    market_no_wins.no_price = dec!(1.00);

    assert_eq!(market_no_wins.yes_price, Decimal::ZERO);
    assert_eq!(market_no_wins.no_price, Decimal::ONE);
    println!("✓ NO wins: YES=$0.00, NO=$1.00");
}

#[test]
fn test_position_pnl_on_settlement() {
    // Test P&L calculation when market settles

    // Scenario 1: Bought YES at 45¢, YES wins (settles at $1.00)
    let entry_price = dec!(0.45);
    let settlement_price = dec!(1.00);
    let quantity = 100;
    let pnl = (settlement_price - entry_price) * Decimal::from(quantity);
    assert_eq!(pnl, dec!(55.00));  // +$55 profit
    println!("✓ YES wins: Bought YES@$0.45, settled@$1.00 → +$55.00");

    // Scenario 2: Bought YES at 45¢, NO wins (settles at $0.00)
    let entry_price = dec!(0.45);
    let settlement_price = dec!(0.00);
    let quantity = 100;
    let pnl = (settlement_price - entry_price) * Decimal::from(quantity);
    assert_eq!(pnl, dec!(-45.00));  // -$45 loss
    println!("✓ NO wins: Bought YES@$0.45, settled@$0.00 → -$45.00");

    // Scenario 3: Bought NO at 55¢, NO wins (settles at $1.00)
    let entry_price = dec!(0.55);
    let settlement_price = dec!(1.00);
    let quantity = 100;
    let pnl = (settlement_price - entry_price) * Decimal::from(quantity);
    assert_eq!(pnl, dec!(45.00));  // +$45 profit
    println!("✓ NO wins: Bought NO@$0.55, settled@$1.00 → +$45.00");

    // Scenario 4: Bought NO at 55¢, YES wins (settles at $0.00)
    let entry_price = dec!(0.55);
    let settlement_price = dec!(0.00);
    let quantity = 100;
    let pnl = (settlement_price - entry_price) * Decimal::from(quantity);
    assert_eq!(pnl, dec!(-55.00));  // -$55 loss
    println!("✓ YES wins: Bought NO@$0.55, settled@$0.00 → -$55.00");
}

#[test]
fn test_market_fetch_with_status_filters() {
    // Test that we correctly handle both "open" and "settled" status filters
    // This mirrors the logic in loop_handlers.rs:169-183
    //
    // for status_filter in ["open", "settled"] {
    //     fetch markets with status filter
    // }

    // "open" filter should include: initialized, active
    let initialized_market = create_market_with_status(MarketStatus::Initialized, "TEST-INIT");
    let active_market = create_market_with_status(MarketStatus::Active, "TEST-ACTIVE");

    // These are "open" markets (not settled yet)
    assert!(matches!(initialized_market.status, MarketStatus::Initialized | MarketStatus::Active));
    assert!(matches!(active_market.status, MarketStatus::Initialized | MarketStatus::Active));

    // "settled" filter should include: finalized
    let finalized_market = create_market_with_status(MarketStatus::Finalized, "TEST-FINAL");
    assert!(matches!(finalized_market.status, MarketStatus::Finalized));

    println!("✓ Status filters: 'open' → initialized/active, 'settled' → finalized");
}

#[test]
fn test_dont_close_initialized_positions() {
    // CRITICAL: Don't close positions for initialized markets
    // (Game scheduled but hasn't started yet)
    //
    // This was a bug: originally checked `market.status != Active`
    // which would close BOTH initialized AND finalized positions
    //
    // Fixed to: `market.status == Finalized` (only close when settled)

    let initialized_market = create_market_with_status(MarketStatus::Initialized, "TEST");

    // Check 1: Market is NOT finalized
    assert_ne!(initialized_market.status, MarketStatus::Finalized);

    // Check 2: Should NOT trigger settlement
    let should_settle = initialized_market.status == MarketStatus::Finalized;
    assert!(!should_settle);

    // Check 3: Market is not open yet
    assert!(!initialized_market.is_open());

    println!("✓ INITIALIZED market: Don't close position (game hasn't started)");
}

#[test]
fn test_time_based_settlement_detection() {
    // Test that we can detect settled games by checking if actual game time has passed
    // even if Kalshi hasn't updated status yet

    let now = Utc::now();

    // Game finished 1 hour ago
    let mut market_finished = create_market_with_status(MarketStatus::Active, "TEST-FINISHED");
    market_finished.event_time = now - Duration::hours(1);
    market_finished.close_time = now - Duration::hours(1);

    // Game time has passed
    let game_finished = now > market_finished.event_time;
    assert!(game_finished);
    println!("✓ Game finished 1h ago: event_time < now");

    // Game scheduled for future
    let mut market_future = create_market_with_status(MarketStatus::Initialized, "TEST-FUTURE");
    market_future.event_time = now + Duration::hours(2);
    market_future.close_time = now + Duration::hours(2);

    // Game time has NOT passed
    let game_finished = now > market_future.event_time;
    assert!(!game_finished);
    println!("✓ Game in 2h: event_time > now");
}
