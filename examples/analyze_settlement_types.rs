use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("🔍 Analyzing Settlement Predictability by Market Type\n");

    // Test specific series
    let test_cases = vec![
        ("KXNFLGAME", "Sports (NFL)", true),   // Has sports
        ("KXNBAGAME", "Sports (NBA)", true),   // Has sports
        ("KXBTC", "Crypto (Bitcoin)", false),   // No sports
        ("INXD", "Economics (S&P 500)", false), // No sports
    ];

    for (series, description, is_sports) in test_cases {
        println!("═══════════════════════════════════════════════════════════════");
        println!("📊 {} - {}", series, description);
        println!("═══════════════════════════════════════════════════════════════");

        let request = GetMarketsRequest {
            limit: Some(5),
            status: Some("open".to_string()),
            series_ticker: Some(series.to_string()),
            ..Default::default()
        };

        let response = client.get_markets(request).await?;

        if response.markets.is_empty() {
            println!("❌ No markets found for {}\n", series);
            continue;
        }

        let market = &response.markets[0];
        let now = Utc::now();

        // Time calculations
        let time_to_close = market.close_time.signed_duration_since(now);
        let time_to_exp = market.expiration_time.signed_duration_since(now);
        
        let expected_str = if let Some(exp) = market.expected_expiration_time {
            let time_to_expected = exp.signed_duration_since(now);
            format!("{} (in {} hours)", exp, time_to_expected.num_hours())
        } else {
            "None".to_string()
        };

        println!("Sample Ticker: {}", market.ticker);
        println!();
        println!("Timing Fields:");
        println!("  close_time:               {} (in {} hours)", market.close_time, time_to_close.num_hours());
        println!("  expiration_time:          {} (in {} hours)", market.expiration_time, time_to_exp.num_hours());
        println!("  expected_expiration_time: {}", expected_str);
        println!();

        // Calculate differences
        let close_exp_diff_hours = (market.expiration_time.timestamp() - market.close_time.timestamp()).abs() / 3600;
        
        if let Some(expected) = market.expected_expiration_time {
            let expected_close_diff_hours = (market.close_time.timestamp() - expected.timestamp()).abs() / 3600;
            
            println!("Time Differences:");
            println!("  close_time vs expiration_time:  {} hours apart", close_exp_diff_hours);
            println!("  expected vs close_time:         {} hours apart", expected_close_diff_hours);
            println!();
            
            if is_sports {
                println!("❌ SPORTS MARKET - UNPREDICTABLE SETTLEMENT:");
                println!("   - expected_expiration_time = GAME START ({} hours from now)", 
                    expected.signed_duration_since(now).num_hours());
                println!("   - close_time = {} hours AFTER game start", expected_close_diff_hours);
                println!("   - Actual settlement = unknown (depends on game length + overtime)");
                println!("   - Settlement logic will NOT work (triggers before game ends)");
            } else {
                if expected_close_diff_hours < 2 {
                    println!("✅ NON-SPORTS MARKET - PREDICTABLE SETTLEMENT:");
                    println!("   - expected_expiration_time ≈ close_time (within {} hours)", expected_close_diff_hours);
                    println!("   - Settlement time is KNOWN in advance");
                    println!("   - Settlement logic WILL work correctly");
                } else {
                    println!("⚠️  Large time gap - investigate further");
                }
            }
        } else {
            println!("Time Differences:");
            println!("  close_time vs expiration_time: {} hours apart", close_exp_diff_hours);
            println!();
            
            if close_exp_diff_hours < 2 {
                println!("✅ PREDICTABLE SETTLEMENT:");
                println!("   - close_time ≈ expiration_time (within {} hours)", close_exp_diff_hours);
                println!("   - No expected_expiration_time, but times are aligned");
                println!("   - Settlement logic WILL work using close_time");
            } else {
                println!("⚠️  Times differ by {} hours - may be unpredictable", close_exp_diff_hours);
            }
        }
        
        println!();
    }

    println!("═══════════════════════════════════════════════════════════════");
    println!("RECOMMENDATION:");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("Settlement-aware exits should be DISABLED for sports markets:");
    println!("  - Sports: expected_expiration_time = game START (not END)");
    println!("  - Can't predict when game will actually end (overtime, delays)");
    println!();
    println!("Settlement-aware exits CAN work for:");
    println!("  - Crypto: Precise settlement times (e.g., daily at 2:00 PM)");
    println!("  - Economics: Data releases at scheduled times");
    println!("  - Any market where close_time ≈ actual settlement time");
    println!();
    println!("Implementation approach:");
    println!("  if !market.is_sports_market() && settlement_enabled {{");
    println!("      // Apply settlement logic");
    println!("  }}");

    Ok(())
}
