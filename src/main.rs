//! Calchas - Prediction Market Trading Bot
//!
//! Main entry point for the trading bot application.

use std::time::Duration;
use calchas::app_state::AppState;
use calchas::config::AppConfig;
use calchas::loop_handlers::{
    fetch_active_markets, evaluate_strategies, process_entry_signal,
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

                // 1. Fetch markets from Kalshi (using time window from strategies)
                let markets = match fetch_active_markets(
                    &state.kalshi_client,
                    state.time_range_config.min_time_to_event_minutes,
                    state.time_range_config.max_time_to_event_minutes,
                ).await {
                    Ok(m) => {
                        tracing::info!("Fetched {} active markets", m.len());
                        m
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch markets: {}", e);
                        continue;
                    }
                };

                // 2. Evaluate strategies
                let signal_market_pairs = evaluate_strategies(state, &markets);
                if !signal_market_pairs.is_empty() {
                    tracing::info!("Generated {} entry signals", signal_market_pairs.len());
                }

                // 3. Process entry signals
                // Risk manager will prevent duplicates and enforce position limits
                for (signal, _market) in signal_market_pairs {
                    if let Err(e) = process_entry_signal(state, signal).await {
                        tracing::error!("Failed to process entry signal: {}", e);
                    }
                }

                // 4. Update position prices and check exits
                if let Err(e) = update_and_check_positions(state, &markets).await {
                    tracing::error!("Failed to update positions: {}", e);
                }

                // 5. Print status
                print_status(state);

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
