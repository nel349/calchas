//! Deep investigation of Kalshi API response
//! Validate all assumptions in src/kalshi/types.rs conversion logic

use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=============================================================================");
    println!("DEEP INVESTIGATION: KALSHI API RESPONSE");
    println!("=============================================================================");
    println!();

    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    let request = GetMarketsRequest {
        status: Some("open".to_string()),
        limit: Some(10),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    println!("Response metadata:");
    println!("  cursor: {:?}", response.cursor);
    println!("  markets count: {}", response.markets.len());
    println!();

    // Analyze each market in detail
    for (i, market) in response.markets.iter().take(5).enumerate() {
        println!("=============================================================================");
        println!("MARKET #{}: {}", i + 1, market.ticker);
        println!("=============================================================================");
        println!();

        // Raw field values
        println!("--- RAW FIELDS FROM API ---");
        println!("ticker: {:?}", market.ticker);
        println!("event_ticker: {:?}", market.event_ticker);
        println!("market_type: {:?}", market.market_type);
        println!("title: {:?}", market.title);
        println!("subtitle: {:?}", market.subtitle);
        println!("yes_sub_title: {:?}", market.yes_sub_title);
        println!("no_sub_title: {:?}", market.no_sub_title);
        println!();

        println!("--- DATES/TIMES ---");
        println!("created_time: {}", market.created_time);
        println!("open_time: {}", market.open_time);
        println!("close_time: {}", market.close_time);
        println!("expiration_time: {}", market.expiration_time);
        println!();

        println!("--- STATUS & CATEGORY ---");
        println!("status: {:?}", market.status);
        println!("category: {:?} (empty: {})", market.category, market.category.is_empty());
        println!();

        println!("--- PRICES (in cents) ---");
        println!("response_price_units: {:?}", market.response_price_units);
        println!("yes_bid: {} cents", market.yes_bid);
        println!("yes_ask: {} cents", market.yes_ask);
        println!("no_bid: {} cents", market.no_bid);
        println!("no_ask: {} cents", market.no_ask);
        println!("last_price: {} cents", market.last_price);
        println!();

        // Check our price conversion assumptions
        println!("--- PRICE CONVERSION CHECK ---");
        let yes_bid_decimal = Decimal::new(market.yes_bid, 2);
        let yes_ask_decimal = Decimal::new(market.yes_ask, 2);
        let no_bid_decimal = Decimal::new(market.no_bid, 2);
        let no_ask_decimal = Decimal::new(market.no_ask, 2);

        println!("yes_bid: {} cents → ${}", market.yes_bid, yes_bid_decimal);
        println!("yes_ask: {} cents → ${}", market.yes_ask, yes_ask_decimal);
        println!("no_bid: {} cents → ${}", market.no_bid, no_bid_decimal);
        println!("no_ask: {} cents → ${}", market.no_ask, no_ask_decimal);

        let yes_avg = (yes_bid_decimal + yes_ask_decimal) / Decimal::from(2);
        let no_avg = (no_bid_decimal + no_ask_decimal) / Decimal::from(2);

        println!("yes_price (bid+ask)/2: ${}", yes_avg);
        println!("no_price (bid+ask)/2: ${}", no_avg);
        println!("Sum check: yes + no = ${} (should be ~$1.00)", yes_avg + no_avg);
        println!();

        println!("--- VOLUME & OPEN INTEREST ---");
        println!("volume: {} (negative sentinel: {})", market.volume, market.volume < 0);
        println!("volume_24h: {} (negative sentinel: {})", market.volume_24h, market.volume_24h < 0);
        println!("open_interest: {} (negative sentinel: {})", market.open_interest, market.open_interest < 0);
        println!("liquidity: {} (negative sentinel: {})", market.liquidity, market.liquidity < 0);

        // Check our conversion
        let converted_volume = market.volume.max(0) as u64;
        let converted_oi = market.open_interest.max(0) as u64;
        println!("After max(0) conversion:");
        println!("  volume: {}", converted_volume);
        println!("  open_interest: {}", converted_oi);
        println!();

        println!("--- OTHER FIELDS ---");
        println!("result: {:?}", market.result);
        println!("can_close_early: {}", market.can_close_early);
        println!("notional_value: {} cents", market.notional_value);
        println!();

        // Now convert using our From impl and compare
        let converted: calchas::models::Market = market.clone().into();

        println!("--- CONVERTED MARKET (our implementation) ---");
        println!("id: {:?}", converted.id);
        println!("ticker: {}", converted.ticker);
        println!("title: {}", converted.title);
        println!("category: {:?}", converted.category);
        println!("sub_category: {:?}", converted.sub_category);
        println!("status: {:?}", converted.status);
        println!("yes_price: ${}", converted.yes_price);
        println!("no_price: ${}", converted.no_price);
        println!("volume: {}", converted.volume);
        println!("open_interest: {}", converted.open_interest);
        println!("event_time: {}", converted.event_time);
        println!("close_time: {}", converted.close_time);
        println!("created_at: {}", converted.created_at);
        println!("updated_at: {}", converted.updated_at);
        println!();

        // Validation checks
        println!("--- VALIDATION CHECKS ---");

        // Check 1: Status conversion
        let expected_status = match market.status.as_str() {
            "active" | "open" => "Open",
            "closed" => "Closed",
            "settled" => "Settled",
            _ => "Unknown"
        };
        println!("✓ Status: '{}' → {:?} (expected: {})",
            market.status, converted.status, expected_status);

        // Check 2: Prices sum to ~$1
        let price_sum = converted.yes_price + converted.no_price;
        let sum_ok = price_sum >= Decimal::new(95, 2) && price_sum <= Decimal::new(105, 2);
        println!("{} Price sum: ${} (should be $0.95-$1.05)",
            if sum_ok { "✓" } else { "✗" },
            price_sum);

        // Check 3: Event time mapping
        let event_time_ok = converted.event_time == market.expiration_time;
        println!("{} Event time: {} (maps to expiration_time: {})",
            if event_time_ok { "✓" } else { "✗" },
            converted.event_time,
            event_time_ok);

        // Check 4: Close time mapping
        let close_time_ok = converted.close_time == market.close_time;
        println!("{} Close time: {} (maps correctly: {})",
            if close_time_ok { "✓" } else { "✗" },
            converted.close_time,
            close_time_ok);

        // Check 5: Created time mapping
        let created_ok = converted.created_at == market.created_time;
        println!("{} Created at: {} (maps to created_time: {})",
            if created_ok { "✓" } else { "✗" },
            converted.created_at,
            created_ok);

        // Check 6: Subtitle mapping
        let subtitle_ok = converted.sub_category == Some(market.subtitle.clone());
        println!("{} Sub-category: {:?} (maps to subtitle: {})",
            if subtitle_ok { "✓" } else { "✗" },
            converted.sub_category,
            subtitle_ok);

        println!();
    }

    println!("=============================================================================");
    println!("INVESTIGATION COMPLETE");
    println!("=============================================================================");
    println!();
    println!("KEY FINDINGS TO CHECK:");
    println!("1. Are all status values handled correctly?");
    println!("2. Do bid/ask averages make sense as 'price'?");
    println!("3. Do yes_price + no_price sum to $1?");
    println!("4. Are sentinel values (negative numbers) handled?");
    println!("5. Are date fields mapped correctly?");
    println!("6. Is subtitle the right field for sub_category?");

    Ok(())
}
