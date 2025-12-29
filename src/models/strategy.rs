// Strategy data model
// Section 4.2 of TECHNICAL_ARCHITECTURE.md

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::market::MarketCategory;

// =============================================================================
// NEWTYPE PATTERN - StrategyId
// =============================================================================

/// Unique strategy identifier (newtype pattern)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StrategyId(pub String);

impl StrategyId {
    pub fn new(id: String) -> Self {
        StrategyId(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// =============================================================================
// ENUMS
// =============================================================================

/// Which side of the market to enter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntrySide {
    Yes,            // Always buy Yes side
    No,             // Always buy No side
    CheaperSide,    // Buy cheaper side only (< 50¢)
    ExpensiveSide,  // Buy expensive side only (> 50¢)
    Both,           // Trade both sides (volatility hedge)
}

/// Order type for entry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,  // Immediate execution (taker fee)
    Limit,   // Price-based execution (maker fee)
}

/// Position sizing unit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSizeUnit {
    Contracts,  // Fixed number of contracts
    Dollars,    // Dollar amount (including fees)
}

// =============================================================================
// FILTER STRUCTS
// =============================================================================

/// Market selection criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyFilters {
    // Category filters
    pub categories: Option<Vec<MarketCategory>>,
    pub exclude_categories: Option<Vec<MarketCategory>>,

    // Price range filters (in cents, 0-100)
    pub min_price: Option<Decimal>,
    pub max_price: Option<Decimal>,

    // Liquidity filters
    pub min_volume: Option<u64>,
    pub min_open_interest: Option<u64>,

    // Timing filters (in minutes)
    pub min_time_to_event_minutes: Option<u32>,
    pub max_time_to_event_minutes: Option<u32>,

    // Momentum filters
    /// Minimum price movement percentage required in lookback period
    /// Example: 2.0 = price must have moved at least 2% in last hour
    pub min_momentum_pct: Option<Decimal>,

    /// Lookback period for momentum calculation (in minutes)
    /// Example: 60 = check price change over last hour
    pub momentum_lookback_minutes: Option<u32>,

    // Orderbook filters (checked before entry execution)
    /// Maximum spread (in cents) to accept
    /// Example: 0.05 = reject if spread > 5 cents
    pub max_spread_cents: Option<Decimal>,

    /// Minimum liquidity at best price (number of contracts)
    /// Example: 50 = reject if fewer than 50 contracts available
    pub min_best_price_quantity: Option<u64>,
}

// =============================================================================
// ENTRY RULES
// =============================================================================

/// Position entry logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryRules {
    // Side selection
    pub side: EntrySide,

    // Position sizing
    pub position_size: u64,  // Number of contracts OR dollar amount
    pub position_size_unit: PositionSizeUnit,  // Contracts or Dollars

    // Order type
    pub order_type: OrderType,

    // Optional limit price offset (for limit orders)
    // e.g., -0.01 means bid 1¢ below current price
    pub limit_price_offset: Option<Decimal>,
}

// =============================================================================
// EXIT RULES
// =============================================================================

/// Position exit criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitRules {
    // Profit target (percentage)
    pub take_profit_pct: Option<Decimal>,

    // Loss limit (percentage)
    pub stop_loss_pct: Option<Decimal>,

    // Trailing stop distance (percentage from peak)
    pub trailing_stop_pct: Option<Decimal>,

    // Trailing stop activation threshold (percentage profit before trailing starts)
    pub trailing_stop_activation_pct: Option<Decimal>,

    // Maximum hold time (minutes)
    pub max_hold_time_minutes: Option<u32>,

    // Exit order type
    pub exit_order_type: OrderType,
}

// =============================================================================
// RISK LIMITS
// =============================================================================

/// Risk management constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    // Position limits
    pub max_concurrent_positions: u32,

    // Loss limits
    pub max_daily_loss_usd: Option<Decimal>,
    pub max_position_loss_usd: Option<Decimal>,

    // Cooldown after loss (minutes)
    pub loss_cooldown_minutes: Option<u32>,
}

