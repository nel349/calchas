// Phase 1.2: Control Flow Crash Course
// if/else, loops, match - demonstrated with trading examples

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn main() {
    println!("=== CONTROL FLOW CRASH COURSE ===\n");

    // ==========================================================================
    // PART 1: IF/ELSE - Conditional logic
    // ==========================================================================
    println!("🔀 Part 1: if/else");
    println!("------------------\n");

    let price = dec!(0.11);
    let threshold = dec!(0.20);

    // Simple if
    if price < threshold {
        println!("Price ${} is below threshold ${} - BUY SIGNAL", price, threshold);
    }
    println!();

    // if/else
    let exit_price = dec!(0.24);
    let entry_price = dec!(0.11);

    if exit_price > entry_price {
        println!("Exit ${} > Entry ${} - PROFITABLE", exit_price, entry_price);
    } else {
        println!("Exit ${} <= Entry ${} - NOT PROFITABLE", exit_price, entry_price);
    }
    println!();

    // if/else if/else
    let profit_pct = dec!(100.0);

    if profit_pct > dec!(50.0) {
        println!("Profit {}% - EXCELLENT TRADE", profit_pct);
    } else if profit_pct > dec!(20.0) {
        println!("Profit {}% - GOOD TRADE", profit_pct);
    } else if profit_pct > dec!(0.0) {
        println!("Profit {}% - SMALL WIN", profit_pct);
    } else {
        println!("Profit {}% - LOSS", profit_pct);
    }
    println!();

    // if as an expression (returns value)
    let position_size = if profit_pct > dec!(50.0) {
        200  // Large position for high confidence
    } else {
        100  // Standard position
    };
    println!("Position size based on confidence: {} contracts", position_size);
    println!();

    // ==========================================================================
    // PART 2: LOOPS - Iteration
    // ==========================================================================
    println!("🔁 Part 2: Loops");
    println!("----------------\n");

    // for loop - iterate over range
    println!("Price movements over 5 periods:");
    let prices = [dec!(0.11), dec!(0.15), dec!(0.18), dec!(0.22), dec!(0.24)];

    for (i, price) in prices.iter().enumerate() {
        println!("  Period {}: ${}", i + 1, price);
    }
    println!();

    // for loop - iterate over collection
    println!("Checking multiple markets:");
    let market_tickers = vec!["RAIN-NY-2024", "SNOW-CO-2024", "TEMP-TX-2024"];

    for ticker in &market_tickers {
        println!("  Processing market: {}", ticker);
    }
    println!();

    // while loop - conditional iteration
    println!("Waiting for profitable exit:");
    let mut current_price = dec!(0.11);
    let target_price = dec!(0.20);
    let mut period = 0;

    while current_price < target_price && period < 10 {
        period += 1;
        current_price += dec!(0.02);
        println!("  Period {}: Price ${}", period, current_price);
    }

    if current_price >= target_price {
        println!("✅ Target reached at period {}!", period);
    } else {
        println!("❌ Target not reached after {} periods", period);
    }
    println!();

    // loop - infinite loop with break
    println!("Monitoring position:");
    let mut position_value = dec!(11.00);
    let stop_loss = dec!(9.00);
    let take_profit = dec!(24.00);
    let mut tick = 0;

    loop {
        tick += 1;
        position_value += dec!(1.50); // Simulate price movement

        if position_value <= stop_loss {
            println!("  Tick {}: ${} - STOP LOSS HIT", tick, position_value);
            break;
        } else if position_value >= take_profit {
            println!("  Tick {}: ${} - TAKE PROFIT HIT", tick, position_value);
            break;
        } else if tick > 20 {
            println!("  Tick {}: ${} - MAX ITERATIONS", tick, position_value);
            break;
        }
    }
    println!();

    // continue - skip iteration
    println!("Filtering markets (skip invalid):");
    let market_prices = vec![
        ("RAIN-NY", Some(dec!(0.11))),
        ("SNOW-CO", None),  // No price available
        ("TEMP-TX", Some(dec!(0.45))),
        ("WIND-CA", None),
    ];

    for (ticker, price) in &market_prices {
        if price.is_none() {
            println!("  {} - SKIP (no price)", ticker);
            continue;  // Skip to next iteration
        }
        println!("  {} - ${}", ticker, price.unwrap());
    }
    println!();

    // ==========================================================================
    // PART 3: MATCH - Pattern matching
    // ==========================================================================
    println!("🎯 Part 3: match (Pattern Matching)");
    println!("-----------------------------------\n");

    // Match with simple enum
    #[derive(Debug)]
    enum OrderStatus {
        Pending,
        PartiallyFilled,
        Filled,
        Cancelled,
    }

    let status = OrderStatus::Filled;

    match status {
        OrderStatus::Pending => println!("Order status: Waiting..."),
        OrderStatus::PartiallyFilled => println!("Order status: Partial fill"),
        OrderStatus::Filled => println!("Order status: Completed!"),
        OrderStatus::Cancelled => println!("Order status: Cancelled"),
    }
    println!();

    // Match with values
    let profit_pct = 75;

    let rating = match profit_pct {
        0..=10 => "Poor",
        11..=30 => "Fair",
        31..=60 => "Good",
        61..=100 => "Excellent",
        _ => "Exceptional",  // Catch-all pattern
    };
    println!("Trade rating for {}% profit: {}", profit_pct, rating);
    println!();

    // Match with Option
    let maybe_price = Some(dec!(0.24));

    match maybe_price {
        Some(price) => println!("Price available: ${}", price),
        None => println!("Price unavailable"),
    }
    println!();

    // if let - single pattern match
    let maybe_exit_price = Some(dec!(0.24));

    if let Some(exit) = maybe_exit_price {
        println!("if let: Exit price is ${}", exit);
    } else {
        println!("if let: No exit price");
    }
    println!();

    // Match as expression (returns value)
    let position_action = match current_price {
        p if p < dec!(0.10) => "BUY",
        p if p > dec!(0.90) => "SELL",
        _ => "HOLD",
    };
    println!("Action for price ${}: {}", current_price, position_action);
    println!();

    // ==========================================================================
    // PART 4: Combining control flow
    // ==========================================================================
    println!("🔗 Part 4: Real Trading Example");
    println!("-------------------------------\n");

    let markets = vec![
        ("RAIN-NY", dec!(0.08)),
        ("SNOW-CO", dec!(0.15)),
        ("TEMP-TX", dec!(0.45)),
        ("WIND-CA", dec!(0.92)),
    ];

    println!("Scanning markets for opportunities:");

    for (ticker, price) in markets {
        // Skip if price is in dead zone (0.30-0.70)
        if price > dec!(0.30) && price < dec!(0.70) {
            println!("  {} @ ${} - SKIP (dead zone)", ticker, price);
            continue;
        }

        // Determine action based on price
        let action = if price < dec!(0.20) {
            "BUY (undervalued)"
        } else if price > dec!(0.80) {
            "SELL (overvalued)"
        } else {
            "WATCH"
        };

        println!("  {} @ ${} - {}", ticker, price, action);
    }
    println!();

    // ==========================================================================
    // SUMMARY
    // ==========================================================================
    println!("📝 SUMMARY");
    println!("----------\n");

    println!("Control flow tools:");
    println!("  if/else:  Conditional execution");
    println!("  for:      Iterate over collections");
    println!("  while:    Loop while condition is true");
    println!("  loop:     Infinite loop (use break)");
    println!("  match:    Pattern matching (exhaustive)");
    println!("  if let:   Single pattern matching");
    println!();

    println!("Key points:");
    println!("  ✅ match is EXHAUSTIVE (must handle all cases)");
    println!("  ✅ if/match can be expressions (return values)");
    println!("  ✅ Use continue to skip, break to exit");
    println!("  ✅ Pattern matching is very powerful in Rust");
}
