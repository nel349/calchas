use super::*;
use crate::models::*;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Simple error wrapper for parsing failures
#[derive(Debug)]
struct ParseError(String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// Helper: Safely convert u64 to i64 for SQLite storage
/// SQLite INTEGER is i64, so we must validate range
fn u64_to_i64_safe(value: u64, field_name: &str) -> Result<i64, ParseError> {
    i64::try_from(value).map_err(|_| {
        ParseError(format!(
            "{} value {} exceeds i64::MAX (SQLite INTEGER limit)",
            field_name, value
        ))
    })
}

/// Helper: Safely convert i64 from SQLite to u64
/// Validates that database value is non-negative
fn i64_to_u64_safe(value: i64, field_name: &str) -> Result<u64, ParseError> {
    u64::try_from(value).map_err(|_| {
        ParseError(format!(
            "{} has negative value {} in database (data corruption?)",
            field_name, value
        ))
    })
}

/// SQLite-backed storage implementation
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// Create new SQLite store and run migrations
    pub fn new<P: AsRef<Path>>(path: P) -> StorageResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| StorageError::Migration(format!("Failed to create directory: {}", e)))?;
        }

        let mut conn = Connection::open(path)?;

        // Run migrations using refinery
        refinery::embed_migrations!("migrations");
        let runner = migrations::runner();
        runner
            .run(&mut conn)
            .map_err(|e| StorageError::Migration(e.to_string()))?;

        // Enable foreign keys and WAL mode for better concurrency
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        conn.pragma_update(None, "journal_mode", "WAL")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// In-memory database (for testing)
    #[allow(dead_code)]
    pub fn in_memory() -> StorageResult<Self> {
        let mut conn = Connection::open_in_memory()?;

        refinery::embed_migrations!("migrations");
        let runner = migrations::runner();
        runner
            .run(&mut conn)
            .map_err(|e| StorageError::Migration(e.to_string()))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl TradeStore for SqliteStore {
    fn insert_trade(&self, trade: &Trade) -> StorageResult<()> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        // Validate u64 → i64 conversions (SQLite INTEGER is i64)
        let entry_qty = u64_to_i64_safe(trade.entry_quantity, "entry_quantity")
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let exit_qty = u64_to_i64_safe(trade.exit_quantity, "exit_quantity")
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        conn.execute(
            "INSERT INTO trades (
                id, position_id, market_id, strategy_id,
                entry_order_id, entry_price, entry_quantity, entry_timestamp,
                exit_order_id, exit_price, exit_quantity, exit_timestamp, exit_reason,
                gross_pnl, fees, net_pnl, return_pct, hold_duration_seconds, notes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                trade.id.0.to_string(),
                trade.position_id.0.to_string(),
                trade.market_id.0.clone(),
                trade.strategy_id.0.clone(),
                trade.entry_order_id.0.clone(),
                trade.entry_price.to_string(),
                entry_qty,
                trade.entry_timestamp.to_rfc3339(),
                trade.exit_order_id.0.clone(),
                trade.exit_price.to_string(),
                exit_qty,
                trade.exit_timestamp.to_rfc3339(),
                format!("{:?}", trade.exit_reason),
                trade.gross_pnl.to_string(),
                trade.fees.to_string(),
                trade.net_pnl.to_string(),
                trade.return_pct.to_string(),
                trade.hold_duration.num_seconds(),
                trade.notes.clone(),
            ],
        )?;

        Ok(())
    }

    fn get_trades(
        &self,
        strategy_id: &StrategyId,
        limit: Option<usize>,
    ) -> StorageResult<Vec<Trade>> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        let limit_clause = limit
            .map(|l| format!("LIMIT {}", l))
            .unwrap_or_default();
        let query = format!(
            "SELECT * FROM trades WHERE strategy_id = ?1 ORDER BY exit_timestamp DESC {}",
            limit_clause
        );

        let mut stmt = conn.prepare(&query)?;
        let trades = stmt
            .query_map([&strategy_id.0], |row| parse_trade_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(trades)
    }

    fn get_trades_by_date_range(
        &self,
        strategy_id: &StrategyId,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> StorageResult<Vec<Trade>> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        let mut stmt = conn.prepare(
            "SELECT * FROM trades
             WHERE strategy_id = ?1
             AND DATE(exit_timestamp) BETWEEN ?2 AND ?3
             ORDER BY exit_timestamp DESC",
        )?;

        let trades = stmt
            .query_map(
                params![
                    &strategy_id.0,
                    start_date.format("%Y-%m-%d").to_string(),
                    end_date.format("%Y-%m-%d").to_string(),
                ],
                |row| parse_trade_row(row),
            )?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(trades)
    }

    fn count_trades(&self, strategy_id: &StrategyId) -> StorageResult<usize> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trades WHERE strategy_id = ?1",
            [&strategy_id.0],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }
}

