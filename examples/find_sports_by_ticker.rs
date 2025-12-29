//! Find sports markets by ticker pattern
//!
//! Since category field is empty, we identify sports by ticker patterns:
//! - KXNFLGAME-* (NFL games)
//! - KXNBAGAME-* (NBA games)
//! - KXMLBGAME-* (MLB games)
//! - etc.

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use chrono::Utc;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🏈 Finding Sports Markets by Ticker Pattern");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch markets
    tracing::info!("Fetching first 5000 markets...");
    let mut all_markets = Vec::new();
    let mut cursor = None;

    for _page in 1..=5 {
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: None,
            min_close_ts: None,
            max_close_ts: None,
        };

        let response = client.get_markets(request).await?;
        all_markets.extend(response.markets);
        cursor = response.cursor;
        if cursor.is_none() {
            break;
        }
    }

    tracing::info!("Total markets: {}", all_markets.len());
    tracing::info!("");

    // Sports ticker patterns
    let patterns = [
        ("KXNFLGAME", "NFL Games"),
        ("KXNFLWINS", "NFL Season Wins"),
        ("KXNFLPLAYOFFS", "NFL Playoffs"),
        ("KXNBAGAME", "NBA Games"),
        ("KXNBAWINS", "NBA Season Wins"),
        ("KXMLBGAME", "MLB Games"),
        ("KXNHLGAME", "NHL Games"),
        ("KXSOCCERGAME", "Soccer Games"),
    ];

    for (pattern, label) in &patterns {
        let matches: Vec<_> = all_markets
            .iter()
            .filter(|m| m.ticker.starts_with(pattern))
            .collect();

        if !matches.is_empty() {
            tracing::info!("=== {} ({}) ===", label, matches.len());

            // Show top 10 by volume
            let mut sorted = matches.clone();
            sorted.sort_by_key(|m| std::cmp::Reverse(m.volume));

            for (i, market) in sorted.iter().take(10).enumerate() {
                let yes_price = Decimal::new(market.yes_ask as i64, 2);
                let no_price = Decimal::new(market.no_ask as i64, 2);
                let time_to_close = (market.close_time - Utc::now()).num_minutes();

                tracing::info!(
                    "  {}. {} | YES: ${:.2} NO: ${:.2} | Vol: {} OI: {} | Close: {}min",
                    i + 1,
                    market.ticker,
                    yes_price,
                    no_price,
                    market.volume,
                    market.open_interest,
                    time_to_close
                );
            }
            tracing::info!("");
        }
    }

    // Apply strategy filters to NFL games specifically
    tracing::info!("=== Applying Strategy Filters to NFL Games ===");
    tracing::info!("");

    let nfl_games: Vec<_> = all_markets
        .iter()
        .filter(|m| m.ticker.starts_with("KXNFLGAME"))
        .collect();

    tracing::info!("Total NFL games: {}", nfl_games.len());

    // Filter 1: Volume >= 10000
    let after_volume: Vec<_> = nfl_games
        .iter()
        .filter(|m| m.volume >= 10000)
        .collect();
    tracing::info!("  After volume >= 10000: {} markets", after_volume.len());

    // Filter 2: Volume >= 2000 (lower threshold)
    let after_volume_low: Vec<_> = nfl_games
        .iter()
        .filter(|m| m.volume >= 2000)
        .collect();
    tracing::info!("  After volume >= 2000: {} markets", after_volume_low.len());

    // Filter 3: Volume >= 1000 (even lower)
    let after_volume_verylow: Vec<_> = nfl_games
        .iter()
        .filter(|m| m.volume >= 1000)
        .collect();
    tracing::info!("  After volume >= 1000: {} markets", after_volume_verylow.len());

    tracing::info!("");

    // Filter 4: Open interest >= 2000
    let after_oi: Vec<_> = after_volume_low
        .iter()
        .filter(|m| m.open_interest >= 2000)
        .collect();
    tracing::info!("  After OI >= 2000: {} markets", after_oi.len());

    // Filter 5: Price range 0.25-0.75
    let min_price = Decimal::new(25, 2);
    let max_price = Decimal::new(75, 2);
    let after_price: Vec<_> = after_oi
        .iter()
        .filter(|m| {
            let yes_price = Decimal::new(m.yes_ask as i64, 2);
            yes_price >= min_price && yes_price <= max_price
        })
        .collect();
    tracing::info!("  After price 0.25-0.75: {} markets", after_price.len());
    tracing::info!("");

    // Show what passes all filters
    if !after_price.is_empty() {
        tracing::info!("=== Markets Passing All Filters ===");
        for market in after_price.iter().take(10) {
            let yes_price = Decimal::new(market.yes_ask as i64, 2);
            let no_price = Decimal::new(market.no_ask as i64, 2);
            let time_to_close = (market.close_time - Utc::now()).num_minutes();

            tracing::info!(
                "  {} | YES: ${:.2} NO: ${:.2} | Vol: {} OI: {} | Close: {}min",
                market.ticker,
                yes_price,
                no_price,
                market.volume,
                market.open_interest,
                time_to_close
            );
        }
    } else {
        tracing::warn!("⚠️  NO markets pass all filters");
    }

    Ok(())
}
