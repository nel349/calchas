//! Fetch markets by series_ticker
//!
//! Try fetching specific sports series

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use chrono::Utc;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🎯 Fetching Markets by Series Ticker");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Sports series to try
    let series = vec![
        ("KXNFLGAME", "NFL Games"),
        ("KXNCAAMBGAME", "NCAA Men's Basketball"),
        ("KXNBAGAME", "NBA Games"),
        ("KXMLBGAME", "MLB Games"),
        ("KXNFLWINS", "NFL Season Wins"),
    ];

    for (series_ticker, label) in series {
        tracing::info!("=== {} ({}) ===", label, series_ticker);

        let request = GetMarketsRequest {
            limit: Some(100),
            cursor: None,
            status: Some("open".to_string()),
            series_ticker: Some(series_ticker.to_string()),
            min_close_ts: None,
            max_close_ts: None,
        };

        match client.get_markets(request).await {
            Ok(response) => {
                tracing::info!("  Found {} markets", response.markets.len());

                if !response.markets.is_empty() {
                    // Show first 5
                    for (i, market) in response.markets.iter().take(5).enumerate() {
                        let yes_price = Decimal::new(market.yes_ask as i64, 2);
                        let no_price = Decimal::new(market.no_ask as i64, 2);
                        let time_to_close = (market.close_time - Utc::now()).num_hours();

                        tracing::info!(
                            "  {}. {} | YES: ${:.2} NO: ${:.2} | Vol: {} OI: {} | Close: {}h",
                            i + 1,
                            market.ticker,
                            yes_price,
                            no_price,
                            market.volume,
                            market.open_interest,
                            time_to_close
                        );
                    }

                    // Check how many pass our volume filter
                    let with_volume: Vec<_> = response.markets
                        .iter()
                        .filter(|m| m.volume >= 10000)
                        .collect();
                    tracing::info!("  Markets with volume >= 10000: {}", with_volume.len());

                    let with_low_volume: Vec<_> = response.markets
                        .iter()
                        .filter(|m| m.volume >= 1000)
                        .collect();
                    tracing::info!("  Markets with volume >= 1000: {}", with_low_volume.len());
                }

                tracing::info!("");
            }
            Err(e) => {
                tracing::error!("  Error: {}", e);
                tracing::info!("");
            }
        }
    }

    Ok(())
}
