//! Fetch real orderbook example
//!
//! This example demonstrates fetching a real orderbook from the Kalshi API
//! and using the RealOrderbookProvider.
//!
//! Run with:
//! ```bash
//! cargo run --example fetch_orderbook
//! ```

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::trading::{RealOrderbookProvider, OrderbookProvider};
use calchas::models::MarketId;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔮 Calchas - Real Orderbook Fetching Example");
    tracing::info!("");

    // Load configuration
    tracing::info!("Loading configuration from .env and config/config.toml");
    let config = AppConfig::load_with_env_default()?;

    // Create Kalshi client
    tracing::info!("Creating Kalshi API client ({})", config.kalshi.base_url());
    let client = Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Test 1: Fetch orderbook directly via client
    tracing::info!("");
    tracing::info!("=== TEST 1: Direct API Call ===");

    // First, get some active markets to test with
    tracing::info!("Fetching active markets to find a test ticker...");
    let markets_response = client.get_markets(calchas::kalshi::types::GetMarketsRequest {
        limit: Some(10),
        cursor: None,
        status: Some("open".to_string()),
        series_ticker: None,
        min_close_ts: None,
        max_close_ts: None,
    }).await?;

    if markets_response.markets.is_empty() {
        tracing::error!("No active markets found!");
        return Ok(());
    }

    let test_ticker = &markets_response.markets[0].ticker;
    tracing::info!("Using market: {}", test_ticker);

    tracing::info!("Fetching orderbook via client.get_orderbook()...");
    let orderbook_response = client.get_orderbook(test_ticker, None).await?;

    tracing::info!("✅ Received orderbook:");

    if let Some(ref orderbook_data) = orderbook_response.orderbook {
        tracing::info!("  YES side: {} price levels", orderbook_data.yes.len());
        tracing::info!("  NO side: {} price levels", orderbook_data.no.len());

        if let Some((price, qty)) = orderbook_data.yes.first() {
            tracing::info!("  Best YES ask: {} cents ({} contracts)", price, qty);
        }

        if let Some((price, qty)) = orderbook_data.no.first() {
            tracing::info!("  Best NO ask: {} cents ({} contracts)", price, qty);
        }
    } else {
        tracing::warn!("  Orderbook is null (no liquidity)");
    }

    // Test 2: Fetch via RealOrderbookProvider
    tracing::info!("");
    tracing::info!("=== TEST 2: RealOrderbookProvider ===");

    let provider = RealOrderbookProvider::new(client.clone());
    let market_id = MarketId::new(test_ticker.clone());

    tracing::info!("Fetching orderbook via RealOrderbookProvider...");
    let orderbook = provider.get_orderbook(&market_id).await?;

    match orderbook {
        Some(ob) => {
            tracing::info!("✅ Received orderbook:");
            tracing::info!("  Market ID: {}", ob.market_id.as_str());
            tracing::info!("  YES asks: {} levels", ob.yes_asks.len());
            tracing::info!("  NO asks: {} levels", ob.no_asks.len());

            if let Some(best_yes_price) = ob.yes_best_ask() {
                let best_yes_qty = ob.yes_best_ask_quantity();
                tracing::info!("  Best YES ask: ${:.2} ({} contracts)", best_yes_price, best_yes_qty);
            }

            if let Some(best_no_price) = ob.no_best_ask() {
                let best_no_qty = ob.no_best_ask_quantity();
                tracing::info!("  Best NO ask: ${:.2} ({} contracts)", best_no_price, best_no_qty);
            }

            if let Some(spread) = ob.spread() {
                tracing::info!("  Spread: ${:.4}", spread);
            }

            // Test orderbook helper methods
            let yes_qty = ob.yes_best_ask_quantity();
            let no_qty = ob.no_best_ask_quantity();
            tracing::info!("  YES best ask quantity: {}", yes_qty);
            tracing::info!("  NO best ask quantity: {}", no_qty);
        }
        None => {
            tracing::warn!("No orderbook returned (market might be inactive)");
        }
    }

    // Test 3: Fetch multiple orderbooks
    tracing::info!("");
    tracing::info!("=== TEST 3: Multiple Orderbooks ===");

    let test_markets: Vec<_> = markets_response.markets
        .iter()
        .take(3)
        .map(|m| MarketId::new(m.ticker.clone()))
        .collect();

    tracing::info!("Fetching orderbooks for {} markets...", test_markets.len());

    for market_id in test_markets {
        match provider.get_orderbook(&market_id).await {
            Ok(Some(ob)) => {
                tracing::info!("  {} - YES: {} levels, NO: {} levels",
                    market_id.as_str(),
                    ob.yes_asks.len(),
                    ob.no_asks.len()
                );
            }
            Ok(None) => {
                tracing::warn!("  {} - No orderbook available", market_id.as_str());
            }
            Err(e) => {
                tracing::error!("  {} - Error: {}", market_id.as_str(), e);
            }
        }
    }

    tracing::info!("");
    tracing::info!("✅ All tests completed successfully!");

    Ok(())
}
