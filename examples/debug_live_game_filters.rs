//! Debug why LIVE games aren't passing strategy filters
//!
//! Run: cargo run --example debug_live_game_filters

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::strategy::{StrategyLoader, StrategyEvaluator};
use calchas::trading::{PriceTracker, VolumeTracker, OrderFlowTracker};
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
    let strategy = StrategyLoader::load("strategies/order-flow-imbalance.json")?;
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

    // Convert to internal Market type
    let markets: Vec<calchas::models::Market> = live_games
        .iter()
        .take(10)
        .map(|m| m.clone().into())
        .collect();

    // Initialize trackers
    let mut price_tracker = PriceTracker::new();
    let mut volume_tracker = VolumeTracker::new();
    let order_flow_tracker = OrderFlowTracker::new();

    // Record initial data for all markets
    for market in &markets {
        price_tracker.record_price(
            &market.id,
            market.yes_price,
            market.no_price
        );
        volume_tracker.record_volume(&market.id, market.volume);
    }

    // Evaluate all LIVE markets
    tracing::info!("═══════════════════════════════════════════════════════════");
    tracing::info!("Evaluating {} LIVE games against strategy filters", markets.len());
    tracing::info!("═══════════════════════════════════════════════════════════");
    tracing::info!("");

    match StrategyEvaluator::evaluate(
        &markets,
        &strategy,
        Some(&price_tracker),
        Some(&volume_tracker),
        Some(&order_flow_tracker),
    ) {
        Ok(signals) => {
            tracing::info!("✅ Generated {} signals from {} LIVE games", signals.len(), markets.len());
            tracing::info!("");

            for signal in &signals {
                let market = markets.iter().find(|m| m.id == signal.market_id).unwrap();
                tracing::info!("Signal: {}", market.ticker);
                tracing::info!("  Side: {:?}", signal.side);
                tracing::info!("  Price: YES ${:.4} | NO ${:.4}", market.yes_price, market.no_price);
                tracing::info!("  Volume: {} | 24h: {}", market.volume, market.volume_24h);
                tracing::info!("");
            }

            // Show rejected markets
            let signal_market_ids: std::collections::HashSet<_> = signals.iter().map(|s| &s.market_id).collect();
            let rejected_markets: Vec<_> = markets.iter()
                .filter(|m| !signal_market_ids.contains(&m.id))
                .collect();

            if !rejected_markets.is_empty() {
                tracing::info!("❌ Rejected {} LIVE games (failed filters)", rejected_markets.len());
                for market in rejected_markets.iter().take(5) {
                    tracing::info!("  - {} (${:.4})", market.ticker, market.yes_price);
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Evaluation failed: {:?}", e);
        }
    }

    Ok(())
}
