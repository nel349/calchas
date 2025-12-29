//! Search for specific game by ticker pattern
//!
//! Looking for: KXNFLGAME-25DEC28CHISF

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

    tracing::info!("🔍 Searching for specific NFL game: CHI vs SF (Dec 28)");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch ALL markets (no pagination limit)
    tracing::info!("Fetching all open markets...");
    let mut all_markets = Vec::new();
    let mut cursor = None;
    let mut page = 0;

    loop {
        page += 1;
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: None,
            min_close_ts: None,
            max_close_ts: None,
        };

        let response = client.get_markets(request).await?;
        let count = response.markets.len();
        all_markets.extend(response.markets);

        if page % 5 == 0 {
            tracing::info!("  Page {}: {} markets (total: {})", page, count, all_markets.len());
        }

        cursor = response.cursor;
        if cursor.is_none() {
            break;
        }

        if page >= 20 {
            tracing::warn!("Stopping at 20 pages (20,000 markets)");
            break;
        }
    }

    tracing::info!("Total markets fetched: {}", all_markets.len());
    tracing::info!("");

    // Search for the specific game
    let target_patterns = [
        "KXNFLGAME-25DEC28CHISF",
        "KXNFLGAME-25DEC28SF",
        "25DEC28CHI",
        "25DEC28SF",
        "CHISF",
    ];

    tracing::info!("=== Searching for target game ===");
    for pattern in &target_patterns {
        let matches: Vec<_> = all_markets
            .iter()
            .filter(|m| m.ticker.contains(pattern))
            .collect();

        if !matches.is_empty() {
            tracing::info!("Found {} matches for pattern '{}':", matches.len(), pattern);
            for market in matches {
                let yes_price = Decimal::new(market.yes_ask as i64, 2);
                let no_price = Decimal::new(market.no_ask as i64, 2);
                let time_to_close = (market.close_time - Utc::now()).num_hours();

                tracing::info!(
                    "  {} | {} | YES: ${:.2} NO: ${:.2} | Vol: {} OI: {} | Close: {}h",
                    market.ticker,
                    market.title,
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

    // Search for any NFL games happening soon (next 7 days)
    let now = Utc::now();
    let week_from_now = now + chrono::Duration::days(7);

    let nfl_games_soon: Vec<_> = all_markets
        .iter()
        .filter(|m| {
            m.ticker.starts_with("KXNFL")
                && m.close_time > now
                && m.close_time < week_from_now
        })
        .collect();

    tracing::info!("=== NFL markets closing in next 7 days: {} ===", nfl_games_soon.len());
    for market in nfl_games_soon.iter().take(20) {
        let yes_price = Decimal::new(market.yes_ask as i64, 2);
        let time_to_close = (market.close_time - Utc::now()).num_hours();

        tracing::info!(
            "  {} | YES: ${:.2} | Vol: {} | Close: {}h",
            market.ticker,
            yes_price,
            market.volume,
            time_to_close
        );
    }
    tracing::info!("");

    // Search for any markets with "CHI" or "SF" in ticker
    let team_markets: Vec<_> = all_markets
        .iter()
        .filter(|m| m.ticker.contains("CHI") || m.ticker.contains("SF"))
        .collect();

    tracing::info!("=== Markets with CHI or SF: {} ===", team_markets.len());
    for market in team_markets.iter().take(10) {
        let yes_price = Decimal::new(market.yes_ask as i64, 2);
        let time_to_close = (market.close_time - Utc::now()).num_hours();

        tracing::info!(
            "  {} | {} | YES: ${:.2} | Vol: {} | Close: {}h",
            market.ticker,
            market.title,
            yes_price,
            market.volume,
            time_to_close
        );
    }

    Ok(())
}
