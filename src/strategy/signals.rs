//! Entry signal generation
//!
//! This module defines the `EntrySignal` type, which represents a trading
//! opportunity identified by the strategy engine. Signals contain all information
//! needed by the trading logic to open a position.
//!
//! # Signal Lifecycle
//!
//! 1. **Generation**: `EntrySignal::from_market()` creates signals from matching markets
//! 2. **Consumption**: Trading logic (Phase 4) converts signals to orders
//! 3. **Execution**: Orders are filled, positions are opened
//!
//! # Example
//!
//! ```no_run
//! use calchas::strategy::signals::EntrySignal;
//! use calchas::models::{Market, Strategy};
//!
//! # let market: Market = todo!();
//! # let strategy: Strategy = todo!();
//! let signals = EntrySignal::from_market(&market, &strategy);
//! for signal in signals {
//!     println!("Signal: {} contracts of {:?} @ ${:.2}",
//!         signal.position_size,
//!         signal.side,
//!         signal.recommended_price);
//! }
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::strategy::{EntrySide, OrderType};
use crate::models::{Market, MarketId, Strategy, StrategyId};

// =============================================================================
// ENUMS
// =============================================================================

/// Which side of the market to buy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalSide {
    /// Buy Yes side
    Yes,
    /// Buy No side
    No,
}

// =============================================================================
// MAIN STRUCT
// =============================================================================

/// Entry signal representing a trading opportunity
///
/// An `EntrySignal` is generated when a market matches a strategy's filter criteria.
/// It contains all information needed to open a position, including which side to buy,
/// how many contracts, and at what price.
///
/// # Examples
///
/// ```no_run
/// # use calchas::strategy::signals::{EntrySignal, SignalSide};
/// # use calchas::models::{Market, Strategy};
/// # let market: Market = todo!();
/// # let strategy: Strategy = todo!();
/// let signals = EntrySignal::from_market(&market, &strategy);
///
/// for signal in signals {
///     println!("Market: {}", signal.market_ticker);
///     println!("Side: {:?}", signal.side);
///     println!("Size: {} contracts", signal.position_size);
///     println!("Price: ${:.2}", signal.recommended_price);
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySignal {
    // Market information
    /// Unique identifier for the market
    pub market_id: MarketId,

    /// Market ticker symbol
    pub market_ticker: String,

    /// Full market title/description
    pub market_title: String,

    // Strategy that generated this signal
    /// ID of the strategy that generated this signal
    pub strategy_id: StrategyId,

    /// Name of the strategy that generated this signal
    pub strategy_name: String,

    // Signal details
    /// Which side to buy (Yes or No)
    pub side: SignalSide,

    /// Recommended price for this side (in dollars, 0.00-1.00 range)
    pub recommended_price: Decimal,

    /// Number of contracts to buy
    pub position_size: u64,

    /// Order type (Market or Limit)
    pub order_type: OrderType,

    /// Offset from current price for limit orders (e.g., -0.01 means 1¢ below)
    pub limit_price_offset: Option<Decimal>,

    // Timing
    /// When this signal was generated
    pub generated_at: DateTime<Utc>,

    /// Hours until the event occurs
    pub time_to_event_hours: f64,

    // Context (for logging/debugging)
    /// Total volume traded in this market
    pub market_volume: u64,

    /// Open interest (outstanding contracts)
    pub market_open_interest: u64,
}

impl EntrySignal {
    /// Generate entry signal(s) from a market that matches strategy filters
    ///
    /// This function converts a matching market into one or more entry signals
    /// based on the strategy's entry rules. The number of signals depends on
    /// the `EntrySide` configuration:
    ///
    /// - `CheaperSide`: 1 signal for the cheaper side
    /// - `ExpensiveSide`: 1 signal for the expensive side
    /// - `Both`: 2 signals (one for each side)
    ///
    /// # Arguments
    ///
    /// * `market` - The market to generate signals for
    /// * `strategy` - The strategy configuration
    ///
    /// # Returns
    ///
    /// A vector of entry signals (1 or 2 signals depending on strategy)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use calchas::strategy::signals::EntrySignal;
    /// # use calchas::models::{Market, Strategy};
    /// # let market: Market = todo!();
    /// # let strategy: Strategy = todo!();
    /// let signals = EntrySignal::from_market(&market, &strategy);
    ///
    /// // CheaperSide or ExpensiveSide: 1 signal
    /// // Both: 2 signals
    /// assert!(!signals.is_empty());
    /// ```
    pub fn from_market(market: &Market, strategy: &Strategy) -> Vec<EntrySignal> {
        let sides = Self::determine_side(market, &strategy.entry_rules.side);

        // Calculate time to event in hours
        let now = Utc::now();
        let time_to_event = market.event_time.signed_duration_since(now);
        let time_to_event_hours = time_to_event.num_seconds() as f64 / 3600.0;

        sides
            .into_iter()
            .map(|(side, price)| EntrySignal {
                // Market information
                market_id: market.id.clone(),
                market_ticker: market.ticker.clone(),
                market_title: market.title.clone(),

                // Strategy information
                strategy_id: strategy.id.clone(),
                strategy_name: strategy.name.clone(),

                // Signal details
                side,
                recommended_price: price,
                position_size: strategy.entry_rules.position_size,
                order_type: strategy.entry_rules.order_type.clone(),
                limit_price_offset: strategy.entry_rules.limit_price_offset,

                // Timing
                generated_at: now,
                time_to_event_hours,

                // Context
                market_volume: market.volume,
                market_open_interest: market.open_interest,
            })
            .collect()
    }