impl PositionStore for SqliteStore {
    fn save_position(&self, position: &Position) -> StorageResult<()> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        // Validate u64 → i64 conversion (SQLite INTEGER is i64)
        let quantity = u64_to_i64_safe(position.quantity, "quantity")
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        // Use REPLACE for upsert (INSERT OR REPLACE)
        conn.execute(
            "REPLACE INTO positions (
                id, market_id, strategy_id, side,
                entry_price, quantity, entry_timestamp, entry_order_id,
                current_price, unrealized_pnl, peak_pnl,
                take_profit_price, stop_loss_price,
                trailing_stop_distance, trailing_stop_activation_pct, expiry_time,
                status, exit_order_id, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                position.id.0.to_string(),
                position.market_id.0.clone(),
                position.strategy_id.0.clone(),
                format!("{:?}", position.side),
                position.entry_price.to_string(),
                quantity,
                position.entry_timestamp.to_rfc3339(),
                position.entry_order_id.0.clone(),
                position.current_price.to_string(),
                position.unrealized_pnl.to_string(),
                position.peak_pnl.to_string(),
                position.exit_target.take_profit_price.map(|p| p.to_string()),
                position.exit_target.stop_loss_price.map(|p| p.to_string()),
                position.exit_target.trailing_stop_distance.map(|d| d.to_string()),
                None::<String>, // trailing_stop_activation_pct not in Position struct
                position.exit_target.expiry_time.map(|t| t.to_rfc3339()),
                format!("{:?}", position.status),
                position.exit_order_id.as_ref().map(|id| id.0.clone()),
                position.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    fn get_active_positions(&self) -> StorageResult<Vec<Position>> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        let mut stmt = conn.prepare(
            "SELECT * FROM positions WHERE status = 'Active' OR status = 'ExitPending'",
        )?;

        let positions = stmt
            .query_map([], |row| parse_position_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(positions)
    }

    fn delete_position(&self, position_id: &PositionId) -> StorageResult<()> {
        let conn = self.conn.lock().expect("Database mutex poisoned");
        conn.execute(
            "DELETE FROM positions WHERE id = ?1",
            [position_id.0.to_string()],
        )?;
        Ok(())
    }

    fn delete_all_positions(&self) -> StorageResult<()> {
        let conn = self.conn.lock().expect("Database mutex poisoned");
        conn.execute("DELETE FROM positions", [])?;
        Ok(())
    }
}

impl DailyStatsStore for SqliteStore {
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
    ) -> StorageResult<()> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        let is_profitable = net_pnl > Decimal::ZERO;

