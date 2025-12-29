//! Arbitrage trading module
//!
//! This module implements arbitrage strategies for Kalshi prediction markets.
//!
//! # Strategy Types
//!
//! ## Phase 1: Cross-Market Arbitrage
//!
//! Exploits price inefficiencies where YES + NO prices don't sum to $1.00.
//!
//! **Example:**
//! ```text
//! Market: "Will Bitcoin hit $110K by Dec 31?"
//! YES ask: $0.48
//! NO ask: $0.47
//! Total cost: $0.95
//!
//! At settlement, one side pays $1.00 → Guaranteed $0.05 profit (5.3%)
//! ```
//!
//! **Detection:**
//! - Scan all markets for orderbook data
//! - Calculate: YES ask + NO ask
//! - If sum < $0.98 (after fees): arbitrage opportunity
//!
//! **Execution:**
//! - Buy BOTH YES and NO simultaneously
//! - Lock in guaranteed profit
//! - Hold until settlement
//!
//! ## Phase 2: Correlated Market Arbitrage (Future)
//!
//! Exploits mispricing between statistically related markets.
//! Requires historical data collection from Phase 1.
//!
//! # Module Structure
//!
//! - `opportunity` - Arbitrage opportunity data model
//! - `cross_market` - Cross-market arbitrage detector
//! - `calculator` - Profit calculations and filtering
//! - `executor` - Order execution for arbitrage trades (Phase 1 Week 2)

pub mod opportunity;
pub mod cross_market;
pub mod calculator;

pub use opportunity::ArbitrageOpportunity;
pub use cross_market::CrossMarketDetector;
pub use calculator::ArbitrageCalculator;
