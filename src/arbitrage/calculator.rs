//! Arbitrage profit calculator and opportunity filter
//!
//! Handles profit calculations, risk assessment, and opportunity filtering
//! based on user-defined thresholds.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::arbitrage::ArbitrageOpportunity;

/// Configuration for arbitrage opportunity filtering
#[derive(Debug, Clone)]
pub struct ArbitrageConfig {
    /// Minimum profit percentage to consider (e.g., 0.03 = 3%)
    ///
    /// Should cover fees (~3% round-trip) + buffer
    pub min_profit_pct: Decimal,

    /// Maximum profit percentage (e.g., 0.50 = 50%)
    ///
    /// Filters out illiquid markets with unrealistic spreads
    pub max_profit_pct: Decimal,

    /// Minimum total cost (YES ask + NO ask)
    ///
    /// Filters out dead/abandoned markets with penny bids
    pub min_total_cost: Decimal,

    /// Minimum quantity available (contracts)
    ///
    /// Ensures sufficient liquidity for execution
    pub min_quantity: u64,

    /// Minimum hours to settlement
    ///
    /// Avoids execution risk on markets settling soon
    pub min_hours_to_settlement: i64,

    /// Maximum total cost per opportunity
    ///
    /// Controls capital deployment per trade
    pub max_capital_per_trade: Decimal,
}

impl Default for ArbitrageConfig {
    fn default() -> Self {
        ArbitrageConfig {
            // Covers 3% fees + 0.5% buffer
            min_profit_pct: Decimal::new(35, 3), // 0.035 = 3.5%

            // Filter out illiquid markets
            max_profit_pct: Decimal::new(50, 2), // 0.50 = 50%

            // Filter out dead markets with penny bids
            min_total_cost: Decimal::new(30, 2), // 0.30 = $0.30

            // Enough for meaningful trade size
            min_quantity: 50,

            // Avoid markets settling within 24 hours
            min_hours_to_settlement: 24,

            // Limit risk per trade (for $500 capital, this allows ~5 concurrent)
            max_capital_per_trade: Decimal::new(100, 0),
        }
    }
}

impl ArbitrageConfig {
    /// Create config for small capital ($500)
    pub fn for_small_capital() -> Self {
        ArbitrageConfig {
            min_profit_pct: Decimal::new(40, 3), // 4% - be selective
            max_profit_pct: Decimal::new(50, 2), // 50% max
            min_total_cost: Decimal::new(30, 2), // $0.30 min
            min_quantity: 40, // Can trade smaller sizes
            min_hours_to_settlement: 12, // More flexible on timing
            max_capital_per_trade: Decimal::new(75, 0), // $75 max
        }
    }

    /// Create config for medium capital ($1000-3000)
    pub fn for_medium_capital() -> Self {
        ArbitrageConfig {
            min_profit_pct: Decimal::new(30, 3), // 3% - standard
            max_profit_pct: Decimal::new(50, 2), // 50% max
            min_total_cost: Decimal::new(30, 2), // $0.30 min
            min_quantity: 50,
            min_hours_to_settlement: 24,
            max_capital_per_trade: Decimal::new(150, 0), // $150 max
        }
    }

    /// Create config for large capital ($3000+)
    pub fn for_large_capital() -> Self {
        ArbitrageConfig {
            min_profit_pct: Decimal::new(25, 3), // 2.5% - take more opportunities
            max_profit_pct: Decimal::new(50, 2), // 50% max
            min_total_cost: Decimal::new(30, 2), // $0.30 min
            min_quantity: 75,
            min_hours_to_settlement: 48, // Prefer longer-term stability
            max_capital_per_trade: Decimal::new(300, 0), // $300 max
        }
    }
}

/// Arbitrage opportunity calculator and filter
#[derive(Clone)]
pub struct ArbitrageCalculator {
    pub config: ArbitrageConfig,
}

impl ArbitrageCalculator {
    /// Create new calculator with configuration
    pub fn new(config: ArbitrageConfig) -> Self {
        ArbitrageCalculator { config }
    }

    /// Create calculator with default configuration
    pub fn with_defaults() -> Self {
        ArbitrageCalculator {
            config: ArbitrageConfig::default(),
        }
    }

