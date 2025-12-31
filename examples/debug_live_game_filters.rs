//! Debug why LIVE games aren't passing strategy filters
//!
//! Run: cargo run --example debug_live_game_filters

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::strategy::loader::load_strategy;
use calchas::strategy::evaluator::evaluate_strategy;
use calchas::trading::PriceTracker;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Debug: Why aren't LIVE games passing filters?");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Load strategy
    let strategy = load_strategy("strategies/order-flow-imbalance.json")?;
    tracing::info!("Loaded strategy: {}", strategy.name);
    tracing::info!("");

    // Fetch LIVE games (college football + NBA)
    let sports_series = vec!["KXNCAAFGAME", "KXNBAGAME"];

    let mut all_markets = Vec::new();
    for series in &sports_series {
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: None,
            status: Some("open".to_string()),
            series_ticker: Some(series.to_string()),
            min_close_ts: None,
            max_close_ts: None,
        };

        let response = client.get_markets(request).await?;
        all_markets.extend(response.markets);
    }

    tracing::info!("Fetched {} markets", all_markets.len());

    // Filter to LIVE games (expiring soon)
    let now = Utc::now();
    let mut live_games = Vec::new();

    for market in &all_markets {
        if let Some(exp_time) = market.expected_expiration_time {
            let hours_to_exp = (exp_time - now).num_hours();
            let vol_ratio = if market.volume > 0 {
                (market.volume_24h as f64 / market.volume as f64) * 100.0
            } else {
                0.0
            };

            // LIVE heuristic
            if hours_to_exp >= 0 && hours_to_exp <= 6 && vol_ratio > 30.0 && market.volume > 1000 {
                live_games.push(market.clone());
            }
        }
    }

    tracing::info!("Found {} LIVE games (expiring <6h, >30% recent volume)", live_games.len());
    tracing::info!("");

    // Test each LIVE game against strategy filters
    let mut price_tracker = PriceTracker::new();

    for market in live_games.iter().take(10) {
        let market_converted: calchas::models::Market = market.clone().into();

        tracing::info!("════════════════════════════════════════════════════════════");
        tracing::info!("Market: {}", market.ticker);
        tracing::info!("Event: {}", market.event_ticker);
        tracing::info!("Title: {}", market.title);
        tracing::info!("Volume: {} | 24h: {}", market.volume, market.volume_24h);
        tracing::info!("Price: YES ${:.4} | NO ${:.4}",
            market_converted.yes_price,
            market_converted.no_price
        );
        tracing::info!("");

        // Simulate price tracking (need at least 2 snapshots for momentum)
        price_tracker.record_price(
            &market_converted.id,
            market_converted.yes_price,
            market_converted.no_price
        );

        // Wait a bit and record again to simulate time passing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        price_tracker.record_price(
            &market_converted.id,
            market_converted.yes_price,
            market_converted.no_price
        );

        // Evaluate against strategy
        let result = evaluate_strategy(
            &strategy,
            &market_converted,
            &price_tracker,
            &client
        ).await;

        match result {
            Ok(Some(signal)) => {
                tracing::info!("✅ PASSES FILTERS!");
                tracing::info!("   Signal: {:?}", signal.direction);
                tracing::info!("   Confidence: {:.2}", signal.confidence);
            }
            Ok(None) => {
                tracing::info!("❌ REJECTED - Strategy returned None (filters failed)");
                tracing::info!("   Check: momentum, OFI, volume, price range");
            }
            Err(e) => {
                tracing::info!("❌ ERROR: {}", e);
            }
        }
        tracing::info!("");
    }

    Ok(())
}
