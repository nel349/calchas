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
//! let signals = StrategyEvaluator::evaluate(&markets, &strategy)?;
//! println!("Generated {} signals", signals.len());
//! # Ok(())
//! # }
//! ```

use chrono::Utc;
use rust_decimal::Decimal;

use crate::models::strategy::{EntrySide, StrategyFilters};
use crate::models::{Market, MarketCategory, Strategy, StrategyId};
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
    /// let signals = StrategyEvaluator::evaluate(&markets, &strategy)?;
    /// println!("Found {} signals", signals.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn evaluate(
        markets: &[Market],
        strategy: &Strategy,
    ) -> Result<Vec<EntrySignal>, EvaluationError> {
        // Check if strategy is active
        if !strategy.is_active() {
            return Err(EvaluationError::StrategyDisabled(strategy.id.clone()));
        }

        // Validate strategy configuration
        Self::validate_strategy(strategy)?;

        // Filter markets and generate signals
        let signals: Vec<EntrySignal> = markets
            .iter()
            .filter(|market| Self::matches_filters(market, &strategy.filters, &strategy.entry_rules.side))
            .flat_map(|market| EntrySignal::from_market(market, strategy))
            .collect();

        Ok(signals)
    }

    /// Evaluate all markets against all strategies
    ///
    /// # Arguments
    ///
    /// * `markets` - Slice of markets to evaluate
    /// * `strategies` - Slice of strategies to evaluate against
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<EntrySignal>)` - All signals generated from all strategies
    /// * `Err(EvaluationError)` - A strategy is disabled or invalid
    pub fn evaluate_all(
        markets: &[Market],
        strategies: &[Strategy],
    ) -> Result<Vec<EntrySignal>, EvaluationError> {
        let mut all_signals = Vec::new();

        for strategy in strategies {
            let signals = Self::evaluate(markets, strategy)?;
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
    ///
    /// # Returns
    ///
    /// `true` if the market passes all filters, `false` otherwise
    pub fn matches_filters(
        market: &Market,
        filters: &StrategyFilters,
        entry_side: &EntrySide,
    ) -> bool {
        // Must pass ALL filter checks
        Self::matches_category(market, &filters.categories, &filters.exclude_categories)
            && Self::matches_price(market, filters.min_price, filters.max_price, entry_side)
            && Self::matches_volume(market, filters.min_volume)
            && Self::matches_open_interest(market, filters.min_open_interest)
            && Self::matches_time_to_event(
                market,
                filters.min_time_to_event_hours,
                filters.max_time_to_event_hours,
            )
    }

    // =========================================================================
    // PRIVATE FILTER FUNCTIONS
    // =========================================================================

    /// Check if market's category matches include/exclude filters
    ///
    /// Logic:
    /// - If exclude list exists and market is in it: false
    /// - If include list exists and market is NOT in it: false
    /// - Otherwise: true
    fn matches_category(
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
    /// - Both: at least one side must be in range
    fn matches_price(
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
            EntrySide::CheaperSide => vec![market.cheaper_side_price()],
            EntrySide::ExpensiveSide => vec![market.expensive_side_price()],
            EntrySide::Both => vec![market.yes_price, market.no_price],
        };

        // At least one price must be in range
        prices_to_check.iter().any(|price| {
            let above_min = min_price.map_or(true, |min| *price >= min);
            let below_max = max_price.map_or(true, |max| *price <= max);
            above_min && below_max
        })
    }

    /// Check if market has sufficient volume
    fn matches_volume(market: &Market, min_volume: Option<u64>) -> bool {
        match min_volume {
            None => true,
            Some(min) => market.volume >= min,
        }
    }

    /// Check if market has sufficient open interest
    fn matches_open_interest(market: &Market, min_oi: Option<u64>) -> bool {
        match min_oi {
            None => true,
            Some(min) => market.open_interest >= min,
        }
    }

    /// Check if market's event is within the acceptable time window
    ///
    /// Calculates hours until event and checks if it's within [min, max] bounds.
    /// Markets with past events (negative duration) are rejected.
    fn matches_time_to_event(
        market: &Market,
        min_hours: Option<u32>,
        max_hours: Option<u32>,
    ) -> bool {
        let now = Utc::now();
        let time_to_event = market.event_time.signed_duration_since(now);
        let hours = time_to_event.num_seconds() as f64 / 3600.0;

        // Reject past events (negative hours)
        if hours < 0.0 {
            return false;
        }

        // Check min bound
        if let Some(min) = min_hours {
            if hours < min as f64 {
                return false;
            }
        }

        // Check max bound
        if let Some(max) = max_hours {
            if hours > max as f64 {
                return false;
            }
        }

        true
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
            filters.min_time_to_event_hours,
            filters.max_time_to_event_hours,
        ) {
            if min > max {
                return Err(EvaluationError::InvalidStrategy(
                    "min_time_to_event_hours cannot be greater than max_time_to_event_hours"
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
            category,
            sub_category: None,
            status: crate::models::MarketStatus::Open,
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
                order_type: crate::models::strategy::OrderType::Market,
                limit_price_offset: None,
            },
            exit_rules: ExitRules {
                take_profit_pct: Some(dec!(50.0)),
                stop_loss_pct: Some(dec!(30.0)),
                trailing_stop_pct: None,
                max_hold_time_hours: Some(24),
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

        // Yes side (0.15) is in range [0.10, 0.20]
        assert!(StrategyEvaluator::matches_price(
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
        market.event_time = Utc::now() + Duration::hours(12);

        assert!(StrategyEvaluator::matches_time_to_event(
            &market,
            Some(2),
            Some(24)
        ));
    }

    #[test]
    fn test_matches_time_to_event_too_soon() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_time = Utc::now() + Duration::hours(1);

        assert!(!StrategyEvaluator::matches_time_to_event(
            &market,
            Some(2),
            Some(24)
        ));
    }

    #[test]
    fn test_matches_time_to_event_too_late() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_time = Utc::now() + Duration::hours(48);

        assert!(!StrategyEvaluator::matches_time_to_event(
            &market,
            Some(2),
            Some(24)
        ));
    }

    #[test]
    fn test_matches_time_to_event_past_event() {
        let mut market = create_test_market(MarketCategory::Sports, dec!(0.50), dec!(0.50));
        market.event_time = Utc::now() - Duration::hours(1);

        assert!(!StrategyEvaluator::matches_time_to_event(
            &market,
            Some(2),
            Some(24)
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
            min_price: Some(dec!(0.80)),
            max_price: Some(dec!(0.20)), // min > max
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_hours: None,
            max_time_to_event_hours: None,
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
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_hours: Some(48),
            max_time_to_event_hours: Some(2), // min > max
        };

        let strategy = create_test_strategy(filters, EntrySide::CheaperSide);

        let result = StrategyEvaluator::validate_strategy(&strategy);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_time_to_event_hours cannot be greater"));
    }

    // =========================================================================
    // EVALUATE INTEGRATION TESTS
    // =========================================================================

    #[test]
    fn test_evaluate_single_match() {
        let filters = StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            min_price: Some(dec!(0.10)),
            max_price: Some(dec!(0.20)),
            min_volume: Some(500),
            min_open_interest: None,
            min_time_to_event_hours: Some(2),
            max_time_to_event_hours: Some(48),
        };

        let strategy = create_test_strategy(filters, EntrySide::CheaperSide);
        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        let signals = StrategyEvaluator::evaluate(&[market], &strategy).unwrap();
        assert_eq!(signals.len(), 1);
    }

    #[test]
    fn test_evaluate_no_matches() {
        let filters = StrategyFilters {
            categories: Some(vec![MarketCategory::Politics]),
            exclude_categories: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_hours: None,
            max_time_to_event_hours: None,
        };

        let strategy = create_test_strategy(filters, EntrySide::CheaperSide);
        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        let signals = StrategyEvaluator::evaluate(&[market], &strategy).unwrap();
        assert_eq!(signals.len(), 0);
    }

    #[test]
    fn test_evaluate_disabled_strategy() {
        let filters = StrategyFilters {
            categories: None,
            exclude_categories: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_hours: None,
            max_time_to_event_hours: None,
        };

        let mut strategy = create_test_strategy(filters, EntrySide::CheaperSide);
        strategy.enabled = false;

        let market = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));

        let result = StrategyEvaluator::evaluate(&[market], &strategy);
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
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_hours: None,
            max_time_to_event_hours: None,
        };

        let strategy = create_test_strategy(filters, EntrySide::Both);
        let market = create_test_market(MarketCategory::Sports, dec!(0.45), dec!(0.55));

        let signals = StrategyEvaluator::evaluate(&[market], &strategy).unwrap();
        assert_eq!(signals.len(), 2); // Both sides generate signals
    }

    #[test]
    fn test_evaluate_all_multiple_strategies() {
        let filters1 = StrategyFilters {
            categories: Some(vec![MarketCategory::Sports]),
            exclude_categories: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_hours: None,
            max_time_to_event_hours: None,
        };

        let filters2 = StrategyFilters {
            categories: Some(vec![MarketCategory::Politics]),
            exclude_categories: None,
            min_price: None,
            max_price: None,
            min_volume: None,
            min_open_interest: None,
            min_time_to_event_hours: None,
            max_time_to_event_hours: None,
        };

        let strategy1 = create_test_strategy(filters1, EntrySide::CheaperSide);
        let strategy2 = create_test_strategy(filters2, EntrySide::CheaperSide);

        let market1 = create_test_market(MarketCategory::Sports, dec!(0.15), dec!(0.85));
        let market2 = create_test_market(MarketCategory::Politics, dec!(0.20), dec!(0.80));

        let signals =
            StrategyEvaluator::evaluate_all(&[market1, market2], &[strategy1, strategy2])
                .unwrap();

        assert_eq!(signals.len(), 2); // One signal per strategy
    }
}
