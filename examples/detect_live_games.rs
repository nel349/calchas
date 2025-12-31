//! Detect LIVE games (currently being played) vs scheduled games
//!
//! Run: cargo run --example detect_live_games

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Detecting LIVE games vs scheduled games");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Fetch ALL sports markets - comprehensive list
    let sports_series = vec![
        // US Sports
        "KXNFLGAME",     // NFL
        "KXNBAGAME",     // NBA (M)
        "KXWNBAGAME",    // WNBA (W)
        "KXNHLGAME",     // NHL
        "KXMLBGAME",     // MLB
        "KXNCAAFGAME",   // College Football (M)
        "KXNCAABGAME",   // College Basketball (M)
        "KXWCAABGAME",   // Women's College Basketball

        // International Soccer
        "KXEPLGAME",         // English Premier League
        "KXLALIGAGAME",      // Spanish La Liga
        "KXSAUDIPLGAME",     // Saudi Pro League
        "KXBUNDESLIGATOP4",  // Bundesliga (Top 4 predictions)
    ];

    tracing::info!("Fetching sports markets...");
    let mut all_markets = Vec::new();

    for series in &sports_series {
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: None,
            status: Some("open".to_string()),
            series_ticker: Some(series.to_string()),
            min_close_ts: None,
            max_close_ts: None,
        };

        let response = client.get_markets(request).await?;
        all_markets.extend(response.markets);
    }

    tracing::info!("Fetched {} total sports markets", all_markets.len());
    tracing::info!("");

    // Analyze LIVE vs SCHEDULED
    let now = Utc::now();

    let mut live_candidates = Vec::new();
    let mut scheduled = Vec::new();

    for market in &all_markets {
        // Parse expected expiration
        let exp_time = match market.expected_expiration_time {
            Some(time) => time,
            None => continue, // Skip markets without expiration time
        };
        let hours_to_exp = (exp_time - now).num_hours();

        // Calculate volume ratio (24h volume vs total)
        let vol_ratio = if market.volume > 0 {
            (market.volume_24h as f64 / market.volume as f64) * 100.0
        } else {
            0.0
        };

        // LIVE game heuristics (actively being played RIGHT NOW):
        // 1. Expiring within next 2 hours (game in final stages)
        //    Sports games last 2-3h, so if ending in <2h, it's in progress
        // 2. High recent volume (>30% of total in last 24h)
        // 3. Reasonable total volume (>1000)

        let is_likely_live =
            hours_to_exp >= 0 && hours_to_exp <= 2 &&
            vol_ratio > 30.0 &&
            market.volume > 1000;

        if is_likely_live {
            live_candidates.push((market, exp_time, hours_to_exp, vol_ratio));
        } else if hours_to_exp > 6 {
            scheduled.push((market, hours_to_exp));
        }
    }

    // Display LIVE games
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("🔴 LIKELY LIVE GAMES: {} (happening NOW or very soon)", live_candidates.len());
    tracing::info!("═══════════════════════════════════════════════════════════════");

    for (market, exp_time, hours_to_exp, vol_ratio) in &live_candidates {
        let price_cents = market.last_price;
        let price = price_cents as f64 / 100.0;

        tracing::info!("");
        tracing::info!("🏈 {}", market.ticker);
        tracing::info!("   Event: {}", market.event_ticker);
        tracing::info!("   Title: {}", market.title);
        tracing::info!("   Last Price: ${:.2}", price);
        tracing::info!("   ⏱️  Expires in: {}h {}m",
            hours_to_exp,
            ((*exp_time - now).num_minutes() % 60).abs()
        );
        tracing::info!("   📊 Volume: {} total | {} 24h ({:.1}% recent)",
            market.volume,
            market.volume_24h,
            vol_ratio
        );
        tracing::info!("   🔥 WHY LIVE: Expiring soon + {:.0}% of volume in last 24h",
            vol_ratio
        );
        tracing::info!("");
        tracing::info!("   ✅ VALIDATION DATA (verify against Kalshi UI):");
        tracing::info!("      expected_expiration_time: {:?}", market.expected_expiration_time);
        tracing::info!("      close_time: {}", market.close_time);
        tracing::info!("      volume: {}", market.volume);
        tracing::info!("      volume_24h: {} ({} sentinel?)", market.volume_24h, if market.volume_24h == -1 { "YES" } else { "NO" });
        tracing::info!("      status: {}", market.status);
    }

    // Display upcoming scheduled games
    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("📅 SCHEDULED GAMES (future): {}", scheduled.len().min(10));
    tracing::info!("═══════════════════════════════════════════════════════════════");

    for (market, hours_to_exp) in scheduled.iter().take(10) {
        let days = hours_to_exp / 24;
        let hours = hours_to_exp % 24;

        tracing::info!("   {} | Starts in: {}d {}h | Vol: {}",
            market.ticker,
            days,
            hours,
            market.volume
        );
    }

    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("DETECTION CRITERIA:");
    tracing::info!("  LIVE = Expires in <2h + >30% volume in 24h + >1000 total volume");
    tracing::info!("  Rationale: Sports games last 2-3h. If ending in <2h, game is in progress.");
    tracing::info!("  These are games being played RIGHT NOW (quarters/periods active)");
    tracing::info!("═══════════════════════════════════════════════════════════════");

    Ok(())
}
