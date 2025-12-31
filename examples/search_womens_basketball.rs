//! Search for Women's Basketball games to find the series ticker
//!
//! Run: cargo run --example search_womens_basketball

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Searching for Women's Basketball games");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Search for known Women's Basketball games from user's list
    let search_terms = vec![
        "Seattle",
        "Santa Clara",
        "Pepperdine",
        "Gonzaga",
        "women",
        "WOMEN",
    ];

    // Fetch ALL open markets
    let request = GetMarketsRequest {
        limit: Some(1000),
        cursor: None,
        status: Some("open".to_string()),
        series_ticker: None,
        min_close_ts: None,
        max_close_ts: None,
    };

    let response = client.get_markets(request).await?;

    tracing::info!("Searching {} markets for Women's Basketball games...", response.markets.len());
    tracing::info!("");

    let mut found = 0;
    for market in &response.markets {
        // Check if title matches Women's Basketball
        if search_terms.iter().any(|term| market.title.contains(term))
            || market.event_ticker.contains("WBB")
            || market.event_ticker.contains("WNCAAB")
        {
            found += 1;
            tracing::info!("Found: {}", market.ticker);
            tracing::info!("  Event Ticker: {}", market.event_ticker);
            tracing::info!("  Title: {}", market.title);
            tracing::info!("  Series: {}", &market.event_ticker.split('-').next().unwrap_or(""));
            tracing::info!("");

            if found >= 10 {
                break;
            }
        }
    }

    if found == 0 {
        tracing::warn!("No Women's Basketball games found in first 1000 markets");
        tracing::info!("This suggests Women's Basketball might not have markets right now,");
        tracing::info!("OR they are beyond the first 1000 markets in API order.");
    }

    Ok(())
}
