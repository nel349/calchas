//! Calchas - Prediction Market Trading Bot
//!
//! Main entry point for the trading bot application.

use std::time::Duration;
use calchas::app_state::AppState;
use calchas::config::AppConfig;
use calchas::loop_handlers::{
    fetch_all_markets, fetch_markets_by_ids, evaluate_strategies, process_entry_signal,
    update_and_check_positions, print_status, scan_arbitrage_opportunities,
    display_arbitrage_opportunities,
};
use calchas::trading::OrderbookProvider;
use clap::Parser;

/// Calchas Trading Bot - Choose your strategy mode
#[derive(Parser)]
#[command(name = "calchas")]
#[command(about = "Prediction market trading bot for Kalshi", long_about = None)]
struct Args {
    /// Trading mode: arbitrage (math-based, hedged) or strategy (JSON-defined strategies)
    #[arg(long, value_enum)]
    mode: TradingMode,
}

/// Trading strategy mode
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum TradingMode {
    /// Cross-market arbitrage (hedged, guaranteed profit)
    Arbitrage,
    /// Strategy-based trading (executes strategies from JSON files)
    Strategy,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize logging with RUST_LOG environment variable support
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("🔮 Calchas - Prediction Market Trading Bot");
    tracing::info!("Mode: {:?} (SIMULATION - paper trading)", args.mode);
    tracing::info!("");

    // Load configuration (.env first, then config file)
    tracing::info!("Loading configuration from .env and config/config.toml");
    let config = AppConfig::load_with_env_default()?;

    // Initialize application state
    let mut state = AppState::new(config).await?;

    // Recover open positions from database (Phase 5)
    match state.storage.get_active_positions() {
        Ok(positions) => {
            if !positions.is_empty() {
                tracing::info!("🔄 Recovered {} open positions from database", positions.len());
                for position in positions {
                    tracing::info!("   - {} (Market: {}, Entry: ${:.2})",
                        position.id.0,
                        position.market_id.0,
                        position.entry_price
                    );
                    state.position_manager.insert_position_for_recovery(position);
                }
            } else {
                tracing::info!("✓ No positions to recover (starting fresh)");
            }
        }
        Err(e) => {
            tracing::warn!("⚠️  Failed to recover positions from database: {}", e);
            tracing::warn!("   Continuing with empty positions (non-fatal)");
        }
    }

    // Recover historical trades into metrics tracker (Phase 5)
    let mut total_trades_recovered = 0;
    for strategy_id in state.strategies.keys() {
        match state.storage.get_trades(strategy_id, None) {
            Ok(trades) => {
                for trade in &trades {
                    state.metrics_tracker.record_trade(trade);
                }
                total_trades_recovered += trades.len();
            }
            Err(e) => {
                tracing::warn!("⚠️  Failed to recover trades for strategy {}: {}", strategy_id.0, e);
            }
        }
    }
    if total_trades_recovered > 0 {
        tracing::info!("🔄 Recovered {} historical trades into metrics tracker", total_trades_recovered);
    } else {
        tracing::info!("✓ No historical trades to recover (starting fresh)");
    }

    tracing::info!("Starting trading loop (polling every 10 seconds)...");
    tracing::info!("Press Ctrl+C to stop");
    tracing::info!("");

    // Dispatch to appropriate mode
    match args.mode {
        TradingMode::Arbitrage => {
            tracing::info!("🎯 ARBITRAGE MODE: Scanning for cross-market opportunities");
            tracing::info!("    Strategy: Buy YES + NO when total < $0.98");
            tracing::info!("    Risk: Hedged (guaranteed profit at settlement)");
            tracing::info!("");
            run_arbitrage_mode(&mut state).await?;
        }
        TradingMode::Strategy => {
            tracing::info!("📈 STRATEGY MODE: Executing strategies from JSON files");
            tracing::info!("    Source: strategies/*.json");
            tracing::info!("    Risk: Varies by strategy (see JSON configs)");
            tracing::info!("");
            run_strategy_mode(&mut state).await?;
        }
    }

    tracing::info!("");
    tracing::info!("Shutting down gracefully...");
    tracing::info!("Final metrics:");
    print_status(&state);

    Ok(())
}

