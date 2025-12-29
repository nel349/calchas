//! Arbitrage opportunity model
//!
//! Represents a detected arbitrage opportunity with all relevant data
//! for execution and profit tracking.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::MarketId;

/// Type of arbitrage opportunity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArbitrageType {
    /// Cross-market arbitrage (YES + NO < $1.00)
    CrossMarket,

    /// Correlated market arbitrage (statistical edge) - Phase 2
    Correlated,

    /// Temporal constraint arbitrage (logical impossibility) - Phase 2
    Temporal,
}

/// Arbitrage opportunity detected in the market
///
/// Represents a guaranteed or statistical profit opportunity
/// from price inefficiencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Unique identifier for this opportunity
    pub id: String,

    /// Type of arbitrage
    pub arbitrage_type: ArbitrageType,

    /// Market where arbitrage exists
    pub market_id: MarketId,

    /// Market title for display
    pub market_title: String,

    /// YES side ask price (price to BUY YES)
    pub yes_ask: Decimal,

    /// NO side ask price (price to BUY NO)
    pub no_ask: Decimal,

    /// Quantity available (minimum of YES and NO liquidity)
    pub quantity: u64,

    /// Total cost to execute both sides (YES + NO)
    pub total_cost: Decimal,

    /// Expected profit at settlement
    pub expected_profit: Decimal,

    /// Expected profit percentage
    pub profit_pct: Decimal,

    /// Time to settlement (for capital velocity calculation)
    pub time_to_settlement: chrono::Duration,

    /// When this opportunity was detected
    pub detected_at: DateTime<Utc>,

    /// Settlement timestamp (when market resolves)
    pub settlement_time: DateTime<Utc>,
}

impl ArbitrageOpportunity {
    /// Create a new cross-market arbitrage opportunity
    ///
    /// # Arguments
    ///
    /// * `market_id` - Market identifier
    /// * `market_title` - Human-readable market title
    /// * `yes_ask` - YES side ask price
    /// * `no_ask` - NO side ask price
    /// * `quantity` - Available liquidity (contracts)
    /// * `settlement_time` - When market settles
    pub fn new_cross_market(
        market_id: MarketId,
        market_title: String,
        yes_ask: Decimal,
        no_ask: Decimal,
        quantity: u64,
        settlement_time: DateTime<Utc>,
    ) -> Self {
        let detected_at = Utc::now();
        let total_cost = yes_ask + no_ask;
        let expected_profit = Decimal::ONE - total_cost;
        let profit_pct = if total_cost > Decimal::ZERO {
            expected_profit / total_cost
        } else {
            Decimal::ZERO
        };

        let time_to_settlement = settlement_time.signed_duration_since(detected_at);

        // Generate unique ID from market + timestamp
        let id = format!(
            "arb_{}_{}",
            market_id.as_str(),
            detected_at.timestamp_millis()
        );

        ArbitrageOpportunity {
            id,
            arbitrage_type: ArbitrageType::CrossMarket,
            market_id,
            market_title,
            yes_ask,
            no_ask,
            quantity,
            total_cost,
            expected_profit,
            profit_pct,
            time_to_settlement,
            detected_at,
            settlement_time,
        }
    }

    /// Calculate annual ROI based on time to settlement
    ///
    /// Annualized return helps compare opportunities with different
    /// settlement times.
    ///
    /// # Returns
    ///
    /// Annualized ROI as a percentage (e.g., 1.80 = 180%)
    pub fn annualized_roi(&self) -> Decimal {
        let days_to_settlement = self.time_to_settlement.num_days();

        if days_to_settlement <= 0 {
            // Settling today or already past - use daily ROI
            return self.profit_pct * Decimal::from(365);
        }

        // Annualize: (profit% / days) * 365
        (self.profit_pct / Decimal::from(days_to_settlement)) * Decimal::from(365)
    }

    /// Calculate expected profit in USD for a given position size
    ///
    /// # Arguments
    ///
    /// * `capital` - Total capital to deploy (USD)
    ///
    /// # Returns
    ///
    /// Expected profit in USD
    pub fn expected_profit_usd(&self, capital: Decimal) -> Decimal {
        capital * self.profit_pct
    }

    /// Check if opportunity meets minimum profit threshold
    ///
    /// # Arguments
    ///
    /// * `min_profit_pct` - Minimum profit percentage (e.g., 0.03 = 3%)
    ///
    /// # Returns
    ///
    /// True if profit meets or exceeds threshold
    pub fn meets_threshold(&self, min_profit_pct: Decimal) -> bool {
        self.profit_pct >= min_profit_pct
    }

    /// Check if opportunity has sufficient liquidity
    ///
    /// # Arguments
    ///
    /// * `min_quantity` - Minimum required contracts
    ///
    /// # Returns
    ///
    /// True if quantity meets or exceeds minimum
    pub fn has_liquidity(&self, min_quantity: u64) -> bool {
        self.quantity >= min_quantity
    }

