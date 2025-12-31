use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};
use calchas::models::Market;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("🧪 Testing Settlement Predictability Logic\n");
    println!("Testing with REAL Kalshi markets to verify is_settlement_predictable()\n");

    // Test cases: crypto (predictable) and sports (unpredictable)
    let test_cases = vec![
        ("KXBTC", "Crypto (Bitcoin)", true),
        ("KXETH", "Crypto (Ethereum)", true),
        ("KXNFLGAME", "Sports (NFL)", false),
        ("KXNBAGAME", "Sports (NBA)", false),
    ];

    for (series, description, expected_predictable) in test_cases {
        println!("═══════════════════════════════════════════════════════════════");
        println!("📊 {} - {}", series, description);
        println!("═══════════════════════════════════════════════════════════════");

        let request = GetMarketsRequest {
            limit: Some(3),
            status: Some("open".to_string()),
            series_ticker: Some(series.to_string()),
            ..Default::default()
        };

        let response = client.get_markets(request).await?;

        if response.markets.is_empty() {
            println!("❌ No markets found for {}\n", series);
            continue;
        }

        // Convert KalshiMarket to our internal Market type (which has is_settlement_predictable())
        let kalshi_market = &response.markets[0];
        let market: Market = kalshi_market.clone().into();

        // Calculate time gap
        let diff_seconds = (market.close_time.timestamp() - market.event_time.timestamp()).abs();
        let diff_hours = diff_seconds / 3600;

        println!("Sample Ticker: {}", market.ticker);
        println!();
        println!("Timing Analysis:");
        println!("  event_time:  {}", market.event_time);
        println!("  close_time:  {}", market.close_time);
        println!("  Time gap:    {} hours", diff_hours);
        println!();

        // Test is_settlement_predictable() method
        let is_predictable = market.is_settlement_predictable();

        println!("Settlement Predictability:");
        println!("  is_settlement_predictable(): {}", is_predictable);
        println!("  Expected:                    {}", expected_predictable);

        if is_predictable == expected_predictable {
            println!("  ✅ CORRECT - Method works as expected");
        } else {
            println!("  ❌ INCORRECT - Method returned wrong result!");
        }

        println!();

        if is_predictable {
            println!("✅ Settlement-aware exits WILL APPLY:");
            println!("   - Settlement time is known in advance");
            println!("   - event_time ≈ close_time (within 2 hours)");
            println!("   - Safe to use settlement logic");
        } else {
            println!("⚠️  Settlement-aware exits WILL NOT APPLY:");
            println!("   - Settlement time is unpredictable");
            println!("   - event_time ≠ close_time (gap > 2 hours)");
            println!("   - Settlement logic skipped for safety");
        }

        println!();
    }

    println!("═══════════════════════════════════════════════════════════════");
    println!("SUMMARY:");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("Settlement-aware exit logic now includes predictability check:");
    println!();
    println!("Code in loop_handlers.rs (line 875):");
    println!("  if settlement_enabled");
    println!("      && market.is_settlement_predictable()  // ← NEW CHECK");
    println!("      && state.exit_manager.check_settlement_logic(...) {{");
    println!("      // Execute settlement exit");
    println!("  }}");
    println!();
    println!("Benefits:");
    println!("  ✅ No hardcoded series lists to maintain");
    println!("  ✅ Adapts automatically to new market types");
    println!("  ✅ Data-driven based on actual API timing");
    println!("  ✅ Excludes sports (333h gap) but includes crypto (0-2h gap)");
    println!();
    println!("Safety:");
    println!("  - Sports markets: Settlement logic SKIPPED (event_time = game START)");
    println!("  - Crypto markets: Settlement logic APPLIED (event_time = actual settlement)");
    println!("  - Economics markets: Settlement logic APPLIED (event_time = data release)");

    Ok(())
}