/// Arbitrage mode trading loop
///
/// Scans all markets for cross-market arbitrage opportunities where YES + NO < $0.98.
/// Displays opportunities in real-time but does NOT execute (Week 1 detection only).
///
/// # Arguments
///
/// * `state` - Application state
///
/// # Returns
///
/// Ok(()) when loop exits (Ctrl+C)
async fn run_arbitrage_mode(state: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("✓ Arbitrage mode active - scanning for cross-market opportunities");
    tracing::info!("  Detection only (no execution) - Week 1 validation phase");

    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut iteration = 0u64;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                iteration += 1;
                tracing::info!("=== ARBITRAGE SCAN {} ===", iteration);

                // Scan for arbitrage opportunities
                match scan_arbitrage_opportunities(state).await {
                    Ok(opportunities) => {
                        // Accumulate opportunities for tracking
                        if !opportunities.is_empty() {
                            tracing::info!("📊 Adding {} new opportunities to tracker (total: {})",
                                opportunities.len(),
                                state.arbitrage_opportunities_found.len() + opportunities.len()
                            );
                            state.arbitrage_opportunities_found.extend(opportunities.clone());
                        }

                        // Display top 10 opportunities from this scan
                        display_arbitrage_opportunities(&opportunities, 10);
                    }
                    Err(e) => {
                        tracing::error!("Arbitrage scan failed: {}", e);
                    }
                }

                // Also check existing positions (if any from previous runs)
                if state.position_manager.active_position_count() > 0 {
                    tracing::info!("Checking {} existing positions", state.position_manager.active_position_count());

                    let market_ids: Vec<_> = state.position_manager.get_active_positions()
                        .iter()
                        .map(|p| p.market_id.clone())
                        .collect();

                    match fetch_markets_by_ids(&state.kalshi_client, &market_ids).await {
                        Ok(markets) => {
                            if let Err(e) = update_and_check_positions(state, &markets).await {
                                tracing::error!("Failed to update positions: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch position markets: {}", e);
                        }
                    }
                }

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

/// Strategy mode trading loop
///
/// Executes strategies loaded from JSON files in the strategies/ directory.
/// This is the original trading loop logic.
///
/// # Arguments
///
/// * `state` - Application state
///
/// # Returns
///
/// Ok(()) when loop exits (Ctrl+C)
async fn run_strategy_mode(state: &mut AppState) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("✓ Strategy mode active - evaluating momentum-based strategies");
    tracing::info!("  Loaded {} strategies from strategies/*.json", state.strategies.len());

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

                let current_positions = state.position_manager.active_position_count();
                let has_capacity = current_positions < max_concurrent;

                // 1. Fetch markets based on capacity
                let all_markets = if has_capacity {
                    // We have capacity - scan ALL markets for new opportunities
                    tracing::info!("Position capacity: {}/{} - Scanning for entries", current_positions, max_concurrent);

                    // Collect ALL series_tickers from ALL enabled strategies (deduplicated)
                    let series_tickers: Option<Vec<String>> = {
                        let all_tickers: std::collections::HashSet<String> = state.strategies.values()
                            .filter_map(|s| s.filters.series_ticker.as_ref())
                            .flatten()
                            .cloned()
                            .collect();

                        if all_tickers.is_empty() {
                            None  // No series filter - fetch all markets
                        } else {
                            Some(all_tickers.into_iter().collect())
                        }
                    };

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
                                // Record volume for volume spike detection (Phase 1)
                                state.volume_tracker.record_volume(&market.id, market.volume);
                            }
                            tracing::info!("📊 Recorded prices and volume for {} markets", m.len());
                            tracing::info!("    Price tracker: {} markets, Volume tracker: {} markets",
                                state.price_tracker.market_count(),
                                state.volume_tracker.market_count());

                            // OPTIMIZATION: Two-phase filtering to reduce orderbook API calls
                            // Phase 1: Apply basic filters (no orderbook needed) to narrow 10,000 → ~50-100
                            // Phase 2: Fetch orderbooks only for candidates, apply OFI filter
                            let needs_ofi = state.strategies.values()
                                .any(|s| s.filters.min_order_flow_imbalance.is_some());

                            if needs_ofi && !m.is_empty() {
                                tracing::info!("🔍 Pre-filtering markets for OFI tracking (Phase 1: basic filters)...");

                                // Collect candidates that pass basic filters
                                let mut candidates = std::collections::HashSet::new();
                                for strategy in state.strategies.values().filter(|s| s.filters.min_order_flow_imbalance.is_some()) {
                                    for market in &m {
                                        if calchas::strategy::evaluator::StrategyEvaluator::matches_basic_filters(
                                            market,
                                            &strategy.filters,
                                            &strategy.entry_rules.side,
                                            Some(&state.price_tracker),
                                            Some(&state.volume_tracker),
                                        ) {
                                            candidates.insert(market.id.clone());
                                        }
                                    }
                                }

                                tracing::info!("    {} candidates passed basic filters (from {} total = {:.1}% reduction)",
                                    candidates.len(), m.len(),
                                    (1.0 - candidates.len() as f64 / m.len() as f64) * 100.0);

                                // Phase 2: Fetch orderbooks only for candidates
                                if !candidates.is_empty() {
                                    tracing::info!("    Phase 2: Fetching orderbooks for {} candidates...", candidates.len());

                                    let mut orderbooks_fetched = 0;
                                    let mut orderbooks_empty = 0;
                                    let mut orderbooks_failed = 0;

                                    for market_id in &candidates {
                                        match state.orderbook_provider.get_orderbook(market_id).await {
                                            Ok(Some(orderbook)) => {
                                                state.order_flow_tracker.record_orderbook(&orderbook);
                                                orderbooks_fetched += 1;
                                            }
                                            Ok(None) => {
                                                tracing::trace!("No orderbook for {}", market_id.as_str());
                                            }
                                            Err(e) => {
                                                let err_str = e.to_string();
                                                if err_str.contains("Rate limited") {
                                                    tracing::error!("🚨 RATE LIMITED: {}", err_str);
                                                    orderbooks_failed += 1;
                                                } else if err_str.contains("empty") {
                                                    orderbooks_empty += 1;
                                                } else {
                                                    orderbooks_failed += 1;
                                                    tracing::warn!("Orderbook fetch failed for {}: {}", market_id.as_str(), e);
                                                }
                                            }
                                        }
                                    }

                                    tracing::info!("📊 Orderbook summary: {} candidates, {} successful, {} empty, {} failed",
                                        candidates.len(), orderbooks_fetched, orderbooks_empty, orderbooks_failed);

                                    if orderbooks_fetched > 0 {
                                        tracing::info!("    ✓ OFI tracker now has {} markets with orderbook data",
                                            state.order_flow_tracker.market_count());
                                    } else {
                                        tracing::warn!("    ⚠️  No successful orderbooks (all candidates empty/failed)");
                                    }
                                } else {
                                    tracing::info!("    No candidates passed basic filters - skipping orderbook fetches");
                                }
                            }

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
                    let market_ids: Vec<_> = state.position_manager.get_active_positions()
                        .iter()
                        .map(|p| p.market_id.clone())
                        .collect();

                    if market_ids.is_empty() {
                        tracing::info!("No positions to update");
                        continue;
                    }

                    match fetch_markets_by_ids(
                        &state.kalshi_client,
                        &market_ids,
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
                if state.position_manager.active_position_count() > 0 {
                    if let Err(e) = update_and_check_positions(state, &all_markets).await {
                        tracing::error!("Failed to update positions: {}", e);
                    }
                }

                // 5. Print status
                print_status(state);

                // 6. Cleanup old tracker data periodically (every 100 iterations = ~10 minutes)
                if iteration % 100 == 0 {
                    tracing::debug!("Cleaning up old tracker data...");
                    state.price_tracker.cleanup_all();
                    state.volume_tracker.cleanup_all();
                    state.order_flow_tracker.cleanup_all();
                    tracing::debug!("Price tracker: {} markets, Volume tracker: {} markets, OFI tracker: {} markets",
                        state.price_tracker.market_count(),
                        state.volume_tracker.market_count(),
                        state.order_flow_tracker.market_count());
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
