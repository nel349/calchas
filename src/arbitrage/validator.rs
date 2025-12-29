//! Phase 2 Arbitrage Validation Script
//!
//! Validates whether Kalshi markets have correlated arbitrage opportunities
//! based on logical constraints (NOT statistical correlations - that requires
//! historical data collection).
//!
//! Tests two types of LOGIC-BASED arbitrage:
//!
//! 1. **Series-Level Constraints**: Hierarchical impossibilities
//!    - Example: P(make playoffs) MUST >= P(win division)
//!    - Example: P(win division) MUST >= P(win championship)
//!    - If violated → guaranteed arbitrage (sell higher, buy lower)
//!
//! 2. **Temporal Constraints**: Time-based impossibilities
//!    - Example: P(rain by 6pm) MUST >= P(rain by 12pm)
//!    - Example: P(>30 degrees by 5pm) MUST >= P(>30 degrees by 1pm)
//!    - If violated → guaranteed arbitrage
//!
//! These are MATH-BASED edges (like cross-market arbitrage), not statistical.
//! No historical data needed - just logical impossibilities.

use std::collections::HashMap;
use std::sync::Arc;
use rust_decimal::Decimal;

use crate::kalshi::KalshiClient;
use crate::models::Market;

/// Result of Phase 2 validation analysis
#[derive(Debug)]
pub struct ValidationResult {
    /// Total markets scanned
    pub total_markets: usize,

    /// Number of unique events (event_ticker groups)
    pub total_events: usize,

    /// Events with multiple markets (candidates for correlation)
    pub events_with_multiple_markets: usize,

    /// Series-level violations found
    pub series_violations: Vec<SeriesViolation>,

    /// Temporal violations found
    pub temporal_violations: Vec<TemporalViolation>,

    /// Markets grouped by event (for debugging)
    pub event_groups: HashMap<String, Vec<Market>>,
}

/// A series-level constraint violation (logical impossibility)
#[derive(Debug, Clone)]
pub struct SeriesViolation {
    /// Event ticker grouping these markets
    pub event_ticker: String,

    /// Lower-level market (e.g., "Make playoffs")
    pub lower_market: Market,

    /// Higher-level market (e.g., "Win championship")
    pub higher_market: Market,

    /// Price difference (higher_price - lower_price)
    /// Should be <= 0, but is > 0 (violation!)
    pub price_diff: Decimal,

    /// Arbitrage edge percentage
    pub edge_pct: Decimal,
}

/// A temporal constraint violation (time-based impossibility)
#[derive(Debug, Clone)]
pub struct TemporalViolation {
    /// Event ticker grouping these markets
    pub event_ticker: String,

    /// Earlier time market (e.g., "Rain by 12pm")
    pub earlier_market: Market,

    /// Later time market (e.g., "Rain by 6pm")
    pub later_market: Market,

    /// Price difference (earlier_price - later_price)
    /// Should be <= 0, but is > 0 (violation!)
    pub price_diff: Decimal,

    /// Arbitrage edge percentage
    pub edge_pct: Decimal,
}

/// Phase 2 validator
pub struct Phase2Validator {
    kalshi_client: Arc<KalshiClient>,
}

impl Phase2Validator {
    /// Create a new Phase 2 validator
    pub fn new(kalshi_client: Arc<KalshiClient>) -> Self {
        Phase2Validator { kalshi_client }
    }

    /// Run full validation analysis
    ///
    /// Returns detailed statistics and all violations found
    pub async fn validate(&self) -> Result<ValidationResult, Box<dyn std::error::Error>> {
        tracing::info!("🔍 Starting Phase 2 Arbitrage Validation...");
        tracing::info!("   This will analyze ALL Kalshi markets for logical constraint violations");

        // Fetch all markets
        tracing::info!("Step 1/4: Fetching all active markets from Kalshi...");
        let markets = self.fetch_all_markets().await?;
        tracing::info!("   ✓ Fetched {} markets", markets.len());

        // Group by event_ticker
        tracing::info!("Step 2/4: Grouping markets by event_ticker...");
        let event_groups = self.group_by_event(&markets);
        tracing::info!("   ✓ Found {} unique events", event_groups.len());

        // Count events with multiple markets
        let events_with_multiple: usize = event_groups.values()
            .filter(|markets| markets.len() > 1)
            .count();
        tracing::info!("   ✓ {} events have multiple markets (candidates for correlation)", events_with_multiple);

        // Check for series-level violations
        tracing::info!("Step 3/4: Checking for series-level constraint violations...");
        let series_violations = self.check_series_violations(&event_groups);
        tracing::info!("   ✓ Found {} series-level violations", series_violations.len());

        // Check for temporal violations
        tracing::info!("Step 4/4: Checking for temporal constraint violations...");
        let temporal_violations = self.check_temporal_violations(&event_groups);
        tracing::info!("   ✓ Found {} temporal violations", temporal_violations.len());

        Ok(ValidationResult {
            total_markets: markets.len(),
            total_events: event_groups.len(),
            events_with_multiple_markets: events_with_multiple,
            series_violations,
            temporal_violations,
            event_groups,
        })
    }

