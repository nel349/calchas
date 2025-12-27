//! Phase 3 Milestone: Market Filtering & Signal Generation Demo
//!
//! This example demonstrates the complete Phase 3 flow:
//! 1. Load strategies from JSON files
//! 2. Fetch live markets from Kalshi API
//! 3. Evaluate markets against strategy filters
//! 4. Display generated entry signals

use calchas::strategy::{StrategyLoader, StrategyEvaluator, SignalSide};
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;
use calchas::models::Market;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=============================================================================");
    println!("CALCHAS - PHASE 3 MILESTONE: MARKET FILTERING & SIGNAL GENERATION");
    println!("=============================================================================");
    println!();

    // Step 1: Load strategies
    println!("Loading strategies from strategies/ directory...");
    let strategies = StrategyLoader::load_all("strategies")?;

    println!("✓ Loaded {} strategies", strategies.len());
    for strategy in &strategies {
        let status = if strategy.enabled { "ENABLED" } else { "DISABLED" };
        println!("  - {} ({})", strategy.name, status);
    }
    println!();

    // Step 2: Initialize Kalshi client and fetch markets
    println!("Fetching open markets from Kalshi API...");

    // Load config from .env file
    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    // Fetch open markets
    let request = GetMarketsRequest {
        status: Some("open".to_string()),
        limit: Some(100),  // Fetch up to 100 markets
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    // Convert KalshiMarket to Market
    let markets: Vec<Market> = response.markets
        .into_iter()
        .map(|km| km.into())
        .collect();

    println!("✓ Fetched {} markets", markets.len());
    println!();

    // Step 3: Evaluate strategies
    println!("=============================================================================");
    println!("EVALUATING STRATEGIES");
    println!("=============================================================================");
    println!();

    let all_signals = StrategyEvaluator::evaluate_all(&markets, &strategies)?;

    if all_signals.is_empty() {
        println!("⚠ No signals generated. No markets matched strategy filters.");
        println!();
        println!("This is normal if:");
        println!("  - Market conditions don't match strategy criteria");
        println!("  - All strategies are disabled");
        println!("  - No markets are currently open");
        println!();
    } else {
        // Group signals by strategy
        let mut signals_by_strategy: std::collections::HashMap<String, Vec<_>> =
            std::collections::HashMap::new();

        for signal in &all_signals {
            signals_by_strategy
                .entry(signal.strategy_name.clone())
                .or_insert_with(Vec::new)
                .push(signal);
        }

        // Display signals for each strategy
        for (strategy_name, signals) in signals_by_strategy {
            println!("[{}] Evaluating...", strategy_name);
            println!("  Found {} signal(s)", signals.len());
            println!();

            for (i, signal) in signals.iter().enumerate() {
                let side_str = match signal.side {
                    SignalSide::Yes => "Yes",
                    SignalSide::No => "No",
                };

                println!("  Signal #{}:", i + 1);
                println!("    Market: {}", signal.market_ticker);
                println!("    Title: {}", signal.market_title);
                println!("    Side: {}", side_str);
                println!("    Price: ${}", signal.recommended_price);
                println!("    Size: {} contracts", signal.position_size);
                println!("    Time to event: {:.1} hours", signal.time_to_event_hours);
                println!("    Volume: {}", signal.market_volume);
                println!("    Open Interest: {}", signal.market_open_interest);
                println!();
            }
        }

        println!("=============================================================================");
        println!("SUMMARY");
        println!("=============================================================================");
        println!();
        println!("Total signals generated: {}", all_signals.len());
        println!("Strategies evaluated: {}", strategies.len());
        println!("Markets analyzed: {}", markets.len());
        println!();
    }

    println!("=============================================================================");
    println!("PHASE 3 MILESTONE COMPLETE!");
    println!("=============================================================================");
    println!();
    println!("✓ Strategy loading (Phase 3.1)");
    println!("✓ Market filtering (Phase 3.2)");
    println!("✓ Signal generation (Phase 3.3)");
    println!();
    println!("Next: Phase 4 - Position Management");
    println!();

    Ok(())
}
