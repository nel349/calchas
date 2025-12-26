// Strategy module - Loading and managing trading strategies
// See TECHNICAL_ARCHITECTURE.md Section 4.2 for design details

pub mod loader;

// Re-export commonly used types
pub use loader::{StrategyLoader, LoaderError};
