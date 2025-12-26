// Order data model
// Section 4.4 of TECHNICAL_ARCHITECTURE.md

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use super::market::MarketId;
use super::position::PositionId;

// =============================================================================
// NEWTYPE PATTERN - OrderId
// =============================================================================

/// Unique order identifier from Kalshi (newtype pattern)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub String);

impl OrderId {
    pub fn new(id: String) -> Self {
        OrderId(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// =============================================================================
// ENUMS
// =============================================================================

/// Which side of the market (Yes or No)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Yes,  // Betting the event will happen
    No,   // Betting the event won't happen
}

/// Order action (Buy or Sell)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderAction {
    Buy,   // Open a position (buy contracts)
    Sell,  // Close a position (sell contracts)
}

/// Order type (Market or Limit)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,  // Immediate execution at current price
    Limit,   // Execute only at specified price or better
}

/// Order status lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,      // Order created, not yet submitted
    Resting,      // Limit order on the book, waiting for fill
    PartialFill,  // Partially filled
    Filled,       // Completely filled
    Cancelled,    // User cancelled before fill
    Rejected,     // Exchange rejected the order
}

// =============================================================================
// MAIN STRUCT
// =============================================================================

/// Represents a Kalshi order (buy or sell)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    // Identification
    pub id: OrderId,
    pub market_id: MarketId,
    pub position_id: Option<PositionId>,  // None for entry orders, Some for exit orders

    // Order details
    pub side: OrderSide,
    pub action: OrderAction,
    pub order_type: OrderType,

    // Pricing
    pub limit_price: Option<Decimal>,  // Required for Limit orders, None for Market
    pub quantity: u64,

    // State
    pub status: OrderStatus,
    pub filled_quantity: u64,
    pub average_fill_price: Option<Decimal>,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// Create a new order
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: OrderId,
        market_id: MarketId,
        position_id: Option<PositionId>,
        side: OrderSide,
        action: OrderAction,
        order_type: OrderType,
        limit_price: Option<Decimal>,
        quantity: u64,
    ) -> Self {
        let now = Utc::now();

        Order {
            id,
            market_id,
            position_id,
            side,
            action,
            order_type,
            limit_price,
            quantity,
            status: OrderStatus::Pending,
            filled_quantity: 0,
            average_fill_price: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if order is completely filled
    pub fn is_filled(&self) -> bool {
        self.status == OrderStatus::Filled
    }

    /// Check if order is active (can still be filled)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Resting | OrderStatus::PartialFill
        )
    }

    /// Get fill percentage (0.0 to 1.0)
    pub fn fill_percentage(&self) -> Decimal {
        if self.quantity == 0 {
            return Decimal::ZERO;
        }

        Decimal::from(self.filled_quantity) / Decimal::from(self.quantity)
    }

    /// Update order status with fill information
    pub fn update_fill(&mut self, filled_qty: u64, fill_price: Decimal) {
        self.filled_quantity = filled_qty;

        // Update average fill price
        if self.average_fill_price.is_none() {
            self.average_fill_price = Some(fill_price);
        }

        // Update status based on fill quantity
        if self.filled_quantity >= self.quantity {
            self.status = OrderStatus::Filled;
        } else if self.filled_quantity > 0 {
            self.status = OrderStatus::PartialFill;
        }

        self.updated_at = Utc::now();
    }

    /// Mark order as cancelled
    pub fn mark_cancelled(&mut self) {
        self.status = OrderStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    /// Mark order as rejected
    pub fn mark_rejected(&mut self) {
        self.status = OrderStatus::Rejected;
        self.updated_at = Utc::now();
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_order() -> Order {
        Order::new(
            OrderId::new("order-123".to_string()),
            MarketId::new("TEST-001".to_string()),
            None,
            OrderSide::Yes,
            OrderAction::Buy,
            OrderType::Limit,
            Some(dec!(0.11)),
            100,
        )
    }

    #[test]
    fn test_order_creation() {
        let order = create_test_order();
        assert_eq!(order.quantity, 100);
        assert_eq!(order.filled_quantity, 0);
        assert_eq!(order.status, OrderStatus::Pending);
    }

    #[test]
    fn test_is_active() {
        let order = create_test_order();
        assert!(order.is_active());
        assert!(!order.is_filled());
    }

    #[test]
    fn test_update_fill_partial() {
        let mut order = create_test_order();
        order.update_fill(50, dec!(0.11));

        assert_eq!(order.filled_quantity, 50);
        assert_eq!(order.status, OrderStatus::PartialFill);
        assert_eq!(order.fill_percentage(), dec!(0.5));
        assert!(order.is_active());
    }

    #[test]
    fn test_update_fill_complete() {
        let mut order = create_test_order();
        order.update_fill(100, dec!(0.11));

        assert_eq!(order.filled_quantity, 100);
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.fill_percentage(), dec!(1.0));
        assert!(order.is_filled());
        assert!(!order.is_active());
    }

    #[test]
    fn test_mark_cancelled() {
        let mut order = create_test_order();
        order.mark_cancelled();

        assert_eq!(order.status, OrderStatus::Cancelled);
        assert!(!order.is_active());
    }

    #[test]
    fn test_mark_rejected() {
        let mut order = create_test_order();
        order.mark_rejected();

        assert_eq!(order.status, OrderStatus::Rejected);
        assert!(!order.is_active());
    }
}
