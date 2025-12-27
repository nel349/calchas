// Kalshi-specific functionality
// This module contains all Kalshi platform-specific logic

// Phase 1 modules
pub mod fees;

// Phase 2 modules (REST API integration)
pub mod error;
pub mod types;
pub mod auth;
pub mod retry;
pub mod client;

// Re-export commonly used types for convenience
pub use client::KalshiClient;
pub use error::KalshiError;
pub use retry::RetryPolicy;
pub use types::{GetMarketsRequest, KalshiMarket, MarketsResponse};
