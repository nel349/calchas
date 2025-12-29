//! Raw market inspector
//!
//! Shows the actual structure of markets from the API
//! to understand what fields are available.

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

    tracing::info!("🔍 Raw Market Inspector");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch first page
    let request = GetMarketsRequest {
        limit: Some(20),
        cursor: None,
        status: Some("open".to_string()),
        series_ticker: None,
        min_close_ts: None,
        max_close_ts: None,
    };

    let response = client.get_markets(request).await?;

    tracing::info!("Received {} markets", response.markets.len());
    tracing::info!("");

    // Show first 5 markets in detail
    for (i, market) in response.markets.iter().take(5).enumerate() {
        tracing::info!("=== Market {} ===", i + 1);
        tracing::info!("  Ticker: {}", market.ticker);
        tracing::info!("  Title: {}", market.title);
        tracing::info!("  Category: {:?}", market.category);
        tracing::info!("  Subtitle: {}", market.subtitle);
        tracing::info!("  Yes ask: {}", market.yes_ask);
        tracing::info!("  No ask: {}", market.no_ask);
        tracing::info!("  Volume: {}", market.volume);
        tracing::info!("  Open interest: {}", market.open_interest);
        tracing::info!("  Close time: {}", market.close_time);
        tracing::info!("");
    }

    // Look for patterns in tickers to identify sports
    let sports_keywords = ["NFL", "NBA", "MLB", "NHL", "SOCCER", "TENNIS", "GOLF"];
    let mut sports_count = 0;

    tracing::info!("=== Searching for sports markets by ticker ===");
    for market in &response.markets {
        for keyword in &sports_keywords {
            if market.ticker.contains(keyword) {
                sports_count += 1;
                tracing::info!(
                    "  FOUND: {} | {} | Vol: {} | YES: ${:.2}",
                    market.ticker,
                    market.title,
                    market.volume,
                    rust_decimal::Decimal::new(market.yes_ask as i64, 2)
                );
                break;
            }
        }
    }

    tracing::info!("");
    tracing::info!("Sports markets found (by ticker): {}/{}", sports_count, response.markets.len());

    Ok(())
}
