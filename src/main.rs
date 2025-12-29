//! Calchas - Prediction Market Trading Bot
//!
//! Main entry point for the trading bot application.

use std::time::Duration;
use calchas::app_state::AppState;
use calchas::config::AppConfig;
use calchas::loop_handlers::{
    fetch_all_markets, fetch_markets_by_ids, evaluate_strategies, process_entry_signal,
    update_and_check_positions, print_status,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔮 Calchas - Prediction Market Trading Bot");
    tracing::info!("Mode: SIMULATION (paper trading)");
    tracing::info!("");

    // Load configuration (.env first, then config file)
    tracing::info!("Loading configuration from .env and config/config.toml");
    let config = AppConfig::load_with_env_default()?;

    // Initialize application state
    let mut state = AppState::new(config).await?;

    tracing::info!("Starting trading loop (polling every 10 seconds)...");
    tracing::info!("Press Ctrl+C to stop");
    tracing::info!("");

    // Main trading loop
    run_trading_loop(&mut state).await?;

    tracing::info!("");
    tracing::info!("Shutting down gracefully...");
    tracing::info!("Final metrics:");
    print_status(&state);

    Ok(())
}

/// Main trading loop
///
/// # Arguments
///
/// * `state` - Application state
///
/// # Returns
///
/// Ok(()) when loop exits (Ctrl+C)
async fn run_trading_loop(state: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut iteration = 0u64;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                iteration += 1;
                tracing::info!("=== ITERATION {} ===", iteration);

                // Get max concurrent positions from risk limits
                let max_concurrent = state.strategies.values()
                    .map(|s| s.risk_limits.max_concurrent_positions as usize)
                    .max()
                    .unwrap_or(5);

                let current_positions = state.positions.len();
                let has_capacity = current_positions < max_concurrent;

                // 1. Fetch markets based on capacity
                let all_markets = if has_capacity {
                    // We have capacity - scan ALL markets for new opportunities
                    tracing::info!("Position capacity: {}/{} - Scanning for entries", current_positions, max_concurrent);

                    // Extract series_tickers from first enabled strategy (if any)
                    let series_tickers = state.strategies.values()
                        .next()
                        .and_then(|s| s.filters.series_ticker.clone());

                    match fetch_all_markets(
                        &state.kalshi_client,
                        state.time_range_config.min_time_to_event_minutes,
                        state.time_range_config.max_time_to_event_minutes,
                        series_tickers,
                    ).await {
                        Ok(m) => {
                            // Record prices for momentum tracking
                            for market in &m {
                                state.price_tracker.record_price(&market.id, market.yes_price, market.no_price);
                            }
                            tracing::info!("📊 Recorded prices for {} markets, tracker now tracking {} markets",
                                m.len(), state.price_tracker.market_count());
                            m
                        },
                        Err(e) => {
                            tracing::error!("Failed to scan markets: {}", e);
                            continue;
                        }
                    }
                } else {
                    // At max capacity - only fetch markets for existing positions
                    tracing::info!("Position capacity: {}/{} (FULL) - Updating positions only", current_positions, max_concurrent);
                    let market_ids: Vec<_> = state.positions.values()
                        .map(|p| p.market_id.clone())
                        .collect();

                    if market_ids.is_empty() {
                        tracing::info!("No positions to update");
                        continue;
                    }

                    match fetch_markets_by_ids(
                        &state.kalshi_client,
                        &market_ids,
                        state.time_range_config.min_time_to_event_minutes,
                        state.time_range_config.max_time_to_event_minutes,
                    ).await {
                        Ok(m) => m,
                        Err(e) => {
                            tracing::error!("Failed to fetch position markets: {}", e);
                            continue;
                        }
                    }
                };

                // 2. Evaluate strategies (only if we have capacity)
                if has_capacity {
                    let signal_market_pairs = evaluate_strategies(state, &all_markets);
                    if !signal_market_pairs.is_empty() {
                        tracing::info!("Generated {} entry signals", signal_market_pairs.len());
                    }

                    // 3. Process entry signals
                    // Risk manager will prevent duplicates and enforce position limits
                    for (signal, market) in signal_market_pairs {
                        if let Err(e) = process_entry_signal(state, signal, &market).await {
                            tracing::error!("Failed to process entry signal: {}", e);
                        }
                    }
                }

                // 4. Update position prices and check exits (ALWAYS)
                if !state.positions.is_empty() {
                    if let Err(e) = update_and_check_positions(state, &all_markets).await {
                        tracing::error!("Failed to update positions: {}", e);
                    }
                }

                // 5. Print status
                print_status(state);

                // 6. Cleanup old price data periodically (every 100 iterations = ~10 minutes)
                if iteration % 100 == 0 {
                    tracing::debug!("Cleaning up old price tracker data...");
                    state.price_tracker.cleanup_all();
                    tracing::debug!("Price tracker now tracking {} markets", state.price_tracker.market_count());
                }

                tracing::info!("");
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received");
                break;
            }
        }
    }

    Ok(())
}
