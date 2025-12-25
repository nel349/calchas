// Decimal helper functions for financial calculations
// Uses rust_decimal to avoid floating-point precision errors

use rust_decimal::Decimal;

/// Calculate profit/loss percentage
/// Example: entry=10, exit=15 → returns 50.0 (meaning 50% gain)
pub fn calculate_return_pct(entry_price: Decimal, exit_price: Decimal) -> Decimal {
    // Formula: ((exit - entry) / entry) * 100
    // Example: ((15 - 10) / 10) * 100 = 50%

    let difference = exit_price - entry_price;
    let ratio = difference / entry_price;
    let percentage = ratio * Decimal::from(100);

    percentage
}

/// Calculate profit/loss in dollar amount
/// Example: entry=10, exit=15, quantity=100 → returns 500 (100 contracts * $5 profit each)
pub fn calculate_profit_usd(
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: u64,  // unsigned 64-bit integer (can't be negative)
) -> Decimal {
    let price_difference = exit_price - entry_price;
    let quantity_decimal = Decimal::from(quantity);

    price_difference * quantity_decimal
}

/// Convert percentage to decimal multiplier
/// Example: 50 (meaning 50%) → returns 0.50
pub fn pct_to_decimal(percentage: Decimal) -> Decimal {
    percentage / Decimal::from(100)
}

/// Convert decimal multiplier to percentage
/// Example: 0.50 → returns 50 (meaning 50%)
pub fn decimal_to_pct(decimal: Decimal) -> Decimal {
    decimal * Decimal::from(100)
}

/// Calculate price after percentage change
/// Example: current_price=20, change_pct=50 → returns 30 (20 + 50% of 20)
pub fn apply_percentage_change(current_price: Decimal, change_pct: Decimal) -> Decimal {
    let change_decimal = pct_to_decimal(change_pct);
    let change_amount = current_price * change_decimal;

    current_price + change_amount
}

// NOTE: Kalshi-specific fee calculations have been moved to src/kalshi/fees.rs
// This keeps platform-specific logic separate from generic utility functions