        // Validate usize → i64 conversions (SQLite INTEGER is i64)
        // Note: usize is platform-dependent (u32 on 32-bit, u64 on 64-bit)
        // We validate as u64 to handle worst case
        let trade_count_i64 = u64_to_i64_safe(trade_count as u64, "trade_count")
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let wins_i64 = u64_to_i64_safe(wins as u64, "wins")
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let losses_i64 = u64_to_i64_safe(losses as u64, "losses")
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        conn.execute(
            "REPLACE INTO daily_stats (
                date, strategy_id, trade_count, wins, losses,
                total_win_amount, total_loss_amount, net_pnl, is_profitable, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                date.format("%Y-%m-%d").to_string(),
                strategy_id.0.clone(),
                trade_count_i64,
                wins_i64,
                losses_i64,
                total_win_amount.to_string(),
                total_loss_amount.to_string(),
                net_pnl.to_string(),
                if is_profitable { 1 } else { 0 },
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    fn get_daily_stats(
        &self,
        strategy_id: &StrategyId,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> StorageResult<Vec<DailyStatsRow>> {
        let conn = self.conn.lock().expect("Database mutex poisoned");

        let mut stmt = conn.prepare(
            "SELECT * FROM daily_stats
             WHERE strategy_id = ?1
             AND date BETWEEN ?2 AND ?3
             ORDER BY date ASC",
        )?;

        let stats = stmt
            .query_map(
                params![
                    &strategy_id.0,
                    start_date.format("%Y-%m-%d").to_string(),
                    end_date.format("%Y-%m-%d").to_string(),
                ],
                |row| {
                    Ok(DailyStatsRow {
                        date: NaiveDate::parse_from_str(&row.get::<_, String>(0)?, "%Y-%m-%d")
                            .map_err(|e| {
                                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                            })?,
                        strategy_id: StrategyId::new(row.get(1)?),
                        trade_count: row.get::<_, i64>(2)? as usize,
                        wins: row.get::<_, i64>(3)? as usize,
                        losses: row.get::<_, i64>(4)? as usize,
                        total_win_amount: Decimal::from_str(&row.get::<_, String>(5)?)
                            .map_err(|e| {
                                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                            })?,
                        total_loss_amount: Decimal::from_str(&row.get::<_, String>(6)?)
                            .map_err(|e| {
                                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                            })?,
                        net_pnl: Decimal::from_str(&row.get::<_, String>(7)?).map_err(|e| {
                            rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                        })?,
                        is_profitable: row.get::<_, i64>(8)? == 1,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(stats)
    }
}

impl Storage for SqliteStore {}

// =============================================================================
// HELPER FUNCTIONS FOR PARSING DATABASE ROWS
// =============================================================================

/// Parse a trade row from the database
fn parse_trade_row(row: &rusqlite::Row) -> rusqlite::Result<Trade> {
    // Parse UUIDs
    let id = Uuid::from_str(&row.get::<_, String>(0)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let position_id = Uuid::from_str(&row.get::<_, String>(1)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse Decimals
    let entry_price = Decimal::from_str(&row.get::<_, String>(5)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let exit_price = Decimal::from_str(&row.get::<_, String>(9)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let gross_pnl = Decimal::from_str(&row.get::<_, String>(13)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let fees = Decimal::from_str(&row.get::<_, String>(14)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let net_pnl = Decimal::from_str(&row.get::<_, String>(15)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let return_pct = Decimal::from_str(&row.get::<_, String>(16)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse timestamps
    let entry_timestamp = DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
        .with_timezone(&Utc);
    let exit_timestamp = DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
        .with_timezone(&Utc);

    // Parse ExitReason from Debug format
    let exit_reason_str = row.get::<_, String>(12)?;
    let exit_reason = parse_exit_reason(&exit_reason_str)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse Duration from seconds
    let hold_duration_seconds = row.get::<_, i64>(17)?;
    let hold_duration = Duration::seconds(hold_duration_seconds);

    // Validate i64 → u64 conversions
    let entry_quantity = i64_to_u64_safe(row.get::<_, i64>(6)?, "entry_quantity")
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let exit_quantity = i64_to_u64_safe(row.get::<_, i64>(10)?, "exit_quantity")
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    Ok(Trade {
        id: TradeId::from_uuid(id),
        position_id: PositionId::from_uuid(position_id),
        market_id: MarketId::new(row.get(2)?),
        strategy_id: StrategyId::new(row.get(3)?),
        entry_order_id: OrderId::new(row.get(4)?),
        entry_price,
        entry_quantity,
        entry_timestamp,
        exit_order_id: OrderId::new(row.get(8)?),
        exit_price,
        exit_quantity,
        exit_timestamp,
        exit_reason,
        gross_pnl,
        fees,
        net_pnl,
        return_pct,
        hold_duration,
        notes: row.get(18)?,
    })
}

/// Parse a position row from the database
fn parse_position_row(row: &rusqlite::Row) -> rusqlite::Result<Position> {
    // Parse UUIDs
    let id = Uuid::from_str(&row.get::<_, String>(0)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse side
    let side_str = row.get::<_, String>(3)?;
    let side = parse_position_side(&side_str)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse Decimals
    let entry_price = Decimal::from_str(&row.get::<_, String>(4)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let current_price = Decimal::from_str(&row.get::<_, String>(8)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let unrealized_pnl = Decimal::from_str(&row.get::<_, String>(9)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let peak_pnl = Decimal::from_str(&row.get::<_, String>(10)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse optional Decimals for exit targets
    let take_profit_price = row
        .get::<_, Option<String>>(11)?
        .map(|s| Decimal::from_str(&s))
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let stop_loss_price = row
        .get::<_, Option<String>>(12)?
        .map(|s| Decimal::from_str(&s))
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let trailing_stop_distance = row
        .get::<_, Option<String>>(13)?
        .map(|s| Decimal::from_str(&s))
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse optional expiry time
    let expiry_time = row
        .get::<_, Option<String>>(15)?
        .map(|s| DateTime::parse_from_rfc3339(&s))
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
        .map(|dt| dt.with_timezone(&Utc));

    // Parse timestamps
    let entry_timestamp = DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
        .with_timezone(&Utc);
    let updated_at = DateTime::parse_from_rfc3339(&row.get::<_, String>(18)?)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
        .with_timezone(&Utc);

    // Parse status
    let status_str = row.get::<_, String>(16)?;
    let status = parse_position_status(&status_str)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Parse optional exit_order_id
    let exit_order_id = row.get::<_, Option<String>>(17)?.map(OrderId::new);

    // Validate i64 → u64 conversion
    let quantity = i64_to_u64_safe(row.get::<_, i64>(5)?, "quantity")
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    Ok(Position {
        id: PositionId::from_uuid(id),
        market_id: MarketId::new(row.get(1)?),
        strategy_id: StrategyId::new(row.get(2)?),
        side,
        entry_price,
        quantity,
        entry_timestamp,
        entry_order_id: OrderId::new(row.get(7)?),
        current_price,
        unrealized_pnl,
        peak_pnl,
        exit_target: ExitTarget {
            take_profit_price,
            stop_loss_price,
            trailing_stop_distance,
            expiry_time,
        },
        exit_order_id,
        status,
        updated_at,
    })
}

// =============================================================================
// ENUM PARSERS
// =============================================================================

/// Parse ExitReason from Debug format string
fn parse_exit_reason(s: &str) -> Result<ExitReason, ParseError> {
    match s {
        "TakeProfit" => Ok(ExitReason::TakeProfit),
        "StopLoss" => Ok(ExitReason::StopLoss),
        "TrailingStop" => Ok(ExitReason::TrailingStop),
        "MaxHoldTime" => Ok(ExitReason::MaxHoldTime),
        "ManualExit" => Ok(ExitReason::ManualExit),
        "StrategyDisabled" => Ok(ExitReason::StrategyDisabled),
        "MarketClosed" => Ok(ExitReason::MarketClosed),
        "SettlementCutLoss" => Ok(ExitReason::SettlementCutLoss),
        _ => Err(ParseError(format!("Unknown ExitReason: {}", s))),
    }
}

/// Parse PositionSide from Debug format string
fn parse_position_side(s: &str) -> Result<PositionSide, ParseError> {
    match s {
        "Yes" => Ok(PositionSide::Yes),
        "No" => Ok(PositionSide::No),
        _ => Err(ParseError(format!("Unknown PositionSide: {}", s))),
    }
}

/// Parse PositionStatus from Debug format string
fn parse_position_status(s: &str) -> Result<PositionStatus, ParseError> {
    match s {
        "Active" => Ok(PositionStatus::Active),
        "ExitPending" => Ok(PositionStatus::ExitPending),
        "Closed" => Ok(PositionStatus::Closed),
        "Error" => Ok(PositionStatus::Error),
        _ => Err(ParseError(format!("Unknown PositionStatus: {}", s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use chrono::{Duration, Utc};
    use rust_decimal_macros::dec;

    /// Helper: Create test trade
    fn create_test_trade() -> Trade {
        Trade {
            id: TradeId::new(),
            position_id: PositionId::new(),
            market_id: MarketId::new("KXBTC-24DEC31-T50000".to_string()),
            strategy_id: StrategyId::new("momentum-test".to_string()),
            entry_order_id: OrderId::new("order-entry-1".to_string()),
            entry_price: dec!(0.42),
            entry_quantity: 100,
            entry_timestamp: Utc::now() - Duration::hours(2),
            exit_order_id: OrderId::new("order-exit-1".to_string()),
            exit_price: dec!(0.52),
            exit_quantity: 100,
            exit_timestamp: Utc::now(),
            exit_reason: ExitReason::TakeProfit,
            gross_pnl: dec!(10.00),
            fees: dec!(0.20),
            net_pnl: dec!(9.80),
            return_pct: dec!(23.33),
            hold_duration: Duration::hours(2),
            notes: Some("Test trade".to_string()),
        }
    }

    /// Helper: Create test position
    fn create_test_position() -> Position {
        Position {
            id: PositionId::new(),
            market_id: MarketId::new("KXBTC-24DEC31-T50000".to_string()),
            strategy_id: StrategyId::new("momentum-test".to_string()),
            side: PositionSide::Yes,
            entry_price: dec!(0.42),
            quantity: 100,
            entry_timestamp: Utc::now() - Duration::hours(1),
            entry_order_id: OrderId::new("order-entry-1".to_string()),
            current_price: dec!(0.45),
            unrealized_pnl: dec!(3.00),
            peak_pnl: dec!(5.00),
            exit_target: ExitTarget {
                take_profit_price: Some(dec!(0.50)),
                stop_loss_price: Some(dec!(0.38)),
                trailing_stop_distance: Some(dec!(0.02)),
                expiry_time: Some(Utc::now() + Duration::hours(24)),
            },
            exit_order_id: None,
            status: PositionStatus::Active,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_insert_and_retrieve_trade() {
        let store = SqliteStore::in_memory().unwrap();
        let trade = create_test_trade();

        // Insert trade
        store.insert_trade(&trade).unwrap();

        // Retrieve trade
        let trades = store
            .get_trades(&trade.strategy_id, Some(10))
            .unwrap();

        assert_eq!(trades.len(), 1);
        let retrieved = &trades[0];

        // Verify all fields
        assert_eq!(retrieved.id, trade.id);
        assert_eq!(retrieved.position_id, trade.position_id);
        assert_eq!(retrieved.market_id, trade.market_id);
        assert_eq!(retrieved.strategy_id, trade.strategy_id);
        assert_eq!(retrieved.entry_price, trade.entry_price);
        assert_eq!(retrieved.entry_quantity, trade.entry_quantity);
        assert_eq!(retrieved.exit_price, trade.exit_price);
        assert_eq!(retrieved.exit_quantity, trade.exit_quantity);
        assert_eq!(retrieved.gross_pnl, trade.gross_pnl);
        assert_eq!(retrieved.fees, trade.fees);
        assert_eq!(retrieved.net_pnl, trade.net_pnl);
        assert_eq!(retrieved.return_pct, trade.return_pct);
        assert_eq!(retrieved.notes, trade.notes);
    }

    #[test]
    fn test_get_trades_with_limit() {
        let store = SqliteStore::in_memory().unwrap();
        let strategy_id = StrategyId::new("test-strategy".to_string());

        // Insert 5 trades
        for i in 0..5 {
            let mut trade = create_test_trade();
            trade.strategy_id = strategy_id.clone();
            trade.exit_timestamp = Utc::now() - Duration::hours(i);
            store.insert_trade(&trade).unwrap();
        }

        // Get with limit
        let trades = store.get_trades(&strategy_id, Some(3)).unwrap();
        assert_eq!(trades.len(), 3);

        // Get all
        let all_trades = store.get_trades(&strategy_id, None).unwrap();
        assert_eq!(all_trades.len(), 5);
    }

    #[test]
    fn test_get_trades_by_date_range() {
        let store = SqliteStore::in_memory().unwrap();
        let strategy_id = StrategyId::new("test-strategy".to_string());

        // Insert trades on different days
        let today = Utc::now();
        let yesterday = today - Duration::days(1);
        let two_days_ago = today - Duration::days(2);

        let mut trade1 = create_test_trade();
        trade1.strategy_id = strategy_id.clone();
        trade1.exit_timestamp = two_days_ago;
        store.insert_trade(&trade1).unwrap();

        let mut trade2 = create_test_trade();
        trade2.strategy_id = strategy_id.clone();
        trade2.exit_timestamp = yesterday;
        store.insert_trade(&trade2).unwrap();

        let mut trade3 = create_test_trade();
        trade3.strategy_id = strategy_id.clone();
        trade3.exit_timestamp = today;
        store.insert_trade(&trade3).unwrap();

        // Query date range (yesterday to today)
        let trades = store
            .get_trades_by_date_range(
                &strategy_id,
                yesterday.date_naive(),
                today.date_naive(),
            )
            .unwrap();

        assert_eq!(trades.len(), 2);
    }

    #[test]
    fn test_count_trades() {
        let store = SqliteStore::in_memory().unwrap();
        let strategy_id = StrategyId::new("test-strategy".to_string());

        // Initially zero
        assert_eq!(store.count_trades(&strategy_id).unwrap(), 0);

        // Insert 3 trades
        for _ in 0..3 {
            let mut trade = create_test_trade();
            trade.strategy_id = strategy_id.clone();
            store.insert_trade(&trade).unwrap();
        }

        assert_eq!(store.count_trades(&strategy_id).unwrap(), 3);
    }

    #[test]
    fn test_save_and_retrieve_position() {
        let store = SqliteStore::in_memory().unwrap();
        let position = create_test_position();

        // Save position
        store.save_position(&position).unwrap();

        // Retrieve active positions
        let positions = store.get_active_positions().unwrap();
        assert_eq!(positions.len(), 1);

        let retrieved = &positions[0];
        assert_eq!(retrieved.id, position.id);
        assert_eq!(retrieved.market_id, position.market_id);
        assert_eq!(retrieved.strategy_id, position.strategy_id);
        assert_eq!(retrieved.side, position.side);
        assert_eq!(retrieved.entry_price, position.entry_price);
        assert_eq!(retrieved.quantity, position.quantity);
        assert_eq!(retrieved.current_price, position.current_price);
        assert_eq!(retrieved.unrealized_pnl, position.unrealized_pnl);
        assert_eq!(retrieved.peak_pnl, position.peak_pnl);
        assert_eq!(
            retrieved.exit_target.take_profit_price,
            position.exit_target.take_profit_price
        );
        assert_eq!(
            retrieved.exit_target.stop_loss_price,
            position.exit_target.stop_loss_price
        );
    }

    #[test]
    fn test_position_upsert() {
        let store = SqliteStore::in_memory().unwrap();
        let mut position = create_test_position();

        // Insert position
        store.save_position(&position).unwrap();
        assert_eq!(store.get_active_positions().unwrap().len(), 1);

        // Update position (upsert)
        position.current_price = dec!(0.48);
        position.unrealized_pnl = dec!(6.00);
        store.save_position(&position).unwrap();

        // Should still be 1 position (not 2)
        let positions = store.get_active_positions().unwrap();
        assert_eq!(positions.len(), 1);

        // Verify updated values
        assert_eq!(positions[0].current_price, dec!(0.48));
        assert_eq!(positions[0].unrealized_pnl, dec!(6.00));
    }

    #[test]
    fn test_delete_position() {
        let store = SqliteStore::in_memory().unwrap();
        let position = create_test_position();

        // Save and verify
        store.save_position(&position).unwrap();
        assert_eq!(store.get_active_positions().unwrap().len(), 1);

        // Delete
        store.delete_position(&position.id).unwrap();
        assert_eq!(store.get_active_positions().unwrap().len(), 0);
    }

    #[test]
    fn test_delete_all_positions() {
        let store = SqliteStore::in_memory().unwrap();

        // Create 3 positions
        for _ in 0..3 {
            let position = create_test_position();
            store.save_position(&position).unwrap();
        }

        assert_eq!(store.get_active_positions().unwrap().len(), 3);

        // Delete all
        store.delete_all_positions().unwrap();
        assert_eq!(store.get_active_positions().unwrap().len(), 0);
    }

    #[test]
    fn test_save_and_retrieve_daily_stats() {
        let store = SqliteStore::in_memory().unwrap();
        let date = Utc::now().date_naive();
        let strategy_id = StrategyId::new("test-strategy".to_string());

        // Save daily stats
        store
            .save_daily_stats(
                date,
                &strategy_id,
                10,
                7,
                3,
                dec!(100.00),
                dec!(30.00),
                dec!(70.00),
            )
            .unwrap();

        // Retrieve
        let stats = store
            .get_daily_stats(&strategy_id, date, date)
            .unwrap();

        assert_eq!(stats.len(), 1);
        let row = &stats[0];
        assert_eq!(row.date, date);
        assert_eq!(row.strategy_id, strategy_id);
        assert_eq!(row.trade_count, 10);
        assert_eq!(row.wins, 7);
        assert_eq!(row.losses, 3);
        assert_eq!(row.total_win_amount, dec!(100.00));
        assert_eq!(row.total_loss_amount, dec!(30.00));
        assert_eq!(row.net_pnl, dec!(70.00));
        assert!(row.is_profitable);
    }

    #[test]
    fn test_daily_stats_composite_primary_key() {
        let store = SqliteStore::in_memory().unwrap();
        let date = Utc::now().date_naive();
        let strategy1 = StrategyId::new("strategy-1".to_string());
        let strategy2 = StrategyId::new("strategy-2".to_string());

        // Save stats for two different strategies on same day
        store
            .save_daily_stats(date, &strategy1, 5, 3, 2, dec!(50.00), dec!(20.00), dec!(30.00))
            .unwrap();

        store
            .save_daily_stats(date, &strategy2, 8, 5, 3, dec!(80.00), dec!(30.00), dec!(50.00))
            .unwrap();

        // Retrieve for strategy 1
        let stats1 = store.get_daily_stats(&strategy1, date, date).unwrap();
        assert_eq!(stats1.len(), 1);
        assert_eq!(stats1[0].trade_count, 5);

        // Retrieve for strategy 2
        let stats2 = store.get_daily_stats(&strategy2, date, date).unwrap();
        assert_eq!(stats2.len(), 1);
        assert_eq!(stats2[0].trade_count, 8);
    }

    #[test]
    fn test_daily_stats_upsert() {
        let store = SqliteStore::in_memory().unwrap();
        let date = Utc::now().date_naive();
        let strategy_id = StrategyId::new("test-strategy".to_string());

        // Insert initial stats
        store
            .save_daily_stats(date, &strategy_id, 5, 3, 2, dec!(50.00), dec!(20.00), dec!(30.00))
            .unwrap();

        // Update stats (upsert)
        store
            .save_daily_stats(
                date,
                &strategy_id,
                10,
                7,
                3,
                dec!(100.00),
                dec!(30.00),
                dec!(70.00),
            )
            .unwrap();

        // Should have only 1 row (not 2)
        let stats = store.get_daily_stats(&strategy_id, date, date).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].trade_count, 10); // Updated value
    }

    #[test]
    fn test_decimal_precision_preservation() {
        let store = SqliteStore::in_memory().unwrap();
        let mut trade = create_test_trade();

        // Use precise decimal values
        trade.entry_price = dec!(0.123456789);
        trade.exit_price = dec!(0.987654321);
        trade.net_pnl = dec!(123.456789012345);

        store.insert_trade(&trade).unwrap();

        let trades = store.get_trades(&trade.strategy_id, None).unwrap();
        assert_eq!(trades[0].entry_price, dec!(0.123456789));
        assert_eq!(trades[0].exit_price, dec!(0.987654321));
        assert_eq!(trades[0].net_pnl, dec!(123.456789012345));
    }

    #[test]
    fn test_exit_reason_serialization() {
        let store = SqliteStore::in_memory().unwrap();

        // Test all exit reasons
        let exit_reasons = vec![
            ExitReason::TakeProfit,
            ExitReason::StopLoss,
            ExitReason::TrailingStop,
            ExitReason::MaxHoldTime,
            ExitReason::ManualExit,
            ExitReason::StrategyDisabled,
            ExitReason::MarketClosed,
            ExitReason::SettlementCutLoss,
        ];

        for reason in exit_reasons {
            let mut trade = create_test_trade();
            trade.exit_reason = reason.clone();
            store.insert_trade(&trade).unwrap();

            let trades = store.get_trades(&trade.strategy_id, Some(1)).unwrap();
            assert_eq!(trades[0].exit_reason, reason);
        }
    }

    #[test]
    fn test_position_status_serialization() {
        let store = SqliteStore::in_memory().unwrap();

        // Test all position statuses
        let statuses = vec![
            PositionStatus::Active,
            PositionStatus::ExitPending,
            PositionStatus::Closed,
            PositionStatus::Error,
        ];

        for status in statuses {
            let mut position = create_test_position();
            position.status = status.clone();
            store.save_position(&position).unwrap();

            // Note: get_active_positions only returns Active/ExitPending
            // For Closed/Error, we'd need a different query method
            if matches!(status, PositionStatus::Active | PositionStatus::ExitPending) {
                let positions = store.get_active_positions().unwrap();
                assert!(positions.iter().any(|p| p.status == status));
            }
        }
    }

    #[test]
    fn test_optional_fields_none() {
        let store = SqliteStore::in_memory().unwrap();
        let mut position = create_test_position();

        // Set optional fields to None
        position.exit_target = ExitTarget {
            take_profit_price: None,
            stop_loss_price: None,
            trailing_stop_distance: None,
            expiry_time: None,
        };
        position.exit_order_id = None;

        store.save_position(&position).unwrap();

        let positions = store.get_active_positions().unwrap();
        assert_eq!(positions[0].exit_target.take_profit_price, None);
        assert_eq!(positions[0].exit_target.stop_loss_price, None);
        assert_eq!(positions[0].exit_order_id, None);
    }

    #[test]
    fn test_safe_integer_conversion_helpers() {
        // Test u64_to_i64_safe
        assert!(u64_to_i64_safe(100, "test").is_ok());
        assert!(u64_to_i64_safe(i64::MAX as u64, "test").is_ok());
        assert!(u64_to_i64_safe(u64::MAX, "test").is_err()); // Overflow

        // Test i64_to_u64_safe
        assert!(i64_to_u64_safe(100, "test").is_ok());
        assert!(i64_to_u64_safe(0, "test").is_ok());
        assert!(i64_to_u64_safe(-1, "test").is_err()); // Negative
    }

    #[test]
    fn test_trade_with_max_quantity() {
        let store = SqliteStore::in_memory().unwrap();
        let mut trade = create_test_trade();

        // Use maximum safe i64 value for quantity
        trade.entry_quantity = i64::MAX as u64;
        trade.exit_quantity = i64::MAX as u64;

        store.insert_trade(&trade).unwrap();

        let trades = store.get_trades(&trade.strategy_id, None).unwrap();
        assert_eq!(trades[0].entry_quantity, i64::MAX as u64);
        assert_eq!(trades[0].exit_quantity, i64::MAX as u64);
    }
}
