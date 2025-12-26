// Example: Load strategy from JSON file
// Demonstrates the Strategy JSON Loader

use calchas::strategy::StrategyLoader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Calchas Strategy Loader Demo ===\n");

    // Load underdog hunter strategy
    println!("📂 Loading underdog_hunter.json...");
    let underdog = StrategyLoader::load("strategies/underdog_hunter.json")?;

    println!("✅ Successfully loaded: {}", underdog.name);
    println!("   ID: {}", underdog.id.as_str());
    println!("   Version: {}", underdog.version);
    println!("   Description: {}", underdog.description);
    println!("   Enabled: {}", underdog.enabled);
    println!("\n📊 Strategy Details:");
    println!("   Entry Side: {:?}", underdog.entry_rules.side);
    println!("   Position Size: {} contracts", underdog.entry_rules.position_size);
    println!("   Take Profit: {:?}%", underdog.exit_rules.take_profit_pct);
    println!("   Stop Loss: {:?}%", underdog.exit_rules.stop_loss_pct);
    println!("   Max Concurrent Positions: {}", underdog.risk_limits.max_concurrent_positions);
    println!("   Max Daily Loss: ${:?}", underdog.risk_limits.max_daily_loss_usd);

    println!("\n---\n");

    // Load volatility hedge strategy
    println!("📂 Loading volatility_hedge.json...");
    let hedge = StrategyLoader::load("strategies/volatility_hedge.json")?;

    println!("✅ Successfully loaded: {}", hedge.name);
    println!("   ID: {}", hedge.id.as_str());
    println!("   Version: {}", hedge.version);
    println!("   Description: {}", hedge.description);
    println!("\n📊 Strategy Details:");
    println!("   Entry Side: {:?}", hedge.entry_rules.side);
    println!("   Position Size: {} contracts", hedge.entry_rules.position_size);
    println!("   Take Profit: {:?}%", hedge.exit_rules.take_profit_pct);
    println!("   Trailing Stop: {:?}%", hedge.exit_rules.trailing_stop_pct);
    println!("   Max Concurrent Positions: {}", hedge.risk_limits.max_concurrent_positions);

    println!("\n---\n");

    // Load all strategies from directory
    println!("📁 Loading all strategies from strategies/ directory...");
    let all_strategies = StrategyLoader::load_all("strategies")?;

    println!("✅ Loaded {} strategies:", all_strategies.len());
    for strategy in &all_strategies {
        println!("   - {} ({})", strategy.name, strategy.id.as_str());
    }

    println!("\n---\n");

    // Pretty-print JSON of first strategy
    println!("🔍 Pretty-printed JSON of '{}':", underdog.name);
    let json_pretty = serde_json::to_string_pretty(&underdog)?;
    println!("{}", json_pretty);

    Ok(())
}
