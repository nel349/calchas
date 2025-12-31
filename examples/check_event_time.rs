use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("Fetching NFL markets to verify event timing...\n");

    // Fetch NFL markets
    let request = GetMarketsRequest {
        limit: Some(10),
        status: Some("open".to_string()),
        series_ticker: Some("KXNFLGAME".to_string()),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    println!("Got {} NFL markets from API\n", response.markets.len());

    if response.markets.is_empty() {
        println!("No NFL markets found!");
        return Ok(());
    }

    // Check first market for timing fields
    let market = &response.markets[0];

    println!("═══════════════════════════════════════════════════════════");
    println!("TIMING FIELDS FOR: {}", market.ticker);
    println!("═══════════════════════════════════════════════════════════\n");

    println!("close_time:               {}", market.close_time);
    println!("expiration_time:          {}", market.expiration_time);
    println!("expected_expiration_time: {:?}", market.expected_expiration_time);
    println!();

    if let Some(expected) = market.expected_expiration_time {
        println!("✅ expected_expiration_time IS POPULATED");
        println!("   This is the actual game time: {}", expected);
    } else {
        println!("❌ expected_expiration_time IS NONE");
        println!("   Falling back to expiration_time: {}", market.expiration_time);
    }

    println!("\n═══════════════════════════════════════════════════════════");
    println!("CHECKING ALL {} MARKETS:", response.markets.len());
    println!("═══════════════════════════════════════════════════════════\n");

    let mut has_expected = 0;
    let mut missing_expected = 0;

    for m in &response.markets {
        if m.expected_expiration_time.is_some() {
            has_expected += 1;
        } else {
            missing_expected += 1;
            println!("⚠️  {} - NO expected_expiration_time", m.ticker);
        }
    }

    println!("\nSummary:");
    println!("  ✅ With expected_expiration_time: {}/{}", has_expected, response.markets.len());
    println!("  ❌ Missing expected_expiration_time: {}/{}", missing_expected, response.markets.len());

    if has_expected == response.markets.len() {
        println!("\n🎉 ALL markets have expected_expiration_time - settlement logic is safe!");
    } else if missing_expected == response.markets.len() {
        println!("\n⚠️  NO markets have expected_expiration_time - falling back to expiration_time");
    } else {
        println!("\n⚠️  MIXED - some markets missing expected_expiration_time");
    }

    Ok(())
}
