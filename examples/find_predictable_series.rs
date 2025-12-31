use calchas::config::AppConfig;
use calchas::kalshi::{KalshiClient, GetMarketsRequest};
use std::collections::{HashMap, HashSet};
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    println!("🔍 Finding Series with Predictable Settlement Times\n");
    println!("Criteria: expected_expiration_time is within 2 hours of close_time\n");

    // Fetch large sample
    let request = GetMarketsRequest {
        limit: Some(1000),
        status: Some("open".to_string()),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;
    println!("Fetched {} markets\n", response.markets.len());

    // Analyze each series
    let mut series_analysis: HashMap<String, SeriesStats> = HashMap::new();

    for market in &response.markets {
        let series = market.ticker.split('-').next().unwrap_or("UNKNOWN").to_string();

        let stats = series_analysis.entry(series.clone()).or_insert_with(|| SeriesStats {
            series: series.clone(),
            total: 0,
            predictable: 0,
            unpredictable: 0,
            missing_expected: 0,
            sample_ticker: market.ticker.clone(),
        });

        stats.total += 1;

        if let Some(expected) = market.expected_expiration_time {
            // Calculate time difference in hours
            let diff_seconds = (market.close_time.timestamp() - expected.timestamp()).abs();
            let diff_hours = diff_seconds / 3600;

            if diff_hours <= 2 {
                stats.predictable += 1;
            } else {
                stats.unpredictable += 1;
            }
        } else {
            // No expected_expiration_time - check if close_time ≈ expiration_time
            let diff_seconds = (market.close_time.timestamp() - market.expiration_time.timestamp()).abs();
            let diff_hours = diff_seconds / 3600;

            if diff_hours <= 2 {
                stats.predictable += 1;
            } else {
                stats.missing_expected += 1;
            }
        }
    }

    println!("═══════════════════════════════════════════════════════════════");
    println!("SERIES WITH PREDICTABLE SETTLEMENT (100% predictable):");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut predictable_series: Vec<_> = series_analysis.values()
        .filter(|s| s.predictable == s.total && s.total > 0)
        .collect();
    predictable_series.sort_by(|a, b| b.total.cmp(&a.total));

    let mut fully_predictable_prefixes = HashSet::new();

    for stats in &predictable_series {
        println!("{:30} - {} markets (100% predictable)", stats.series, stats.total);
        println!("   Sample: {}", stats.sample_ticker);
        fully_predictable_prefixes.insert(stats.series.clone());
    }

    if predictable_series.is_empty() {
        println!("(none found in sample)");
    }

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("SERIES WITH UNPREDICTABLE SETTLEMENT (0% predictable):");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut unpredictable_series: Vec<_> = series_analysis.values()
        .filter(|s| s.unpredictable == s.total && s.total > 0)
        .collect();
    unpredictable_series.sort_by(|a, b| b.total.cmp(&a.total));

    let mut fully_unpredictable_prefixes = HashSet::new();

    for stats in &unpredictable_series {
        println!("{:30} - {} markets (0% predictable - all sports?)", stats.series, stats.total);
        println!("   Sample: {}", stats.sample_ticker);
        fully_unpredictable_prefixes.insert(stats.series.clone());
    }

    if unpredictable_series.is_empty() {
        println!("(none found in sample)");
    }

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("SERIES WITH MIXED PREDICTABILITY:");
    println!("═══════════════════════════════════════════════════════════════\n");

    let mut mixed_series: Vec<_> = series_analysis.values()
        .filter(|s| s.predictable > 0 && (s.unpredictable > 0 || s.missing_expected > 0))
        .collect();
    mixed_series.sort_by(|a, b| b.total.cmp(&a.total));

    for stats in &mixed_series {
        let pct = (stats.predictable as f64 / stats.total as f64) * 100.0;
        println!("{:30} - {}/{} predictable ({:.0}%)", stats.series, stats.predictable, stats.total, pct);
        println!("   Sample: {}", stats.sample_ticker);
    }

    if mixed_series.is_empty() {
        println!("(none found in sample)");
    }

    println!("\n═══════════════════════════════════════════════════════════════");
    println!("RECOMMENDED IMPLEMENTATION:");
    println!("═══════════════════════════════════════════════════════════════\n");

    println!("Approach 1: Whitelist fully predictable series");
    println!("  - Add these series to SETTLEMENT_PREDICTABLE_SERIES:");
    for series in &fully_predictable_prefixes {
        println!("    \"{}\"", series);
    }

    println!("\nApproach 2: Blacklist sports series (more robust)");
    println!("  - Exclude these series from settlement logic:");
    for series in &fully_unpredictable_prefixes {
        if series.contains("GAME") {
            println!("    \"{}\"  // Sports", series);
        }
    }

    println!("\nApproach 3: Runtime check (best - data-driven)");
    println!("  fn is_settlement_predictable(&self) -> bool {{");
    println!("      if let Some(expected) = self.expected_expiration_time {{");
    println!("          let diff = (self.close_time - expected).num_hours().abs();");
    println!("          diff <= 2  // Within 2 hours = predictable");
    println!("      }} else {{");
    println!("          let diff = (self.close_time - self.expiration_time).num_hours().abs();");
    println!("          diff <= 2");
    println!("      }}");
    println!("  }}");

    println!("\n💡 RECOMMENDATION: Use Approach 3 (runtime check)");
    println!("   - No hardcoded series lists to maintain");
    println!("   - Adapts to new market types automatically");
    println!("   - Based on actual API data");

    Ok(())
}

#[derive(Debug)]
struct SeriesStats {
    series: String,
    total: usize,
    predictable: usize,
    unpredictable: usize,
    missing_expected: usize,
    sample_ticker: String,
}