// =============================================================================
// MAIN STRUCT
// =============================================================================

/// Trading strategy loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    // Metadata
    pub id: StrategyId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,

    // Strategy configuration
    pub filters: StrategyFilters,
    pub entry_rules: EntryRules,
    pub exit_rules: ExitRules,
    pub risk_limits: RiskLimits,
}

impl Strategy {
    /// Check if strategy is active and ready to trade
    pub fn is_active(&self) -> bool {
        self.enabled
    }

    /// Get position size for entry
    pub fn get_position_size(&self) -> u64 {
        self.entry_rules.position_size
    }

    /// Check if we should enter on the cheaper side
    pub fn trades_cheaper_side(&self) -> bool {
        matches!(
            self.entry_rules.side,
            EntrySide::CheaperSide | EntrySide::Both
        )
    }

    /// Check if we should enter on the expensive side
    pub fn trades_expensive_side(&self) -> bool {
        matches!(
            self.entry_rules.side,
            EntrySide::ExpensiveSide | EntrySide::Both
        )
    }

    /// Check if we should always enter on Yes side
    pub fn trades_yes_side(&self) -> bool {
        matches!(
            self.entry_rules.side,
            EntrySide::Yes | EntrySide::Both
        )
    }

    /// Check if we should always enter on No side
    pub fn trades_no_side(&self) -> bool {
        matches!(
            self.entry_rules.side,
            EntrySide::No | EntrySide::Both
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

    fn create_test_strategy() -> Strategy {
        Strategy {
            id: StrategyId::new("underdog-hunter-v1".to_string()),
            name: "Underdog Hunter".to_string(),
            description: "Buy cheap underdogs in sports markets".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            filters: StrategyFilters {
                categories: Some(vec![MarketCategory::Sports]),
                exclude_categories: None,
                min_price: Some(dec!(0.05)),
                max_price: Some(dec!(0.20)),
                min_volume: Some(1000),
                min_open_interest: None,
                min_time_to_event_minutes: Some(120),
                max_time_to_event_minutes: Some(2880),
                min_momentum_pct: None,
                momentum_lookback_minutes: None,
                max_spread_cents: None,
                min_best_price_quantity: None,
            },
            entry_rules: EntryRules {
                side: EntrySide::CheaperSide,
                position_size: 100,
                position_size_unit: PositionSizeUnit::Contracts,
                order_type: OrderType::Limit,
                limit_price_offset: Some(dec!(-0.01)),
            },
            exit_rules: ExitRules {
                take_profit_pct: Some(dec!(50.0)),
                stop_loss_pct: Some(dec!(30.0)),
                trailing_stop_pct: None,
                trailing_stop_activation_pct: None,
                max_hold_time_minutes: Some(1440),
                exit_order_type: OrderType::Market,
            },
            risk_limits: RiskLimits {
                max_concurrent_positions: 5,
                max_daily_loss_usd: Some(dec!(100.00)),
                max_position_loss_usd: Some(dec!(50.00)),
                loss_cooldown_minutes: Some(60),
            },
        }
    }

    #[test]
    fn test_strategy_is_active() {
        let strategy = create_test_strategy();
        assert!(strategy.is_active());
    }

    #[test]
    fn test_get_position_size() {
        let strategy = create_test_strategy();
        assert_eq!(strategy.get_position_size(), 100);
    }

    #[test]
    fn test_trades_cheaper_side() {
        let strategy = create_test_strategy();
        assert!(strategy.trades_cheaper_side());
        assert!(!strategy.trades_expensive_side());
    }

    #[test]
    fn test_strategy_serialization() {
        let strategy = create_test_strategy();

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&strategy).unwrap();

        // Deserialize back
        let deserialized: Strategy = serde_json::from_str(&json).unwrap();

        assert_eq!(strategy.id, deserialized.id);
        assert_eq!(strategy.name, deserialized.name);
        assert_eq!(strategy.enabled, deserialized.enabled);
    }
}
