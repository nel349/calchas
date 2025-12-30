//! Market evaluation and filtering logic
//!
//! This module implements the core strategy engine functionality:
//! - Filtering markets by strategy criteria (category, price, volume, timing)
//! - Evaluating market matches against entry rules
//! - Orchestrating signal generation
//!
//! # Example
//!
//! ```no_run
//! use calchas::strategy::{StrategyLoader, StrategyEvaluator};
//! use calchas::models::Market;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let strategy = StrategyLoader::load("strategies/underdog_hunter.json")?;
//! let markets: Vec<Market> = vec![/* fetch from API */];
//!
//! let signals = StrategyEvaluator::evaluate(&markets, &strategy, None, None, None)?;
//! println!("Generated {} signals", signals.len());
//! # Ok(())
//! # }
//! ```

use chrono::{Utc, Duration};
use rust_decimal::Decimal;

use crate::models::strategy::{EntrySide, StrategyFilters};
use crate::models::{Market, MarketCategory, Strategy, StrategyId};
use crate::trading::PriceTracker;
use super::signals::EntrySignal;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur during market evaluation
#[derive(Debug)]
pub enum EvaluationError {
    /// Strategy is disabled
    StrategyDisabled(StrategyId),

    /// Invalid strategy configuration
    InvalidStrategy(String),
}

/// Result of momentum check (for detailed filtering stats)
#[derive(Debug, Clone, Copy)]
enum MomentumCheckResult {
    /// Momentum check passed
    Pass,
    /// No price data available (<2 snapshots)
    NoData,
    /// Has data but movement is insufficient
    InsufficientMovement,
}

/// Result of volume spike check (for detailed filtering stats)
#[derive(Debug, Clone, Copy)]
enum VolumeCheckResult {
    /// Volume spike check passed
    Pass,
    /// No volume data available (<3 snapshots)
    NoData,
    /// Has data but spike is insufficient
    InsufficientSpike,
}

/// Result of order flow imbalance check (for detailed filtering stats)
#[derive(Debug, Clone, Copy)]
enum OrderFlowCheckResult {
    /// Order flow imbalance check passed
    Pass,
    /// No order flow data available
    NoData,
    /// Has data but imbalance is insufficient
    InsufficientImbalance,
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvaluationError::StrategyDisabled(id) => {
                write!(f, "Strategy {} is disabled", id.as_str())
            }
            EvaluationError::InvalidStrategy(msg) => {
                write!(f, "Invalid strategy: {}", msg)
            }
        }
    }
}

impl std::error::Error for EvaluationError {}

// =============================================================================
// STRATEGY EVALUATOR
// =============================================================================

/// Strategy evaluator for filtering markets and generating signals
///
/// The evaluator filters markets against strategy criteria and generates
/// entry signals for matches. This is a stateless utility struct.
pub struct StrategyEvaluator;

