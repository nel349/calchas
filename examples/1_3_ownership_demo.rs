// Phase 1.3: Ownership and Memory Demonstration
// This example shows how Rust's ownership system works in a trading context

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn main() {
    println!("=== RUST OWNERSHIP DEMO ===\n");

    // ==========================================================================
    // PART 1: STACK vs HEAP
    // ==========================================================================
    println!("📚 Part 1: Stack vs Heap");
    println!("------------------------");

    // Stack: Fixed-size data (very fast)
    // - Primitives: i32, u64, Decimal, bool, etc.
    // - Known size at compile time
    let price = dec!(0.11);           // Lives on stack
    let quantity: u64 = 100;          // Lives on stack

    // Heap: Dynamic-size data (slower, but flexible)
    // - String, Vec, etc.
    // - Size can change at runtime
    let market_name = String::from("RAIN-24"); // Lives on heap

    println!("Stack data (fixed size): price={}, quantity={}", price, quantity);
    println!("Heap data (dynamic): market_name={}", market_name);
    println!();

    // ==========================================================================
    // PART 2: OWNERSHIP - "Each value has ONE owner"
    // ==========================================================================
    println!("📦 Part 2: Ownership");
    println!("-------------------");

    let strategy_name = String::from("Underdog Hunter");
    println!("Created: strategy_name = '{}'", strategy_name);

    // MOVE: Ownership transfers to new variable
    let active_strategy = strategy_name;
    println!("Moved to: active_strategy = '{}'", active_strategy);

    // ❌ This would ERROR: strategy_name was MOVED
    // println!("Can't use anymore: {}", strategy_name);
    // Error: "value borrowed here after move"

    println!("✅ Only active_strategy can use the value now");
    println!();

    // ==========================================================================
    // PART 3: REFERENCES - "Borrow without taking ownership"
    // ==========================================================================
    println!("🔗 Part 3: References (Borrowing)");
    println!("---------------------------------");

    let market = String::from("RAIN-NY-2024");

    // Pass by reference (&) - BORROW, don't MOVE
    print_market_info(&market);  // market is borrowed
    print_market_info(&market);  // can borrow again!

    println!("✅ Original still valid: {}", market);
    println!();

    // ==========================================================================
    // PART 4: MUTABLE REFERENCES - "Borrow and modify"
    // ==========================================================================
    println!("✏️  Part 4: Mutable References");
    println!("------------------------------");

    let mut position_size = 100;
    println!("Initial position: {} contracts", position_size);

    // Borrow mutably to change the value
    increase_position(&mut position_size, 50);
    println!("After increase: {} contracts", position_size);

    increase_position(&mut position_size, 25);
    assert!(position_size == 175, "Position size should be 175");
    println!("After another increase: {} contracts", position_size);
    println!();

    // ==========================================================================
    // PART 5: THE RULES - Why Rust is safe
    // ==========================================================================
    println!("⚖️  Part 5: Ownership Rules");
    println!("---------------------------");
    println!("Rule 1: Each value has ONE owner");
    println!("Rule 2: When owner goes out of scope, value is dropped (freed)");
    println!("Rule 3: Either ONE mutable reference OR many immutable references");
    println!("        (Never both at the same time!)");
    println!();

    // This prevents:
    // - Use after free
    // - Double free
    // - Data races
    // - Null pointer dereferences

    println!("✅ These rules eliminate entire classes of bugs at COMPILE TIME!");
    println!();

    // ==========================================================================
    // PART 6: REAL TRADING EXAMPLE
    // ==========================================================================
    println!("💰 Part 6: Real Trading Example");
    println!("-------------------------------");

    // Immutable reference: Can read, can't modify
    let entry_price = dec!(0.11);
    let exit_price = dec!(0.24);

    let profit = calculate_profit(&entry_price, &exit_price, 100);
    println!("Profit calculation (borrowed prices): ${:.2}", profit);
    println!("Original prices still valid: entry={}, exit={}", entry_price, exit_price);
    println!();

    // Mutable reference: Can modify
    let mut trade_log = Vec::new();
    record_trade(&mut trade_log, "Bought RAIN-NY at $0.11");
    record_trade(&mut trade_log, "Sold RAIN-NY at $0.24");

    println!("Trade log:");
    for (i, entry) in trade_log.iter().enumerate() {
        println!("  {}. {}", i + 1, entry);
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Takes an immutable reference - can READ but not MODIFY
fn print_market_info(market: &String) {
    println!("  Market info: {}", market);
    // Can't modify: market.push_str("-EXTRA") would ERROR
}

/// Takes a mutable reference - can READ and MODIFY
fn increase_position(position: &mut u64, amount: u64) {
    *position += amount;  // The * "dereferences" to get the actual value
}

/// Borrows values to calculate, returns owned result
fn calculate_profit(entry: &Decimal, exit: &Decimal, qty: u64) -> Decimal {
    (*exit - *entry) * Decimal::from(qty)
}

/// Borrows vector mutably to add entries
fn record_trade(log: &mut Vec<String>, entry: &str) {
    log.push(String::from(entry));
}
