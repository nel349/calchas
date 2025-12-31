use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("Checking CRYPTO markets (KXBTC)...\n");

    let request = GetMarketsRequest {
        limit: Some(5),
        status: Some("open".to_string()),
        series_ticker: Some("KXBTC".to_string()),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    if response.markets.is_empty() {
        println!("No crypto markets found!");
        return Ok(());
    }

    let market = &response.markets[0];

    println!("Ticker: {}", market.ticker);
    println!("expected_expiration_time: {:?}", market.expected_expiration_time);
    println!("expiration_time:          {}", market.expiration_time);
    println!("close_time:               {}", market.close_time);

    if let Some(expected) = market.expected_expiration_time {
        println!("\n✅ Crypto has expected_expiration_time: {}", expected);
    } else {
        println!("\n⚠️  Crypto missing expected_expiration_time, using expiration_time fallback");
        println!("   This is fine - expiration_time IS the actual settlement for crypto");
    }

    Ok(())
}
