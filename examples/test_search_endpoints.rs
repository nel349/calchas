//! Test new search/discovery endpoints
//!
//! Tests:
//! - GET /search/tags_by_categories
//! - GET /search/filters_by_sport

use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    tracing::info!("🔍 Testing Search/Discovery Endpoints");
    tracing::info!("");

    let config = AppConfig::load_with_env_default()?;
    let client = std::sync::Arc::new(KalshiClient::from_config(&config.kalshi)?);

    // Test 1: Get tags by categories
    tracing::info!("=== TEST 1: GET /search/tags_by_categories ===");
    match client.get_tags_by_categories().await {
        Ok(response) => {
            tracing::info!("✅ Success! Found {} categories", response.tags_by_categories.len());

            // Show first 5 categories
            for (i, (category, tags_opt)) in response.tags_by_categories.iter().take(5).enumerate() {
                match tags_opt {
                    Some(tags) => {
                        tracing::info!("  {}. {} ({} tags)", i + 1, category, tags.len());
                        // Show first 3 tags
                        for tag in tags.iter().take(3) {
                            tracing::info!("     - {}", tag);
                        }
                    }
                    None => {
                        tracing::info!("  {}. {} (no tags)", i + 1, category);
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed: {}", e);
        }
    }
    tracing::info!("");

    // Test 2: Get filters by sport
    tracing::info!("=== TEST 2: GET /search/filters_by_sport ===");
    match client.get_filters_by_sport().await {
        Ok(response) => {
            tracing::info!("✅ Success! Found {} sports", response.filters_by_sports.len());
            tracing::info!("");

            // Show details for each sport
            for (sport, filters) in &response.filters_by_sports {
                tracing::info!("Sport: {}", sport);
                tracing::info!("  Scopes ({}):", filters.scopes.len());
                for scope in &filters.scopes {
                    tracing::info!("    - {}", scope);
                }
                tracing::info!("  Competitions ({}):", filters.competitions.len());
                for (comp, details) in filters.competitions.iter().take(3) {
                    tracing::info!("    - {}: {} scopes", comp, details.scopes.len());
                }
                tracing::info!("");
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed: {}", e);
        }
    }

    tracing::info!("✅ All tests completed!");

    Ok(())
}
