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

// Re-exports
pub use error::TradingError;
pub use simulator::{OrderSimulator, SimulatedFill};
