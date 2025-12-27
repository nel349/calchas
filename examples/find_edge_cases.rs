//! Find edge cases in Kalshi markets
//! Look for: zero prices, wide spreads, negative values, unusual data

use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=============================================================================");
    println!("SEARCHING FOR EDGE CASES IN KALSHI MARKETS");
    println!("=============================================================================");
    println!();

    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    let request = GetMarketsRequest {
        status: Some("open".to_string()),
        limit: Some(100),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    println!("Analyzing {} markets for edge cases...\n", response.markets.len());

    // Track edge cases
    let mut zero_bids = Vec::new();
    let mut zero_asks = Vec::new();
    let mut wide_spreads = Vec::new();
    let mut negative_volumes = Vec::new();
    let mut negative_oi = Vec::new();
    let mut sum_not_100 = Vec::new();
    let mut unusual_status = Vec::new();

    for market in &response.markets {
        let ticker = &market.ticker;

        // Check for zero prices
        if market.yes_bid == 0 {
            zero_bids.push((ticker.clone(), "yes_bid"));
        }
        if market.yes_ask == 0 {
            zero_asks.push((ticker.clone(), "yes_ask"));
        }
        if market.no_bid == 0 {
            zero_bids.push((ticker.clone(), "no_bid"));
        }
        if market.no_ask == 0 {
            zero_asks.push((ticker.clone(), "no_ask"));
        }

        // Check for wide spreads (>10 cents)
        let yes_spread = market.yes_ask - market.yes_bid;
        let no_spread = market.no_ask - market.no_bid;

        if yes_spread > 10 {
            wide_spreads.push((ticker.clone(), "yes", yes_spread));
        }
        if no_spread > 10 {
            wide_spreads.push((ticker.clone(), "no", no_spread));
        }

        // Check for negative sentinel values
        if market.volume < 0 {
            negative_volumes.push((ticker.clone(), market.volume));
        }
        if market.open_interest < 0 {
            negative_oi.push((ticker.clone(), market.open_interest));
        }

        // Check if bid+ask sums to 100 cents
        let yes_mid = (market.yes_bid + market.yes_ask) / 2;
        let no_mid = (market.no_bid + market.no_ask) / 2;
        let sum = yes_mid + no_mid;

        // Allow 5 cent tolerance due to rounding
        if sum < 95 || sum > 105 {
            sum_not_100.push((ticker.clone(), sum, yes_mid, no_mid));
        }

        // Check for unusual status
        if market.status != "active" && market.status != "open" && market.status != "closed" && market.status != "settled" {
            unusual_status.push((ticker.clone(), market.status.clone()));
        }
    }

    // Report findings
    println!("=== EDGE CASE FINDINGS ===\n");

    println!("1. ZERO PRICES:");
    if zero_bids.is_empty() && zero_asks.is_empty() {
        println!("   ✓ No zero prices found");
    } else {
        println!("   Zero bids: {}", zero_bids.len());
        for (ticker, field) in zero_bids.iter().take(5) {
            println!("     - {}: {}", ticker, field);
        }
        println!("   Zero asks: {}", zero_asks.len());
        for (ticker, field) in zero_asks.iter().take(5) {
            println!("     - {}: {}", ticker, field);
        }
    }
    println!();

    println!("2. WIDE SPREADS (>10 cents):");
    if wide_spreads.is_empty() {
        println!("   ✓ No unusually wide spreads found");
    } else {
        println!("   Found {} wide spreads", wide_spreads.len());
        for (ticker, side, spread) in wide_spreads.iter().take(5) {
            println!("     - {}: {} side spread = {} cents", ticker, side, spread);
        }
    }
    println!();

    println!("3. NEGATIVE SENTINEL VALUES:");
    if negative_volumes.is_empty() {
        println!("   ✓ No negative volumes");
    } else {
        println!("   Negative volumes: {}", negative_volumes.len());
        for (ticker, vol) in negative_volumes.iter().take(5) {
            println!("     - {}: volume = {}", ticker, vol);
        }
    }

    if negative_oi.is_empty() {
        println!("   ✓ No negative open interest");
    } else {
        println!("   Negative OI: {}", negative_oi.len());
        for (ticker, oi) in negative_oi.iter().take(5) {
            println!("     - {}: open_interest = {}", ticker, oi);
        }
    }
    println!();

    println!("4. PRICE SUMS != 100 CENTS:");
    if sum_not_100.is_empty() {
        println!("   ✓ All prices sum to ~100 cents");
    } else {
        println!("   ⚠ {} markets don't sum to 100 cents", sum_not_100.len());
        for (ticker, sum, yes_mid, no_mid) in sum_not_100.iter().take(5) {
            println!("     - {}: sum={} (yes={}, no={})", ticker, sum, yes_mid, no_mid);
        }
    }
    println!();

    println!("5. UNUSUAL STATUS VALUES:");
    if unusual_status.is_empty() {
        println!("   ✓ All status values are known");
    } else {
        println!("   ⚠ {} markets with unusual status", unusual_status.len());
        for (ticker, status) in unusual_status.iter() {
            println!("     - {}: status = {:?}", ticker, status);
        }
    }
    println!();

    // Test our conversion on an edge case if we found any
    if !wide_spreads.is_empty() {
        let (ticker, _, _) = &wide_spreads[0];
        let market = response.markets.iter().find(|m| &m.ticker == ticker).unwrap();

        println!("=== TESTING CONVERSION ON EDGE CASE ===");
        println!("Market: {}", ticker);
        println!("Raw prices:");
        println!("  yes: {} bid, {} ask (spread: {})", market.yes_bid, market.yes_ask, market.yes_ask - market.yes_bid);
        println!("  no: {} bid, {} ask (spread: {})", market.no_bid, market.no_ask, market.no_ask - market.no_bid);

        let converted: calchas::models::Market = market.clone().into();
        println!("Converted:");
        println!("  yes_price: ${}", converted.yes_price);
        println!("  no_price: ${}", converted.no_price);
        println!("  sum: ${}", converted.yes_price + converted.no_price);
        println!();
    }

    println!("=============================================================================");
    println!("EDGE CASE INVESTIGATION COMPLETE");
    println!("=============================================================================");

    Ok(())
}
