//! Trading error types

use std::fmt;

/// Errors that can occur during trading operations
#[derive(Debug)]
pub enum TradingError {
    /// Market not found for price lookup
    MarketNotFound(String),

    /// Order validation failed
    InvalidOrder(String),

    /// Order not filled (cannot open position)
    OrderNotFilled(String),

    /// Position not found
    PositionNotFound(String),

    /// No exit condition met (cannot close position)
    NoExitCondition,

    /// Risk limit violated
    RiskLimitViolated(String),

    /// Mutex lock error
    LockError,

    /// API error from Kalshi client
    KalshiError(crate::kalshi::KalshiError),
}

impl fmt::Display for TradingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradingError::MarketNotFound(id) => {
                write!(f, "Market not found: {}", id)
            }
            TradingError::InvalidOrder(msg) => {
                write!(f, "Invalid order: {}", msg)
            }
            TradingError::OrderNotFilled(order_id) => {
                write!(f, "Order not filled: {}", order_id)
            }
            TradingError::PositionNotFound(id) => {
                write!(f, "Position not found: {}", id)
            }
            TradingError::NoExitCondition => {
                write!(f, "No exit condition met")
            }
            TradingError::RiskLimitViolated(msg) => {
                write!(f, "Risk limit violated: {}", msg)
            }
            TradingError::LockError => {
                write!(f, "Failed to acquire lock")
            }
            TradingError::KalshiError(err) => {
                write!(f, "Kalshi API error: {}", err)
            }
        }
    }
}

impl std::error::Error for TradingError {}

impl From<crate::kalshi::KalshiError> for TradingError {
    fn from(err: crate::kalshi::KalshiError) -> Self {
        TradingError::KalshiError(err)
    }
}
