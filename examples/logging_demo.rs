// Example: Demonstrate logging infrastructure
// Shows different log levels and structured logging

use calchas::utils::logging;
use calchas::strategy::StrategyLoader;
use tracing::{info, warn, error, debug, trace};

fn main() {
    // Initialize logging (reads RUST_LOG env var)
    // Try running with: RUST_LOG=debug cargo run --example logging_demo
    logging::init();

    info!("=== Calchas Logging Demo ===");
    info!("Logging initialized successfully");

    // Demonstrate different log levels
    trace!("This is a TRACE message (most verbose)");
    debug!("This is a DEBUG message");
    info!("This is an INFO message (default level)");
    warn!("This is a WARN message");
    error!("This is an ERROR message");

    info!("");
    info!("--- Structured Logging Example ---");

    // Structured logging with fields
    let strategy_file = "strategies/underdog_hunter.json";
    info!(
        strategy_file = %strategy_file,
        "Loading strategy"
    );

    match StrategyLoader::load(strategy_file) {
        Ok(strategy) => {
            info!(
                strategy_name = %strategy.name,
                strategy_id = %strategy.id.as_str(),
                position_size = strategy.entry_rules.position_size,
                "Strategy loaded successfully"
            );

            debug!(
                take_profit = ?strategy.exit_rules.take_profit_pct,
                stop_loss = ?strategy.exit_rules.stop_loss_pct,
                max_positions = strategy.risk_limits.max_concurrent_positions,
                "Strategy configuration"
            );
        }
        Err(e) => {
            error!(
                error = %e,
                strategy_file = %strategy_file,
                "Failed to load strategy"
            );
        }
    }

    info!("");
    info!("--- Simulated Trading Activity ---");

    // Simulate some trading activity
    simulate_trading();

    info!("");
    info!("=== Demo Complete ===");
    info!("Try different log levels:");
    info!("  RUST_LOG=trace cargo run --example logging_demo");
    info!("  RUST_LOG=debug cargo run --example logging_demo");
    info!("  RUST_LOG=info cargo run --example logging_demo");
    info!("  RUST_LOG=warn cargo run --example logging_demo");
}

fn simulate_trading() {
    info!("Starting trading simulation");

    debug!("Fetching markets from Kalshi API (simulated)");
    trace!("HTTP GET https://api.kalshi.com/markets");

    info!(
        market_count = 42,
        "Markets fetched"
    );

    debug!("Evaluating markets against strategy filters");

    let matching_markets = 5;
    info!(
        matching_markets = matching_markets,
        total_markets = 42,
        "Found matching markets"
    );

    for i in 1..=matching_markets {
        debug!(
            market_id = format!("MARKET-{:03}", i),
            price = format!("${:.2}", 0.10 + (i as f64) * 0.02),
            "Evaluating market"
        );

        if i == 3 {
            warn!(
                market_id = format!("MARKET-{:03}", i),
                reason = "Price too volatile",
                "Skipping market"
            );
            continue;
        }

        info!(
            market_id = format!("MARKET-{:03}", i),
            action = "BUY",
            quantity = 100,
            "Placing order"
        );

        trace!("Order submitted to exchange");
    }

    info!("Trading simulation complete");
}
