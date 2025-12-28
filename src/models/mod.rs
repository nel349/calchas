// Models module - Core data structures for Calchas
// See TECHNICAL_ARCHITECTURE.md Section 4 for design details

pub mod market;
pub mod strategy;
pub mod order;
pub mod position;
pub mod trade;

// Re-export commonly used types
pub use market::{Market, MarketId, MarketCategory, MarketStatus, Orderbook, OrderbookLevel};
pub use strategy::{Strategy, StrategyId, EntrySide, StrategyFilters, EntryRules, ExitRules, RiskLimits};
pub use order::{Order, OrderId, OrderSide, OrderAction, OrderType, OrderStatus};
pub use position::{Position, PositionId, PositionSide, PositionStatus, ExitTarget};
pub use trade::{Trade, TradeId, ExitReason};
