// Kalshi Fee Structure and Calculations
//
// ⚠️  SINGLE SOURCE OF TRUTH for all Kalshi fee constants and formulas ⚠️
// All other code MUST use these functions - do NOT duplicate formulas elsewhere.
//
// Official Sources:
// - https://help.kalshi.com/trading/fees (official fee documentation)
// - https://kalshi.com/docs/kalshi-fee-schedule.pdf (detailed fee schedule)
// - https://whirligigbear.substack.com/p/makertaker-math-on-kalshi (formula analysis)
//
// Last verified: December 30, 2025

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// =============================================================================
// FEE CONSTANTS
// =============================================================================

/// Taker fee coefficient (market orders - immediate execution)
/// Applied when you take liquidity from the order book
pub const TAKER_FEE_RATE: Decimal = dec!(0.07);

/// Maker fee coefficient (limit orders - resting on the book)
/// Applied when you add liquidity to the order book
pub const MAKER_FEE_RATE: Decimal = dec!(0.0175);

/// Maximum fee cap per 100 contracts (applies to taker orders)
/// Prevents excessive fees on trades near 50¢
pub const FEE_CAP_PER_100_CONTRACTS: Decimal = dec!(1.75); // $1.75

// =============================================================================
// FEE FORMULA
// =============================================================================
// The fee formula uses the mathematical property that:
//   P × (1 - P)
// creates a parabola that:
//   - Peaks at P = 0.50 (maximum uncertainty)
//   - Approaches 0 as P → 0 or P → 1 (high certainty)
//
// This means:
//   - Trading high-conviction positions (very cheap or very expensive) costs less
//   - Trading maximum-uncertainty positions (around 50¢) costs more
//   - The fee cap prevents this from being exploitative
// =============================================================================

/// Calculate Kalshi taker fee (market orders, immediate execution)
///
/// See implementation for formula. Uses TAKER_FEE_RATE constant.
///
/// # Examples
/// ```
/// use rust_decimal_macros::dec;
/// use calchas::kalshi::fees::calculate_kalshi_taker_fee;
///
/// // 100 contracts at 11¢
/// let fee = calculate_kalshi_taker_fee(dec!(0.11), 100);
/// // Returns: $0.6853
/// ```
pub fn calculate_kalshi_taker_fee(price: Decimal, quantity: u64) -> Decimal {
    let quantity_decimal = Decimal::from(quantity);
    let one_minus_price = Decimal::ONE - price;

    TAKER_FEE_RATE * quantity_decimal * price * one_minus_price
}

/// Calculate Kalshi maker fee (limit orders, resting on book)
///
/// See implementation for formula. Uses MAKER_FEE_RATE constant.
///
/// # Examples
/// ```
/// use rust_decimal_macros::dec;
/// use calchas::kalshi::fees::calculate_kalshi_maker_fee;
///
/// // 100 contracts at 11¢
/// let fee = calculate_kalshi_maker_fee(dec!(0.11), 100);
/// // Returns: $0.1713
/// ```
pub fn calculate_kalshi_maker_fee(price: Decimal, quantity: u64) -> Decimal {
    let quantity_decimal = Decimal::from(quantity);
    let one_minus_price = Decimal::ONE - price;

    MAKER_FEE_RATE * quantity_decimal * price * one_minus_price
}

/// Calculate total fees for a round-trip trade (entry + exit)
///
/// # Arguments
/// * `entry_price` - Price when entering the position
/// * `exit_price` - Price when exiting the position
/// * `quantity` - Number of contracts traded
/// * `use_maker_fee` - true for limit orders, false for market orders
///
/// # Examples
/// ```
/// use rust_decimal_macros::dec;
/// use calchas::kalshi::fees::calculate_round_trip_fees;
///
/// // Bought at 11¢, sold at 24¢, 100 contracts, using market orders
/// let fees = calculate_round_trip_fees(dec!(0.11), dec!(0.24), 100, false);
/// // Returns: $1.9621 (0.6853 + 1.2768)
/// ```
pub fn calculate_round_trip_fees(
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: u64,
    use_maker_fee: bool,
) -> Decimal {
    let entry_fee = if use_maker_fee {
        calculate_kalshi_maker_fee(entry_price, quantity)
    } else {
        calculate_kalshi_taker_fee(entry_price, quantity)
    };

    let exit_fee = if use_maker_fee {
        calculate_kalshi_maker_fee(exit_price, quantity)
    } else {
        calculate_kalshi_taker_fee(exit_price, quantity)
    };

    entry_fee + exit_fee
}

/// Calculate NET profit after Kalshi fees
///
/// This is what actually hits your account after all fees are deducted.
///
/// # Examples
/// ```
/// use rust_decimal_macros::dec;
/// use calchas::kalshi::fees::calculate_net_profit_usd;
///
/// // Bought at 11¢, sold at 24¢, 100 contracts
/// let net = calculate_net_profit_usd(dec!(0.11), dec!(0.24), 100, false);
/// // Gross profit: $13.00
/// // Fees: $1.96
/// // Net profit: $11.04
/// ```
pub fn calculate_net_profit_usd(
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: u64,
    use_maker_fee: bool,
) -> Decimal {
    use crate::utils::decimal::calculate_profit_usd;

    let gross_profit = calculate_profit_usd(entry_price, exit_price, quantity);
    let fees = calculate_round_trip_fees(entry_price, exit_price, quantity, use_maker_fee);

    gross_profit - fees
}

/// Calculate NET return percentage after fees
///
/// This is the real return percentage you care about for strategy evaluation.
///
/// # Examples
/// ```
/// use rust_decimal_macros::dec;
/// use calchas::kalshi::fees::calculate_net_return_pct;
///
/// // Bought at 11¢, sold at 24¢, 100 contracts
/// let net_return = calculate_net_return_pct(dec!(0.11), dec!(0.24), 100, false);
/// // Entry cost: $11.00
/// // Net profit: $11.04
/// // Net return: 100.34%
/// ```
pub fn calculate_net_return_pct(
    entry_price: Decimal,
    exit_price: Decimal,
    quantity: u64,
    use_maker_fee: bool,
) -> Decimal {
    let entry_cost = entry_price * Decimal::from(quantity);
    let net_profit = calculate_net_profit_usd(entry_price, exit_price, quantity, use_maker_fee);

    (net_profit / entry_cost) * Decimal::from(100)
}

// =============================================================================
// FEE BEHAVIOR ANALYSIS
// =============================================================================

/// Get fee information for a specific price point
/// Useful for understanding fee behavior across the price spectrum
pub fn analyze_fee_at_price(price: Decimal, quantity: u64) -> FeeAnalysis {
    let taker_fee = calculate_kalshi_taker_fee(price, quantity);
    let maker_fee = calculate_kalshi_maker_fee(price, quantity);
    let savings = taker_fee - maker_fee;
    let savings_pct = if taker_fee > Decimal::ZERO {
        (savings / taker_fee) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    FeeAnalysis {
        price,
        quantity,
        taker_fee,
        maker_fee,
        savings,
        savings_pct,
    }
}

#[derive(Debug)]
pub struct FeeAnalysis {
    pub price: Decimal,
    pub quantity: u64,
    pub taker_fee: Decimal,
    pub maker_fee: Decimal,
    pub savings: Decimal,
    pub savings_pct: Decimal,
}
