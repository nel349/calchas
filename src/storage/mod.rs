//! Database persistence layer
//!
//! Provides SQLite-backed storage for trades, positions, and metrics.
//! All database operations are synchronous (rusqlite is blocking).

use crate::models::{Position, PositionId, StrategyId, Trade};
use chrono::NaiveDate;
use rust_decimal::Decimal;

mod sqlite;
pub use sqlite::SqliteStore;

/// Result type for storage operations
pub type StorageResult<T> = Result<T, StorageError>;

/// Storage errors
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Trade persistence operations
pub trait TradeStore: Send + Sync {
    /// Insert a completed trade
    fn insert_trade(&self, trade: &Trade) -> StorageResult<()>;

    /// Get all trades for a strategy (ordered by exit time DESC)
    fn get_trades(
        &self,
        strategy_id: &StrategyId,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Trade>>;

    /// Get trades within date range
    fn get_trades_by_date_range(
        &self,
        strategy_id: &StrategyId,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> StorageResult<Vec<Trade>>;

    /// Count total trades for strategy
    fn count_trades(&self, strategy_id: &StrategyId) -> StorageResult<usize>;
}

/// Position persistence operations
pub trait PositionStore: Send + Sync {
    /// Insert or update position (upsert)
    fn save_position(&self, position: &Position) -> StorageResult<()>;

    /// Get all active positions
    fn get_active_positions(&self) -> StorageResult<Vec<Position>>;

    /// Delete position (when closed)
    fn delete_position(&self, position_id: &PositionId) -> StorageResult<()>;

    /// Delete all positions (for cleanup)
    fn delete_all_positions(&self) -> StorageResult<()>;
}

/// Daily stats persistence
pub trait DailyStatsStore: Send + Sync {
    /// Insert or update daily stats
    fn save_daily_stats(
        &self,
        date: NaiveDate,
        strategy_id: &StrategyId,
        trade_count: usize,
        wins: usize,
        losses: usize,
        total_win_amount: Decimal,
        total_loss_amount: Decimal,
        net_pnl: Decimal,
    ) -> StorageResult<()>;

    /// Get daily stats for date range
    fn get_daily_stats(
        &self,
        strategy_id: &StrategyId,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> StorageResult<Vec<DailyStatsRow>>;
}

/// Row from daily_stats table
#[derive(Debug, Clone)]
pub struct DailyStatsRow {
    pub date: NaiveDate,
    pub strategy_id: StrategyId,
    pub trade_count: usize,
    pub wins: usize,
    pub losses: usize,
    pub total_win_amount: Decimal,
    pub total_loss_amount: Decimal,
    pub net_pnl: Decimal,
    pub is_profitable: bool,
}

/// Combined storage trait (all operations)
pub trait Storage: TradeStore + PositionStore + DailyStatsStore + Send + Sync {}