    /// Fetch all active markets from Kalshi
    async fn fetch_all_markets(&self) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
        use crate::kalshi::types::GetMarketsRequest;

        let now = chrono::Utc::now();
        let min_close = now + chrono::Duration::hours(1);
        let max_close = now + chrono::Duration::days(30);

        let mut all_markets = Vec::new();
        let mut request = GetMarketsRequest {
            limit: Some(1000),
            cursor: None,
            status: Some("open".to_string()),
            min_close_ts: Some(min_close.timestamp()),
            max_close_ts: Some(max_close.timestamp()),
            ..Default::default()
        };

        let mut page = 0;
        loop {
            page += 1;
            tracing::info!("   📡 Fetching page {} from Kalshi API...", page);

            let start = std::time::Instant::now();
            let response = self.kalshi_client.get_markets(request.clone()).await?;
            let elapsed = start.elapsed();

            tracing::info!("   ✓ Received {} markets from page {} in {:.2}s (total: {})",
                response.markets.len(), page, elapsed.as_secs_f64(), all_markets.len() + response.markets.len());

            for kalshi_market in response.markets {
                let market: Market = kalshi_market.into();
                all_markets.push(market);
            }

            match response.cursor {
                Some(cursor) if !cursor.is_empty() => {
                    request.cursor = Some(cursor);
                }
                _ => break,
            }
        }

