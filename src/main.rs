// Declare modules
mod utils;
mod kalshi;

// Import specific functions we want to use
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use utils::decimal::{
    calculate_return_pct,
    calculate_profit_usd,
};
use kalshi::fees::{
    calculate_round_trip_fees,
    calculate_net_profit_usd,
    calculate_net_return_pct,
};

fn main() {
    println!("🔮 Calchas - Prediction Market Trading Bot\n");

    // Test our utility functions with a real trade example
    // Bought underdog at 11¢, sold at 24¢ (the proven trade from PRD)
    let entry_price = dec!(0.11);   // 11 cents
    let exit_price = dec!(0.24);    // 24 cents
    let quantity = 100;             // 100 contracts

    println!("📊 Trade Analysis:");
    println!("   Entry: ${}", entry_price);
    println!("   Exit:  ${}", exit_price);
    println!("   Quantity: {} contracts", quantity);
    println!("   Entry cost: ${:.2}", entry_price * Decimal::from(quantity));
    println!();

    // Gross calculations (before fees)
    let gross_return = calculate_return_pct(entry_price, exit_price);
    let gross_profit = calculate_profit_usd(entry_price, exit_price, quantity);

    println!("💵 Gross Performance (before fees):");
    println!("   Return: {:.2}%", gross_return);
    println!("   Profit: ${:.2}", gross_profit);
    println!();

    // Fee calculations (using market orders = taker fees)
    let fees_taker = calculate_round_trip_fees(entry_price, exit_price, quantity, false);
    let net_profit_taker = calculate_net_profit_usd(entry_price, exit_price, quantity, false);
    let net_return_taker = calculate_net_return_pct(entry_price, exit_price, quantity, false);

    println!("📉 Fees (Market Orders - Taker):");
    println!("   Total fees: ${:.2}", fees_taker);
    println!();

    println!("✅ Net Performance (after fees):");
    println!("   Net profit: ${:.2}", net_profit_taker);
    println!("   Net return: {:.2}%", net_return_taker);
    println!();

    // Show difference between maker vs taker fees
    let fees_maker = calculate_round_trip_fees(entry_price, exit_price, quantity, true);
    let net_profit_maker = calculate_net_profit_usd(entry_price, exit_price, quantity, true);
    let savings = fees_taker - fees_maker;

    println!("💡 Tip: Using Limit Orders (Maker):");
    println!("   Maker fees: ${:.2}", fees_maker);
    println!("   Net profit: ${:.2}", net_profit_maker);
    println!("   Savings: ${:.2} ({:.1}% less fees!)", savings, (savings / fees_taker) * dec!(100));
    println!();

    // Show fee behavior at extreme prices
    println!("📐 Fee Formula Behavior (100 contracts):");
    println!("   Price  | Taker Fee | Notes");
    println!("   -------|-----------|---------------------------");

    let test_prices = vec![
        (dec!(0.01), "1¢ (very cheap)"),
        (dec!(0.11), "11¢ (your entry)"),
        (dec!(0.24), "24¢ (your exit)"),
        (dec!(0.50), "50¢ (max fee point)"),
        (dec!(0.75), "75¢ (expensive)"),
        (dec!(0.99), "99¢ (nearly certain)"),
    ];

    for (price, label) in test_prices {
        let fee = calculate_round_trip_fees(price, price, 100, false) / dec!(2.0); // Single side
        println!("   {}  | ${:<9.4} | {}", price, fee, label);
    }

    println!();
    println!("   💰 Fee is highest at 50¢ (maximum uncertainty)");
    println!("   💰 Fee approaches $0 as price → $0 or $1");
    println!("   💰 Fee CAP: $1.75 per 100 contracts (never exceeds this)");
}
