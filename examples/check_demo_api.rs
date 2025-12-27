// Quick check to see what's available in Kalshi Demo API
// This fetches ALL markets (no status filter) to diagnose demo environment

use calchas::config::AppConfig;
use calchas::kalshi::{GetMarketsRequest, KalshiClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking Kalshi Demo API...\n");

    // Load configuration
    let config = AppConfig::load_with_env_default()?;
    println!("API: {}", if config.kalshi.use_demo { "DEMO" } else { "PRODUCTION" });
    println!("Key ID: {}", config.kalshi.api_key_id);
    println!();

    // Create client
    let client = KalshiClient::from_config(&config.kalshi)?;
    println!("Base URL: {}\n", client.base_url());

    // Test 1: Fetch ALL markets (no filters)
    println!("Test 1: Fetching ALL markets (no status filter)...");
    let request = GetMarketsRequest {
        limit: Some(100),
        ..Default::default()
    };

    match client.get_markets(request).await {
        Ok(response) => {
            println!("✓ Success! Found {} markets", response.markets.len());

            if response.markets.is_empty() {
                println!("\n⚠️  Demo environment has ZERO markets");
                println!("This could mean:");
                println!("  - Demo API is empty/reset");
                println!("  - Authentication is working but no data available");
                println!("  - You might need to use production API instead");
            } else {
                println!("\nMarket statuses:");
                let mut status_counts = std::collections::HashMap::new();
                for market in &response.markets {
                    *status_counts.entry(market.status.as_str()).or_insert(0) += 1;
                }
                for (status, count) in status_counts {
                    println!("  - {}: {}", status, count);
                }

                println!("\nSample market:");
                if let Some(market) = response.markets.first() {
                    println!("  Ticker: {}", market.ticker);
                    println!("  Title: {}", market.title);
                    println!("  Status: {}", market.status);
                    println!("  Category: {}", market.category);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed: {}", e);
            println!("\nThis indicates an authentication or API error.");
            return Err(e.into());
        }
    }

    println!();

    // Test 2: Try fetching only "open" markets
    println!("Test 2: Fetching only OPEN markets...");
    let request = GetMarketsRequest {
        limit: Some(100),
        status: Some("open".to_string()),
        ..Default::default()
    };

    match client.get_markets(request).await {
        Ok(response) => {
            println!("✓ Found {} open markets", response.markets.len());
        }
        Err(e) => {
            println!("✗ Failed: {}", e);
        }
    }

    println!();
    println!("=============================================================================");
    println!("CONCLUSION");
    println!("=============================================================================");
    println!("If you see 0 markets, the Kalshi Demo API might be empty right now.");
    println!("Consider:");
    println!("  1. Using production API (set CALCHAS__KALSHI__USE_DEMO=false)");
    println!("  2. Contacting Kalshi support about demo environment");
    println!("  3. Continuing with Phase 3 development (filtering logic works regardless)");
    println!();

    Ok(())
}