        Ok(all_markets)
    }

    /// Group markets by event_ticker
    fn group_by_event(&self, markets: &[Market]) -> HashMap<String, Vec<Market>> {
        let mut groups: HashMap<String, Vec<Market>> = HashMap::new();

        for market in markets {
            groups.entry(market.event_ticker.clone())
                .or_insert_with(Vec::new)
                .push(market.clone());
        }

        groups
    }

    /// Check for series-level constraint violations
    ///
    /// Logic: If market A is "Make playoffs" and market B is "Win championship",
    /// then P(championship) CANNOT exceed P(playoffs) - that's impossible!
    ///
    /// Currently, this is a HEURISTIC based on title keywords.
    /// TODO: Parse Kalshi's series hierarchy from API metadata (if available)
    fn check_series_violations(&self, event_groups: &HashMap<String, Vec<Market>>) -> Vec<SeriesViolation> {
        let mut violations = Vec::new();
        let mut examples_shown = 0;

        // Keywords that indicate hierarchy levels (broader → specific)
        let hierarchy_keywords = vec![
            vec!["qualify", "make playoffs", "playoff"],  // Broader (easier)
            vec!["division", "conference"],                // Mid-level
            vec!["championship", "title", "win"],          // Specific (harder)
        ];

        for (_event_ticker, markets) in event_groups {
            // Only check events with multiple markets
            if markets.len() < 2 {
                continue;
            }

            // Show first 5 examples of event groups with hierarchy keywords
            if examples_shown < 5 && markets.iter().any(|m| {
                let title = m.title.to_lowercase();
                hierarchy_keywords.iter().any(|level| {
                    level.iter().any(|kw| title.contains(kw))
                })
            }) {
                examples_shown += 1;
                tracing::info!("   📋 Example #{}: Event has {} markets", examples_shown, markets.len());
                for market in markets.iter().take(3) {
                    tracing::info!("      - {} → YES ask: ${:.2}",
                        market.title.chars().take(60).collect::<String>(),
                        market.yes_ask
                    );
                }
                if markets.len() > 3 {
                    tracing::info!("      ... and {} more markets", markets.len() - 3);
                }
            }

            // Check all pairs for hierarchy violations
            for i in 0..markets.len() {
                for j in 0..markets.len() {
                    if i == j {
                        continue;
                    }

                    let market_a = &markets[i];
                    let market_b = &markets[j];

                    // Check if A is broader than B (lower in hierarchy)
                    if self.is_broader_than(&market_a.title, &market_b.title, &hierarchy_keywords) {
                        // A is broader, so P(A) MUST >= P(B)
                        // If P(B) > P(A), that's a violation!
                        if market_b.yes_ask > market_a.yes_ask {
                            let price_diff = market_b.yes_ask - market_a.yes_ask;
                            let edge_pct = price_diff / market_a.yes_ask;

                            violations.push(SeriesViolation {
                                event_ticker: market_a.event_ticker.clone(),
                                lower_market: market_a.clone(),
                                higher_market: market_b.clone(),
                                price_diff,
                                edge_pct,
                            });
                        }
                    }
                }
            }
        }

        violations
    }

    /// Check if title_a is broader than title_b in the hierarchy
    fn is_broader_than(&self, title_a: &str, title_b: &str, hierarchy: &[Vec<&str>]) -> bool {
        let title_a_lower = title_a.to_lowercase();
        let title_b_lower = title_b.to_lowercase();

        let level_a = hierarchy.iter().position(|keywords| {
            keywords.iter().any(|kw| title_a_lower.contains(kw))
        });

        let level_b = hierarchy.iter().position(|keywords| {
            keywords.iter().any(|kw| title_b_lower.contains(kw))
        });

        match (level_a, level_b) {
            (Some(a), Some(b)) => a < b,  // Lower index = broader
            _ => false,
        }
    }

    /// Check for temporal constraint violations
    ///
    /// Logic: If market A is "Rain by 12pm" and market B is "Rain by 6pm",
    /// then P(rain by 6pm) MUST >= P(rain by 12pm) - time only moves forward!
    ///
    /// Currently, this is a HEURISTIC based on parsing time strings.
    /// TODO: Use Kalshi's actual time metadata if available in API
    fn check_temporal_violations(&self, event_groups: &HashMap<String, Vec<Market>>) -> Vec<TemporalViolation> {
        let mut violations = Vec::new();
        let mut examples_shown = 0;

        for (_event_ticker, markets) in event_groups {
            // Only check events with multiple markets
            if markets.len() < 2 {
                continue;
            }

            // Show first 5 examples with time-based markets
            let has_times = markets.iter().filter(|m| {
                self.extract_time_from_title(&m.title).is_some()
            }).count();

            if examples_shown < 5 && has_times >= 2 {
                examples_shown += 1;
                tracing::info!("   📋 Example #{}: Event has {} markets with times", examples_shown, markets.len());
                for market in markets.iter().filter(|m| self.extract_time_from_title(&m.title).is_some()).take(3) {
                    let time = self.extract_time_from_title(&market.title).unwrap();
                    tracing::info!("      - {} → Time: {}h, YES ask: ${:.2}",
                        market.title.chars().take(50).collect::<String>(),
                        time,
                        market.yes_ask
                    );
                }
            }

            // Check all pairs for temporal violations
            for i in 0..markets.len() {
                for j in 0..markets.len() {
                    if i == j {
                        continue;
                    }

                    let market_a = &markets[i];
                    let market_b = &markets[j];

                    // Try to parse times from titles
                    if let (Some(time_a), Some(time_b)) = (
                        self.extract_time_from_title(&market_a.title),
                        self.extract_time_from_title(&market_b.title),
                    ) {
                        // If A is earlier than B, then P(B) MUST >= P(A)
                        if time_a < time_b && market_a.yes_ask > market_b.yes_ask {
                            let price_diff = market_a.yes_ask - market_b.yes_ask;
                            let edge_pct = price_diff / market_b.yes_ask;

                            violations.push(TemporalViolation {
                                event_ticker: market_a.event_ticker.clone(),
                                earlier_market: market_a.clone(),
                                later_market: market_b.clone(),
                                price_diff,
                                edge_pct,
                            });
                        }
                    }
                }
            }
        }

        violations
    }

    /// Extract time from market title (heuristic)
    ///
    /// Examples:
    /// - "Rain by 12pm" → Some(12)
    /// - "Temperature above 30 by 6pm" → Some(18)  // 6pm = 18 in 24-hour format
    /// - "Will Lakers win?" → None
    fn extract_time_from_title(&self, title: &str) -> Option<u32> {
        let title_lower = title.to_lowercase();

        // Pattern: "by Xpm" or "by Xam"
        if let Some(idx) = title_lower.find("by ") {
            let after_by = &title_lower[idx + 3..];

            // Try to parse hour
            if let Some(hour_str) = after_by.split_whitespace().next() {
                // Remove "am" or "pm"
                let hour_str = hour_str.replace("am", "").replace("pm", "");

                if let Ok(hour) = hour_str.parse::<u32>() {
                    // Convert to 24-hour format if pm
                    let is_pm = after_by.contains("pm");
                    let hour_24 = if is_pm && hour < 12 {
                        hour + 12
                    } else if !is_pm && hour == 12 {
                        0  // 12am = midnight
                    } else {
                        hour
                    };

                    return Some(hour_24);
                }
            }
        }

        None
    }
}