impl StrategyEvaluator {
    /// Evaluate markets against a single strategy
    ///
    /// Filters the provided markets using the strategy's filter criteria,
    /// then generates entry signals for each matching market.
    ///
    /// # Arguments
    ///
    /// * `markets` - Slice of markets to evaluate
    /// * `strategy` - Strategy to evaluate against
    /// * `price_tracker` - Price tracker for momentum analysis (optional)
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<EntrySignal>)` - All signals generated (may be empty if no matches)
    /// * `Err(EvaluationError)` - Strategy is disabled or invalid
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use calchas::strategy::{StrategyLoader, StrategyEvaluator};
    /// # use calchas::models::Market;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let strategy = StrategyLoader::load("strategies/underdog_hunter.json")?;
    /// let markets: Vec<Market> = vec![/* ... */];
    ///
    /// let signals = StrategyEvaluator::evaluate(&markets, &strategy, None, None, None)?;
    /// println!("Found {} signals", signals.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn evaluate(
        markets: &[Market],
        strategy: &Strategy,
        price_tracker: Option<&PriceTracker>,
        volume_tracker: Option<&crate::trading::VolumeTracker>,
        order_flow_tracker: Option<&crate::trading::OrderFlowTracker>,
    ) -> Result<Vec<EntrySignal>, EvaluationError> {
        // Check if strategy is active
        if !strategy.is_active() {
            return Err(EvaluationError::StrategyDisabled(strategy.id.clone()));
        }

        // Validate strategy configuration
        Self::validate_strategy(strategy)?;

        // Track filter rejections for debugging
        let mut rejected_category = 0;
        let mut rejected_price = 0;
        let mut rejected_volume = 0;
        let mut rejected_open_interest = 0;
        let mut rejected_time_window = 0;
        let mut rejected_momentum = 0;
        let mut rejected_momentum_no_data = 0;
        let mut rejected_momentum_insufficient = 0;
        let mut rejected_volume_spike = 0;
        let mut rejected_volume_spike_no_data = 0;
        let mut rejected_volume_spike_insufficient = 0;
        let mut rejected_order_flow = 0;
        let mut rejected_order_flow_no_data = 0;
        let mut rejected_order_flow_insufficient = 0;
        let total_markets = markets.len();

        // Filter markets and generate signals
        let signals: Vec<EntrySignal> = markets
            .iter()
            .filter(|market| {
                // Check each filter individually for diagnostics
                if !Self::matches_category(market, &strategy.filters.categories, &strategy.filters.exclude_categories) {
                    rejected_category += 1;
                    return false;
                }
                if !Self::matches_price(market, strategy.filters.min_price, strategy.filters.max_price, &strategy.entry_rules.side) {
                    rejected_price += 1;
                    return false;
                }
                if !Self::matches_volume(market, strategy.filters.min_volume) {
                    rejected_volume += 1;
                    return false;
                }
                if !Self::matches_open_interest(market, strategy.filters.min_open_interest) {
                    rejected_open_interest += 1;
                    return false;
                }
                if !Self::matches_time_to_event(
                    market,
                    strategy.filters.min_time_to_event_minutes,
                    strategy.filters.max_time_to_event_minutes,
                ) {
                    rejected_time_window += 1;
                    return false;
                }
                let momentum_result = Self::check_momentum_detailed(
                    market,
                    strategy.filters.min_momentum_pct,
                    strategy.filters.momentum_lookback_minutes,
                    price_tracker,
                );
                match momentum_result {
                    MomentumCheckResult::Pass => {},
                    MomentumCheckResult::NoData => {
                        rejected_momentum += 1;
                        rejected_momentum_no_data += 1;
                        return false;
                    },
                    MomentumCheckResult::InsufficientMovement => {
                        rejected_momentum += 1;
                        rejected_momentum_insufficient += 1;
                        return false;
                    },
                }

                // Check volume spike filter (Phase 1)
                let volume_spike_result = Self::check_volume_spike_detailed(
                    market,
                    strategy.filters.min_volume_spike_pct,
                    strategy.filters.volume_spike_lookback_minutes,
                    volume_tracker,
                );
                match volume_spike_result {
                    VolumeCheckResult::Pass => {},
                    VolumeCheckResult::NoData => {
                        rejected_volume_spike += 1;
                        rejected_volume_spike_no_data += 1;
                        return false;
                    },
                    VolumeCheckResult::InsufficientSpike => {
                        rejected_volume_spike += 1;
                        rejected_volume_spike_insufficient += 1;
                        return false;
                    },
                }

                // Check order flow imbalance filter (Phase 2)
                let order_flow_result = Self::check_order_flow_detailed(
                    market,
                    strategy.filters.min_order_flow_imbalance,
                    order_flow_tracker,
                );
                match order_flow_result {
                    OrderFlowCheckResult::Pass => {},
                    OrderFlowCheckResult::NoData => {
                        rejected_order_flow += 1;
                        rejected_order_flow_no_data += 1;
                        return false;
                    },
                    OrderFlowCheckResult::InsufficientImbalance => {
                        rejected_order_flow += 1;
                        rejected_order_flow_insufficient += 1;
                        return false;
                    },
                }

                true
            })
            .flat_map(|market| EntrySignal::from_market(market, strategy))
            .collect();

        // Log filter rejection breakdown if no signals generated
        if signals.is_empty() && total_markets > 0 {
            tracing::warn!(
                "Filter breakdown for {} ({} markets): category={}, price={}, volume={}, open_interest={}, time_window={}, momentum={} (no_data={}, insufficient={}), volume_spike={} (no_data={}, insufficient={}), order_flow={} (no_data={}, insufficient={})",
                strategy.name,
                total_markets,
                rejected_category,
                rejected_price,
                rejected_volume,
                rejected_open_interest,
                rejected_time_window,
                rejected_momentum,
                rejected_momentum_no_data,
                rejected_momentum_insufficient,
                rejected_volume_spike,
                rejected_volume_spike_no_data,
                rejected_volume_spike_insufficient,
                rejected_order_flow,
                rejected_order_flow_no_data,
                rejected_order_flow_insufficient
            );
        }

        Ok(signals)
    }

    /// Evaluate all markets against all strategies
    ///
    /// # Arguments
    ///
    /// * `markets` - Slice of markets to evaluate
    /// * `strategies` - Slice of strategies to evaluate against
    /// * `price_tracker` - Price tracker for momentum analysis (optional)
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<EntrySignal>)` - All signals generated from all strategies
    /// * `Err(EvaluationError)` - A strategy is disabled or invalid
    pub fn evaluate_all(
        markets: &[Market],
        strategies: &[Strategy],
        price_tracker: Option<&PriceTracker>,
        volume_tracker: Option<&crate::trading::VolumeTracker>,
        order_flow_tracker: Option<&crate::trading::OrderFlowTracker>,
    ) -> Result<Vec<EntrySignal>, EvaluationError> {
        let mut all_signals = Vec::new();

        for strategy in strategies {
            let signals = Self::evaluate(markets, strategy, price_tracker, volume_tracker, order_flow_tracker)?;
            all_signals.extend(signals);
        }

        Ok(all_signals)
    }

    /// Check if a market matches strategy filters
    ///
    /// # Arguments
    ///
    /// * `market` - The market to check
    /// * `filters` - Strategy filters to match against
    /// * `entry_side` - Entry side configuration (needed for price filtering)
    /// * `price_tracker` - Price tracker for momentum checks (optional)
    ///
    /// # Returns
    ///
    /// `true` if the market passes all filters, `false` otherwise
    pub fn matches_filters(
        market: &Market,
        filters: &StrategyFilters,
        entry_side: &EntrySide,
        price_tracker: Option<&PriceTracker>,
        volume_tracker: Option<&crate::trading::VolumeTracker>,
        order_flow_tracker: Option<&crate::trading::OrderFlowTracker>,
    ) -> bool {
        // Must pass ALL filter checks
        Self::matches_category(market, &filters.categories, &filters.exclude_categories)
            && Self::matches_price(market, filters.min_price, filters.max_price, entry_side)
            && Self::matches_volume(market, filters.min_volume)
            && Self::matches_open_interest(market, filters.min_open_interest)
            && Self::matches_time_to_event(
                market,
                filters.min_time_to_event_minutes,
                filters.max_time_to_event_minutes,
            )
            && Self::matches_momentum(
                market,
                filters.min_momentum_pct,
                filters.momentum_lookback_minutes,
                price_tracker,
            )
            && Self::matches_volume_spike(
                market,
                filters.min_volume_spike_pct,
                filters.volume_spike_lookback_minutes,
                volume_tracker,
            )
            && Self::matches_order_flow(
                market,
                filters.min_order_flow_imbalance,
                order_flow_tracker,
            )
    }

    // =========================================================================
    // FILTER FUNCTIONS (public for debugging)
    // =========================================================================

    /// Check if market's category matches include/exclude filters
    ///
    /// Logic:
    /// - If exclude list exists and market is in it: false
    /// - If include list exists and market is NOT in it: false
    /// - Otherwise: true
    pub fn matches_category(
        market: &Market,
        categories: &Option<Vec<MarketCategory>>,
        exclude_categories: &Option<Vec<MarketCategory>>,
    ) -> bool {
        // Check exclude list first (if market is excluded, reject immediately)
        if let Some(excluded) = exclude_categories {
            if excluded.contains(&market.category) {
                return false;
            }
        }

        // Check include list (if specified, market must be in it)
        if let Some(included) = categories {
            if !included.contains(&market.category) {
                return false;
            }
        }

        // No filters or passed all checks
        true
    }

    /// Check if market's price is within the acceptable range
    ///
    /// The price check depends on the entry side:
    /// - CheaperSide: check cheaper side price
    /// - ExpensiveSide: check expensive side price
    /// - Both: BOTH sides must be in range (since we'll trade both)
    pub fn matches_price(
        market: &Market,
        min_price: Option<Decimal>,
        max_price: Option<Decimal>,
        entry_side: &EntrySide,
    ) -> bool {
        // If no price filters, pass
        if min_price.is_none() && max_price.is_none() {
            return true;
        }

        // Get the price(s) we care about based on entry side
        let prices_to_check: Vec<Decimal> = match entry_side {
            EntrySide::Yes => vec![market.yes_price],
            EntrySide::No => vec![market.no_price],
            EntrySide::CheaperSide => vec![market.cheaper_side_price()],
            EntrySide::ExpensiveSide => vec![market.expensive_side_price()],
            EntrySide::Both => vec![market.yes_price, market.no_price],
        };

        // For Both: ALL prices must be in range (we're trading both sides)
        // For CheaperSide/ExpensiveSide: the single price must be in range
        prices_to_check.iter().all(|price| {
            let above_min = min_price.map_or(true, |min| *price >= min);
            let below_max = max_price.map_or(true, |max| *price <= max);
            above_min && below_max
        })
    }

    /// Check if market has sufficient volume
    pub fn matches_volume(market: &Market, min_volume: Option<u64>) -> bool {
        match min_volume {
            None => true,
            Some(min) => market.volume >= min,
        }
    }

    /// Check if market has sufficient open interest
    pub fn matches_open_interest(market: &Market, min_oi: Option<u64>) -> bool {
        match min_oi {
            None => true,
            Some(min) => market.open_interest >= min,
        }
    }

    /// Check if market's close time is within the acceptable time window
    ///
    /// Calculates minutes until market closes and checks if it's within [min, max] bounds.
    /// Markets that are already closed (negative duration) are rejected.
    ///
    /// TIMING BUG FIX: Different market types use different timing fields:
    /// - Crypto markets (KXBTC, KXETH): use close_time (accurate to the minute)
    /// - Sports/Politics markets: use event_time (close_time is placeholder ~14 days out)
    pub fn matches_time_to_event(
        market: &Market,
        min_minutes: Option<u32>,
        max_minutes: Option<u32>,
    ) -> bool {
        let now = Utc::now();

        // Use close_time for crypto (accurate), event_time for sports (close_time is placeholder)
        let time_to_close = if market.is_crypto_market() {
            market.close_time.signed_duration_since(now)
        } else {
            market.event_time.signed_duration_since(now)
        };

        let minutes = time_to_close.num_seconds() as f64 / 60.0;

        // Reject markets that are already closed (negative minutes)
        if minutes < 0.0 {
            return false;
        }

        // Check min bound
        if let Some(min) = min_minutes {
            if minutes < min as f64 {
                return false;
            }
        }

        // Check max bound
        if let Some(max) = max_minutes {
            if minutes > max as f64 {
                return false;
            }
        }

        true
    }

    /// Check momentum with detailed result (for filtering stats)
    fn check_momentum_detailed(
        market: &Market,
        min_momentum_pct: Option<Decimal>,
        lookback_minutes: Option<u32>,
        price_tracker: Option<&PriceTracker>,
    ) -> MomentumCheckResult {
        // If no momentum filter configured, pass
        if min_momentum_pct.is_none() || lookback_minutes.is_none() {
            return MomentumCheckResult::Pass;
        }

        // If no price tracker provided, pass
        let tracker = match price_tracker {
            Some(t) => t,
            None => return MomentumCheckResult::Pass,
        };

        let min_pct = min_momentum_pct.unwrap();
        let lookback_mins = lookback_minutes.unwrap();
        let lookback_duration = Duration::minutes(lookback_mins as i64);

        // Calculate actual momentum
        let actual_momentum = tracker.calculate_momentum(&market.id, lookback_duration);

        match actual_momentum {
            None => MomentumCheckResult::NoData,
            Some(momentum_value) => {
                if momentum_value.abs() >= min_pct {
                    MomentumCheckResult::Pass
                } else {
                    MomentumCheckResult::InsufficientMovement
                }
            }
        }
    }

    /// Check if market has sufficient momentum (price movement)
    ///
    /// # Arguments
    ///
    /// * `market` - The market to check
    /// * `min_momentum_pct` - Minimum price change percentage required
    /// * `lookback_minutes` - How far back to look for price history
    /// * `price_tracker` - Price tracker with historical data
    ///
    /// # Returns
    ///
    /// `true` if momentum filter passes or is not enabled, `false` otherwise
    pub fn matches_momentum(
        market: &Market,
        min_momentum_pct: Option<Decimal>,
        lookback_minutes: Option<u32>,
        price_tracker: Option<&PriceTracker>,
    ) -> bool {
        matches!(
            Self::check_momentum_detailed(market, min_momentum_pct, lookback_minutes, price_tracker),
            MomentumCheckResult::Pass
        )
    }

    /// Internal volume spike check with detailed result (for filtering stats)
    fn check_volume_spike_detailed(
        market: &Market,
        min_spike_pct: Option<Decimal>,
        lookback_minutes: Option<u32>,
        volume_tracker: Option<&crate::trading::VolumeTracker>,
    ) -> VolumeCheckResult {
        // If no volume spike filter configured, pass
        if min_spike_pct.is_none() || lookback_minutes.is_none() {
            return VolumeCheckResult::Pass;
        }

        // If no volume tracker provided, pass
        let tracker = match volume_tracker {
            Some(t) => t,
            None => return VolumeCheckResult::Pass,
        };

        let min_pct = min_spike_pct.unwrap();
        let lookback_mins = lookback_minutes.unwrap();
        let lookback_duration = Duration::minutes(lookback_mins as i64);

        // Calculate actual volume spike
        let actual_spike = tracker.calculate_volume_spike(&market.id, lookback_duration);

        match actual_spike {
            None => VolumeCheckResult::NoData,
            Some(spike_value) => {
                if spike_value >= min_pct {
                    VolumeCheckResult::Pass
                } else {
                    VolumeCheckResult::InsufficientSpike
                }
            }
        }
    }

    /// Check if market has sufficient volume spike (sharp money detection)
    ///
    /// # Arguments
    ///
    /// * `market` - The market to check
    /// * `min_spike_pct` - Minimum volume spike percentage required
    /// * `lookback_minutes` - How far back to look for volume history
    /// * `volume_tracker` - Volume tracker with historical data
    ///
    /// # Returns
    ///
    /// `true` if volume spike filter passes or is not enabled, `false` otherwise
    pub fn matches_volume_spike(
        market: &Market,
        min_spike_pct: Option<Decimal>,
        lookback_minutes: Option<u32>,
        volume_tracker: Option<&crate::trading::VolumeTracker>,
    ) -> bool {
        matches!(
            Self::check_volume_spike_detailed(market, min_spike_pct, lookback_minutes, volume_tracker),
            VolumeCheckResult::Pass
        )
    }

    /// Internal order flow imbalance check with detailed result (for filtering stats)
    fn check_order_flow_detailed(
        market: &Market,
        min_imbalance: Option<Decimal>,
        order_flow_tracker: Option<&crate::trading::OrderFlowTracker>,
    ) -> OrderFlowCheckResult {
        // If no order flow filter configured, pass
        if min_imbalance.is_none() {
            return OrderFlowCheckResult::Pass;
        }

        // If no tracker provided, pass
        let tracker = match order_flow_tracker {
            Some(t) => t,
            None => return OrderFlowCheckResult::Pass,
        };

        let min_ofi = min_imbalance.unwrap();

        // Calculate actual order flow imbalance
        let actual_ofi = tracker.calculate_ofi(&market.id);

        match actual_ofi {
            None => OrderFlowCheckResult::NoData,
            Some(ofi_value) => {
                // Check absolute value (both +OFI and -OFI can be signals)
                if ofi_value.abs() >= min_ofi {
                    OrderFlowCheckResult::Pass
                } else {
                    OrderFlowCheckResult::InsufficientImbalance
                }
            }
        }
    }

    /// Check if market has sufficient order flow imbalance (buy/sell pressure)
    ///
    /// # Arguments
    ///
    /// * `market` - The market to check
    /// * `min_imbalance` - Minimum OFI required (absolute value)
    /// * `order_flow_tracker` - Order flow tracker with orderbook history
    ///
    /// # Returns
    ///
    /// `true` if order flow imbalance filter passes or is not enabled, `false` otherwise
    ///
    /// **Note:** This checks absolute value, so both bullish (+OFI) and bearish (-OFI)
    /// pressure signals are detected. The strategy can then decide which direction to trade.
    pub fn matches_order_flow(
        market: &Market,
        min_imbalance: Option<Decimal>,
        order_flow_tracker: Option<&crate::trading::OrderFlowTracker>,
    ) -> bool {
        matches!(
            Self::check_order_flow_detailed(market, min_imbalance, order_flow_tracker),
            OrderFlowCheckResult::Pass
        )
    }

    /// Validate strategy configuration for logical consistency
    fn validate_strategy(strategy: &Strategy) -> Result<(), EvaluationError> {
        let filters = &strategy.filters;

        // Check min_price <= max_price
        if let (Some(min), Some(max)) = (filters.min_price, filters.max_price) {
            if min > max {
                return Err(EvaluationError::InvalidStrategy(
                    "min_price cannot be greater than max_price".to_string(),
                ));
            }
        }

        // Check min_time <= max_time
        if let (Some(min), Some(max)) = (
            filters.min_time_to_event_minutes,
            filters.max_time_to_event_minutes,
        ) {
            if min > max {
                return Err(EvaluationError::InvalidStrategy(
                    "min_time_to_event_minutes cannot be greater than max_time_to_event_minutes"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::strategy::{EntryRules, ExitRules, RiskLimits};
    use crate::models::MarketId;
    use chrono::Duration;
    use rust_decimal_macros::dec;

    // =========================================================================
    // FACTORY FUNCTIONS
    // =========================================================================

    fn create_test_market(category: MarketCategory, yes_price: Decimal, no_price: Decimal) -> Market {
        Market {
            id: MarketId::new("TEST-001".to_string()),
            ticker: "TEST-MARKET".to_string(),
            title: "Test Market".to_string(),
            event_ticker: "TEST-EVENT".to_string(),
            category,
            sub_category: None,
            status: crate::models::MarketStatus::Active,
            yes_price,
            no_price,
            yes_bid: yes_price - dec!(0.01),
            yes_ask: yes_price + dec!(0.01),
            no_bid: no_price - dec!(0.01),
            no_ask: no_price + dec!(0.01),
            volume: 1000,
            open_interest: 500,
            event_time: Utc::now() + Duration::hours(24),
            close_time: Utc::now() + Duration::hours(23),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_strategy(filters: StrategyFilters, side: EntrySide) -> Strategy {
        Strategy {
            id: StrategyId::new("test-strategy".to_string()),
            name: "Test Strategy".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            filters,
            entry_rules: EntryRules {
                side,
                position_size: 100,
                position_size_unit: crate::models::strategy::PositionSizeUnit::Contracts,
                order_type: crate::models::strategy::OrderType::Market,
                limit_price_offset: None,
            },
            exit_rules: ExitRules {
                take_profit_pct: Some(dec!(50.0)),
                stop_loss_pct: Some(dec!(30.0)),
                trailing_stop_pct: None,
                trailing_stop_activation_pct: None,
                max_hold_time_minutes: Some(1440),
                exit_order_type: crate::models::strategy::OrderType::Market,
            },
            risk_limits: RiskLimits {
                max_concurrent_positions: 5,
                max_daily_loss_usd: Some(dec!(100.00)),
                max_position_loss_usd: None,
                loss_cooldown_minutes: None,
            },
        }
    }

    // =========================================================================
    // CATEGORY FILTER TESTS
    // =========================================================================

    #[test]
    fn test_matches_category_included() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        let categories = Some(vec![MarketCategory::Sports, MarketCategory::Politics]);

        assert!(StrategyEvaluator::matches_category(
            &market,
            &categories,
            &None
        ));
    }

    #[test]
    fn test_matches_category_not_included() {
        let market = create_test_market(MarketCategory::Weather, dec!(0.50), dec!(0.50));
        let categories = Some(vec![MarketCategory::Sports, MarketCategory::Politics]);

        assert!(!StrategyEvaluator::matches_category(
            &market,
            &categories,
            &None
        ));
    }

    #[test]
    fn test_matches_category_excluded() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        let exclude = Some(vec![MarketCategory::Sports]);

        assert!(!StrategyEvaluator::matches_category(
            &market,
            &None,
            &exclude
        ));
    }

    #[test]
    fn test_matches_category_no_filter() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));

        assert!(StrategyEvaluator::matches_category(&market, &None, &None));
    }

    #[test]
    fn test_matches_category_in_both_lists_excluded_wins() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        let categories = Some(vec![MarketCategory::Sports]);
        let exclude = Some(vec![MarketCategory::Sports]);

        assert!(!StrategyEvaluator::matches_category(
            &market,
            &categories,
            &exclude
        ));
    }

    // =========================================================================
    // PRICE FILTER TESTS
    // =========================================================================

    #[test]
    fn test_matches_price_underdog_in_range() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        assert!(StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::CheaperSide
        ));
    }

    #[test]
    fn test_matches_price_underdog_below_min() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.05), dec!(0.95));

        assert!(!StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::CheaperSide
        ));
    }

    #[test]
    fn test_matches_price_underdog_above_max() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.25), dec!(0.75));

        assert!(!StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::CheaperSide
        ));
    }

    #[test]
    fn test_matches_price_favorite_in_range() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.85), dec!(0.15));

        assert!(StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.80)),
            Some(dec!(0.90)),
            &EntrySide::ExpensiveSide
        ));
    }

    #[test]
    fn test_matches_price_both_sides_one_in_range() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        // Yes side (0.15) is in range [0.10, 0.20], but No side (0.85) is NOT
        // For Both: ALL sides must be in range
        assert!(!StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::Both
        ));
    }

    #[test]
    fn test_matches_price_both_sides_both_in_range() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.18));

        // Both Yes (0.15) and No (0.18) are in range [0.10, 0.20]
        assert!(StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::Both
        ));
    }

    #[test]
    fn test_matches_price_both_sides_neither_in_range() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.05), dec!(0.95));

        // Neither side is in range [0.10, 0.20]
        assert!(!StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::Both
        ));
    }

    #[test]
    fn test_matches_price_exactly_at_boundary() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.10), dec!(0.90));

        // Price exactly at min (inclusive)
        assert!(StrategyEvaluator::matches_price(
            &market,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::CheaperSide
        ));

        let market2 = create_test_market(MarketCategory::Sports, dec!(0.20), dec!(0.80));

        // Price exactly at max (inclusive)
        assert!(StrategyEvaluator::matches_price(
            &market2,
            Some(dec!(0.10)),
            Some(dec!(0.20)),
            &EntrySide::CheaperSide
        ));
    }

    #[test]
    fn test_matches_price_no_filter() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));

        assert!(StrategyEvaluator::matches_price(
            &market,
            None,
            None,
            &EntrySide::CheaperSide
        ));
    }

    // =========================================================================
    // VOLUME FILTER TESTS
    // =========================================================================

    #[test]
    fn test_matches_volume_above_threshold() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.volume = 2000;

        assert!(StrategyEvaluator::matches_volume(&market, Some(1000)));
    }

    #[test]
    fn test_matches_volume_below_threshold() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.volume = 500;

        assert!(!StrategyEvaluator::matches_volume(&market, Some(1000)));
    }

    #[test]
    fn test_matches_volume_exactly_at_threshold() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.volume = 1000;

        assert!(StrategyEvaluator::matches_volume(&market, Some(1000)));
    }

    #[test]
    fn test_matches_volume_no_filter() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));

        assert!(StrategyEvaluator::matches_volume(&market, None));
    }

    // =========================================================================
    // OPEN INTEREST FILTER TESTS
    // =========================================================================

    #[test]
    fn test_matches_open_interest_above_threshold() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.open_interest = 1000;

        assert!(StrategyEvaluator::matches_open_interest(
            &market,
            Some(500)
        ));
    }

    #[test]
    fn test_matches_open_interest_below_threshold() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.open_interest = 200;

        assert!(!StrategyEvaluator::matches_open_interest(
            &market,
            Some(500)
        ));
    }

    #[test]
    fn test_matches_open_interest_no_filter() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));

        assert!(StrategyEvaluator::matches_open_interest(&market, None));
    }

    // =========================================================================
    // TIME TO EVENT FILTER TESTS
    // =========================================================================

    #[test]
    fn test_matches_time_to_event_in_window() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.close_time = Utc::now() + Duration::hours(12);

        assert!(StrategyEvaluator::matches_time_to_event(
            &market,
            Some(120),  // 2 hours in minutes
            Some(1440)  // 24 hours in minutes
        ));
    }

    #[test]
    fn test_matches_time_to_event_too_soon() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_ticker = "KXBTC15M".to_string(); // Crypto market uses close_time
        market.close_time = Utc::now() + Duration::hours(1);

        assert!(!StrategyEvaluator::matches_time_to_event(
            &market,
            Some(120),  // 2 hours in minutes
            Some(1440)  // 24 hours in minutes
        ));
    }

    #[test]
    fn test_matches_time_to_event_too_late() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_ticker = "KXBTC15M".to_string(); // Crypto market uses close_time
        market.close_time = Utc::now() + Duration::hours(48);

        assert!(!StrategyEvaluator::matches_time_to_event(
            &market,
            Some(120),  // 2 hours in minutes
            Some(1440)  // 24 hours in minutes
        ));
    }

    #[test]
    fn test_matches_time_to_event_past_event() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_ticker = "KXBTC15M".to_string(); // Crypto market uses close_time
        market.close_time = Utc::now() - Duration::hours(1);

        assert!(!StrategyEvaluator::matches_time_to_event(
            &market,
            Some(120),  // 2 hours in minutes
            Some(1440)  // 24 hours in minutes
        ));
    }

    #[test]
    fn test_matches_time_to_event_no_filter() {
        let market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));

        assert!(StrategyEvaluator::matches_time_to_event(&market, None, None));
    }

    // =========================================================================
    // VALIDATION TESTS
    // =========================================================================

    #[test]
    fn test_validate_strategy_invalid_price_range() {
        let filters = StrategyFilters {
            categories: None,
            exclude_categories: None,
            series_ticker: None,
            min_price: Some(dec!(0.80)),
            max_price: Some(dec!(0.20)), // min > max
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let strategy = create_test_strategy(filters, EntrySide::CheaperSide);

        let result = StrategyEvaluator::validate_strategy(&strategy);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_price cannot be greater than max_price"));
    }

    #[test]
    fn test_validate_strategy_invalid_time_range() {
        let filters = StrategyFilters {
            categories: None,
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_minutes: Some(2880),
            max_time_to_event_minutes: Some(120), // min > max
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let strategy = create_test_strategy(filters, EntrySide::CheaperSide);

        let result = StrategyEvaluator::validate_strategy(&strategy);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_time_to_event_minutes cannot be greater"));
    }

    // =========================================================================
    // EVALUATE INTEGRATION TESTS
    // =========================================================================

    #[test]
    fn test_evaluate_single_match() {
        let filters = StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            series_ticker: None,
            min_price: Some(dec!(0.10)),
            max_price: Some(dec!(0.20)),
            min_volume: Some(500),
            min_open_interest: None,
            min_time_to_event_minutes: Some(120),
            max_time_to_event_minutes: Some(2880),
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let strategy = create_test_strategy(filters, EntrySide::CheaperSide);
        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        let signals = StrategyEvaluator::evaluate(&[market], &strategy, None, None, None).unwrap();
        assert_eq!(signals.len(), 1);
    }

    #[test]
    fn test_evaluate_no_matches() {
        let filters = StrategyFilters {
            categories: Some(vec![MarketCategory::Politics]),
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let strategy = create_test_strategy(filters, EntrySide::CheaperSide);
        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        let signals = StrategyEvaluator::evaluate(&[market], &strategy, None, None, None).unwrap();
        assert_eq!(signals.len(), 0);
    }

    #[test]
    fn test_evaluate_disabled_strategy() {
        let filters = StrategyFilters {
            categories: None,
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let mut strategy = create_test_strategy(filters, EntrySide::CheaperSide);
        strategy.enabled = false;

        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        let result = StrategyEvaluator::evaluate(&[market], &strategy, None, None, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EvaluationError::StrategyDisabled(_)
        ));
    }

    #[test]
    fn test_evaluate_both_sides_generates_two_signals() {
        let filters = StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let strategy = create_test_strategy(filters, EntrySide::Both);
        let market = create_test_market(MarketCategory::Sports, dec!(0.45), dec!(0.55));

        let signals = StrategyEvaluator::evaluate(&[market], &strategy, None, None, None).unwrap();
        assert_eq!(signals.len(), 2); // Both sides generate signals
    }

    #[test]
    fn test_evaluate_all_multiple_strategies() {
        let filters1 = StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let filters2 = StrategyFilters {
            categories: Some(vec![MarketCategory::Politics]),
            exclude_categories: None,
            series_ticker: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_minutes: None,
            max_time_to_event_minutes: None,
            min_momentum_pct: None,
            momentum_lookback_minutes: None,
            min_volume_spike_pct: None,
            volume_spike_lookback_minutes: None,
            min_order_flow_imbalance: None,
            max_spread_cents: None,
            min_best_price_quantity: None,
        };

        let strategy1 = create_test_strategy(filters1, EntrySide::CheaperSide);
        let strategy2 = create_test_strategy(filters2, EntrySide::CheaperSide);

        let market1 = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));
        let market2 = create_test_market(MarketCategory::Politics, dec!(0.20), dec!(0.80));

        let signals =
            StrategyEvaluator::evaluate_all(&[market1, market2], &[strategy1, strategy2], None, None, None)
                .unwrap();

        assert_eq!(signals.len(), 2); // One signal per strategy
    }

    // =========================================================================
    // TIMING BUG FIX TESTS
    // =========================================================================

    #[test]
    fn test_is_crypto_market_detection() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));

        // Crypto markets
        market.event_ticker = "KXBTC".to_string();
        assert!(market.is_crypto_market());
        market.event_ticker = "KXBTC15M".to_string();
        assert!(market.is_crypto_market());
        market.event_ticker = "KXBTC-25DEC2919".to_string();
        assert!(market.is_crypto_market());
        market.event_ticker = "KXETH".to_string();
        assert!(market.is_crypto_market());
        market.event_ticker = "KXETH1H".to_string();
        assert!(market.is_crypto_market());
        market.event_ticker = "KXETH-110K-2025".to_string();
        assert!(market.is_crypto_market());

        // Non-crypto markets
        market.event_ticker = "KXNBAGAME".to_string();
        assert!(!market.is_crypto_market());
        market.event_ticker = "KXNFLGAME".to_string();
        assert!(!market.is_crypto_market());
        market.event_ticker = "KXNCAAWBGAME".to_string();
        assert!(!market.is_crypto_market());
        market.event_ticker = "KXPOLITICS".to_string();
        assert!(!market.is_crypto_market());
        market.event_ticker = "TEST-EVENT".to_string();
        assert!(!market.is_crypto_market());
    }

    #[test]
    fn test_time_filter_uses_close_time_for_crypto() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_ticker = "KXBTC15M".to_string();
        market.close_time = Utc::now() + Duration::minutes(30); // Accurate for crypto
        market.event_time = Utc::now() + Duration::days(14);    // Placeholder

        // Should use close_time (30 minutes), not event_time (14 days)
        assert!(StrategyEvaluator::matches_time_to_event(&market, Some(20), Some(40)));
        assert!(!StrategyEvaluator::matches_time_to_event(&market, Some(50), Some(100)));
    }

    #[test]
    fn test_time_filter_uses_event_time_for_sports() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_ticker = "KXNBAGAME".to_string();
        market.event_time = Utc::now() + Duration::hours(2);   // Accurate (game end)
        market.close_time = Utc::now() + Duration::days(14);   // Placeholder

        // Should use event_time (120 minutes), not close_time (14 days)
        assert!(StrategyEvaluator::matches_time_to_event(&market, Some(100), Some(150)));
        assert!(!StrategyEvaluator::matches_time_to_event(&market, Some(200), Some(300)));
    }

    #[test]
    fn test_time_filter_rejects_past_events() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));

        // Crypto market with close_time in the past
        market.event_ticker = "KXBTC".to_string();
        market.close_time = Utc::now() - Duration::minutes(10);
        assert!(!StrategyEvaluator::matches_time_to_event(&market, None, None));

        // Sports market with event_time in the past
        market.event_ticker = "KXNFLGAME".to_string();
        market.event_time = Utc::now() - Duration::minutes(10);
        assert!(!StrategyEvaluator::matches_time_to_event(&market, None, None));
    }
}
