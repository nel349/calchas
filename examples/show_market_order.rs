//! Show the ORDER markets are evaluated (to diagnose prioritization issue)
//!
//! Run: cargo run --example show_market_order

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Showing market evaluation ORDER (to diagnose prioritization)");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch ALL open markets (like the bot does)
    let request = GetMarketsRequest {
        limit: Some(1000),
        cursor: None,
        status: Some("open".to_string()),
        series_ticker: None,
        min_close_ts: None,
        max_close_ts: None,
    };

    let response = client.get_markets(request).await?;
    let markets = response.markets;

    tracing::info!("Fetched {} markets - showing first 50 in API order:", markets.len());
    tracing::info!("");

    let now = Utc::now();

    for (i, market) in markets.iter().take(50).enumerate() {
        let hours_to_exp = if let Some(exp_time) = market.expected_expiration_time {
            (exp_time - now).num_hours()
        } else {
            (market.close_time - now).num_hours()
        };

        let vol_ratio = if market.volume > 0 {
            (market.volume_24h as f64 / market.volume as f64) * 100.0
        } else {
            0.0
        };

        let price = market.last_price as f64 / 100.0;

        tracing::info!(
            "#{:<3} {} | Price: ${:.2} | Vol: {:>8} | 24h: {:>3.0}% | Exp: {:>4}h | {}",
            i + 1,
            market.ticker,
            price,
            market.volume,
            vol_ratio,
            hours_to_exp,
            if hours_to_exp <= 6 && vol_ratio > 30.0 { "🔴 LIVE" } else { "" }
        );
    }

    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("PROBLEM: Bot evaluates markets in THIS ORDER!");
    tracing::info!("If max_concurrent_positions = 30, bot trades first 30 that pass filters.");
    tracing::info!("LIVE games at position #50+ NEVER GET EVALUATED!");
    tracing::info!("═══════════════════════════════════════════════════════════════");

    Ok(())
}
