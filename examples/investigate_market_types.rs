use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};
use std::collections::HashMap;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("🔍 Investigating Market Types and Settlement Predictability\n");

    // Fetch a large sample of markets
    let request = GetMarketsRequest {
        limit: Some(1000),
        status: Some("open".to_string()),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;
    println!("Fetched {} markets\n", response.markets.len());

    // Group by series prefix (first part before dash)
    let mut series_map: HashMap<String, Vec<&calchas::kalshi::KalshiMarket>> = HashMap::new();
    
    for market in &response.markets {
        let series = market.ticker.split('-').next().unwrap_or("UNKNOWN").to_string();
        series_map.entry(series).or_insert_with(Vec::new).push(market);
    }

    println!("Found {} unique series\n", series_map.len());
    println!("═══════════════════════════════════════════════════════════════");
    println!("SERIES BREAKDOWN:");
    println!("═══════════════════════════════════════════════════════════════\n");

    // Sort by count
    let mut series_counts: Vec<_> = series_map.iter().collect();
    series_counts.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    for (series, markets) in &series_counts {
        println!("{:20} - {} markets", series, markets.len());
    }

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("SETTLEMENT TIME ANALYSIS:");
    println!("═══════════════════════════════════════════════════════════════\n");

    // Analyze specific series for settlement predictability
    let test_series = vec![
        ("KXBTC", "Crypto (Bitcoin)"),
        ("KXETH", "Crypto (Ethereum)"),
        ("KXNFLGAME", "Sports (NFL)"),
        ("KXNBAGAME", "Sports (NBA)"),
        ("INXD", "Economics (Stock Index)"),
        ("KXCPI", "Economics (CPI)"),
        ("KXUNRATE", "Economics (Unemployment)"),
        ("KXHIGHTEMP", "Weather (Temperature)"),
        ("KXRAIN", "Weather (Rainfall)"),
    ];

    for (series, description) in test_series {
        if let Some(markets) = series_map.get(series) {
            if markets.is_empty() {
                continue;
            }
            
            let market = markets[0];
            let now = Utc::now();
            
            // Calculate time differences
            let time_to_close = market.close_time.signed_duration_since(now);
            let time_to_expiration = market.expiration_time.signed_duration_since(now);
            
            let has_expected = market.expected_expiration_time.is_some();
            let expected_str = if let Some(exp) = market.expected_expiration_time {
                let time_to_expected = exp.signed_duration_since(now);
                format!("{} (in {} hours)", exp, time_to_expected.num_hours())
            } else {
                "None".to_string()
            };

            println!("📊 {} - {}", series, description);
            println!("   Ticker: {}", market.ticker);
            println!("   close_time:               {} (in {} hours)", market.close_time, time_to_close.num_hours());
            println!("   expiration_time:          {} (in {} hours)", market.expiration_time, time_to_expiration.num_hours());
            println!("   expected_expiration_time: {}", expected_str);
            
            // Determine settlement predictability
            let close_exp_diff = (market.expiration_time.timestamp() - market.close_time.timestamp()).abs();
            
            if close_exp_diff < 3600 {
                println!("   ✅ PREDICTABLE: close_time ≈ expiration_time (within 1 hour)");
            } else {
                println!("   ⚠️  UNPREDICTABLE: close_time and expiration_time differ by {} hours", close_exp_diff / 3600);
            }
            
            if has_expected {
                let expected = market.expected_expiration_time.unwrap();
                let exp_close_diff = (market.close_time.timestamp() - expected.timestamp()).abs();
                
                if exp_close_diff < 3600 {
                    println!("   ✅ expected_expiration_time ≈ close_time (event ends when trading ends)");
                } else {
                    println!("   ⚠️  expected_expiration_time is {} hours before close_time (game start ≠ game end)", exp_close_diff / 3600);
                }
            }
            
            println!();
        }
    }

    println!("═══════════════════════════════════════════════════════════════");
    println!("SETTLEMENT PREDICTABILITY SUMMARY:");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    println!("✅ PREDICTABLE (settlement time is known in advance):");
    println!("   - Crypto: Markets settle at specific time (e.g., 2:00 PM daily)");
    println!("   - Economics: Data releases at scheduled times (e.g., CPI at 8:30 AM)");
    println!("   - Weather: Observations at specific times (e.g., high temp at midnight)");
    println!();
    
    println!("⚠️  UNPREDICTABLE (settlement time varies):");
    println!("   - Sports: Games end when they end (overtime, delays, etc.)");
    println!("   - expected_expiration_time = game START, not game END");
    println!();
    
    println!("💡 RECOMMENDATION:");
    println!("   Settlement-aware exits should ONLY apply to:");
    println!("   - Markets where close_time ≈ expiration_time (within 1 hour)");
    println!("   - This excludes sports but includes crypto, economics, weather");

    Ok(())
}
