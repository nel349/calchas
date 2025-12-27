//! Debug raw API response to see what Kalshi is actually sending

use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=============================================================================");
    println!("DEBUG: RAW KALSHI API RESPONSE");
    println!("=============================================================================");
    println!();

    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    let request = GetMarketsRequest {
        status: Some("open".to_string()),
        limit: Some(5),
        ..Default::default()
    };

    println!("Request parameters:");
    println!("  status: {:?}", request.status);
    println!("  limit: {:?}", request.limit);
    println!();

    let response = client.get_markets(request).await?;

    println!("Response cursor: {:?}", response.cursor);
    println!("Markets count: {}", response.markets.len());
    println!();

    // Print first 3 raw markets
    for (i, market) in response.markets.iter().take(3).enumerate() {
        println!("=== RAW MARKET #{} ===", i + 1);
        println!("ticker: {}", market.ticker);
        println!("title: {}", market.title);
        println!("category: {:?}", market.category);
        println!("status: {:?}", market.status);
        println!("yes_bid: {} cents", market.yes_bid);
        println!("yes_ask: {} cents", market.yes_ask);
        println!("no_bid: {} cents", market.no_bid);
        println!("no_ask: {} cents", market.no_ask);
        println!("last_price: {} cents", market.last_price);
        println!("volume: {}", market.volume);
        println!("open_interest: {}", market.open_interest);
        println!("event_ticker: {}", market.event_ticker);
        println!("close_time: {}", market.close_time);
        println!("expiration_time: {}", market.expiration_time);
        println!();

        // Now convert and show what we get
        let converted: calchas::models::Market = market.clone().into();
        println!("=== CONVERTED MARKET #{} ===", i + 1);
        println!("category: {:?}", converted.category);
        println!("status: {:?}", converted.status);
        println!("yes_price: {}", converted.yes_price);
        println!("no_price: {}", converted.no_price);
        println!();
    }

    Ok(())
}
