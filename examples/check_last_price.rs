//! Check if last_price is better than bid/ask midpoint

use calchas::kalshi::client::KalshiClient;
use calchas::kalshi::types::GetMarketsRequest;
use calchas::config::AppConfig;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    let request = GetMarketsRequest {
        status: Some("open".to_string()),
        limit: Some(100),
        ..Default::default()
    };

    let response = client.get_markets(request).await?;

    println!("=============================================================================");
    println!("COMPARING PRICE CALCULATION METHODS");
    println!("=============================================================================");
    println!();
    println!("Showing 10 markets with different price representations:\n");

    for (i, market) in response.markets.iter().take(10).enumerate() {
        println!("Market #{}: {}", i + 1, market.ticker);
        println!("  Title: {}", market.title);
        println!();

        // Method 1: Bid/Ask Midpoint (current approach)
        let yes_mid = Decimal::new((market.yes_bid + market.yes_ask) / 2, 2);
        let no_mid = Decimal::new((market.no_bid + market.no_ask) / 2, 2);

        // Method 2: Last traded price
        let last = Decimal::new(market.last_price, 2);

        // Method 3: Best ask (what you'd pay to buy)
        let yes_ask = Decimal::new(market.yes_ask, 2);
        let no_ask = Decimal::new(market.no_ask, 2);

        println!("  Yes side:");
        println!("    Bid/Ask: {} / {} (spread: {} cents)",
            market.yes_bid, market.yes_ask, market.yes_ask - market.yes_bid);
        println!("    Midpoint: ${}", yes_mid);
        println!("    Best Ask (buy price): ${}", yes_ask);
        println!("    Last Price: ${}", last);
        println!();

        println!("  No side:");
        println!("    Bid/Ask: {} / {} (spread: {} cents)",
            market.no_bid, market.no_ask, market.no_ask - market.no_bid);
        println!("    Midpoint: ${}", no_mid);
        println!("    Best Ask (buy price): ${}", no_ask);
        println!();

        // Check for zero bids
        if market.yes_bid == 0 || market.no_bid == 0 {
            println!("  ⚠ WARNING: Zero bid detected!");
            println!();
        }

        // Check for wide spreads
        let yes_spread = market.yes_ask - market.yes_bid;
        let no_spread = market.no_ask - market.no_bid;
        if yes_spread > 20 || no_spread > 20 {
            println!("  ⚠ WARNING: Wide spread (>{} cents)", yes_spread.max(no_spread));
            println!();
        }
    }

    println!("=============================================================================");
    println!("RECOMMENDATION");
    println!("=============================================================================");
    println!();
    println!("Options for 'price' field in Market model:");
    println!();
    println!("1. Bid/Ask Midpoint (CURRENT)");
    println!("   - Pro: Represents fair value");
    println!("   - Pro: Always available");
    println!("   - Con: Not a tradeable price");
    println!("   - Con: Can be misleading with wide spreads");
    println!("   - Con: Issues when bid = 0");
    println!();
    println!("2. Last Price");
    println!("   - Pro: Actual traded price");
    println!("   - Con: May be stale (no recent trades)");
    println!("   - Con: Might be far from current market");
    println!();
    println!("3. Best Ask (what you pay to buy)");
    println!("   - Pro: Actual executable price");
    println!("   - Pro: Matches our 'buy cheaper side' strategy");
    println!("   - Con: Biased toward buy side");
    println!("   - Con: Changes frequently");
    println!();
    println!("4. Keep separate bid/ask fields");
    println!("   - Pro: Full market information");
    println!("   - Con: More complex model");
    println!("   - Con: Strategies need to handle bid/ask");

    Ok(())
}
