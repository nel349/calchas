use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("Fetching ANY live sports market...\n");

    let series_list = vec!["KXNBAGAME", "KXNCAAWBGAME", "KXNFLGAME"];

    for series in series_list {
        let request = GetMarketsRequest {
            limit: Some(5),
            status: Some("open".to_string()),
            series_ticker: Some(series.to_string()),
            ..Default::default()
        };

        match client.get_markets(request).await {
            Ok(response) => {
                if !response.markets.is_empty() {
                    let market = &response.markets[0];

                    println!("═══════════════════════════════════════════════════════════");
                    println!("SERIES: {}", series);
                    println!("RAW MARKET DATA - ALL FIELDS:");
                    println!("═══════════════════════════════════════════════════════════\n");

                    // Print as pretty JSON to see EVERYTHING
                    let json = serde_json::to_string_pretty(&market)?;
                    println!("{}", json);

                    println!("\n═══════════════════════════════════════════════════════════");
                    println!("FIELD COUNT AND ANALYSIS:");
                    println!("═══════════════════════════════════════════════════════════\n");

                    // Count fields
                    if let serde_json::Value::Object(map) = serde_json::to_value(&market)? {
                        println!("Total fields in response: {}", map.len());
                        println!("\nAll field names:");
                        for (key, _) in map.iter() {
                            println!("  - {}", key);
                        }
                    }

                    return Ok(());
                }
            }
            Err(e) => println!("Error fetching {}: {}", series, e),
        }
    }

    println!("No markets found");
    Ok(())
}
