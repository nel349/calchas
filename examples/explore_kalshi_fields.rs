//! Explore all Kalshi market fields to understand what data is available

use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    // Try with series_ticker to get sports/politics markets
    println!("=== TRYING TO FIND CATEGORIZED MARKETS ===\n");

    let request = GetMarketsRequest {
        status: Some("open".to_string()),
        limit: Some(100),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    // Look for markets with non-empty categories
    let mut category_examples: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    for market in &response.markets {
        let cat = if market.category.is_empty() {
            "EMPTY".to_string()
        } else {
            market.category.clone()
        };

        category_examples.entry(cat).or_insert_with(Vec::new).push(market.ticker.clone());
    }

    println!("Category distribution ({} markets total):", response.markets.len());
    for (cat, tickers) in category_examples.iter() {
        println!("  '{}': {} markets", cat, tickers.len());
        // Show first 3 examples
        for (i, ticker) in tickers.iter().take(3).enumerate() {
            println!("    Example {}: {}", i + 1, ticker);
        }
    }

    println!("\n=== CHECKING IF CATEGORIES EXIST IN API ===");
    println!("It appears Kalshi may not be populating the 'category' field.");
    println!("We might need to:");
    println!("  1. Use event_ticker patterns (e.g., 'KX' prefix for prediction markets)");
    println!("  2. Parse market titles for keywords");
    println!("  3. Accept that category data is not available from this API");

    Ok(())
}
