# CALCHAS TECHNICAL ARCHITECTURE
## Prediction Market Trading Bot - System Design

**Version:** 1.0
**Date:** December 2025
**Status:** Design Phase

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Architecture Principles](#2-architecture-principles)
3. [High-Level Architecture](#3-high-level-architecture)
4. [Core Data Models](#4-core-data-models)
5. [Module Structure](#5-module-structure)
6. [Component Details](#6-component-details)
7. [Concurrency Model](#7-concurrency-model)
8. [Data Flow](#8-data-flow)
9. [Error Handling Strategy](#9-error-handling-strategy)
10. [Configuration Management](#10-configuration-management)
11. [Database Schema](#11-database-schema)
12. [API Contracts](#12-api-contracts)
13. [Testing Strategy](#13-testing-strategy)
14. [Deployment Architecture](#14-deployment-architecture)
15. [Key Architectural Decisions](#15-key-architectural-decisions)

---

## 1. System Overview

### 1.1 Purpose
Calchas is an automated trading bot for prediction markets (Kalshi, Polymarket) that executes volatility-based strategies on sports events. It monitors markets in real-time, evaluates opportunities using JSON-defined strategies, and manages positions with automated exits.

### 1.2 Design Goals
- **Correctness First:** Never miss an exit, never lose track of positions
- **Real-time Responsiveness:** <500ms from price update to decision
- **Strategy Flexibility:** Hot-reload strategies without daemon restart
- **Observability:** Know exactly what the bot is doing at all times
- **Fail-Safe:** Graceful degradation when APIs are unreachable

### 1.3 Non-Goals (for MVP)
- ❌ High-frequency trading (millisecond-level latency)
- ❌ Complex ML models (keep strategies rule-based)
- ❌ Multi-user support (single trader only)
- ❌ Mobile app (web dashboard is enough)

---

## 2. Architecture Principles

### 2.1 From Harbinger's PRINCIPLES.md
- **No Mock Data:** Use real data or return "Not Implemented"
- **No Premature Abstractions:** Build real things first, extract patterns later
- **Simple Before Smart:** If-statements before ML models
- **Honest Code:** Name things what they actually are

### 2.2 Rust-Specific Principles
- **Zero-Copy Where Possible:** Use references instead of clones
- **Type Safety:** Use newtypes to prevent ID mix-ups (MarketId, PositionId)
- **Explicit Error Handling:** Every error path is handled, no panics in production
- **Fearless Concurrency:** Use Rust's type system to prevent data races

### 2.3 Domain Principles
- **Position Tracking is Critical:** If we lose track of an open position, we lose money
- **Idempotency:** Reprocessing the same market update should be safe
- **Audit Trail:** Every trade decision must be logged with reasoning

---

## 3. High-Level Architecture

### 3.1 System Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         CALCHAS DAEMON                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    RUNTIME SUPERVISOR                     │  │
│  │  - Spawns and manages all tasks                          │  │
│  │  - Handles graceful shutdown (Ctrl+C)                    │  │
│  │  - Reconnects on failures                                │  │
│  └──────────────────────────────────────────────────────────┘  │
│                          │                                      │
│                          ├───────────────┬───────────────┐      │
│                          │               │               │      │
│  ┌──────────────────┐  ┌─────────────┐  ┌────────────┐  ┌────┐│
│  │  Platform Layer  │  │  Strategy   │  │  Position  │  │Web ││
│  │                  │  │  Engine     │  │  Manager   │  │UI  ││
│  │  ┌────────────┐  │  │             │  │            │  │    ││
│  │  │  Kalshi    │  │  │             │  │            │  │    ││
│  │  │  Client    │  │  │             │  │            │  │    ││
│  │  └─────┬──────┘  │  │             │  │            │  │    ││
│  │        │         │  │             │  │            │  │    ││
│  │  REST  │  WS     │  │             │  │            │  │    ││
│  └────────┼─────────┘  └─────────────┘  └────────────┘  └────┘│
│           │                                                     │
└───────────┼─────────────────────────────────────────────────────┘
            │
            │ HTTP/WebSocket
            ▼
    ┌───────────────┐
    │  Kalshi API   │
    │               │
    │ REST + WS     │
    └───────────────┘
```

### 3.2 Communication Pattern

**Event-Driven Architecture with Message Passing:**

```
Market Update (WS) → Platform Layer → Broadcast Channel
                                             │
                    ┌────────────────────────┼────────────────────┐
                    ▼                        ▼                    ▼
              Strategy Engine         Position Manager      Web UI
              (evaluates rules)      (checks exits)     (displays live)
                    │
                    ▼
              Order Command → Platform Layer → Kalshi API
                                      │
                                      ▼
                              Position Manager
                              (tracks fill)
```

**Key Insight:** All components communicate via **channels**, not shared state. This prevents data races and makes the system easier to reason about.

---

## 4. Core Data Models

### 4.1 Market (from Kalshi API)

**Purpose:** Represents a prediction market event

```rust
/// Unique identifier for a market
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MarketId(String);

/// A prediction market (e.g., "NFL: Chiefs to score next")
#[derive(Debug, Clone)]
struct Market {
    id: MarketId,
    ticker: String,              // e.g., "INXDKNFL-24FEB11-T2.5"
    title: String,               // Human-readable description
    category: MarketCategory,    // Sports, Politics, Crypto, etc.
    sub_category: Option<String>, // e.g., "american_football"

    // Market state
    status: MarketStatus,        // Open, Closed, Settled

    // Current pricing
    yes_price: Decimal,          // Current YES price (0-100 cents)
    no_price: Decimal,           // Current NO price (0-100 cents)

    // Liquidity
    volume_usd: Decimal,         // Total $ traded
    open_interest: u64,          // Contracts outstanding

    // Timing
    event_time: Option<DateTime<Utc>>,  // When event occurs
    close_time: DateTime<Utc>,          // Market closes
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketCategory {
    Sports,
    Politics,
    Economics,
    Crypto,
    Entertainment,
    Weather,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarketStatus {
    PreLaunch,      // Not yet tradable
    Open,           // Currently tradable
    Closed,         // Event occurred, awaiting settlement
    Settled,        // Payouts distributed
    Finalized,      // Permanently closed
}
```

**Design Decisions:**
- **MarketId newtype:** Prevents accidentally using ticker string as ID
- **Decimal for prices:** No floating-point precision issues (use `rust_decimal` crate)
- **Enum for status:** Compile-time guarantees we handle all states

---

### 4.2 Strategy (from JSON files)

**Purpose:** Defines entry/exit rules for a trading strategy

```rust
/// Unique identifier for a strategy
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StrategyId(String);

/// A trading strategy loaded from JSON
#[derive(Debug, Clone, Deserialize)]
struct Strategy {
    #[serde(skip)]  // Generated from filename
    id: StrategyId,

    name: String,
    description: String,
    version: String,
    enabled: bool,  // Can be toggled without deleting file

    // Market filtering
    filters: StrategyFilters,

    // Position entry rules
    entry: EntryRules,

    // Position exit rules
    exit: ExitRules,

    // Risk management
    risk: RiskLimits,
}

#[derive(Debug, Clone, Deserialize)]
struct StrategyFilters {
    categories: Vec<MarketCategory>,
    platforms: Vec<Platform>,

    // Price constraints
    min_favorite_price: Option<Decimal>,  // Only if favorite >= this
    max_underdog_price: Option<Decimal>,  // Only if underdog <= this

    // Liquidity constraints
    min_liquidity_usd: Decimal,
    min_open_interest: Option<u64>,

    // Timing constraints
    game_status: Vec<GameStatus>,  // PreGame, Live, etc.
}

#[derive(Debug, Clone, Deserialize)]
struct EntryRules {
    side: EntrySide,           // Underdog, Favorite, Both
    amount_usd: Decimal,       // Size per position
    order_type: OrderType,     // Market, Limit

    // For limit orders
    limit_price_offset: Option<Decimal>,  // +/- cents from current
}

#[derive(Debug, Clone, Deserialize)]
enum EntrySide {
    UnderdogOnly,   // Buy cheap side only
    FavoriteOnly,   // Buy expensive side only
    Both,           // Volatility hedge strategy
}

#[derive(Debug, Clone, Deserialize)]
struct ExitRules {
    take_profit_pct: Decimal,      // Exit at +X% gain
    stop_loss_pct: Decimal,         // Exit at -X% loss
    trailing_stop_pct: Option<Decimal>,  // Trail by X% from peak
    max_hold_minutes: Option<u64>,  // Force exit after duration
}

#[derive(Debug, Clone, Deserialize)]
struct RiskLimits {
    max_concurrent_positions: usize,     // Max open positions
    max_daily_loss_usd: Decimal,         // Stop trading if hit
    cooldown_after_loss_minutes: u64,    // Wait after loss before next trade
}
```

**Design Decisions:**
- **Deserialize from JSON:** Strategies are config, not code
- **Optional fields:** Use `Option<T>` for truly optional constraints
- **Decimal for money:** Never use f64 for financial calculations
- **Enabled flag:** Disable without deleting (good for A/B testing)

---

### 4.3 Position

**Purpose:** Tracks an open trading position

```rust
/// Unique identifier for a position
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PositionId(Uuid);

/// An open position (bet on a market)
#[derive(Debug, Clone)]
struct Position {
    id: PositionId,
    market_id: MarketId,
    strategy_id: StrategyId,

    // Entry details
    side: OrderSide,           // Yes or No
    entry_price: Decimal,      // Price we entered at
    quantity: u64,             // Number of contracts
    entry_time: DateTime<Utc>,

    // Current state
    current_price: Decimal,    // Latest market price
    unrealized_pnl: Decimal,   // Current profit/loss
    peak_pnl: Decimal,         // Highest PnL reached (for trailing stop)

    // Exit tracking
    exit_target: ExitTarget,   // When to exit
    exit_order_id: Option<OrderId>,  // If exit order placed

    // Risk state
    status: PositionStatus,
}

#[derive(Debug, Clone)]
enum PositionStatus {
    Active,           // Position open, monitoring
    ExitPending,      // Exit order submitted, awaiting fill
    Closed,           // Position exited
    Error(String),    // Something went wrong (e.g., API down)
}

#[derive(Debug, Clone)]
struct ExitTarget {
    take_profit_price: Decimal,
    stop_loss_price: Decimal,
    trailing_stop_distance: Option<Decimal>,
    expiry_time: Option<DateTime<Utc>>,
}
```

**Design Decisions:**
- **PositionId = Uuid:** Globally unique, can't collide
- **Track peak PnL:** Required for trailing stops
- **ExitTarget struct:** All exit criteria in one place
- **Status enum:** Explicit state machine

---

### 4.4 Order

**Purpose:** Represents a Kalshi order (buy or sell)

```rust
/// Unique identifier for an order (from Kalshi)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OrderId(String);

/// An order to buy/sell contracts
#[derive(Debug, Clone)]
struct Order {
    id: OrderId,
    market_id: MarketId,
    position_id: Option<PositionId>,  // None for entry orders (position not created yet)

    // Order details
    side: OrderSide,       // Yes or No
    action: OrderAction,   // Buy or Sell
    order_type: OrderType,

    // Pricing
    price: Decimal,        // Limit price (or market)
    quantity: u64,         // Contracts to buy/sell

    // State
    status: OrderStatus,
    filled_quantity: u64,
    average_fill_price: Option<Decimal>,

    // Timing
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderSide {
    Yes,  // Buy YES contracts (think event will happen)
    No,   // Buy NO contracts (think event won't happen)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderAction {
    Buy,   // Open position
    Sell,  // Close position
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderType {
    Market,  // Execute immediately at current price
    Limit,   // Execute only at specified price or better
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderStatus {
    Pending,      // Order submitted, not yet on book
    Resting,      // Limit order on book, not filled
    PartialFill,  // Some contracts filled
    Filled,       // All contracts filled
    Cancelled,    // Order cancelled
    Rejected,     // Order rejected by exchange
}
```

**Design Decisions:**
- **OrderId from Kalshi:** We don't generate these, exchange does
- **Action vs Side:** Action = Buy/Sell, Side = Yes/No (two orthogonal concepts)
- **Track fill price:** Actual execution price may differ from limit

---

### 4.5 Trade (Historical Record)

**Purpose:** Immutable record of a completed trade (for analytics)

```rust
/// Unique identifier for a trade record
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TradeId(Uuid);

/// A completed trade (entry + exit)
#[derive(Debug, Clone)]
struct Trade {
    id: TradeId,
    position_id: PositionId,
    market_id: MarketId,
    strategy_id: StrategyId,

    // Entry
    entry_order_id: OrderId,
    entry_price: Decimal,
    entry_quantity: u64,
    entry_time: DateTime<Utc>,

    // Exit
    exit_order_id: OrderId,
    exit_price: Decimal,
    exit_quantity: u64,
    exit_time: DateTime<Utc>,
    exit_reason: ExitReason,

    // Performance
    gross_pnl: Decimal,        // Exit price - Entry price
    fees: Decimal,             // Kalshi fees
    net_pnl: Decimal,          // Gross - Fees
    return_pct: Decimal,       // (Net PnL / Entry Cost) * 100
    hold_duration: Duration,   // Time held

    // Metadata
    notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExitReason {
    TakeProfit,
    StopLoss,
    TrailingStop,
    MaxHoldTime,
    ManualExit,       // User intervention
    StrategyDisabled, // Strategy was turned off
    MarketClosed,     // Event occurred
}
```

**Design Decisions:**
- **Separate from Position:** Position is mutable, Trade is immutable history
- **Track exit reason:** Critical for strategy analysis
- **Store fees separately:** Need to know actual cost vs theoretical
- **Duration in trade:** Helps analyze holding time effectiveness

---

## 5. Module Structure

### 5.1 Crate Organization

```
calchas/
├── Cargo.toml
├── src/
│   ├── main.rs                    # Entry point, CLI arg parsing
│   ├── lib.rs                     # Exposes public API
│   │
│   ├── config/
│   │   ├── mod.rs                 # Config loading, validation
│   │   └── types.rs               # AppConfig struct
│   │
│   ├── models/
│   │   ├── mod.rs
│   │   ├── market.rs              # Market, MarketId, MarketCategory
│   │   ├── strategy.rs            # Strategy, StrategyFilters, EntryRules
│   │   ├── position.rs            # Position, PositionId, ExitTarget
│   │   ├── order.rs               # Order, OrderId, OrderStatus
│   │   └── trade.rs               # Trade, TradeId, ExitReason
│   │
│   ├── platforms/
│   │   ├── mod.rs                 # Exchange trait
│   │   ├── kalshi/
│   │   │   ├── mod.rs
│   │   │   ├── client.rs          # KalshiClient (REST API)
│   │   │   ├── websocket.rs       # WebSocket price feed
│   │   │   ├── types.rs           # Kalshi-specific API types
│   │   │   └── error.rs           # KalshiError
│   │   └── polymarket/            # (Future: v1.5)
│   │       └── mod.rs
│   │
│   ├── strategy/
│   │   ├── mod.rs
│   │   ├── loader.rs              # Load strategies from JSON files
│   │   ├── engine.rs              # Evaluate markets against strategies
│   │   └── evaluator.rs           # Market filtering logic
│   │
│   ├── trading/
│   │   ├── mod.rs
│   │   ├── position_manager.rs    # Track positions, check exits
│   │   ├── order_executor.rs      # Place/cancel orders
│   │   └── risk_manager.rs        # Enforce risk limits
│   │
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── sqlite.rs              # SQLite database interface
│   │   ├── migrations.rs          # Schema migrations
│   │   └── queries.rs             # Prepared queries
│   │
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── supervisor.rs          # Spawns/manages all tasks
│   │   ├── shutdown.rs            # Graceful shutdown handler
│   │   └── channels.rs            # Channel type definitions
│   │
│   ├── web/
│   │   ├── mod.rs
│   │   ├── server.rs              # Axum HTTP server
│   │   ├── handlers.rs            # REST endpoints
│   │   ├── websocket.rs           # WebSocket for live updates
│   │   └── state.rs               # Shared web state
│   │
│   └── utils/
│       ├── mod.rs
│       ├── logging.rs             # Tracing setup
│       ├── time.rs                # Time utilities
│       └── decimal.rs             # Decimal helpers
│
├── frontend/                       # React UI (separate build)
│   ├── package.json
│   ├── src/
│   │   ├── App.tsx
│   │   ├── components/
│   │   └── api/                   # API client for Calchas backend
│   └── dist/                      # Built static files
│
├── strategies/                     # Strategy JSON files
│   ├── momentum_scalp.json
│   ├── volatility_hedge.json
│   └── examples/
│
├── config/
│   ├── default.toml               # Default config
│   └── production.toml            # Production overrides
│
├── migrations/                     # SQL migrations
│   ├── 001_initial_schema.sql
│   └── 002_add_trades_table.sql
│
└── tests/
    ├── integration/
    │   ├── kalshi_client_test.rs
    │   └── strategy_engine_test.rs
    └── fixtures/
        ├── mock_markets.json
        └── mock_strategies.json
```

### 5.2 Dependency Graph

**Principle:** Dependencies flow downward, no circular deps

```
main.rs
  │
  ├─→ config
  ├─→ runtime::supervisor
  └─→ web::server
        │
        ├─→ platforms::kalshi
        │      │
        │      └─→ models (Market, Order)
        │
        ├─→ strategy::engine
        │      │
        │      ├─→ strategy::loader
        │      └─→ models (Strategy, Market)
        │
        └─→ trading::position_manager
               │
               ├─→ trading::order_executor
               │      │
               │      └─→ platforms::kalshi
               │
               └─→ models (Position, Order, Trade)
```

**Key Insight:** `models` is the foundation - all other modules depend on it, but it depends on nothing else.

---

## 6. Component Details

### 6.1 Platform Layer (platforms::kalshi)

**Responsibility:** Communicate with Kalshi API (REST + WebSocket)

#### 6.1.1 KalshiClient (REST API)

```rust
pub struct KalshiClient {
    http_client: reqwest::Client,
    base_url: String,
    auth_token: RwLock<Option<String>>,  // Cached auth token
}

impl KalshiClient {
    /// Create new client
    pub async fn new(email: &str, password: &str, use_demo: bool) -> Result<Self>;

    /// Authentication
    pub async fn login(&self) -> Result<String>;
    pub async fn logout(&self) -> Result<()>;

    /// Market data
    pub async fn get_markets(&self, filters: MarketFilters) -> Result<Vec<Market>>;
    pub async fn get_market(&self, market_id: &MarketId) -> Result<Market>;

    /// Trading
    pub async fn place_order(&self, order: NewOrder) -> Result<Order>;
    pub async fn cancel_order(&self, order_id: &OrderId) -> Result<()>;
    pub async fn get_order(&self, order_id: &OrderId) -> Result<Order>;

    /// Portfolio
    pub async fn get_positions(&self) -> Result<Vec<Position>>;
    pub async fn get_balance(&self) -> Result<Balance>;
}
```

**Design Decisions:**
- **RwLock for token:** Multiple read-only requests can proceed, write locks when refreshing
- **Async methods:** All network I/O is async (Tokio)
- **Result return types:** Every call can fail (network, auth, rate limit)

#### 6.1.2 KalshiWebSocket

```rust
pub struct KalshiWebSocket {
    ws_stream: WebSocketStream<...>,
    subscriptions: HashSet<MarketId>,
}

impl KalshiWebSocket {
    /// Connect to WebSocket
    pub async fn connect(auth_token: &str) -> Result<Self>;

    /// Subscribe to market updates
    pub async fn subscribe(&mut self, market_ids: &[MarketId]) -> Result<()>;

    /// Unsubscribe from markets
    pub async fn unsubscribe(&mut self, market_ids: &[MarketId]) -> Result<()>;

    /// Receive next price update (blocking)
    pub async fn next_update(&mut self) -> Result<PriceUpdate>;
}

#[derive(Debug, Clone)]
pub struct PriceUpdate {
    pub market_id: MarketId,
    pub yes_price: Decimal,
    pub no_price: Decimal,
    pub timestamp: DateTime<Utc>,
}
```

**Design Decisions:**
- **Stream-based:** WebSocket is naturally a stream of updates
- **Backpressure:** If we can't keep up, WebSocket will buffer (handle reconnect if overload)
- **Track subscriptions:** Know what we're listening to

---

### 6.2 Strategy Engine (strategy::engine)

**Responsibility:** Evaluate markets against strategy rules, decide entries

```rust
pub struct StrategyEngine {
    strategies: Arc<RwLock<HashMap<StrategyId, Strategy>>>,
}

impl StrategyEngine {
    /// Create engine with loaded strategies
    pub fn new(strategies: Vec<Strategy>) -> Self;

    /// Reload strategies from disk (hot reload)
    pub async fn reload_strategies(&self, path: &Path) -> Result<()>;

    /// Evaluate a market against all active strategies
    /// Returns matching strategies and their entry signals
    pub fn evaluate(&self, market: &Market) -> Vec<EntrySignal>;

    /// Check if a specific strategy matches this market
    fn matches_filters(strategy: &Strategy, market: &Market) -> bool;
}

#[derive(Debug, Clone)]
pub struct EntrySignal {
    pub strategy_id: StrategyId,
    pub market_id: MarketId,
    pub side: OrderSide,
    pub amount_usd: Decimal,
    pub order_type: OrderType,
    pub reasoning: String,  // For logging why we entered
}
```

**Design Decisions:**
- **Shared strategies:** Multiple tasks can read strategies concurrently (RwLock)
- **Stateless evaluation:** Engine doesn't track positions, just evaluates rules
- **Returns signals:** Doesn't execute orders itself (separation of concerns)

---

### 6.3 Position Manager (trading::position_manager)

**Responsibility:** Track open positions, monitor for exit conditions

```rust
pub struct PositionManager {
    positions: Arc<RwLock<HashMap<PositionId, Position>>>,
    db: Arc<SqliteDatabase>,
    order_executor: Arc<OrderExecutor>,
}

impl PositionManager {
    /// Create new position from filled entry order
    pub async fn open_position(
        &self,
        order: &Order,
        strategy_id: StrategyId,
        exit_rules: ExitRules,
    ) -> Result<PositionId>;

    /// Update position with new market price
    pub async fn update_price(
        &self,
        position_id: &PositionId,
        new_price: Decimal,
    ) -> Result<()>;

    /// Check if any positions should exit
    /// Returns positions that triggered exit
    pub async fn check_exits(&self) -> Result<Vec<PositionId>>;

    /// Close a position (place exit order)
    pub async fn close_position(
        &self,
        position_id: &PositionId,
        reason: ExitReason,
    ) -> Result<OrderId>;

    /// Get all active positions
    pub async fn get_active_positions(&self) -> Vec<Position>;
}
```

**Design Decisions:**
- **Shared state:** Positions are shared across tasks (need RwLock)
- **Database-backed:** Positions persisted to SQLite (survive restarts)
- **Exit checking is polled:** Check every N seconds if exit conditions met
- **OrderExecutor integration:** Position manager doesn't call Kalshi directly

---

### 6.4 Order Executor (trading::order_executor)

**Responsibility:** Place orders, track fills, handle retries

```rust
pub struct OrderExecutor {
    kalshi: Arc<KalshiClient>,
    db: Arc<SqliteDatabase>,
}

impl OrderExecutor {
    /// Submit an entry order (open position)
    pub async fn execute_entry(
        &self,
        signal: &EntrySignal,
    ) -> Result<OrderId>;

    /// Submit an exit order (close position)
    pub async fn execute_exit(
        &self,
        position: &Position,
        reason: ExitReason,
    ) -> Result<OrderId>;

    /// Poll order status until filled or cancelled
    pub async fn wait_for_fill(
        &self,
        order_id: &OrderId,
        timeout: Duration,
    ) -> Result<Order>;

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &OrderId) -> Result<()>;
}
```

**Design Decisions:**
- **Retry logic:** Network failures shouldn't kill orders (exponential backoff)
- **Timeout on fills:** Don't wait forever for limit orders
- **Database logging:** Every order attempt logged for audit trail

---

### 6.5 Risk Manager (trading::risk_manager)

**Responsibility:** Enforce risk limits, prevent over-trading

```rust
pub struct RiskManager {
    daily_stats: Arc<RwLock<DailyStats>>,
    db: Arc<SqliteDatabase>,
}

impl RiskManager {
    /// Check if a new position would violate risk limits
    pub async fn check_new_position(
        &self,
        strategy: &Strategy,
        amount_usd: Decimal,
    ) -> Result<RiskDecision>;

    /// Record a trade (updates daily stats)
    pub async fn record_trade(&self, trade: &Trade) -> Result<()>;

    /// Reset daily stats (called at midnight)
    pub async fn reset_daily_stats(&self) -> Result<()>;
}

#[derive(Debug)]
pub enum RiskDecision {
    Approved,
    Rejected(RejectionReason),
}

#[derive(Debug)]
pub enum RejectionReason {
    MaxConcurrentPositions,
    DailyLossLimitReached,
    InCooldownPeriod,
    InsufficientBalance,
}

#[derive(Debug, Default)]
struct DailyStats {
    trades_today: u32,
    net_pnl_today: Decimal,
    active_positions: usize,
    last_loss_time: Option<DateTime<Utc>>,
}
```

**Design Decisions:**
- **Explicit risk checks:** Every position request goes through risk manager
- **Daily stats in memory:** Fast access, backed by database
- **Rejection reasons:** Clear why trade was blocked (for logging)

---

## 7. Concurrency Model

### 7.1 Task Architecture

**Calchas runs as a multi-task async daemon:**

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load configuration
    let config = Config::load("config/default.toml")?;

    // 2. Initialize shared state
    let kalshi = Arc::new(KalshiClient::new(...).await?);
    let strategies = Arc::new(RwLock::new(load_strategies("strategies/")?));
    let positions = Arc::new(RwLock::new(HashMap::new()));
    let db = Arc::new(SqliteDatabase::connect(...)?);

    // 3. Create channels
    let (price_tx, price_rx) = broadcast::channel(1000);  // Price updates
    let (signal_tx, signal_rx) = mpsc::channel(100);      // Entry signals
    let (exit_tx, exit_rx) = mpsc::channel(100);          // Exit commands

    // 4. Spawn tasks
    let ws_task = tokio::spawn(websocket_task(kalshi.clone(), price_tx));
    let strategy_task = tokio::spawn(strategy_evaluation_task(
        price_rx.resubscribe(),
        strategies.clone(),
        signal_tx,
    ));
    let position_task = tokio::spawn(position_monitoring_task(
        price_rx.resubscribe(),
        positions.clone(),
        exit_tx,
    ));
    let executor_task = tokio::spawn(order_execution_task(
        signal_rx,
        exit_rx,
        kalshi.clone(),
        positions.clone(),
    ));
    let web_task = tokio::spawn(web_server_task(...));

    // 5. Wait for shutdown signal (Ctrl+C)
    tokio::signal::ctrl_c().await?;

    // 6. Graceful shutdown
    shutdown_all_tasks(vec![ws_task, strategy_task, position_task, executor_task, web_task]).await?;

    Ok(())
}
```

### 7.2 Task Breakdown

#### Task 1: WebSocket Listener
**Responsibility:** Receive price updates from Kalshi, broadcast to other tasks

```rust
async fn websocket_task(
    kalshi: Arc<KalshiClient>,
    price_tx: broadcast::Sender<PriceUpdate>,
) -> Result<()> {
    let mut ws = KalshiWebSocket::connect(...).await?;

    loop {
        match ws.next_update().await {
            Ok(update) => {
                // Broadcast to all subscribers
                let _ = price_tx.send(update);
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                // Reconnect logic
                tokio::time::sleep(Duration::from_secs(5)).await;
                ws = KalshiWebSocket::connect(...).await?;
            }
        }
    }
}
```

**Key Points:**
- **Broadcast channel:** Multiple tasks can receive same price update
- **Auto-reconnect:** Network failures don't crash daemon
- **No shared state:** Just forwards messages

#### Task 2: Strategy Evaluator
**Responsibility:** Evaluate price updates, generate entry signals

```rust
async fn strategy_evaluation_task(
    mut price_rx: broadcast::Receiver<PriceUpdate>,
    strategies: Arc<RwLock<HashMap<StrategyId, Strategy>>>,
    signal_tx: mpsc::Sender<EntrySignal>,
) -> Result<()> {
    while let Ok(update) = price_rx.recv().await {
        // Get current market data
        let market = fetch_market_details(&update.market_id).await?;

        // Evaluate against all strategies
        let strategies = strategies.read().await;
        for strategy in strategies.values() {
            if matches_strategy(strategy, &market) {
                let signal = EntrySignal {
                    strategy_id: strategy.id.clone(),
                    market_id: market.id.clone(),
                    // ... signal details
                };

                signal_tx.send(signal).await?;
            }
        }
    }

    Ok(())
}
```

**Key Points:**
- **Read-only access to strategies:** Uses RwLock::read() (non-blocking)
- **Sends signals, doesn't execute:** Separation of concerns
- **Stateless:** Doesn't track what it's already signaled (executor handles dedup)

#### Task 3: Position Monitor
**Responsibility:** Check open positions, trigger exits

```rust
async fn position_monitoring_task(
    mut price_rx: broadcast::Receiver<PriceUpdate>,
    positions: Arc<RwLock<HashMap<PositionId, Position>>>,
    exit_tx: mpsc::Sender<ExitCommand>,
) -> Result<()> {
    // Also check on interval (in case no price updates)
    let mut interval = tokio::time::interval(Duration::from_secs(10));

    loop {
        tokio::select! {
            // New price update
            Ok(update) = price_rx.recv() => {
                check_exits_for_market(&update.market_id, &positions, &exit_tx).await?;
            }

            // Periodic check (every 10s)
            _ = interval.tick() => {
                check_all_exits(&positions, &exit_tx).await?;
            }
        }
    }
}

async fn check_exits_for_market(
    market_id: &MarketId,
    positions: &Arc<RwLock<HashMap<PositionId, Position>>>,
    exit_tx: &mpsc::Sender<ExitCommand>,
) -> Result<()> {
    let positions = positions.read().await;

    for position in positions.values() {
        if position.market_id == *market_id {
            if let Some(reason) = should_exit(position) {
                exit_tx.send(ExitCommand {
                    position_id: position.id.clone(),
                    reason,
                }).await?;
            }
        }
    }

    Ok(())
}
```

**Key Points:**
- **Dual triggers:** Price updates OR periodic timer (don't miss exits)
- **Read-only position access:** Fast check without blocking
- **Sends exit commands:** Doesn't execute orders directly

#### Task 4: Order Executor
**Responsibility:** Execute entry/exit orders

```rust
async fn order_execution_task(
    mut signal_rx: mpsc::Receiver<EntrySignal>,
    mut exit_rx: mpsc::Receiver<ExitCommand>,
    kalshi: Arc<KalshiClient>,
    positions: Arc<RwLock<HashMap<PositionId, Position>>>,
) -> Result<()> {
    loop {
        tokio::select! {
            // New entry signal
            Some(signal) = signal_rx.recv() => {
                // Check risk limits
                if risk_manager.approve(&signal).await? {
                    // Place order
                    let order = kalshi.place_order(...).await?;

                    // Wait for fill
                    let filled_order = wait_for_fill(&order.id).await?;

                    // Create position
                    let mut positions = positions.write().await;
                    let position = Position::from_order(filled_order);
                    positions.insert(position.id.clone(), position);
                }
            }

            // Exit command
            Some(exit_cmd) = exit_rx.recv() => {
                let positions = positions.read().await;
                if let Some(position) = positions.get(&exit_cmd.position_id) {
                    // Place exit order
                    let order = kalshi.place_order(...).await?;

                    // Update position status
                    drop(positions);  // Release read lock
                    let mut positions = positions.write().await;
                    positions.get_mut(&exit_cmd.position_id)
                        .unwrap()
                        .status = PositionStatus::ExitPending;
                }
            }
        }
    }
}
```

**Key Points:**
- **Single executor:** Only one task places orders (no race conditions)
- **Write lock for position updates:** Mutates shared state safely
- **Sequential processing:** Processes one signal at a time (could parallelize later)

---

### 7.3 Channel Types & Usage

| Channel | Type | Purpose | Senders | Receivers |
|---------|------|---------|---------|-----------|
| `price_updates` | `broadcast` | Price updates from WebSocket | 1 (WebSocket task) | 2 (Strategy, Position) |
| `entry_signals` | `mpsc` | Entry signals from strategy engine | 1 (Strategy task) | 1 (Executor task) |
| `exit_commands` | `mpsc` | Exit commands from position monitor | 1 (Position task) | 1 (Executor task) |
| `shutdown` | `broadcast` | Shutdown signal | 1 (Supervisor) | All tasks |

**Design Decisions:**
- **Broadcast for prices:** Multiple consumers need same data
- **MPSC for commands:** Single executor ensures sequential processing
- **Bounded channels:** Backpressure if consumer can't keep up (prevents memory leak)

---

## 8. Data Flow

### 8.1 Entry Flow (Opening a Position)

```
1. WebSocket receives price update for Market X
        │
        ▼
2. Broadcast to price_updates channel
        │
        ├────────────────────────┐
        ▼                        ▼
3. Strategy Task         Position Task (ignores, X not in positions)
   - Fetches Market X details
   - Evaluates against all strategies
   - Strategy "momentum_scalp" matches!
        │
        ▼
4. Send EntrySignal to entry_signals channel
        │
        ▼
5. Executor Task receives signal
   - Check risk limits (RiskManager)
   - Risk approved!
        │
        ▼
6. Place order via KalshiClient
        │
        ▼
7. Kalshi API confirms order
        │
        ▼
8. Poll order status until filled
        │
        ▼
9. Order filled! Create Position
   - Calculate exit targets (take profit, stop loss)
   - Insert into positions HashMap
   - Persist to SQLite
        │
        ▼
10. Position now tracked by Position Monitor
```

### 8.2 Exit Flow (Closing a Position)

```
1. WebSocket receives price update for Market Y
        │
        ▼
2. Broadcast to price_updates channel
        │
        ├────────────────────────┐
        ▼                        ▼
   Strategy Task          Position Task
   (ignores, no match)    - Position Z is in Market Y
                          - Update Z's current price
                          - Check exit conditions:
                            * Current PnL = +55%
                            * Take profit = +50%
                            * TRIGGER!
        │
        ▼
3. Send ExitCommand to exit_commands channel
        │
        ▼
4. Executor Task receives command
   - Lookup Position Z
   - Place exit order (sell contracts)
        │
        ▼
5. Kalshi API confirms order
        │
        ▼
6. Poll order status until filled
        │
        ▼
7. Order filled! Close Position
   - Calculate final PnL
   - Create Trade record
   - Remove from positions HashMap
   - Persist Trade to SQLite
        │
        ▼
8. Trade complete, logged for analytics
```

---

## 9. Error Handling Strategy

### 9.1 Error Type Hierarchy

```rust
/// Top-level application error
#[derive(Debug, thiserror::Error)]
pub enum CalchasError {
    #[error("Kalshi API error: {0}")]
    Kalshi(#[from] KalshiError),

    #[error("Strategy error: {0}")]
    Strategy(#[from] StrategyError),

    #[error("Database error: {0}")]
    Database(#[from] SqliteError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Position not found: {0}")]
    PositionNotFound(PositionId),

    #[error("Order failed: {0}")]
    OrderFailed(String),
}

/// Kalshi-specific errors
#[derive(Debug, thiserror::Error)]
pub enum KalshiError {
    #[error("Authentication failed")]
    AuthFailed,

    #[error("Rate limited, retry after {0}s")]
    RateLimited(u64),

    #[error("Market not found: {0}")]
    MarketNotFound(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("WebSocket disconnected")]
    WebSocketDisconnected,
}

/// Strategy-related errors
#[derive(Debug, thiserror::Error)]
pub enum StrategyError {
    #[error("Invalid strategy file: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("Strategy validation failed: {0}")]
    ValidationFailed(String),

    #[error("Strategy not found: {0}")]
    NotFound(String),
}
```

### 9.2 Error Handling Patterns

#### Pattern 1: Retry with Backoff (Network Errors)

```rust
async fn fetch_market_with_retry(
    client: &KalshiClient,
    market_id: &MarketId,
) -> Result<Market> {
    let mut retries = 0;
    let max_retries = 3;

    loop {
        match client.get_market(market_id).await {
            Ok(market) => return Ok(market),
            Err(KalshiError::RateLimited(seconds)) => {
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                retries += 1;
            }
            Err(KalshiError::Network(e)) if retries < max_retries => {
                let backoff = Duration::from_secs(2u64.pow(retries));
                warn!("Network error, retrying in {:?}: {}", backoff, e);
                tokio::time::sleep(backoff).await;
                retries += 1;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

#### Pattern 2: Graceful Degradation (WebSocket Disconnect)

```rust
async fn websocket_task_with_reconnect(
    kalshi: Arc<KalshiClient>,
    price_tx: broadcast::Sender<PriceUpdate>,
) -> Result<()> {
    loop {
        match run_websocket_loop(&kalshi, &price_tx).await {
            Ok(_) => {
                // WebSocket closed cleanly (shutdown)
                break;
            }
            Err(KalshiError::WebSocketDisconnected) => {
                error!("WebSocket disconnected, reconnecting in 5s...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                // Loop continues, reconnects
            }
            Err(e) => {
                error!("Fatal WebSocket error: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}
```

#### Pattern 3: Log and Continue (Non-Critical Failures)

```rust
async fn check_exits(&self) -> Result<()> {
    let positions = self.positions.read().await;

    for position in positions.values() {
        // If one position check fails, continue with others
        if let Err(e) = self.check_single_exit(position).await {
            error!(
                position_id = %position.id,
                error = %e,
                "Failed to check exit for position, will retry next cycle"
            );
            // Don't propagate error, continue checking other positions
        }
    }

    Ok(())
}
```

### 9.3 Critical vs Non-Critical Errors

**Critical (Crash Daemon):**
- Database corruption
- Invalid configuration at startup
- Out of memory

**Non-Critical (Log and Continue):**
- Single market fetch fails
- WebSocket disconnects (reconnect)
- Single position check fails
- Rate limited (wait and retry)

---

## 10. Configuration Management

### 10.1 Config File Structure (TOML)

```toml
# config/default.toml

[kalshi]
email = "your-email@example.com"
password = "your-password"
use_demo = true  # true = demo API, false = production
websocket_url = "wss://demo-api.kalshi.co/trade-api/ws/v2"
api_base_url = "https://demo-api.kalshi.co/trade-api/v2"

[runtime]
strategy_dir = "strategies/"
reload_strategies_interval_secs = 60  # Hot reload every minute
position_check_interval_secs = 10     # Check exits every 10s

[database]
path = "data/calchas.db"
max_connections = 5

[web]
host = "127.0.0.1"
port = 8420
serve_frontend = true
frontend_dir = "frontend/dist"

[logging]
level = "info"  # trace, debug, info, warn, error
format = "json"  # json or pretty
log_file = "logs/calchas.log"

[risk]
# Global risk limits (can be overridden per strategy)
max_total_positions = 10
max_total_exposure_usd = 1000.00
```

### 10.2 Config Struct

```rust
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub kalshi: KalshiConfig,
    pub runtime: RuntimeConfig,
    pub database: DatabaseConfig,
    pub web: WebConfig,
    pub logging: LoggingConfig,
    pub risk: RiskConfig,
}

impl AppConfig {
    /// Load from file, merge with env vars
    pub fn load(path: &str) -> Result<Self> {
        let mut config = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("CALCHAS"))
            .build()?;

        config.try_deserialize()
    }
}
```

**Design Decisions:**
- **TOML format:** Human-readable, supports comments
- **Environment variable overrides:** `CALCHAS_KALSHI__EMAIL=...` overrides config file
- **Sensitive data:** Passwords should come from env vars, not checked into git
- **Validation on load:** Fail fast if config is invalid

---

## 11. Database Schema

### 11.1 SQLite Tables

```sql
-- Markets (cached from Kalshi)
CREATE TABLE markets (
    id TEXT PRIMARY KEY,
    ticker TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    category TEXT NOT NULL,
    sub_category TEXT,
    status TEXT NOT NULL,
    yes_price REAL NOT NULL,
    no_price REAL NOT NULL,
    volume_usd REAL NOT NULL,
    open_interest INTEGER NOT NULL,
    event_time TIMESTAMP,
    close_time TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Positions (currently open)
CREATE TABLE positions (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL,
    strategy_id TEXT NOT NULL,
    side TEXT NOT NULL,  -- 'Yes' or 'No'
    entry_price REAL NOT NULL,
    quantity INTEGER NOT NULL,
    entry_time TIMESTAMP NOT NULL,
    current_price REAL NOT NULL,
    unrealized_pnl REAL NOT NULL,
    peak_pnl REAL NOT NULL,
    status TEXT NOT NULL,  -- 'Active', 'ExitPending', 'Closed'
    take_profit_price REAL NOT NULL,
    stop_loss_price REAL NOT NULL,
    trailing_stop_distance REAL,
    expiry_time TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (market_id) REFERENCES markets(id)
);

-- Orders (all orders, historical)
CREATE TABLE orders (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL,
    position_id TEXT,
    side TEXT NOT NULL,
    action TEXT NOT NULL,  -- 'Buy' or 'Sell'
    order_type TEXT NOT NULL,  -- 'Market' or 'Limit'
    price REAL NOT NULL,
    quantity INTEGER NOT NULL,
    status TEXT NOT NULL,
    filled_quantity INTEGER NOT NULL DEFAULT 0,
    average_fill_price REAL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (market_id) REFERENCES markets(id),
    FOREIGN KEY (position_id) REFERENCES positions(id)
);

-- Trades (completed positions, immutable)
CREATE TABLE trades (
    id TEXT PRIMARY KEY,
    position_id TEXT NOT NULL,
    market_id TEXT NOT NULL,
    strategy_id TEXT NOT NULL,
    entry_order_id TEXT NOT NULL,
    entry_price REAL NOT NULL,
    entry_quantity INTEGER NOT NULL,
    entry_time TIMESTAMP NOT NULL,
    exit_order_id TEXT NOT NULL,
    exit_price REAL NOT NULL,
    exit_quantity INTEGER NOT NULL,
    exit_time TIMESTAMP NOT NULL,
    exit_reason TEXT NOT NULL,
    gross_pnl REAL NOT NULL,
    fees REAL NOT NULL,
    net_pnl REAL NOT NULL,
    return_pct REAL NOT NULL,
    hold_duration_secs INTEGER NOT NULL,
    notes TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (market_id) REFERENCES markets(id),
    FOREIGN KEY (entry_order_id) REFERENCES orders(id),
    FOREIGN KEY (exit_order_id) REFERENCES orders(id)
);

-- Daily stats (for risk management)
CREATE TABLE daily_stats (
    date DATE PRIMARY KEY,
    trades_count INTEGER NOT NULL DEFAULT 0,
    net_pnl REAL NOT NULL DEFAULT 0.0,
    gross_pnl REAL NOT NULL DEFAULT 0.0,
    total_fees REAL NOT NULL DEFAULT 0.0,
    win_count INTEGER NOT NULL DEFAULT 0,
    loss_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_positions_status ON positions(status);
CREATE INDEX idx_positions_market ON positions(market_id);
CREATE INDEX idx_trades_strategy ON trades(strategy_id);
CREATE INDEX idx_trades_exit_time ON trades(exit_time);
```

### 11.2 Migration Strategy

**Use `refinery` crate for migrations:**

```rust
// migrations/V1__initial_schema.sql
-- (SQL from above)

// migrations/V2__add_trades_table.sql
ALTER TABLE positions ADD COLUMN notes TEXT;
```

**Run on startup:**

```rust
async fn run_migrations(db: &SqliteDatabase) -> Result<()> {
    embedded_migrations::run(&db.connection)?;
    Ok(())
}
```

---

## 12. API Contracts

### 12.1 Kalshi REST API (External)

**Base URL:** `https://demo-api.kalshi.co/trade-api/v2` (demo)

#### Authentication

```http
POST /login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "password123"
}

Response:
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "member_id": "abc123"
}
```

#### Get Markets

```http
GET /markets?limit=100&series_ticker=INXD&status=open
Authorization: Bearer <token>

Response:
{
  "markets": [
    {
      "ticker": "INXDKNFL-24FEB11-T2.5",
      "event_ticker": "INXD",
      "series_ticker": "INXDKNFL",
      "title": "Will the Dow Jones close above 42,500 on Feb 11?",
      "subtitle": "...",
      "yes_bid": 45,
      "yes_ask": 47,
      "no_bid": 52,
      "no_ask": 54,
      "last_price": 46,
      "volume": 12500,
      "open_interest": 5000,
      "status": "open",
      "close_time": "2024-02-11T21:00:00Z"
    }
  ],
  "cursor": "next_page_token"
}
```

#### Place Order

```http
POST /orders
Authorization: Bearer <token>
Content-Type: application/json

{
  "ticker": "INXDKNFL-24FEB11-T2.5",
  "side": "yes",
  "action": "buy",
  "type": "market",
  "count": 10
}

Response:
{
  "order": {
    "order_id": "ord_abc123",
    "status": "pending",
    "...": "..."
  }
}
```

### 12.2 Kalshi WebSocket API (External)

```javascript
// Connect
ws://demo-api.kalshi.co/trade-api/ws/v2

// Subscribe to markets
{
  "type": "subscribe",
  "market_tickers": ["INXDKNFL-24FEB11-T2.5"]
}

// Price update message
{
  "type": "market_update",
  "market_ticker": "INXDKNFL-24FEB11-T2.5",
  "yes_bid": 45,
  "yes_ask": 47,
  "timestamp": "2024-02-11T15:30:00Z"
}
```

### 12.3 Calchas REST API (Internal - for Web UI)

**Base URL:** `http://localhost:8420/api`

#### Get Active Positions

```http
GET /api/positions
Response:
{
  "positions": [
    {
      "id": "pos_abc123",
      "market_id": "mkt_xyz",
      "strategy_id": "momentum_scalp",
      "side": "yes",
      "entry_price": 15.00,
      "current_price": 22.00,
      "unrealized_pnl": 7.00,
      "return_pct": 46.67,
      "status": "active"
    }
  ]
}
```

#### Get Trades (Historical)

```http
GET /api/trades?limit=50&strategy_id=momentum_scalp
Response:
{
  "trades": [
    {
      "id": "trade_abc123",
      "market_title": "NFL: Chiefs to score next",
      "strategy_id": "momentum_scalp",
      "entry_price": 11.00,
      "exit_price": 24.00,
      "net_pnl": 12.50,
      "return_pct": 118.18,
      "exit_reason": "take_profit",
      "hold_duration_secs": 1800
    }
  ]
}
```

#### Get Strategies

```http
GET /api/strategies
Response:
{
  "strategies": [
    {
      "id": "momentum_scalp",
      "name": "Momentum Scalp",
      "enabled": true,
      "active_positions": 3,
      "total_trades_today": 5,
      "net_pnl_today": 42.50
    }
  ]
}
```

### 12.4 Calchas WebSocket API (Internal - for Web UI)

```javascript
// Connect
ws://localhost:8420/ws

// Subscribe to live updates
{
  "type": "subscribe",
  "channels": ["positions", "trades", "markets"]
}

// Position update
{
  "type": "position_update",
  "position": {
    "id": "pos_abc123",
    "current_price": 23.00,
    "unrealized_pnl": 8.00
  }
}

// New trade
{
  "type": "trade_closed",
  "trade": {
    "id": "trade_xyz",
    "net_pnl": 12.50,
    "return_pct": 118.18
  }
}
```

---

## 13. Testing Strategy

### 13.1 Test Pyramid

```
        ┌───────────────┐
        │  Integration  │  ← Test full flows (few, slow)
        │  Tests        │
        ├───────────────┤
        │               │
        │  Unit Tests   │  ← Test components (many, fast)
        │               │
        └───────────────┘
```

### 13.2 Unit Tests

**What to test:**
- Strategy filtering logic
- PnL calculation
- Exit condition checking
- Order price calculation

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_matches_underdog() {
        let strategy = Strategy {
            filters: StrategyFilters {
                max_underdog_price: Some(dec!(20.0)),
                ..Default::default()
            },
            ..Default::default()
        };

        let market = Market {
            yes_price: dec!(15.0),
            ..Default::default()
        };

        assert!(matches_strategy(&strategy, &market));
    }

    #[test]
    fn test_take_profit_triggered() {
        let position = Position {
            entry_price: dec!(10.0),
            current_price: dec!(16.0),
            exit_target: ExitTarget {
                take_profit_price: dec!(15.0),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(should_exit(&position), Some(ExitReason::TakeProfit));
    }
}
```

### 13.3 Integration Tests

**What to test:**
- Kalshi API client (with mock server)
- Full entry → exit flow (with fake data)
- Database persistence

```rust
// tests/integration/kalshi_client_test.rs

#[tokio::test]
async fn test_fetch_markets() {
    // Use wiremock to simulate Kalshi API
    let mock_server = wiremock::MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/markets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "markets": [...]
        })))
        .mount(&mock_server)
        .await;

    let client = KalshiClient::new_with_base_url(&mock_server.uri());
    let markets = client.get_markets(MarketFilters::default()).await.unwrap();

    assert_eq!(markets.len(), 1);
}
```

### 13.4 Simulation Mode (Paper Trading)

**Mock order execution:**

```rust
pub struct SimulatedKalshiClient {
    markets: Arc<RwLock<HashMap<MarketId, Market>>>,
    orders: Arc<RwLock<Vec<Order>>>,
}

impl SimulatedKalshiClient {
    /// Simulate order fills at current market price
    async fn place_order(&self, order: NewOrder) -> Result<Order> {
        let markets = self.markets.read().await;
        let market = markets.get(&order.market_id).unwrap();

        // Simulate immediate fill at market price
        let filled_order = Order {
            id: OrderId(Uuid::new_v4().to_string()),
            status: OrderStatus::Filled,
            filled_quantity: order.quantity,
            average_fill_price: Some(market.yes_price),
            ..Default::default()
        };

        self.orders.write().await.push(filled_order.clone());
        Ok(filled_order)
    }
}
```

**Usage:**

```bash
# Run in simulation mode (no real money)
calchas daemon --mode simulation --config config/default.toml
```

---

## 14. Deployment Architecture

### 14.1 Production Deployment

```
┌─────────────────────────────────────────┐
│  macOS / Linux Server                   │
├─────────────────────────────────────────┤
│                                         │
│  ┌───────────────────────────────────┐  │
│  │  systemd service                  │  │
│  │  (or launchd on macOS)            │  │
│  │                                   │  │
│  │  calchas daemon                   │  │
│  │    ├─ WebSocket to Kalshi         │  │
│  │    ├─ SQLite DB (data/calchas.db) │  │
│  │    └─ Web UI on :8420             │  │
│  └───────────────────────────────────┘  │
│                                         │
│  ┌───────────────────────────────────┐  │
│  │  Logs (rotated)                   │  │
│  │  logs/calchas.log                 │  │
│  └───────────────────────────────────┘  │
│                                         │
└─────────────────────────────────────────┘
```

### 14.2 systemd Service (Linux)

```ini
# /etc/systemd/system/calchas.service

[Unit]
Description=Calchas Prediction Market Trading Bot
After=network.target

[Service]
Type=simple
User=calchas
WorkingDirectory=/opt/calchas
ExecStart=/opt/calchas/calchas daemon --config /opt/calchas/config/production.toml
Restart=always
RestartSec=10
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
```

### 14.3 Monitoring & Alerting

**Use structured logging + external log aggregation:**

- Logs → File → Ship to Grafana Loki / CloudWatch
- Metrics → Expose Prometheus endpoint (`/metrics`)
- Alerts → Telegram/Discord on:
  - Position exit
  - Daily loss limit hit
  - WebSocket disconnected > 5 min
  - Database errors

---

## 15. Key Architectural Decisions

### 15.1 Decision Log

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| **Rust + Tokio** | Memory safety, async I/O, fast | Python (slower), Go (less type safety) |
| **Message passing (channels)** | Avoids shared mutable state | Shared Arc<RwLock> (more error-prone) |
| **SQLite (not Postgres)** | Simple, embedded, good enough for single user | Postgres (overkill), In-memory only (lose data on crash) |
| **JSON strategies (not code)** | Hot-reload without restart, non-programmers can edit | Rust code (requires recompile), DSL (added complexity) |
| **Broadcast channel for prices** | Multiple consumers need same data | Clone prices for each consumer (wasteful) |
| **Single executor task** | Sequential order processing prevents race conditions | Parallel executors (risk of double-entry) |
| **Decimal type for money** | No floating-point precision errors | f64 (precision issues), integers (awkward API) |
| **Newtype IDs (MarketId, etc.)** | Prevents mixing up IDs | Plain strings (error-prone) |
| **React frontend** | Reuse Harbinger components, real-time WebSocket | HTMX (simpler but less reusable) |
| **Axum web framework** | Modern, fast, good Tokio integration | Actix (more complex), Warp (less ergonomic) |

### 15.2 Trade-offs Accepted

**Correctness > Performance:**
- We use locks (RwLock) instead of lock-free structures → Simpler, safer, fast enough

**Simplicity > Flexibility:**
- Strategies are JSON, not Turing-complete DSL → Can't express complex logic, but easy to understand

**Single-User > Multi-Tenant:**
- No user accounts, auth, billing → Saves months of work, personal tool only

**SQLite > Distributed DB:**
- Can't scale to millions of users → Don't need to, only one trader

---

## 16. PRD Compliance Details

### 16.1 Strategy Type Implementation

The PRD defines 3 strategy types - here's how they map to JSON:

#### Strategy A: Momentum Scalp (Underdog Only)

```json
{
  "name": "nfl_underdog_scalp",
  "filters": {
    "max_underdog_price": 0.20,
    "min_favorite_price": 0.80
  },
  "entry": {
    "side": "underdog_only",
    "amount_usd": 10
  },
  "exit": {
    "take_profit_pct": 50,
    "stop_loss_pct": -60
  }
}
```

#### Strategy B: Volatility Hedge (Both Sides)

```json
{
  "name": "nhl_volatility_hedge",
  "entry": {
    "side": "both",  // Buy YES and NO
    "amount_usd": 10  // $10 per side ($20 total)
  },
  "exit": {
    "take_profit_pct": 15,  // Exit when combined position hits +15%
    "stop_loss_pct": -10
  }
}
```

**Implementation Note:** For `"side": "both"`, the Order Executor places two orders (YES and NO) and tracks them as a single logical position with combined P&L.

#### Strategy C: Hybrid (Conditional Hedge)

```json
{
  "name": "soccer_hybrid",
  "entry": {
    "side": "underdog_only",
    "amount_usd": 10
  },
  "exit": {
    "take_profit_pct": 50,
    "stop_loss_pct": -60
  },
  "risk": {
    "hedge_on_loss_pct": -30  // If down 30%, buy opposite side to hedge
  }
}
```

**Implementation Note:** Position Monitor checks `hedge_on_loss_pct`. If triggered, places opposite-side order to reduce downside risk.

---

### 16.2 Success Metrics Tracking

**From PRD Section 2:** Track simulation/live performance

#### Metrics Tracker Component

```rust
// Add to trading/metrics_tracker.rs

pub struct MetricsTracker {
    db: Arc<SqliteDatabase>,
}

impl MetricsTracker {
    /// Get simulation phase metrics
    pub async fn get_simulation_metrics(&self) -> SimulationMetrics {
        // Query daily_stats table
        SimulationMetrics {
            consecutive_profitable_days: self.calc_consecutive_profitable().await?,
            net_roi: self.calc_total_roi().await?,
            win_rate: self.calc_win_rate().await?,
            avg_profit_per_win: self.calc_avg_winning_trade().await?,
        }
    }

    /// Check if ready to exit simulation mode (go live)
    pub async fn check_exit_to_live_criteria(&self) -> ExitToLiveDecision {
        let metrics = self.get_simulation_metrics().await?;

        let criteria_met = vec![
            metrics.consecutive_profitable_days >= 7,
            metrics.net_roi > 0.0,
            self.max_single_day_loss().await? < 0.15,  // <15% loss any day
            self.strategy_behaves_as_expected().await?,  // Manual validation flag
        ];

        if criteria_met.iter().all(|&x| x) {
            ExitToLiveDecision::Approved
        } else {
            ExitToLiveDecision::NotReady {
                unmet_criteria: criteria_met.iter()
                    .enumerate()
                    .filter(|(_, &met)| !met)
                    .map(|(i, _)| i)
                    .collect(),
            }
        }
    }
}

#[derive(Debug)]
pub struct SimulationMetrics {
    pub consecutive_profitable_days: u32,
    pub net_roi: Decimal,
    pub win_rate: Decimal,  // 0-100%
    pub avg_profit_per_win: Decimal,  // %
}

#[derive(Debug)]
pub enum ExitToLiveDecision {
    Approved,
    NotReady { unmet_criteria: Vec<usize> },
}
```

**Add to daily_stats table:**

```sql
ALTER TABLE daily_stats ADD COLUMN is_profitable BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE daily_stats ADD COLUMN max_single_trade_loss_pct REAL;
```

---

### 16.3 CLI Command Specifications

**From PRD Section 10:** Exact command syntax

```rust
// src/main.rs

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "calchas")]
#[command(about = "Prediction market trading bot", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single strategy (one-off execution)
    Run {
        /// Path to strategy JSON file
        #[arg(short, long)]
        strategy: PathBuf,

        /// Dry-run mode (simulation, no real money)
        #[arg(long)]
        dry_run: bool,

        /// Max number of positions to open
        #[arg(long, default_value = "5")]
        max_positions: usize,
    },

    /// Start daemon (background service + web UI)
    Daemon {
        /// Configuration file path
        #[arg(short, long, default_value = "config/default.toml")]
        config: PathBuf,

        /// Web UI port
        #[arg(short, long, default_value = "8420")]
        port: u16,

        /// Directory containing strategy JSON files
        #[arg(long, default_value = "strategies/")]
        strategies: PathBuf,

        /// Mode: simulation or live
        #[arg(long, default_value = "simulation")]
        mode: TradingMode,
    },

    /// Check if simulation metrics meet exit-to-live criteria
    CheckSimulation {
        /// Database path
        #[arg(long, default_value = "data/calchas.db")]
        db: PathBuf,
    },

    /// Export trade history to CSV
    Export {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        start_date: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        end_date: Option<String>,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum TradingMode {
    Simulation,  // Paper trading (mock orders)
    Live,        // Real money
}
```

**Usage Examples:**

```bash
# One-off strategy run (dry-run)
calchas run --strategy strategies/momentum_scalp.json --dry-run

# Start daemon in simulation mode
calchas daemon --config config/default.toml --port 8420 --mode simulation

# Start daemon in LIVE mode (real money)
calchas daemon --config config/production.toml --mode live

# Check if ready to go live
calchas check-simulation --db data/calchas.db

# Export trades to CSV
calchas export --output trades.csv --start-date 2025-01-01
```

---

### 16.4 Simulation Mode Implementation

**From PRD Section 2.4:** Exit to live criteria

#### Simulation Validator Component

```rust
// Add to trading/simulation_validator.rs

pub struct SimulationValidator {
    metrics_tracker: Arc<MetricsTracker>,
    config: SimulationConfig,
}

#[derive(Debug, Clone)]
pub struct SimulationConfig {
    pub min_consecutive_profitable_days: u32,  // Default: 7
    pub min_net_roi: Decimal,                  // Default: 0.0
    pub max_single_day_loss_pct: Decimal,      // Default: 15.0
}

impl SimulationValidator {
    /// Generate exit-to-live report
    pub async fn generate_report(&self) -> SimulationReport {
        let metrics = self.metrics_tracker.get_simulation_metrics().await?;
        let decision = self.metrics_tracker.check_exit_to_live_criteria().await?;

        SimulationReport {
            metrics,
            decision,
            recommendation: self.generate_recommendation(&metrics, &decision),
        }
    }

    fn generate_recommendation(
        &self,
        metrics: &SimulationMetrics,
        decision: &ExitToLiveDecision,
    ) -> String {
        match decision {
            ExitToLiveDecision::Approved => {
                format!(
                    "✅ APPROVED FOR LIVE TRADING\n\
                     - {} consecutive profitable days (target: 7+)\n\
                     - {:.2}% net ROI\n\
                     - {:.2}% win rate\n\
                     You may proceed to live trading with confidence.",
                    metrics.consecutive_profitable_days,
                    metrics.net_roi * 100.0,
                    metrics.win_rate
                )
            }
            ExitToLiveDecision::NotReady { unmet_criteria } => {
                format!(
                    "⚠️  NOT READY FOR LIVE TRADING\n\
                     Continue simulation until all criteria are met.\n\
                     Unmet criteria: {:?}",
                    unmet_criteria
                )
            }
        }
    }
}

#[derive(Debug)]
pub struct SimulationReport {
    pub metrics: SimulationMetrics,
    pub decision: ExitToLiveDecision,
    pub recommendation: String,
}
```

**CLI Integration:**

```bash
$ calchas check-simulation

📊 SIMULATION PERFORMANCE REPORT
================================

Metrics:
  • Consecutive Profitable Days: 9 ✅ (target: 7+)
  • Net ROI: +12.3% ✅ (target: >0%)
  • Win Rate: 58% ✅ (target: 52%+)
  • Avg Profit per Win: 6.2% ✅ (target: 2%+)
  • Max Single-Day Loss: -8.5% ✅ (target: <15%)

✅ APPROVED FOR LIVE TRADING

You may proceed to live trading with:
  calchas daemon --mode live --config config/production.toml
```

---

### 16.5 Market Category Extensibility

**From PRD Section 4.3:** Support for multiple market categories

The architecture supports all categories via `MarketCategory` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum MarketCategory {
    Sports {
        sport: SportType,
        game_status: GameStatus,
    },
    Politics,
    Economics,
    Crypto,
    Entertainment,
    Weather,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SportType {
    AmericanFootball,
    Hockey,
    Soccer,
    Basketball,
    Baseball,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum GameStatus {
    PreGame,
    Live,
    Final,
    Postponed,
}
```

**Future Extension for Sport-Specific Dynamics:**

```rust
// Future: Add sport-specific strategy hints
pub trait SportStrategy {
    /// Sport-specific momentum indicators
    fn detect_momentum_shift(&self, market: &Market) -> MomentumSignal;
}

impl SportStrategy for FootballStrategy {
    fn detect_momentum_shift(&self, market: &Market) -> MomentumSignal {
        // Example: Detect offensive drive momentum
        // (Would require additional market metadata from Kalshi)
        MomentumSignal::Strong
    }
}
```

**Note:** MVP focuses on generic price-based strategies. Sport-specific logic (offensive drives, possession changes) is a v2.0+ feature.

---

### 16.6 Updated Module Structure

Add new modules to support PRD requirements:

```
src/
├── trading/
│   ├── metrics_tracker.rs        # NEW: Track simulation/live metrics
│   ├── simulation_validator.rs   # NEW: Check exit-to-live criteria
│   └── ...
```

---

## 17. Next Steps: Implementation Roadmap

### Phase 1: Foundation (Week 1-2)
- [ ] Project setup (Cargo.toml, dependencies)
- [ ] Define core data models (models/)
- [ ] Strategy JSON loader (strategy/loader.rs)
- [ ] **CLI command structure** (clap integration)
- [ ] Unit tests for models

### Phase 2: Kalshi Integration (Week 3-4)
- [ ] Kalshi REST client (platforms/kalshi/client.rs)
- [ ] Authentication flow
- [ ] Fetch markets endpoint
- [ ] Place order endpoint
- [ ] **Simulation mode** (SimulatedKalshiClient)
- [ ] Integration tests with mock server

### Phase 3: WebSocket & Real-Time (Week 5-6)
- [ ] Kalshi WebSocket client (platforms/kalshi/websocket.rs)
- [ ] Price update broadcasting
- [ ] Strategy evaluation task
- [ ] Position monitoring task
- [ ] **Support for 3 strategy types** (underdog, both sides, hybrid)

### Phase 4: Order Execution (Week 7-8)
- [ ] Order executor task
- [ ] Position manager (trading/position_manager.rs)
- [ ] Risk manager (trading/risk_manager.rs)
- [ ] SQLite integration (storage/sqlite.rs)
- [ ] **Metrics tracker** (daily stats, ROI, win rate)

### Phase 5: Web Dashboard (Week 9-10)
- [ ] Axum server (web/server.rs)
- [ ] REST API endpoints
- [ ] WebSocket for live updates
- [ ] React frontend (basic)
- [ ] **Performance charts** (ROI, drawdown, win rate)

### Phase 6: Production Ready (Week 11-12)
- [ ] Structured logging (tracing)
- [ ] **CLI commands fully implemented** (run, daemon, check-simulation, export)
- [ ] Configuration management
- [ ] Graceful shutdown
- [ ] Integration tests (full flow)
- [ ] **Simulation validator** (exit-to-live criteria)

---

**Version:** 1.1
**Status:** PRD-Compliant, Ready for Implementation
**Next:** Map syllabus to architecture, begin Phase 1
