//! Market diagnostic tool
//!
//! This example fetches markets with minimal filters to understand:
//! 1. What markets are actually available
//! 2. Why live sports games might not be picked up
//! 3. Which filter is the bottleneck
//!
//! Run with:
//! ```bash
//! cargo run --example diagnose_markets
//! ```

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use chrono::Utc;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Market Diagnostic Tool");
    tracing::info!("");

    // Load configuration
    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch markets with MINIMAL filters (just status=open)
    tracing::info!("=== Fetching ALL open markets (no filters) ===");
    tracing::info!("");

    let mut all_markets = Vec::new();
    let mut cursor = None;
    let max_pages = 5; // Fetch first 5000 markets

    for page in 1..=max_pages {
        tracing::info!("Fetching page {}...", page);

        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: None,
            min_close_ts: None,  // NO time filter
            max_close_ts: None,
        };

        let response = client.get_markets(request).await?;

        tracing::info!("  Received {} markets", response.markets.len());
        all_markets.extend(response.markets);

        cursor = response.cursor;
        if cursor.is_none() {
            tracing::info!("  No more pages");
            break;
        }
    }

    tracing::info!("");
    tracing::info!("Total markets fetched: {}", all_markets.len());
    tracing::info!("");

    // Analyze by category
    let mut by_category: HashMap<String, usize> = HashMap::new();
    for market in &all_markets {
        *by_category.entry(market.category.clone()).or_insert(0) += 1;
    }

    tracing::info!("=== Markets by Category ===");
    let mut categories: Vec<_> = by_category.iter().collect();
    categories.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    for (category, count) in categories {
        tracing::info!("  {}: {}", category, count);
    }
    tracing::info!("");

    // Focus on sports markets
    let sports_markets: Vec<_> = all_markets
        .iter()
        .filter(|m| m.category.to_lowercase().contains("sports")
                 || m.category.to_lowercase().contains("nfl")
                 || m.category.to_lowercase().contains("nba")
                 || m.category.to_lowercase().contains("mlb"))
        .collect();

    tracing::info!("=== Sports Markets: {} ===", sports_markets.len());
    tracing::info!("");

    // Analyze volume distribution for sports
    let mut volume_ranges = HashMap::new();
    for market in &sports_markets {
        let range = match market.volume {
            v if v < 100 => "0-100",
            v if v < 1000 => "100-1K",
            v if v < 5000 => "1K-5K",
            v if v < 10000 => "5K-10K",
            v if v < 50000 => "10K-50K",
            _ => "50K+",
        };
        *volume_ranges.entry(range).or_insert(0) += 1;
    }

    tracing::info!("Volume distribution for sports markets:");
    for range in ["0-100", "100-1K", "1K-5K", "5K-10K", "10K-50K", "50K+"] {
        let count = volume_ranges.get(range).unwrap_or(&0);
        tracing::info!("  {}: {} markets", range, count);
    }
    tracing::info!("");

    // Show top 20 sports markets by volume
    let mut sports_sorted = sports_markets.clone();
    sports_sorted.sort_by_key(|m| std::cmp::Reverse(m.volume));

    tracing::info!("=== Top 20 Sports Markets by Volume ===");
    for (i, market) in sports_sorted.iter().take(20).enumerate() {
        let time_to_close = (market.close_time - Utc::now()).num_minutes();

        tracing::info!(
            "{}. {} | Vol: {} | OI: {} | Closes in: {}min",
            i + 1,
            market.ticker,
            market.volume,
            market.open_interest,
            time_to_close
        );
    }
    tracing::info!("");

    // Apply strategy filters progressively to see where markets drop
    tracing::info!("=== Applying Strategy Filters (Progressive) ===");
    tracing::info!("");

    let min_volume = 10000;
    let min_open_interest = 2000;
    let min_price = rust_decimal::Decimal::new(25, 2); // 0.25
    let max_price = rust_decimal::Decimal::new(75, 2); // 0.75

    let after_volume: Vec<_> = sports_markets
        .iter()
        .filter(|m| m.volume >= min_volume)
        .collect();

    tracing::info!("After volume filter (>= {}): {} markets", min_volume, after_volume.len());

    let after_oi: Vec<_> = after_volume
        .iter()
        .filter(|m| m.open_interest >= min_open_interest)
        .collect();

    tracing::info!("After open interest filter (>= {}): {} markets", min_open_interest, after_oi.len());

    let after_price: Vec<_> = after_oi
        .iter()
        .filter(|m| {
            let yes_price = rust_decimal::Decimal::new(m.yes_ask as i64, 2);
            yes_price >= min_price && yes_price <= max_price
        })
        .collect();

    tracing::info!("After price filter ({}-{}): {} markets", min_price, max_price, after_price.len());
    tracing::info!("");

    // Show what's left after filters
    if !after_price.is_empty() {
        tracing::info!("=== Markets Passing All Filters ===");
        for market in after_price.iter().take(10) {
            let yes_price = rust_decimal::Decimal::new(market.yes_ask as i64, 2);
            let no_price = rust_decimal::Decimal::new(market.no_ask as i64, 2);
            let time_to_close = (market.close_time - Utc::now()).num_minutes();

            tracing::info!(
                "  {} | YES: ${:.2} | NO: ${:.2} | Vol: {} | OI: {} | Closes: {}min",
                market.ticker,
                yes_price,
                no_price,
                market.volume,
                market.open_interest,
                time_to_close
            );
        }
    } else {
        tracing::warn!("⚠️  NO sports markets pass all filters!");
    }
    tracing::info!("");

    // Recommendation
    tracing::info!("=== DIAGNOSIS ===");

    let sports_with_low_volume: Vec<_> = sports_markets
        .iter()
        .filter(|m| m.volume < min_volume && m.volume >= 1000)
        .collect();

    if sports_with_low_volume.len() > 10 {
        tracing::warn!(
            "⚠️  BOTTLENECK FOUND: {} sports markets have volume 1K-10K (excluded by min_volume filter)",
            sports_with_low_volume.len()
        );
        tracing::warn!("   Current filter: min_volume >= {}", min_volume);
        tracing::warn!("   Recommendation: Lower to 1000-2000 to capture live games");
    }

    if after_volume.len() > 0 && after_oi.len() == 0 {
        tracing::warn!("⚠️  BOTTLENECK FOUND: Open interest filter too strict");
        tracing::warn!("   Markets passing volume: {}", after_volume.len());
        tracing::warn!("   Markets passing open interest: {}", after_oi.len());
    }

    if after_oi.len() > 0 && after_price.len() == 0 {
        tracing::warn!("⚠️  BOTTLENECK FOUND: Price filter too strict");
        tracing::warn!("   Markets passing OI: {}", after_oi.len());
        tracing::warn!("   Markets passing price: {}", after_price.len());
    }

    Ok(())
}
