//! Inspect real markets from Kalshi API to understand what's available
//! This helps us create realistic test strategies

use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;
use calchas::models::Market;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=============================================================================");
    println!("INSPECTING REAL MARKETS FROM KALSHI API");
    println!("=============================================================================");
    println!();

    // Load config and create client
    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    // Fetch open markets
    let request = GetMarketsRequest {
        status: Some("open".to_string()),
        limit: Some(20),  // Fetch 20 markets for inspection
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    // Convert to Market
    let markets: Vec<Market> = response.markets
        .into_iter()
        .map(|km| km.into())
        .collect();

    println!("Fetched {} markets\n", markets.len());

    if markets.is_empty() {
        println!("No open markets found!");
        return Ok(());
    }

    // Display first 10 markets in detail
    for (i, market) in markets.iter().take(10).enumerate() {
        let now = Utc::now();
        let time_to_event = (market.event_time - now).num_hours();

        println!("Market #{}:", i + 1);
        println!("  Ticker: {}", market.ticker);
        println!("  Title: {}", market.title);
        println!("  Category: {:?}", market.category);
        println!("  Sub-category: {:?}", market.sub_category);
        println!("  Yes Price: ${}", market.yes_price);
        println!("  No Price: ${}", market.no_price);
        println!("  Volume: {}", market.volume);
        println!("  Open Interest: {}", market.open_interest);
        println!("  Time to Event: {} hours", time_to_event);
        println!("  Status: {:?}", market.status);

        // Analyze which side is cheaper
        if market.yes_price < market.no_price {
            println!("  Cheaper Side: Yes (${} vs ${})", market.yes_price, market.no_price);
        } else {
            println!("  Cheaper Side: No (${} vs ${})", market.no_price, market.yes_price);
        }

        println!();
    }

    // Summary statistics
    println!("=============================================================================");
    println!("SUMMARY STATISTICS");
    println!("=============================================================================");
    println!();

    // Category distribution
    let mut category_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for market in &markets {
        let cat = format!("{:?}", market.category);
        *category_counts.entry(cat).or_insert(0) += 1;
    }

    println!("Categories:");
    for (cat, count) in category_counts.iter() {
        println!("  {}: {}", cat, count);
    }
    println!();

    // Price ranges
    let min_yes = markets.iter().map(|m| m.yes_price).min().unwrap();
    let max_yes = markets.iter().map(|m| m.yes_price).max().unwrap();
    let min_no = markets.iter().map(|m| m.no_price).min().unwrap();
    let max_no = markets.iter().map(|m| m.no_price).max().unwrap();

    println!("Price Ranges:");
    println!("  Yes prices: ${} - ${}", min_yes, max_yes);
    println!("  No prices: ${} - ${}", min_no, max_no);
    println!();

    // Volume ranges
    let min_vol = markets.iter().map(|m| m.volume).min().unwrap();
    let max_vol = markets.iter().map(|m| m.volume).max().unwrap();

    println!("Volume Range: {} - {}", min_vol, max_vol);
    println!();

    // Time to event ranges
    let now = Utc::now();
    let min_tte = markets.iter().map(|m| (m.event_time - now).num_hours()).min().unwrap();
    let max_tte = markets.iter().map(|m| (m.event_time - now).num_hours()).max().unwrap();

    println!("Time to Event Range: {} - {} hours", min_tte, max_tte);
    println!();

    println!("=============================================================================");
    println!("Use this data to create a test strategy that will match real markets!");
    println!("=============================================================================");

    Ok(())
}