    /// Determine which side(s) to trade based on entry rules
    ///
    /// Converts semantic entry rules (cheaper/expensive/both) into concrete
    /// trading actions (buy Yes or No).
    ///
    /// # Arguments
    ///
    /// * `market` - The market with current prices
    /// * `entry_side` - The strategy's entry side configuration
    ///
    /// # Returns
    ///
    /// A vector of (SignalSide, price) tuples:
    /// - CheaperSide: 1 tuple for cheaper side
    /// - ExpensiveSide: 1 tuple for expensive side
    /// - Both: 2 tuples (Yes and No)
    fn determine_side(market: &Market, entry_side: &EntrySide) -> Vec<(SignalSide, Decimal)> {
        match entry_side {
            EntrySide::CheaperSide => {
                // Buy the cheaper side
                if market.yes_price < market.no_price {
                    vec![(SignalSide::Yes, market.yes_price)]
                } else {
                    vec![(SignalSide::No, market.no_price)]
                }
            }
            EntrySide::ExpensiveSide => {
                // Buy the expensive side
                if market.yes_price > market.no_price {
                    vec![(SignalSide::Yes, market.yes_price)]
                } else {
                    vec![(SignalSide::No, market.no_price)]
                }
            }
            EntrySide::Both => {
                // Buy both sides (volatility hedge)
                vec![
                    (SignalSide::Yes, market.yes_price),
                    (SignalSide::No, market.no_price),
                ]
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
    use crate::models::strategy::{EntryRules, ExitRules, RiskLimits, StrategyFilters, OrderType as StrategyOrderType};
    use crate::models::{MarketCategory, MarketStatus, Strategy, StrategyId};
    use chrono::{Duration, Utc};
    use rust_decimal_macros::dec;

    fn create_test_market(yes_price: Decimal, no_price: Decimal) -> Market {
        Market {
            id: MarketId::new("TEST-MARKET-001".to_string()),
            ticker: "TEST-MARKET".to_string(),
            title: "Test Market Title".to_string(),
            category: MarketCategory::Sports,
            sub_category: Some("Test".to_string()),
            status: MarketStatus::Active,
            yes_price,
            no_price,
            volume: 1000,
            open_interest: 500,
            event_time: Utc::now() + Duration::hours(24),
            close_time: Utc::now() + Duration::hours(23),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_strategy(side: EntrySide, position_size: u64) -> Strategy {
        Strategy {
            id: StrategyId::new("test-strategy-id".to_string()),
            name: "Test Strategy".to_string(),
            description: "Test strategy for signals".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            filters: StrategyFilters {
                categories: None,
                exclude_categories: None,
                min_price: None,
                max_price: None,
                min_volume: None,
                min_open_interest: None,
                min_time_to_event_hours: None,
                max_time_to_event_hours: None,
            },
            entry_rules: EntryRules {
                side,
                position_size,
                order_type: StrategyOrderType::Market,
                limit_price_offset: None,
            },
            exit_rules: ExitRules {
                take_profit_pct: Some(dec!(50.0)),
                stop_loss_pct: Some(dec!(30.0)),
                trailing_stop_pct: None,
                max_hold_time_hours: Some(24),
                exit_order_type: StrategyOrderType::Market,
            },
            risk_limits: RiskLimits {
                max_concurrent_positions: 5,
                max_daily_loss_usd: Some(dec!(100.00)),
                max_position_loss_usd: None,
                loss_cooldown_minutes: None,
            },
        }
    }

    #[test]
    fn test_signal_generation_cheaper_side_yes_is_cheaper() {
        let market = create_test_market(dec!(0.30), dec!(0.70));
        let strategy = create_test_strategy(EntrySide::CheaperSide, 100);

        let signals = EntrySignal::from_market(&market, &strategy);

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].side, SignalSide::Yes);
        assert_eq!(signals[0].recommended_price, dec!(0.30));
        assert_eq!(signals[0].position_size, 100);
    }

    #[test]
    fn test_signal_generation_cheaper_side_no_is_cheaper() {
        let market = create_test_market(dec!(0.70), dec!(0.30));
        let strategy = create_test_strategy(EntrySide::CheaperSide, 100);

        let signals = EntrySignal::from_market(&market, &strategy);

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].side, SignalSide::No);
        assert_eq!(signals[0].recommended_price, dec!(0.30));
    }

    #[test]
    fn test_signal_generation_expensive_side_yes_is_expensive() {
        let market = create_test_market(dec!(0.80), dec!(0.20));
        let strategy = create_test_strategy(EntrySide::ExpensiveSide, 50);

        let signals = EntrySignal::from_market(&market, &strategy);

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].side, SignalSide::Yes);
        assert_eq!(signals[0].recommended_price, dec!(0.80));
        assert_eq!(signals[0].position_size, 50);
    }

    #[test]
    fn test_signal_generation_expensive_side_no_is_expensive() {
        let market = create_test_market(dec!(0.25), dec!(0.75));
        let strategy = create_test_strategy(EntrySide::ExpensiveSide, 50);

        let signals = EntrySignal::from_market(&market, &strategy);

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].side, SignalSide::No);
        assert_eq!(signals[0].recommended_price, dec!(0.75));
    }

    #[test]
    fn test_signal_generation_both_sides() {
        let market = create_test_market(dec!(0.45), dec!(0.55));
        let strategy = create_test_strategy(EntrySide::Both, 75);

        let signals = EntrySignal::from_market(&market, &strategy);

        assert_eq!(signals.len(), 2);

        // Check Yes side signal
        assert_eq!(signals[0].side, SignalSide::Yes);
        assert_eq!(signals[0].recommended_price, dec!(0.45));
        assert_eq!(signals[0].position_size, 75);

        // Check No side signal
        assert_eq!(signals[1].side, SignalSide::No);
        assert_eq!(signals[1].recommended_price, dec!(0.55));
        assert_eq!(signals[1].position_size, 75);
    }

    #[test]
    fn test_signal_contains_market_data() {
        let market = create_test_market(dec!(0.30), dec!(0.70));
        let strategy = create_test_strategy(EntrySide::CheaperSide, 100);

        let signals = EntrySignal::from_market(&market, &strategy);
        let signal = &signals[0];

        assert_eq!(signal.market_id, market.id);
        assert_eq!(signal.market_ticker, "TEST-MARKET");
        assert_eq!(signal.market_title, "Test Market Title");
        assert_eq!(signal.market_volume, 1000);
        assert_eq!(signal.market_open_interest, 500);
    }

    #[test]
    fn test_signal_contains_strategy_data() {
        let market = create_test_market(dec!(0.30), dec!(0.70));
        let strategy = create_test_strategy(EntrySide::CheaperSide, 100);

        let signals = EntrySignal::from_market(&market, &strategy);
        let signal = &signals[0];

        assert_eq!(signal.strategy_id.as_str(), "test-strategy-id");
        assert_eq!(signal.strategy_name, "Test Strategy");
    }

    #[test]
    fn test_signal_time_to_event_calculation() {
        let mut market = create_test_market(dec!(0.30), dec!(0.70));
        market.event_time = Utc::now() + Duration::hours(12);
        let strategy = create_test_strategy(EntrySide::CheaperSide, 100);

        let signals = EntrySignal::from_market(&market, &strategy);
        let signal = &signals[0];

        // Time to event should be approximately 12 hours
        // Allow some tolerance for execution time
        assert!(signal.time_to_event_hours >= 11.9);
        assert!(signal.time_to_event_hours <= 12.1);
    }

    #[test]
    fn test_signal_timestamp_is_recent() {
        let market = create_test_market(dec!(0.30), dec!(0.70));
        let strategy = create_test_strategy(EntrySide::CheaperSide, 100);

        let before = Utc::now();
        let signals = EntrySignal::from_market(&market, &strategy);
        let after = Utc::now();

        let signal = &signals[0];
        assert!(signal.generated_at >= before);
        assert!(signal.generated_at <= after);
    }

    #[test]
    fn test_determine_side_cheaper_side() {
        let market = create_test_market(dec!(0.20), dec!(0.80));

        let result = EntrySignal::determine_side(&market, &EntrySide::CheaperSide);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, SignalSide::Yes);
        assert_eq!(result[0].1, dec!(0.20));
    }

    #[test]
    fn test_determine_side_expensive_side() {
        let market = create_test_market(dec!(0.15), dec!(0.85));

        let result = EntrySignal::determine_side(&market, &EntrySide::ExpensiveSide);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, SignalSide::No);
        assert_eq!(result[0].1, dec!(0.85));
    }

    #[test]
    fn test_determine_side_both() {
        let market = create_test_market(dec!(0.42), dec!(0.58));

        let result = EntrySignal::determine_side(&market, &EntrySide::Both);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, SignalSide::Yes);
        assert_eq!(result[0].1, dec!(0.42));
        assert_eq!(result[1].0, SignalSide::No);
        assert_eq!(result[1].1, dec!(0.58));
    }

    #[test]
    fn test_signal_with_limit_order() {
        let market = create_test_market(dec!(0.30), dec!(0.70));
        let mut strategy = create_test_strategy(EntrySide::CheaperSide, 100);
        strategy.entry_rules.order_type = StrategyOrderType::Limit;
        strategy.entry_rules.limit_price_offset = Some(dec!(-0.01));

        let signals = EntrySignal::from_market(&market, &strategy);
        let signal = &signals[0];

        assert_eq!(signal.order_type, StrategyOrderType::Limit);
        assert_eq!(signal.limit_price_offset, Some(dec!(-0.01)));
    }
}
