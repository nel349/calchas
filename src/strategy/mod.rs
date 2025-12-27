// Strategy module - Loading and managing trading strategies
// See TECHNICAL_ARCHITECTURE.md Section 4.2 for design details

pub mod loader;
pub mod evaluator;
pub mod signals;

// Re-export commonly used types
pub use loader::{StrategyLoader, LoaderError};
pub use evaluator::{StrategyEvaluator, EvaluationError};
pub use signals::{EntrySignal, SignalSide};
