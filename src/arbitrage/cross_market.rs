//! Cross-market arbitrage detector
//!
//! Scans Kalshi markets for cross-market arbitrage opportunities where
//! YES ask + NO ask < $1.00 (accounting for fees and buffer).
//!
//! # Algorithm
//!
//! 1. Fetch all active markets from Kalshi
//! 2. For each market, fetch orderbook
//! 3. Check if YES ask + NO ask < settlement value
//! 4. Calculate profit, filter by thresholds
//! 5. Return ranked list of opportunities

use std::sync::Arc;
use tokio::sync::Semaphore;
use futures::future::join_all;

use crate::arbitrage::{ArbitrageCalculator, ArbitrageOpportunity};
use crate::kalshi::KalshiClient;
use crate::models::{Market, MarketStatus};

/// Error types for arbitrage detection
#[derive(Debug)]
pub enum DetectionError {
    /// Kalshi API error
    ApiError(String),

    /// No markets available
    NoMarketsAvailable,
}

impl std::fmt::Display for DetectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectionError::ApiError(msg) => write!(f, "API error: {}", msg),
            DetectionError::NoMarketsAvailable => write!(f, "No markets available to scan"),
        }
    }
}

impl std::error::Error for DetectionError {}

impl From<crate::kalshi::error::KalshiError> for DetectionError {
    fn from(err: crate::kalshi::error::KalshiError) -> Self {
        DetectionError::ApiError(err.to_string())
    }
}

/// Cross-market arbitrage detector
///
/// Scans Kalshi markets and identifies arbitrage opportunities using real-time
/// orderbook data.
pub struct CrossMarketDetector {
    kalshi_client: Arc<KalshiClient>,
    calculator: ArbitrageCalculator,
}

impl CrossMarketDetector {
    /// Create a new cross-market arbitrage detector
    ///
    /// # Arguments
    ///
    /// * `kalshi_client` - Shared Kalshi API client
    /// * `calculator` - Arbitrage calculator with filtering config
    pub fn new(kalshi_client: Arc<KalshiClient>, calculator: ArbitrageCalculator) -> Self {
        CrossMarketDetector {
            kalshi_client,
            calculator,
        }
    }

    /// Clone detector for use in concurrent tasks
    fn clone_for_task(&self) -> Self {
        CrossMarketDetector {
            kalshi_client: self.kalshi_client.clone(),
            calculator: self.calculator.clone(),
        }
    }

