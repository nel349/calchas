//! Check what status values actually exist in the wild

use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("Checking status values across multiple requests...\n");

    let mut status_counts: HashMap<String, usize> = HashMap::new();

    // Try different status filters
    for filter in &[Some("open"), Some("closed"), Some("settled"), None] {
        let request = GetMarketsRequest {
            status: filter.map(|s| s.to_string()),
            limit: Some(100),
            ..Default::default()
        };

        println!("Fetching markets with status filter: {:?}", filter);

        match client.get_markets(request).await {
            Ok(response) => {
                println!("  Got {} markets", response.markets.len());
                for market in &response.markets {
                    *status_counts.entry(market.status.clone()).or_insert(0) += 1;
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }
    }

    println!("\n=== STATUS VALUES FOUND ===\n");

    let mut statuses: Vec<_> = status_counts.iter().collect();
    statuses.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

    for (status, count) in statuses {
        println!("{:20} : {} markets", format!("\"{}\"", status), count);
    }

    println!("\n=== RECOMMENDATIONS ===\n");
    println!("Update MarketStatus enum to match these ACTUAL values:");
    println!("1. Remove unused variants (PreLaunch, Finalized)");
    println!("2. Add Active variant if 'active' is the real value");
    println!("3. Document the mapping clearly");

    Ok(())
}
