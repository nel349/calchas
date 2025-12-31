//! Fetch ALL series tickers from Kalshi's official API
//!
//! Run: cargo run --example fetch_all_kalshi_series

use calchas::config::AppConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct SeriesResponse {
    series: Vec<Series>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Series {
    ticker: String,
    title: String,
    category: String,
    #[serde(default)]
    tags: Vec<String>,
    frequency: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Fetching ALL series tickers from Kalshi API");
    tracing::info!("");

    let _config = AppConfig::load_with_env_default()?;

    // Fetch all series (no filters) - using direct HTTP request
    let url = "https://api.elections.kalshi.com/trade-api/v2/series";

    let http_client = reqwest::Client::new();
    let response = http_client
        .get(url)
        .send()
        .await?
        .json::<SeriesResponse>()
        .await?;

    tracing::info!("Total series: {}", response.series.len());
    tracing::info!("");

    // Filter for sports-related series (game markets)
    let mut sports_series: Vec<&Series> = response.series
        .iter()
        .filter(|s| {
            s.ticker.contains("GAME") ||
            s.category.to_lowercase().contains("sport") ||
            s.tags.iter().any(|t: &String| t.to_lowercase().contains("sport"))
        })
        .collect();

    sports_series.sort_by(|a, b| a.ticker.cmp(&b.ticker));

    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("SPORTS SERIES (containing 'GAME' or sports tags): {}", sports_series.len());
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("");

    for series in &sports_series {
        tracing::info!("{}", series.ticker);
        tracing::info!("  Title: {}", series.title);
        tracing::info!("  Category: {}", series.category);
        tracing::info!("  Frequency: {}", series.frequency);
        if !series.tags.is_empty() {
            tracing::info!("  Tags: {:?}", series.tags);
        }
        tracing::info!("");
    }

    // Generate Rust vector code
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("Copy-paste this into your code:");
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("");
    tracing::info!("let sports_series = vec![");
    for series in &sports_series {
        tracing::info!("    \"{}\",  // {}", series.ticker, series.title);
    }
    tracing::info!("];");

    Ok(())
}
