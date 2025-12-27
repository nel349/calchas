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

// Re-exports
pub use error::TradingError;
pub use simulator::{OrderSimulator, SimulatedFill};
pub use risk_manager::{RiskManager, RiskDecision, RejectionReason};
