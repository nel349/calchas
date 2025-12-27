// Phase 2 Milestone Demo: Fetch markets from Kalshi Demo API
//
// This example demonstrates:
// - Loading configuration with .env file support
// - Creating authenticated Kalshi client
// - Fetching markets from Kalshi REST API
// - Handling pagination
// - Converting Kalshi markets to generic Market model
//
// Usage:
//   1. Copy .env.example to .env
//   2. Fill in your Kalshi Demo API credentials
//   3. cargo run --example fetch_markets

use calchas::config::AppConfig;
use calchas::kalshi::{GetMarketsRequest, KalshiClient};
use rust_decimal::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    calchas::utils::logging::init();

    println!("=============================================================================");
    println!("CALCHAS - PHASE 2 MILESTONE: FETCH KALSHI MARKETS");
    println!("=============================================================================\n");

    // Load configuration from .env + config.toml
    println!("Loading configuration from .env and config/config.toml...");
    let config = AppConfig::load_with_env_default()?;
    println!("✓ Configuration loaded");
    println!("  - Strategy dir: {}", config.runtime.strategy_dir);
    println!("  - Kalshi API: {}", if config.kalshi.use_demo { "DEMO" } else { "PRODUCTION" });
    println!("  - API Key ID: {}\n", config.kalshi.api_key_id);

    // Create Kalshi client with authentication
    println!("Creating authenticated Kalshi client...");
    let client = KalshiClient::from_config(&config.kalshi)?;
    println!("✓ Kalshi client created");
    println!("  - Base URL: {}\n", client.base_url());

    // Fetch first page of open markets
    println!("Fetching open markets from Kalshi API...");
    let request = GetMarketsRequest {
        limit: Some(50),  // Fetch 50 markets per page
        status: Some("open".to_string()),
        cursor: None,
        series_ticker: None,
    };

    let response = client.get_markets(request).await?;
    println!("✓ Fetched {} markets\n", response.markets.len());

    if response.markets.is_empty() {
        println!("⚠️  No markets found in Kalshi Demo API");
        println!("\nThis is normal - the demo environment typically has limited data.");
        println!("Try these options:");
        println!("  1. Remove the status filter to see all markets (including closed)");
        println!("  2. Check https://demo.kalshi.com/markets to see what's available");
        println!("  3. The demo might have zero active markets at certain times\n");
        return Ok(());
    }

    // Display market details
    println!("=============================================================================");
    println!("MARKET DATA");
    println!("=============================================================================\n");

    for (i, market) in response.markets.iter().enumerate() {
        println!("Market #{}: {}", i + 1, market.ticker);
        println!("  Title: {}", market.title);
        println!("  Category: {}", market.category);
        println!("  Status: {}", market.status);

        // Calculate mid prices from bid/ask
        let yes_bid_decimal = Decimal::new(market.yes_bid, 2);
        let yes_ask_decimal = Decimal::new(market.yes_ask, 2);
        let no_bid_decimal = Decimal::new(market.no_bid, 2);
        let no_ask_decimal = Decimal::new(market.no_ask, 2);

        let yes_mid = (yes_bid_decimal + yes_ask_decimal) / Decimal::from(2);
        let no_mid = (no_bid_decimal + no_ask_decimal) / Decimal::from(2);

        println!("  Prices:");
        println!("    Yes: ${:.2} (bid: ${:.2}, ask: ${:.2})",
            yes_mid, yes_bid_decimal, yes_ask_decimal);
        println!("    No:  ${:.2} (bid: ${:.2}, ask: ${:.2})",
            no_mid, no_bid_decimal, no_ask_decimal);

        println!("  Volume: {} contracts", market.volume);
        println!("  Open Interest: {} contracts", market.open_interest);

        println!("  Trading:");
        println!("    Opens: {}", market.open_time);
        println!("    Closes: {}", market.close_time);
        println!("    Expires: {}", market.expiration_time);

        println!();
    }

    // Show pagination info
    if let Some(cursor) = &response.cursor {
        println!("=============================================================================");
        println!("PAGINATION");
        println!("=============================================================================\n");
        println!("More markets available!");
        println!("Next page cursor: {}", cursor);
        println!("\nTo fetch all markets, use client.get_all_markets()");
    } else {
        println!("=============================================================================");
        println!("All markets fetched (no more pages)");
    }

    println!("\n=============================================================================");
    println!("PHASE 2 MILESTONE COMPLETE!");
    println!("=============================================================================\n");
    println!("✓ Successfully authenticated with Kalshi API");
    println!("✓ Fetched and parsed market data");
    println!("✓ Demonstrated RSA-PSS signature authentication");
    println!("✓ Ready to proceed with Phase 3 (Market Filtering)\n");

    Ok(())
}