    /// Filter opportunity based on configuration thresholds
    ///
    /// Checks:
    /// - Profit meets minimum threshold
    /// - Liquidity is sufficient
    /// - Time to settlement is adequate
    /// - Capital required is within limits
    ///
    /// # Arguments
    ///
    /// * `opportunity` - Arbitrage opportunity to evaluate
    ///
    /// # Returns
    ///
    /// True if opportunity passes all filters
    pub fn passes_filters(&self, opportunity: &ArbitrageOpportunity) -> bool {
        // Check minimum profit threshold
        if !opportunity.meets_threshold(self.config.min_profit_pct) {
            return false;
        }

        // Check maximum profit (filter illiquid markets)
        if opportunity.profit_pct > self.config.max_profit_pct {
            return false;
        }

        // Check minimum total cost (filter dead markets)
        if opportunity.total_cost < self.config.min_total_cost {
            return false;
        }

        // Check liquidity
        if !opportunity.has_liquidity(self.config.min_quantity) {
            return false;
        }

        // Check time to settlement
        if !opportunity.has_time(self.config.min_hours_to_settlement) {
            return false;
        }

        // Check capital required (assuming we trade max available quantity)
        let capital_needed = opportunity.capital_required(opportunity.quantity);
        if capital_needed > self.config.max_capital_per_trade {
            return false;
        }

        true
    }

    /// Calculate optimal position size based on available capital
    ///
    /// Determines how many contracts to trade given capital constraints.
    ///
    /// # Arguments
    ///
    /// * `opportunity` - Arbitrage opportunity
    /// * `available_capital` - Total capital available (USD)
    ///
    /// # Returns
    ///
    /// Number of contracts to trade
    pub fn optimal_position_size(
        &self,
        opportunity: &ArbitrageOpportunity,
        available_capital: Decimal,
    ) -> u64 {
        // Calculate max contracts we can afford
        let max_affordable = if opportunity.total_cost > Decimal::ZERO {
            (available_capital / opportunity.total_cost).floor().to_u64().unwrap_or(0)
        } else {
            0
        };

        // Take minimum of:
        // 1. Available liquidity
        // 2. What we can afford
        // 3. Max capital per trade limit
        let max_from_capital_limit = if opportunity.total_cost > Decimal::ZERO {
            (self.config.max_capital_per_trade / opportunity.total_cost)
                .floor()
                .to_u64()
                .unwrap_or(0)
        } else {
            0
        };

        std::cmp::min(
            opportunity.quantity,
            std::cmp::min(max_affordable, max_from_capital_limit),
        )
    }

    /// Rank opportunities by expected value
    ///
    /// Considers both profit percentage and capital velocity (time to settlement).
    /// Faster settlements = better capital efficiency.
    ///
    /// # Arguments
    ///
    /// * `opportunities` - List of opportunities to rank
    ///
    /// # Returns
    ///
    /// Sorted vector (best opportunities first)
    pub fn rank_opportunities(
        &self,
        mut opportunities: Vec<ArbitrageOpportunity>,
    ) -> Vec<ArbitrageOpportunity> {
        opportunities.sort_by(|a, b| {
            // Primary sort: Annualized ROI (higher is better)
            let a_roi = a.annualized_roi();
            let b_roi = b.annualized_roi();

            // If ROI is similar (within 10%), prefer higher absolute profit
            if (a_roi - b_roi).abs() < Decimal::new(10, 2) {
                // Within 0.10 (10%)
                b.profit_pct.partial_cmp(&a.profit_pct).unwrap()
            } else {
                b_roi.partial_cmp(&a_roi).unwrap()
            }
        });

        opportunities
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MarketId;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_passes_filters_all_pass() {
        let config = ArbitrageConfig {
            min_profit_pct: dec!(0.03), // 3%
            max_profit_pct: dec!(0.50), // 50%
            min_total_cost: dec!(0.30), // $0.30
            min_quantity: 50,
            min_hours_to_settlement: 24,
            max_capital_per_trade: dec!(100.00),
        };
        let calculator = ArbitrageCalculator::new(config);

        let settlement = Utc::now() + chrono::Duration::hours(48);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test".to_string(),
            dec!(0.48),
            dec!(0.47), // 5.26% profit, total $0.95
            75,         // qty
            settlement,
        );

        assert!(calculator.passes_filters(&opp));
    }

    #[test]
    fn test_passes_filters_profit_too_low() {
        let config = ArbitrageConfig {
            min_profit_pct: dec!(0.06), // 6% required
            max_profit_pct: dec!(0.50), // 50%
            min_total_cost: dec!(0.30), // $0.30
            min_quantity: 50,
            min_hours_to_settlement: 24,
            max_capital_per_trade: dec!(100.00),
        };
        let calculator = ArbitrageCalculator::new(config);

        let settlement = Utc::now() + chrono::Duration::hours(48);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test".to_string(),
            dec!(0.48),
            dec!(0.47), // Only 5.26% profit
            75,
            settlement,
        );

