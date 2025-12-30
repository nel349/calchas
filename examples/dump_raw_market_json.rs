use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("Fetching Bitcoin market...\n");

    // Fetch Bitcoin markets
    let request = GetMarketsRequest {
        limit: Some(100),
        status: Some("open".to_string()),
        series_ticker: Some("KXBTC".to_string()),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    println!("Got {} markets from API\n", response.markets.len());

    if response.markets.is_empty() {
        println!("No markets returned!");
        return Ok(());
    }

    // Print first few tickers to see what we got
    println!("First few market tickers:");
    for (i, market) in response.markets.iter().take(5).enumerate() {
        println!("  {}: {}", i+1, market.ticker);
    }
    println!();

    // Look for the specific Bitcoin market
    let mut found = false;
    for market in &response.markets {
        if market.ticker.contains("25DEC2919") {
            found = true;
            println!("═══════════════════════════════════════════════════════════");
            println!("FOUND IT! RAW MARKET DATA - ALL FIELDS:");
            println!("═══════════════════════════════════════════════════════════\n");

            // Print as pretty JSON to see EVERYTHING
            let json = serde_json::to_string_pretty(&market)?;
            println!("{}", json);

            println!("\n═══════════════════════════════════════════════════════════");
            break;
        }
    }

    if !found {
        println!("Market with '25DEC2919' not found in these {} markets", response.markets.len());
        println!("Dumping first Bitcoin market instead:\n");

        if !response.markets.is_empty() {
            let json = serde_json::to_string_pretty(&response.markets[0])?;
            println!("{}", json);
        }
    }

    Ok(())
}