    /// Check if settlement is far enough away
    ///
    /// Avoids execution risk on markets settling soon.
    ///
    /// # Arguments
    ///
    /// * `min_hours` - Minimum hours to settlement
    ///
    /// # Returns
    ///
    /// True if settlement is far enough away
    pub fn has_time(&self, min_hours: i64) -> bool {
        self.time_to_settlement.num_hours() >= min_hours
    }

    /// Calculate capital required for this opportunity
    ///
    /// # Arguments
    ///
    /// * `contracts` - Number of contracts to trade
    ///
    /// # Returns
    ///
    /// Total capital required (USD)
    pub fn capital_required(&self, contracts: u64) -> Decimal {
        self.total_cost * Decimal::from(contracts)
    }
}

// =============================================================================
// DISPLAY FORMATTING
// =============================================================================

impl std::fmt::Display for ArbitrageOpportunity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} | YES: ${:.2} NO: ${:.2} | Total: ${:.2} | Profit: {:.1}% | Qty: {} | Settles in {}d",
            self.market_id.as_str(),
            self.yes_ask,
            self.no_ask,
            self.total_cost,
            self.profit_pct * Decimal::from(100),
            self.quantity,
            self.time_to_settlement.num_days(),
        )
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_cross_market_opportunity_creation() {
        let market_id = MarketId::new("KXBTC-110K".to_string());
        let settlement = Utc::now() + chrono::Duration::days(30);

        let opp = ArbitrageOpportunity::new_cross_market(
            market_id.clone(),
            "Will Bitcoin hit $110K?".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        assert_eq!(opp.arbitrage_type, ArbitrageType::CrossMarket);
        assert_eq!(opp.market_id, market_id);
        assert_eq!(opp.yes_ask, dec!(0.48));
        assert_eq!(opp.no_ask, dec!(0.47));
        assert_eq!(opp.total_cost, dec!(0.95));
        assert_eq!(opp.expected_profit, dec!(0.05));
        assert_eq!(opp.quantity, 100);
    }

    #[test]
    fn test_profit_percentage_calculation() {
        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test Market".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        // Expected profit: (1.00 - 0.95) / 0.95 = 0.0526... ≈ 5.26%
        assert!(opp.profit_pct > dec!(0.052));
        assert!(opp.profit_pct < dec!(0.053));
    }

    #[test]
    fn test_annualized_roi() {
        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test Market".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        // 5.26% profit in 30 days
        // Annualized: (0.0526 / 30) * 365 = 0.64 (64%)
        let annualized = opp.annualized_roi();
        assert!(annualized > dec!(0.60));
        assert!(annualized < dec!(0.70));
    }

    #[test]
    fn test_expected_profit_usd() {
        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test Market".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        // With $100 capital, expect ~$5.26 profit
        let profit = opp.expected_profit_usd(dec!(100.00));
        assert!(profit > dec!(5.20));
        assert!(profit < dec!(5.30));
    }

    #[test]
    fn test_meets_threshold() {
        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test Market".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        assert!(opp.meets_threshold(dec!(0.03))); // 3% threshold - PASS
        assert!(opp.meets_threshold(dec!(0.05))); // 5% threshold - PASS
        assert!(!opp.meets_threshold(dec!(0.06))); // 6% threshold - FAIL
    }

    #[test]
    fn test_has_liquidity() {
        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test Market".to_string(),
            dec!(0.48),
            dec!(0.47),
            75, // Only 75 contracts available
            settlement,
        );

        assert!(opp.has_liquidity(50)); // PASS
        assert!(opp.has_liquidity(75)); // PASS
        assert!(!opp.has_liquidity(100)); // FAIL
    }

    #[test]
    fn test_has_time() {
        let settlement = Utc::now() + chrono::Duration::hours(48);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test Market".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        assert!(opp.has_time(24)); // Need 24 hours - PASS (has ~48)
        assert!(opp.has_time(47)); // Need 47 hours - PASS (has ~48)
        assert!(!opp.has_time(72)); // Need 72 hours - FAIL (only has ~48)
    }

    #[test]
    fn test_capital_required() {
        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("TEST".to_string()),
            "Test Market".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        // 100 contracts at $0.95 total cost = $95
        assert_eq!(opp.capital_required(100), dec!(95.00));

        // 50 contracts at $0.95 total cost = $47.50
        assert_eq!(opp.capital_required(50), dec!(47.50));
    }

    #[test]
    fn test_display_formatting() {
        let settlement = Utc::now() + chrono::Duration::days(30);
        let opp = ArbitrageOpportunity::new_cross_market(
            MarketId::new("KXBTC-110K".to_string()),
            "Will Bitcoin hit $110K?".to_string(),
            dec!(0.48),
            dec!(0.47),
            100,
            settlement,
        );

        let display = format!("{}", opp);
        assert!(display.contains("KXBTC-110K"));
        assert!(display.contains("YES: $0.48"));
        assert!(display.contains("NO: $0.47"));
        assert!(display.contains("Total: $0.95"));
        assert!(display.contains("Profit: 5."));
        assert!(display.contains("Qty: 100"));
    }
}
