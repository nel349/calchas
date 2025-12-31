//! Search for Tennessee vs Illinois game and analyze why it's not caught
//!
//! Run: cargo run --example search_tennessee_illinois

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::trading::PriceTracker;
use chrono::Utc;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Searching for Tennessee vs Illinois game");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch ALL markets
    tracing::info!("Fetching all open markets...");
    let mut all_markets = Vec::new();
    let mut cursor = None;
    let mut page = 0;

    loop {
        page += 1;
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: None,
            min_close_ts: None,
            max_close_ts: None,
        };

        let response = client.get_markets(request).await?;
        let count = response.markets.len();
        all_markets.extend(response.markets);

        if page % 5 == 0 {
            tracing::info!("  Page {}: {} markets (total: {})", page, count, all_markets.len());
        }

        cursor = response.cursor;
        if cursor.is_none() {
            break;
        }

        if page >= 20 {
            tracing::warn!("Stopping at 20 pages (20,000 markets)");
            break;
        }
    }

    tracing::info!("Total markets fetched: {}", all_markets.len());
    tracing::info!("");

    // Search for Tennessee vs Illinois
    let target_patterns = [
        "TENNESSEE",
        "ILLINOIS",
        "TENN",
        "ILL",
    ];

    tracing::info!("=== Searching for Tennessee or Illinois ===");
    let mut tenn_ill_markets = Vec::new();

    for market in &all_markets {
        let title_upper = market.title.to_uppercase();
        let ticker_upper = market.ticker.to_uppercase();

        if target_patterns.iter().any(|p| title_upper.contains(p) || ticker_upper.contains(p)) {
            tenn_ill_markets.push(market);
        }
    }

    tracing::info!("Found {} Tennessee/Illinois markets", tenn_ill_markets.len());
    tracing::info!("");

    // Analyze each market
    let mut price_tracker = PriceTracker::new();

    for market in &tenn_ill_markets {
        let market_converted: calchas::models::Market = (*market).clone().into();

        let yes_price_decimal = market_converted.yes_price;
        let no_price_decimal = market_converted.no_price;
        let time_to_close = (market.close_time - Utc::now()).num_hours();

        tracing::info!("════════════════════════════════════════════════════════════");
        tracing::info!("Market: {}", market.ticker);
        tracing::info!("Title: {}", market.title);
        tracing::info!("Event Ticker: {}", market.event_ticker);
        tracing::info!("");
        tracing::info!("Prices:");
        tracing::info!("  YES: ${:.4} (bid: ${:.4}, ask: ${:.4})",
            yes_price_decimal,
            market_converted.yes_bid,
            market_converted.yes_ask);
        tracing::info!("  NO:  ${:.4} (bid: ${:.4}, ask: ${:.4})",
            no_price_decimal,
            market_converted.no_bid,
            market_converted.no_ask);
        tracing::info!("");
        tracing::info!("Volume: {}", market.volume);
        tracing::info!("Open Interest: {}", market.open_interest);
        tracing::info!("Time to close: {}h", time_to_close);
        tracing::info!("Status: {}", market.status);
        tracing::info!("");

        // Check filters
        tracing::info!("FILTER ANALYSIS:");

        // Price filter [0.05 - 0.95]
        let expensive_side = yes_price_decimal.max(no_price_decimal);
        let price_pass = expensive_side >= Decimal::new(5, 2) && expensive_side <= Decimal::new(95, 2);
        tracing::info!("  Price [0.05-0.95]: {} (expensive side: ${:.4})",
            if price_pass { "✅ PASS" } else { "❌ FAIL" },
            expensive_side);

        // Volume filter >= 5000
        let volume_pass = market.volume >= 5000;
        tracing::info!("  Volume >= 5000: {} (actual: {})",
            if volume_pass { "✅ PASS" } else { "❌ FAIL" },
            market.volume);

        // Simulate momentum check - record prices and check
        price_tracker.record_price(&market_converted.id, yes_price_decimal, no_price_decimal);

        // Wait and record again to simulate 5 min lookback
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        price_tracker.record_price(&market_converted.id, yes_price_decimal, no_price_decimal);

        let momentum_5min = price_tracker.calculate_momentum(
            &market_converted.id,
            chrono::Duration::minutes(5)
        );

        let momentum_pass = if let Some(mom) = momentum_5min {
            let pass = mom.abs() >= Decimal::new(5, 1); // 0.5%
            tracing::info!("  Momentum >= 0.5% (5min): {} (actual: {:.2}%)",
                if pass { "✅ PASS" } else { "❌ FAIL" },
                mom);
            pass
        } else {
            tracing::info!("  Momentum >= 0.5% (5min): ❌ FAIL (no data - need 2+ snapshots)");
            false
        };

        tracing::info!("");
        let overall_pass = price_pass && volume_pass && momentum_pass;
        tracing::info!("OVERALL: {}", if overall_pass { "✅ WOULD PASS PHASE 1" } else { "❌ BLOCKED IN PHASE 1" });
        tracing::info!("════════════════════════════════════════════════════════════");
        tracing::info!("");
    }

    Ok(())
}
