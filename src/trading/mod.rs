//! Trading logic and execution
//!
//! This module contains all trading-related functionality including:
//! - Order simulation (Phase 4)
//! - Risk management
//! - Position management
//! - Exit monitoring
//! - Performance metrics tracking

pub mod error;
pub mod simulator;
pub mod risk_manager;
pub mod exit_manager;
pub mod order_executor;
pub mod position_manager;
pub mod metrics_tracker;
pub mod price_tracker;
pub mod orderbook_provider;

// Re-exports
pub use error::TradingError;
pub use simulator::{OrderSimulator, SimulatedFill};
pub use risk_manager::{RiskManager, RiskDecision, RejectionReason};
pub use exit_manager::ExitManager;
pub use order_executor::OrderExecutor;
pub use position_manager::PositionManager;
pub use metrics_tracker::{MetricsTracker, SimulationMetrics, ExitToLiveDecision, DailyRecord};
pub use price_tracker::PriceTracker;
pub use orderbook_provider::{OrderbookProvider, SimulatedOrderbookProvider, RealOrderbookProvider, OrderbookError};