/// Print validation results in a formatted report
pub fn print_validation_report(result: &ValidationResult) {
    println!("\n{}", "=".repeat(80));
    println!("📊 PHASE 2 ARBITRAGE VALIDATION REPORT");
    println!("{}", "=".repeat(80));

    println!("\n📈 MARKET COVERAGE:");
    println!("   Total markets scanned: {}", result.total_markets);
    println!("   Unique events: {}", result.total_events);
    println!("   Events with multiple markets: {} ({:.1}%)",
        result.events_with_multiple_markets,
        (result.events_with_multiple_markets as f64 / result.total_events as f64) * 100.0
    );

    println!("\n🔍 SERIES-LEVEL VIOLATIONS:");
    if result.series_violations.is_empty() {
        println!("   ❌ None found");
    } else {
        println!("   ✅ Found {} violations!", result.series_violations.len());
        println!();
        for (i, violation) in result.series_violations.iter().take(5).enumerate() {
            println!("   [{}] Event: {}", i + 1, violation.event_ticker);
            println!("       Lower market:  {} → ${:.2}",
                violation.lower_market.title.chars().take(50).collect::<String>(),
                violation.lower_market.yes_ask
            );
            println!("       Higher market: {} → ${:.2}",
                violation.higher_market.title.chars().take(50).collect::<String>(),
                violation.higher_market.yes_ask
            );
            println!("       ⚠️  VIOLATION: Higher price (${:.2}) > Lower price (${:.2})",
                violation.higher_market.yes_ask,
                violation.lower_market.yes_ask
            );
            println!("       💰 Edge: {:.2}% | Price diff: ${:.3}",
                violation.edge_pct * Decimal::from(100),
                violation.price_diff
            );
            println!();
        }
        if result.series_violations.len() > 5 {
            println!("   ... and {} more", result.series_violations.len() - 5);
        }
    }

    println!("\n⏰ TEMPORAL VIOLATIONS:");
    if result.temporal_violations.is_empty() {
        println!("   ❌ None found");
    } else {
        println!("   ✅ Found {} violations!", result.temporal_violations.len());
        println!();
        for (i, violation) in result.temporal_violations.iter().take(5).enumerate() {
            println!("   [{}] Event: {}", i + 1, violation.event_ticker);
            println!("       Earlier market: {} → ${:.2}",
                violation.earlier_market.title.chars().take(50).collect::<String>(),
                violation.earlier_market.yes_ask
            );
            println!("       Later market:   {} → ${:.2}",
                violation.later_market.title.chars().take(50).collect::<String>(),
                violation.later_market.yes_ask
            );
            println!("       ⚠️  VIOLATION: Earlier price (${:.2}) > Later price (${:.2})",
                violation.earlier_market.yes_ask,
                violation.later_market.yes_ask
            );
            println!("       💰 Edge: {:.2}% | Price diff: ${:.3}",
                violation.edge_pct * Decimal::from(100),
                violation.price_diff
            );
            println!();
        }
        if result.temporal_violations.len() > 5 {
            println!("   ... and {} more", result.temporal_violations.len() - 5);
        }
    }

    println!("\n📊 SUMMARY:");
    let total_violations = result.series_violations.len() + result.temporal_violations.len();
    if total_violations == 0 {
        println!("   ❌ NO logical arbitrage opportunities found");
        println!("   ⚠️  Kalshi markets are efficiently priced (no constraint violations)");
        println!("   💡 Phase 2 Type 1 (statistical correlations) may still work");
        println!("      → Requires 30 days of historical data collection");
    } else {
        println!("   ✅ Total violations found: {}", total_violations);
        println!("   💰 Phase 2 logical arbitrage IS VIABLE on Kalshi!");
        println!("   📈 Next step: Build execution engine for Phase 2");
    }

    println!("\n{}", "=".repeat(80));
}