        assert!(!calculator.passes_filters(&opp));
    }

    #[test]
    fn test_passes_filters_insufficient_liquidity() {
        let config = ArbitrageConfig {
            min_profit_pct: dec!(0.03),
            max_profit_pct: dec!(0.50),
            min_total_cost: dec!(0.30),
            min_quantity: 100, // Need 100 contracts
            min_hours_to_settlement: 24,
            max_capital_per_trade: dec!(100.00),
        };
        let calculator = ArbitrageCalculator::new(config);

        let settlement = Utc::now() + chrono::Duration::hours(48);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test".to_string(),
            dec!(0.48),
            dec!(0.47),
            75, // Only 75 available
            settlement,
        );

        assert!(!calculator.passes_filters(&opp));
    }

    #[test]
    fn test_passes_filters_settlement_too_soon() {
        let config = ArbitrageConfig {
            min_profit_pct: dec!(0.03),
            max_profit_pct: dec!(0.50),
            min_total_cost: dec!(0.30),
            min_quantity: 50,
            min_hours_to_settlement: 48, // Need 48 hours
            max_capital_per_trade: dec!(100.00),
        };
        let calculator = ArbitrageCalculator::new(config);

        let settlement = Utc::now() + chrono::Duration::hours(24); // Only 24 hours
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test".to_string(),
            dec!(0.48),
            dec!(0.47),
            75,
            settlement,
        );

        assert!(!calculator.passes_filters(&opp));
    }

    #[test]
    fn test_optimal_position_size_unlimited_capital() {
        let calculator = ArbitrageCalculator::with_defaults();

        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test".to_string(),
            dec!(0.48),
            dec!(0.47),
            100, // 100 available
            settlement,
        );

        // With huge capital, should be limited by config max_capital_per_trade ($100)
        let size = calculator.optimal_position_size(&opp, dec!(10000.00));

        // $100 max capital / $0.95 per contract = 105 contracts
        // But limited by availability (100) and max trade size
        // Config default is $100, so: 100 / 0.95 = 105, min with 100 = 100
        assert!(size <= 100);
        assert!(size >= 100); // Should use full liquidity
    }

    #[test]
    fn test_optimal_position_size_limited_capital() {
        let calculator = ArbitrageCalculator::with_defaults();

        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        // With $50 capital, can only afford: $50 / $0.95 = 52 contracts
        let size = calculator.optimal_position_size(&opp, dec!(50.00));
        assert_eq!(size, 52);
    }

    #[test]
    fn test_rank_opportunities_by_roi() {
        let calculator = ArbitrageCalculator::with_defaults();

        // Opportunity A: 5% profit, 30 days to settlement
        let opp_a = ArbitrageOpportunity::new_cross_market(
            MarketId::new("A".to_string()),
            "A".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            Utc::now() + chrono::Duration::days(30),
        );

        // Opportunity B: 3% profit, 7 days to settlement (better annualized ROI)
        let opp_b = ArbitrageOpportunity::new_cross_market(
            MarketId::new("B".to_string()),
            "B".to_string(),
            dec!(0.49),
            dec!(0.48),
            100,
            Utc::now() + chrono::Duration::days(7),
        );

        let opportunities = vec![opp_a.clone(), opp_b.clone()];
        let ranked = calculator.rank_opportunities(opportunities);

        // B should rank higher (better annualized ROI due to faster settlement)
        assert_eq!(ranked[0].market_id.as_str(), "B");
        assert_eq!(ranked[1].market_id.as_str(), "A");
    }

    #[test]
    fn test_config_presets() {
        let small = ArbitrageConfig::for_small_capital();
        assert_eq!(small.min_profit_pct, dec!(0.040)); // 4%
        assert_eq!(small.max_capital_per_trade, dec!(75));

        let medium = ArbitrageConfig::for_medium_capital();
        assert_eq!(medium.min_profit_pct, dec!(0.030)); // 3%
        assert_eq!(medium.max_capital_per_trade, dec!(150));

        let large = ArbitrageConfig::for_large_capital();
        assert_eq!(large.min_profit_pct, dec!(0.025)); // 2.5%
        assert_eq!(large.max_capital_per_trade, dec!(300));
    }
}