    /// Scan all active markets for arbitrage opportunities
    ///
    /// This is the main detection method. It:
    /// 1. Fetches all active markets
    /// 2. Gets orderbook for each
    /// 3. Checks for arbitrage
    /// 4. Filters by configuration
    /// 5. Ranks by profitability
    ///
    /// # Returns
    ///
    /// Sorted list of arbitrage opportunities (best first)
    pub async fn scan(&self) -> Result<Vec<ArbitrageOpportunity>, DetectionError> {
        tracing::info!("Starting arbitrage scan");
        tracing::info!("Step 1/3: Fetching active markets from Kalshi...");

        // Fetch all markets
        let markets = self.fetch_active_markets().await?;

        if markets.is_empty() {
            return Err(DetectionError::NoMarketsAvailable);
        }

        tracing::info!("Step 2/3: Scanning {} active markets for arbitrage (10 concurrent)", markets.len());

        let total_markets = markets.len();
        let scan_start = std::time::Instant::now();

        // Use semaphore to limit concurrency to 10 (stay under 20/sec rate limit)
        let semaphore = Arc::new(Semaphore::new(10));
        let scanned = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let with_arbitrage = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        // Spawn tasks for all markets
        let mut tasks = Vec::new();
        for market in markets {
            let sem = semaphore.clone();
            let scanned_clone = scanned.clone();
            let with_arb_clone = with_arbitrage.clone();
            let detector = self.clone_for_task();

            let task = tokio::spawn(async move {
                // Acquire permit (blocks if 20 tasks already running)
                let _permit = sem.acquire().await.unwrap();

                // Check market
                let result = detector.check_market(&market).await;

                // Update arbitrage counter if we found one
                if let Ok(Some(ref opp)) = result {
                    if detector.calculator.passes_filters(opp) {
                        with_arb_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }

                // Update scanned counter
                let count = scanned_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

                // Log progress every 500 markets
                if count % 500 == 0 {
                    let arb_count = with_arb_clone.load(std::sync::atomic::Ordering::SeqCst);
                    tracing::info!(
                        "   Progress: {}/{} markets scanned, {} opportunities found",
                        count,
                        total_markets,
                        arb_count
                    );
                }

                result
            });

            tasks.push(task);
        }

        // Wait for all tasks concurrently (not sequentially!)
        let results = join_all(tasks).await;

        // Collect opportunities from results
        let mut opportunities = Vec::new();
        for result in results {
            match result {
                Ok(Ok(Some(opportunity))) => {
                    if self.calculator.passes_filters(&opportunity) {
                        opportunities.push(opportunity);
                        // Counter already incremented in task above
                    }
                }
                Ok(Ok(None)) => {
                    // No arbitrage
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to check market: {}", e);
                }
                Err(e) => {
                    tracing::warn!("Task failed: {}", e);
                }
            }
        }

        let final_scanned = scanned.load(std::sync::atomic::Ordering::SeqCst);
        let final_with_arb = with_arbitrage.load(std::sync::atomic::Ordering::SeqCst);
        let scan_duration = scan_start.elapsed();

        tracing::info!(
            "Scan complete: {}/{} markets have arbitrage, {} pass filters (took {:.1}s, {:.0} markets/sec)",
            final_with_arb,
            final_scanned,
            opportunities.len(),
            scan_duration.as_secs_f64(),
            final_scanned as f64 / scan_duration.as_secs_f64()
        );

        tracing::info!("Step 3/3: Ranking opportunities by profitability...");

        // Rank by profitability (annualized ROI)
        let ranked = self.calculator.rank_opportunities(opportunities);

        tracing::info!("✅ Arbitrage scan complete: {} opportunities found", ranked.len());

        Ok(ranked)
    }

    /// Fetch all active markets from Kalshi
    ///
    /// Filters for:
    /// - Active status only
    /// - Excludes markets settling within next hour (execution risk)
    ///
    /// # Returns
    ///
    /// List of active markets ready for arbitrage scanning
    async fn fetch_active_markets(&self) -> Result<Vec<Market>, DetectionError> {
        use crate::kalshi::types::GetMarketsRequest;

        let now = chrono::Utc::now();
        let min_close = now + chrono::Duration::hours(1);  // At least 1 hour away (safety buffer)
        let max_close = now + chrono::Duration::days(7);   // Settle within 7 days (fast capital turnover)

        let mut all_markets = Vec::new();
        let mut request = GetMarketsRequest {
            limit: Some(1000), // Fetch 1000 at a time (same as strategy mode)
            cursor: None,
            status: Some("open".to_string()),
            min_close_ts: Some(min_close.timestamp()),
            max_close_ts: Some(max_close.timestamp()),
            ..Default::default()
        };

        let mut page = 0;
        loop {
            page += 1;
            tracing::debug!("📡 Fetching markets page {} from Kalshi API...", page);

            let start = std::time::Instant::now();
            let response = self.kalshi_client.get_markets(request.clone()).await?;
            let elapsed = start.elapsed();

            tracing::debug!("✓ Received {} markets from page {} in {:.2}s", response.markets.len(), page, elapsed.as_secs_f64());

            // Filter for active markets only
            for kalshi_market in response.markets {
                let market: Market = kalshi_market.into();

                // Only include active markets
                if market.status != MarketStatus::Active {
                    continue;
                }

                // Skip markets settling very soon (execution risk)
                let time_to_close = market.close_time.signed_duration_since(chrono::Utc::now());
                if time_to_close.num_hours() < 1 {
                    continue;
                }

                all_markets.push(market);
            }

            // Check if there are more pages
            match response.cursor {
                Some(cursor) if !cursor.is_empty() => {
                    request.cursor = Some(cursor);
                    tracing::debug!("More pages available, continuing...");
                }
                _ => {
                    tracing::info!("✓ Fetched {} total markets across {} pages", all_markets.len(), page);
                    break;
                }
            }
        }

        Ok(all_markets)
    }

    /// Check a single market for arbitrage opportunity
    ///
    /// Uses bid/ask prices from the market data (no extra API call needed!)
    ///
    /// # Arguments
    ///
    /// * `market` - Market to check
    ///
    /// # Returns
    ///
    /// Arbitrage opportunity if found, None otherwise
    async fn check_market(
        &self,
        market: &Market,
    ) -> Result<Option<ArbitrageOpportunity>, DetectionError> {
        // Use bid/ask prices directly from market data (already fetched in get_markets)
        let yes_ask = market.yes_ask;
        let no_ask = market.no_ask;

        // Skip markets with zero prices (no liquidity or not trading)
        if yes_ask == rust_decimal::Decimal::ZERO || no_ask == rust_decimal::Decimal::ZERO {
            return Ok(None);
        }

        // Check if arbitrage exists (YES ask + NO ask < $1.00)
        let total_cost = yes_ask + no_ask;
        if total_cost >= rust_decimal::Decimal::ONE {
            return Ok(None); // No arbitrage
        }

        // Use min_quantity from config as conservative estimate
        // Real quantity available would require fetching orderbook (5944 extra API calls)
        let quantity = self.calculator.config.min_quantity;

        // Create opportunity
        let opportunity = ArbitrageOpportunity::new_cross_market(
            market.id.clone(),
            market.title.clone(),
            yes_ask,
            no_ask,
            quantity,
            market.close_time,
        );

        // Check if it passes filters before logging
        if self.calculator.passes_filters(&opportunity) {
            let capital_required = opportunity.capital_required(quantity);

            tracing::info!(
                "🎯 ARBITRAGE FOUND: {} | YES ${:.2} + NO ${:.2} = ${:.2} | Profit {:.1}% | Qty {} | Capital ${:.2}",
                market.title.chars().take(50).collect::<String>(),
                yes_ask,
                no_ask,
                opportunity.total_cost,
                opportunity.profit_pct * rust_decimal::Decimal::from(100),
                quantity,
                capital_required
            );
            tracing::info!("   📍 Check now: https://kalshi.com/markets/{}", market.ticker);
        } else {
            tracing::debug!(
                "Arbitrage found but filtered: {} | Profit: {:.1}% | Qty: {}",
                market.id.as_str(),
                opportunity.profit_pct * rust_decimal::Decimal::from(100),
                quantity
            );
        }

        Ok(Some(opportunity))
    }

    /// Scan for opportunities and return the top N by profitability
    ///
    /// Convenience method for getting just the best opportunities.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of opportunities to return
    ///
    /// # Returns
    ///
    /// Top N arbitrage opportunities
    pub async fn scan_top_n(&self, limit: usize) -> Result<Vec<ArbitrageOpportunity>, DetectionError> {
        let all_opportunities = self.scan().await?;

        Ok(all_opportunities.into_iter().take(limit).collect())
    }

    /// Scan for opportunities with total capital above threshold
    ///
    /// Returns opportunities that together require at least `min_capital`.
    /// Useful for deploying a specific amount of capital.
    ///
    /// # Arguments
    ///
    /// * `min_capital` - Minimum total capital to deploy (USD)
    ///
    /// # Returns
    ///
    /// List of opportunities totaling at least `min_capital`
    pub async fn scan_for_capital(
        &self,
        min_capital: rust_decimal::Decimal,
    ) -> Result<Vec<ArbitrageOpportunity>, DetectionError> {
        let all_opportunities = self.scan().await?;

        let mut selected = Vec::new();
        let mut total_capital = rust_decimal::Decimal::ZERO;

        for opp in all_opportunities {
            let capital_required = opp.capital_required(opp.quantity);
            selected.push(opp);
            total_capital += capital_required;

            if total_capital >= min_capital {
                break;
            }
        }

        Ok(selected)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: Real API tests are integration tests (not unit tests)
    // These tests verify the logic, not the API interaction

    #[test]
    fn test_detection_error_display() {
        let err = DetectionError::ApiError("Connection failed".to_string());
        assert_eq!(format!("{}", err), "API error: Connection failed");

        let err = DetectionError::NoMarketsAvailable;
        assert_eq!(format!("{}", err), "No markets available to scan");
    }
}
