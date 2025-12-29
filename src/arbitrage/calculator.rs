//! Arbitrage profit calculator and opportunity filter
//!
//! Handles profit calculations, risk assessment, and opportunity filtering
//! based on user-defined thresholds.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::arbitrage::ArbitrageOpportunity;
use crate::models::Orderbook;

/// Configuration for arbitrage opportunity filtering
#[derive(Debug, Clone)]
pub struct ArbitrageConfig {
    /// Minimum profit percentage to consider (e.g., 0.03 = 3%)
    ///
    /// Should cover fees (~3% round-trip) + buffer
    pub min_profit_pct: Decimal,

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
            min_quantity: 40, // Can trade smaller sizes
            min_hours_to_settlement: 12, // More flexible on timing
            max_capital_per_trade: Decimal::new(75, 0), // $75 max
        }
    }

    /// Create config for medium capital ($1000-3000)
    pub fn for_medium_capital() -> Self {
        ArbitrageConfig {
            min_profit_pct: Decimal::new(30, 3), // 3% - standard
            min_quantity: 50,
            min_hours_to_settlement: 24,
            max_capital_per_trade: Decimal::new(150, 0), // $150 max
        }
    }

    /// Create config for large capital ($3000+)
    pub fn for_large_capital() -> Self {
        ArbitrageConfig {
            min_profit_pct: Decimal::new(25, 3), // 2.5% - take more opportunities
            min_quantity: 75,
            min_hours_to_settlement: 48, // Prefer longer-term stability
            max_capital_per_trade: Decimal::new(300, 0), // $300 max
        }
    }
}

/// Arbitrage opportunity calculator and filter
pub struct ArbitrageCalculator {
    config: ArbitrageConfig,
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

    /// Check if an orderbook presents a cross-market arbitrage opportunity
    ///
    /// Returns true if YES ask + NO ask < (1.00 - fees)
    ///
    /// # Arguments
    ///
    /// * `orderbook` - Market orderbook data
    ///
    /// # Returns
    ///
    /// True if arbitrage opportunity exists
    pub fn has_cross_market_arbitrage(&self, orderbook: &Orderbook) -> bool {
        let yes_ask = match orderbook.yes_best_ask() {
            Some(price) => price,
            None => return false,
        };

        let no_ask = match orderbook.no_best_ask() {
            Some(price) => price,
            None => return false,
        };

        let total_cost = yes_ask + no_ask;

        // Arbitrage exists if total cost < (1.00 - min profit threshold)
        // Example: If min profit is 3%, then total cost must be < 0.97
        total_cost < (Decimal::ONE - self.config.min_profit_pct)
    }

    /// Calculate actual profit percentage from orderbook
    ///
    /// # Arguments
    ///
    /// * `orderbook` - Market orderbook data
    ///
    /// # Returns
    ///
    /// Profit percentage (e.g., 0.053 = 5.3%), or None if no arbitrage
    pub fn calculate_profit_pct(&self, orderbook: &Orderbook) -> Option<Decimal> {
        let yes_ask = orderbook.yes_best_ask()?;
        let no_ask = orderbook.no_best_ask()?;

        let total_cost = yes_ask + no_ask;

        if total_cost >= Decimal::ONE {
            return None; // No arbitrage
        }

        let profit = Decimal::ONE - total_cost;
        let profit_pct = profit / total_cost;

        Some(profit_pct)
    }

    /// Get available quantity for arbitrage (minimum of YES and NO liquidity)
    ///
    /// # Arguments
    ///
    /// * `orderbook` - Market orderbook data
    ///
    /// # Returns
    ///
    /// Maximum quantity that can be traded (limited by smaller side)
    pub fn available_quantity(&self, orderbook: &Orderbook) -> u64 {
        let yes_qty = orderbook.yes_best_ask_quantity();
        let no_qty = orderbook.no_best_ask_quantity();

        std::cmp::min(yes_qty, no_qty)
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
        // Check profit threshold
        if !opportunity.meets_threshold(self.config.min_profit_pct) {
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
    use crate::models::{MarketId, OrderbookLevel};
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn create_test_orderbook(yes_ask: Decimal, no_ask: Decimal, qty: u64) -> Orderbook {
        Orderbook {
            market_id: MarketId::new("TEST".to_string()),
            yes_asks: vec![OrderbookLevel {
                price: yes_ask,
                quantity: qty,
            }],
            no_asks: vec![OrderbookLevel {
                price: no_ask,
                quantity: qty,
            }],
        }
    }

    #[test]
    fn test_has_cross_market_arbitrage_true() {
        let calculator = ArbitrageCalculator::with_defaults();
        let orderbook = create_test_orderbook(dec!(0.48), dec!(0.47), 100);

        // YES 0.48 + NO 0.47 = 0.95 < 0.965 (1.00 - 3.5% threshold)
        assert!(calculator.has_cross_market_arbitrage(&orderbook));
    }

    #[test]
    fn test_has_cross_market_arbitrage_false() {
        let calculator = ArbitrageCalculator::with_defaults();
        let orderbook = create_test_orderbook(dec!(0.50), dec!(0.49), 100);

        // YES 0.50 + NO 0.49 = 0.99 > 0.965 (1.00 - 3.5% threshold)
        assert!(!calculator.has_cross_market_arbitrage(&orderbook));
    }

    #[test]
    fn test_calculate_profit_pct() {
        let calculator = ArbitrageCalculator::with_defaults();
        let orderbook = create_test_orderbook(dec!(0.48), dec!(0.47), 100);

        let profit_pct = calculator.calculate_profit_pct(&orderbook).unwrap();

        // (1.00 - 0.95) / 0.95 = 0.0526... ≈ 5.26%
        assert!(profit_pct > dec!(0.052));
        assert!(profit_pct < dec!(0.053));
    }

    #[test]
    fn test_available_quantity() {
        let calculator = ArbitrageCalculator::with_defaults();

        // Equal quantity on both sides
        let orderbook1 = create_test_orderbook(dec!(0.48), dec!(0.47), 100);
        assert_eq!(calculator.available_quantity(&orderbook1), 100);

        // Different quantities - should return minimum
        let orderbook2 = Orderbook {
            market_id: MarketId::new("TEST".to_string()),
            yes_asks: vec![OrderbookLevel {
                price: dec!(0.48),
                quantity: 75,
            }],
            no_asks: vec![OrderbookLevel {
                price: dec!(0.47),
                quantity: 100,
            }],
        };
        assert_eq!(calculator.available_quantity(&orderbook2), 75);
    }

    #[test]
    fn test_passes_filters_all_pass() {
        let config = ArbitrageConfig {
            min_profit_pct: dec!(0.03), // 3%
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
            dec!(0.47), // 5.26% profit
            75,         // qty
            settlement,
        );

        assert!(calculator.passes_filters(&opp));
    }

    #[test]
    fn test_passes_filters_profit_too_low() {
        let config = ArbitrageConfig {
            min_profit_pct: dec!(0.06), // 6% required
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
