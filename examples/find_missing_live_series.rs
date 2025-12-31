//! Find which series contain the missing LIVE games
//!
//! Run: cargo run --example find_missing_live_series

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use chrono::Utc;
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Finding ALL LIVE game series tickers");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch ALL open markets (no series filter)
    tracing::info!("Fetching ALL open markets (this may take a moment)...");
    let mut all_markets = Vec::new();
    let mut cursor = None;
    let mut page = 0;

    loop {
        page += 1;
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: None,  // Fetch ALL
            min_close_ts: None,
            max_close_ts: None,
        };

        let response = client.get_markets(request).await?;
        let count = response.markets.len();
        all_markets.extend(response.markets);

        if page % 5 == 0 {
            tracing::info!("  Page {}: {} markets (total: {})", page, count, all_markets.len());
        }

        cursor = response.cursor;
        if cursor.is_none() || page >= 15 {
            break;
        }
    }

    tracing::info!("Total markets: {}", all_markets.len());
    tracing::info!("");

    // Find LIVE games and track series
    let now = Utc::now();
    let mut live_series = HashSet::new();
    let mut live_count = 0;

    for market in &all_markets {
        if let Some(exp_time) = market.expected_expiration_time {
            let hours_to_exp = (exp_time - now).num_hours();
            let vol_ratio = if market.volume > 0 {
                (market.volume_24h as f64 / market.volume as f64) * 100.0
            } else {
                0.0
            };

            // LIVE criteria
            if hours_to_exp >= 0 && hours_to_exp <= 6 && vol_ratio > 30.0 && market.volume > 1000 {
                live_count += 1;

                // Extract series from event_ticker
                // Event ticker format: KXNCAABGAME-25DEC30BOSUTA
                // Series ticker: KXNCAABGAME
                let series = if let Some(pos) = market.event_ticker.find('-') {
                    &market.event_ticker[..pos]
                } else {
                    &market.event_ticker
                };

                live_series.insert(series.to_string());

                if live_count <= 5 {
                    tracing::info!("LIVE Example: {} | Series: {} | Title: {}",
                        market.ticker, series, market.title);
                }
            }
        }
    }

    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("Found {} LIVE games across {} unique series:", live_count, live_series.len());
    tracing::info!("═══════════════════════════════════════════════════════════════");

    let mut series_vec: Vec<String> = live_series.into_iter().collect();
    series_vec.sort();

    for series in &series_vec {
        tracing::info!("  - {}", series);
    }

    tracing::info!("");
    tracing::info!("Add these to your sports_series vector:");
    tracing::info!("let sports_series = vec!{:?};", series_vec);

    Ok(())
}
