# CALCHAS TECHNICAL ARCHITECTURE
## Prediction Market Trading Bot - System Design

**Version:** 1.0
**Date:** December 2025

**For build progress, see:** `PROJECT_STATUS.md`

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
16. [PRD Compliance Details](#16-prd-compliance-details)

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

**Key Fields:**
- **MarketId**: Unique identifier (newtype pattern)
- **Ticker**: Exchange symbol (e.g., "INXDKNFL-24FEB11-T2.5")
- **Title**: Human-readable description
- **Category & Sub-category**: Market classification (Sports, Politics, etc.)
- **Status**: Current state (PreLaunch, Open, Closed, Settled, Finalized)
- **Pricing**: Yes/No prices (0-100 cents, using Decimal type)
- **Liquidity**: Volume traded, open interest
- **Timing**: Event time, close time, creation/update timestamps

**Design Decisions:**
- **MarketId newtype:** Prevents accidentally using ticker string as ID
- **Decimal for prices:** No floating-point precision issues (use `rust_decimal` crate)
- **Enum for status:** Compile-time guarantees we handle all states

---

### 4.2 Strategy (from JSON files)

**Purpose:** Defines entry/exit rules for a trading strategy

**Key Components:**
- **StrategyId**: Unique identifier (newtype pattern)
- **Metadata**: Name, description, version, enabled flag
- **Filters**: Market selection criteria (categories, platforms, price ranges, liquidity, timing)
- **Entry Rules**: Position entry logic (side selection, position size, order type)
- **Exit Rules**: Position exit criteria (take profit, stop loss, trailing stop, time-based)
- **Risk Limits**: Risk management constraints (max positions, daily loss limits, cooldown periods)

**Entry Side Options:**
- **UnderdogOnly**: Buy cheap side only
- **FavoriteOnly**: Buy expensive side only
- **Both**: Volatility hedge strategy

**Design Decisions:**
- **Deserialize from JSON:** Strategies are config, not code
- **Optional fields:** Use `Option<T>` for truly optional constraints
- **Decimal for money:** Never use f64 for financial calculations
- **Enabled flag:** Disable without deleting (good for A/B testing)

---

### 4.3 Position

**Purpose:** Tracks an open trading position

**Key Fields:**
- **PositionId**: Unique identifier (UUID-based newtype)
- **References**: MarketId, StrategyId linkage
- **Entry Details**: Side (Yes/No), entry price, quantity, entry timestamp
- **Current State**: Current price, unrealized P&L, peak P&L (for trailing stops)
- **Exit Tracking**: Exit target criteria, exit order ID (if placed)
- **Status**: Active, ExitPending, Closed, or Error state

**Exit Target Components:**
- Take profit price threshold
- Stop loss price threshold
- Trailing stop distance (optional)
- Expiry time (optional)

**Design Decisions:**
- **PositionId = Uuid:** Globally unique, can't collide
- **Track peak PnL:** Required for trailing stops
- **ExitTarget struct:** All exit criteria in one place
- **Status enum:** Explicit state machine

---

### 4.4 Order

**Purpose:** Represents a Kalshi order (buy or sell)

**Key Fields:**
- **OrderId**: Unique identifier from Kalshi (newtype wrapping String)
- **References**: MarketId, optional PositionId (None for entry orders)
- **Order Details**: Side (Yes/No), Action (Buy/Sell), OrderType (Market/Limit)
- **Pricing**: Limit price, quantity
- **State**: Status, filled quantity, average fill price
- **Timestamps**: Created at, updated at

**Order Side:** Yes (event will happen) or No (event won't happen)
**Order Action:** Buy (open position) or Sell (close position)
**Order Type:** Market (immediate execution) or Limit (price-based execution)
**Order Status:** Pending, Resting, PartialFill, Filled, Cancelled, Rejected

**Design Decisions:**
- **OrderId from Kalshi:** We don't generate these, exchange does
- **Action vs Side:** Action = Buy/Sell, Side = Yes/No (two orthogonal concepts)
- **Track fill price:** Actual execution price may differ from limit

---

### 4.5 Trade (Historical Record)

**Purpose:** Immutable record of a completed trade (for analytics)

**Key Fields:**
- **TradeId**: Unique identifier (UUID-based newtype)
- **References**: PositionId, MarketId, StrategyId
- **Entry Details**: Order ID, price, quantity, timestamp
- **Exit Details**: Order ID, price, quantity, timestamp, reason
- **Performance Metrics**: Gross P&L, fees, net P&L, return percentage, hold duration
- **Metadata**: Optional notes field

**Exit Reasons:**
- TakeProfit, StopLoss, TrailingStop
- MaxHoldTime (time-based exit)
- ManualExit (user intervention)
- StrategyDisabled (strategy turned off)
- MarketClosed (event occurred)

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

**Key Components:**
- HTTP client (reqwest)
- Base URL configuration
- Cached auth token (RwLock for thread-safe access)

**Core Methods:**
- **Authentication**: login, logout
- **Market Data**: get_markets (with filters), get_market (by ID)
- **Trading**: place_order, cancel_order, get_order
- **Portfolio**: get_positions, get_balance

**Design Decisions:**
- **RwLock for token:** Multiple read-only requests can proceed, write locks when refreshing
- **Async methods:** All network I/O is async (Tokio)
- **Result return types:** Every call can fail (network, auth, rate limit)

#### 6.1.2 KalshiWebSocket

**Key Components:**
- WebSocket stream (tokio-tungstenite)
- Subscription tracking (HashSet of MarketIds)

**Core Methods:**
- **connect**: Establish WebSocket connection with auth token
- **subscribe**: Subscribe to market price updates
- **unsubscribe**: Unsubscribe from markets
- **next_update**: Receive next price update (blocking async)

**PriceUpdate Structure:**
- Market ID
- Yes price, No price (Decimal)
- Timestamp

**Design Decisions:**
- **Stream-based:** WebSocket is naturally a stream of updates
- **Backpressure:** If we can't keep up, WebSocket will buffer (handle reconnect if overload)
- **Track subscriptions:** Know what we're listening to

---

### 6.2 Strategy Engine (strategy::engine)

**Responsibility:** Evaluate markets against strategy rules, decide entries

**Key Components:**
- Strategy storage (Arc<RwLock<HashMap>> for concurrent read access)
- Strategy evaluator

**Core Methods:**
- **new**: Create engine with loaded strategies
- **reload_strategies**: Hot reload from disk without restart
- **evaluate**: Evaluate market against all active strategies, returns entry signals
- **matches_filters**: Check if strategy matches market criteria

**Entry Signal Structure:**
- Strategy ID, Market ID
- Order side, amount USD, order type
- Reasoning (for audit logging)

**Design Decisions:**
- **Shared strategies:** Multiple tasks can read strategies concurrently (RwLock)
- **Stateless evaluation:** Engine doesn't track positions, just evaluates rules
- **Returns signals:** Doesn't execute orders itself (separation of concerns)

---

### 6.3 Position Manager (trading::position_manager)

**Responsibility:** Track open positions, monitor for exit conditions

**Key Components:**
- Position storage (Arc<RwLock<HashMap>> for concurrent access)
- Database interface (SQLite)
- Order executor reference

**Core Methods:**
- **open_position**: Create position from filled entry order
- **update_price**: Update position with new market price
- **check_exits**: Scan positions for exit triggers, returns position IDs to exit
- **close_position**: Place exit order for position
- **get_active_positions**: Retrieve all open positions

**Design Decisions:**
- **Shared state:** Positions are shared across tasks (need RwLock)
- **Database-backed:** Positions persisted to SQLite (survive restarts)
- **Exit checking is polled:** Check every N seconds if exit conditions met
- **OrderExecutor integration:** Position manager doesn't call Kalshi directly

---

### 6.4 Order Executor (trading::order_executor)

**Responsibility:** Place orders, track fills, handle retries

**Key Components:**
- Kalshi client reference
- Database interface for audit logging

**Core Methods:**
- **execute_entry**: Submit entry order to open position
- **execute_exit**: Submit exit order to close position
- **wait_for_fill**: Poll order status until filled or cancelled (with timeout)
- **cancel_order**: Cancel pending order

**Design Decisions:**
- **Retry logic:** Network failures shouldn't kill orders (exponential backoff)
- **Timeout on fills:** Don't wait forever for limit orders
- **Database logging:** Every order attempt logged for audit trail

---

### 6.5 Risk Manager (trading::risk_manager)

**Responsibility:** Enforce risk limits, prevent over-trading

**Key Components:**
- Daily stats tracking (Arc<RwLock> for shared access)
- Database interface for persistence

**Core Methods:**
- **check_new_position**: Validate if new position would violate risk limits
- **record_trade**: Update daily stats after trade completion
- **reset_daily_stats**: Reset counters at midnight

**Risk Decision Types:**
- Approved
- Rejected (with specific reason: max positions, daily loss limit, cooldown period, insufficient balance)

**Daily Stats Tracked:**
- Trades count today
- Net P&L today
- Active positions count
- Last loss timestamp (for cooldown)

**Design Decisions:**
- **Explicit risk checks:** Every position request goes through risk manager
- **Daily stats in memory:** Fast access, backed by database
- **Rejection reasons:** Clear why trade was blocked (for logging)

---

## 7. Concurrency Model

### 7.1 Task Architecture

**Calchas runs as a multi-task async daemon**

**Main Function Flow:**
1. Load configuration from TOML file
2. Initialize shared state (Kalshi client, strategies, positions, database)
3. Create communication channels (broadcast for prices, mpsc for signals/commands)
4. Spawn concurrent tasks (WebSocket, Strategy, Position, Executor, Web)
5. Wait for shutdown signal (Ctrl+C)
6. Graceful shutdown of all tasks

**Shared State:**
- KalshiClient (Arc-wrapped)
- Strategies (Arc<RwLock<HashMap>>)
- Positions (Arc<RwLock<HashMap>>)
- Database connection (Arc-wrapped)

**Communication Channels:**
- Price updates (broadcast channel, 1000 buffer)
- Entry signals (mpsc channel, 100 buffer)
- Exit commands (mpsc channel, 100 buffer)

### 7.2 Task Breakdown

#### Task 1: WebSocket Listener
**Responsibility:** Receive price updates from Kalshi, broadcast to other tasks

**Logic Flow:**
1. Connect to Kalshi WebSocket
2. Loop: receive price updates
3. Broadcast updates to all subscribers via channel
4. On error: log, wait 5 seconds, reconnect
5. Continue until shutdown signal

**Key Points:**
- **Broadcast channel:** Multiple tasks can receive same price update
- **Auto-reconnect:** Network failures don't crash daemon
- **No shared state:** Just forwards messages

#### Task 2: Strategy Evaluator
**Responsibility:** Evaluate price updates, generate entry signals

**Logic Flow:**
1. Receive price update from broadcast channel
2. Fetch full market details for that market
3. Acquire read lock on strategies
4. For each active strategy, check if market matches filters
5. If match, create EntrySignal with strategy criteria
6. Send signal to executor via mpsc channel
7. Release lock, repeat

**Key Points:**
- **Read-only access to strategies:** Uses RwLock::read() (non-blocking)
- **Sends signals, doesn't execute:** Separation of concerns
- **Stateless:** Doesn't track what it's already signaled (executor handles dedup)

#### Task 3: Position Monitor
**Responsibility:** Check open positions, trigger exits

**Logic Flow:**
1. Use tokio::select! to handle two triggers:
   - **Price Update**: Check positions for that specific market
   - **Periodic Timer**: Check all positions every 10 seconds
2. For each check:
   - Acquire read lock on positions
   - Filter positions by market (if price update) or check all
   - Evaluate exit conditions (take profit, stop loss, trailing stop, max hold time)
   - If exit triggered, send ExitCommand to executor
   - Release lock

**Key Points:**
- **Dual triggers:** Price updates OR periodic timer (don't miss exits)
- **Read-only position access:** Fast check without blocking
- **Sends exit commands:** Doesn't execute orders directly

#### Task 4: Order Executor
**Responsibility:** Execute entry/exit orders

**Logic Flow:**
1. Use tokio::select! to handle two channels:
   - **Entry Signal**: New position to open
   - **Exit Command**: Position to close
2. For entry signal:
   - Check risk limits with RiskManager
   - If approved, place order via KalshiClient
   - Poll order status until filled (wait_for_fill)
   - Acquire write lock on positions
   - Create Position from filled order, insert into HashMap
   - Release lock
3. For exit command:
   - Acquire read lock to get position details
   - Place exit order via KalshiClient
   - Release read lock, acquire write lock
   - Update position status to ExitPending
   - Release lock

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

**CalchasError (Top-level):**
- Kalshi API errors
- Strategy errors
- Database errors
- Configuration errors
- Position not found
- Order failed

**KalshiError (Platform-specific):**
- AuthFailed
- RateLimited(seconds)
- MarketNotFound(id)
- Network(reqwest::Error)
- WebSocketDisconnected

**StrategyError:**
- InvalidJson (serde_json::Error)
- ValidationFailed(reason)
- NotFound(strategy_id)

**Error Handling Approach:** Use thiserror crate for ergonomic error types with automatic From conversions

### 9.2 Error Handling Patterns

#### Pattern 1: Retry with Backoff (Network Errors)

**Logic:**
- Try operation
- On RateLimited error: sleep for specified seconds, retry
- On Network error: exponential backoff (2^retries seconds), max 3 retries
- On other errors: propagate immediately

**Use case:** API calls that may fail transiently

#### Pattern 2: Graceful Degradation (WebSocket Disconnect)

**Logic:**
- Run WebSocket loop
- On clean close: exit task
- On WebSocketDisconnected error: log, sleep 5s, reconnect
- On fatal error: propagate and crash

**Use case:** Long-running connections that need automatic recovery

#### Pattern 3: Log and Continue (Non-Critical Failures)

**Logic:**
- Iterate through collection (e.g., positions)
- For each item, try operation
- On error: log with context, continue to next item
- Don't propagate individual failures

**Use case:** Batch operations where one failure shouldn't block others

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

**Configuration Sections:**

**[kalshi]**
- email, password (credentials)
- use_demo (true = demo API, false = production)
- websocket_url, api_base_url

**[runtime]**
- strategy_dir (path to JSON strategies)
- reload_strategies_interval_secs (hot reload frequency)
- position_check_interval_secs (exit check frequency)

**[database]**
- path (SQLite database file)
- max_connections

**[web]**
- host, port (web server binding)
- serve_frontend, frontend_dir

**[logging]**
- level (trace, debug, info, warn, error)
- format (json or pretty)
- log_file (output path)

**[risk]**
- max_total_positions
- max_total_exposure_usd

### 10.2 Config Loading

**AppConfig Structure:** Contains all config sections (kalshi, runtime, database, web, logging, risk)

**Loading Process:**
1. Load from TOML file
2. Merge with environment variables (CALCHAS_ prefix)
3. Deserialize using serde
4. Validate all required fields

**Design Decisions:**
- **TOML format:** Human-readable, supports comments
- **Environment variable overrides:** `CALCHAS_KALSHI__EMAIL=...` overrides config file
- **Sensitive data:** Passwords should come from env vars, not checked into git
- **Validation on load:** Fail fast if config is invalid

---

## 11. Database Schema

### 11.1 SQLite Tables

**markets** - Cached market data from Kalshi
- Primary key: id
- Fields: ticker, title, category, sub_category, status, yes_price, no_price, volume_usd, open_interest, event_time, close_time, timestamps

**positions** - Currently open positions
- Primary key: id
- Foreign keys: market_id
- Fields: strategy_id, side, entry_price, quantity, entry_time, current_price, unrealized_pnl, peak_pnl, status, exit targets (take_profit, stop_loss, trailing_stop), timestamps

**orders** - All orders (historical)
- Primary key: id
- Foreign keys: market_id, position_id (nullable)
- Fields: side, action, order_type, price, quantity, status, filled_quantity, average_fill_price, timestamps

**trades** - Completed positions (immutable history)
- Primary key: id
- Foreign keys: market_id, position_id, entry_order_id, exit_order_id
- Fields: strategy_id, entry details (price, quantity, time), exit details (price, quantity, time, reason), performance metrics (gross_pnl, fees, net_pnl, return_pct, hold_duration), notes, timestamps

**daily_stats** - Daily performance for risk management
- Primary key: date
- Fields: trades_count, net_pnl, gross_pnl, total_fees, win_count, loss_count, timestamps

**Indexes:**
- positions: status, market_id
- trades: strategy_id, exit_time

### 11.2 Migration Strategy

**Approach:** Use `refinery` crate for SQL migrations

**Process:**
- Migrations stored in migrations/ directory
- Versioned files (V1__initial_schema.sql, V2__add_column.sql, etc.)
- Run embedded migrations on daemon startup
- Fail fast if migration fails

---

## 12. API Contracts

### 12.1 Kalshi REST API (External)

**Base URL:** `https://demo-api.kalshi.co/trade-api/v2` (demo)

#### Authentication
- **POST /login**
- Request: email, password (JSON)
- Response: token, member_id (JWT for authenticated requests)

#### Get Markets
- **GET /markets?limit=100&series_ticker=INXD&status=open**
- Headers: Authorization Bearer token
- Response: Array of markets with ticker, title, prices (yes_bid, yes_ask, no_bid, no_ask), volume, open_interest, status, close_time, pagination cursor

#### Place Order
- **POST /orders**
- Headers: Authorization Bearer token
- Request: ticker, side (yes/no), action (buy/sell), type (market/limit), count
- Response: order object with order_id, status

### 12.2 Kalshi WebSocket API (External)

**Connection:** `ws://demo-api.kalshi.co/trade-api/ws/v2`

**Subscribe Message:**
- type: "subscribe"
- market_tickers: array of ticker strings

**Price Update Message:**
- type: "market_update"
- market_ticker, yes_bid, yes_ask, timestamp

### 12.3 Calchas REST API (Internal - for Web UI)

**Base URL:** `http://localhost:8420/api`

#### Get Active Positions
- **GET /api/positions**
- Response: Array of positions with id, market_id, strategy_id, side, entry_price, current_price, unrealized_pnl, return_pct, status

#### Get Trades (Historical)
- **GET /api/trades?limit=50&strategy_id=momentum_scalp**
- Response: Array of trades with id, market_title, strategy_id, entry/exit prices, net_pnl, return_pct, exit_reason, hold_duration

#### Get Strategies
- **GET /api/strategies**
- Response: Array of strategies with id, name, enabled status, active_positions count, total_trades_today, net_pnl_today

### 12.4 Calchas WebSocket API (Internal - for Web UI)

**Connection:** `ws://localhost:8420/ws`

**Subscribe Message:**
- type: "subscribe"
- channels: array ("positions", "trades", "markets")

**Message Types:**
- **position_update**: Real-time position price/PnL updates
- **trade_closed**: Notification when trade completes
- **market_update**: Market data changes

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
- Strategy filtering logic (check if market matches strategy filters)
- PnL calculation (verify profit/loss math)
- Exit condition checking (take profit, stop loss, trailing stop triggers)
- Order price calculation (limit price offsets)

**Approach:** Standard Rust #[test] functions testing pure logic

### 13.3 Integration Tests

**What to test:**
- Kalshi API client (use wiremock to simulate API responses)
- Full entry → exit flow (with fake data, no real money)
- Database persistence (verify SQLite read/write)

**Approach:** Use tokio::test for async tests, wiremock for HTTP mocking

### 13.4 Simulation Mode (Paper Trading)

**SimulatedKalshiClient:**
- Mock implementation of KalshiClient trait
- Maintains in-memory markets and orders
- Simulates immediate order fills at current market price
- No real API calls, no real money

**Usage:** CLI flag `--mode simulation` runs daemon with simulated orders

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

**Service Configuration:**
- Unit: Description, After=network.target
- Service Type: simple
- User: dedicated calchas user
- Working Directory: /opt/calchas
- ExecStart: /opt/calchas/calchas daemon --config production.toml
- Restart: always, RestartSec=10
- Environment: RUST_LOG=info
- Install: WantedBy=multi-user.target

**File Location:** /etc/systemd/system/calchas.service

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

**Concept:** Buy underdog side only when price is low, exit at 50% profit or -60% loss

**JSON Fields:**
- filters: max_underdog_price (0.20), min_favorite_price (0.80)
- entry: side="underdog_only", amount_usd=10
- exit: take_profit_pct=50, stop_loss_pct=-60

#### Strategy B: Volatility Hedge (Both Sides)

**Concept:** Buy both YES and NO sides simultaneously, profit from volatility

**JSON Fields:**
- entry: side="both" (places two orders), amount_usd=10 per side
- exit: take_profit_pct=15, stop_loss_pct=-10 (combined position)

**Implementation Note:** Order Executor places two orders (YES and NO) and tracks as single logical position with combined P&L

#### Strategy C: Hybrid (Conditional Hedge)

**Concept:** Start with underdog, add hedge if losing

**JSON Fields:**
- entry: side="underdog_only", amount_usd=10
- exit: take_profit_pct=50, stop_loss_pct=-60
- risk: hedge_on_loss_pct=-30 (trigger opposite-side order)

**Implementation Note:** Position Monitor checks hedge trigger, places opposite order if PnL drops 30%

---

### 16.2 Success Metrics Tracking

**From PRD Section 2:** Track simulation/live performance

#### Metrics Tracker Component

**Module:** trading/metrics_tracker.rs

**MetricsTracker Responsibilities:**
- Query daily_stats table for performance metrics
- Calculate consecutive profitable days
- Calculate net ROI, win rate, average profit per win
- Check exit-to-live criteria (7+ profitable days, net positive, max loss <15%)
- Return SimulationMetrics struct or ExitToLiveDecision enum

**SimulationMetrics Fields:**
- consecutive_profitable_days
- net_roi
- win_rate (0-100%)
- avg_profit_per_win (%)

**ExitToLiveDecision:**
- Approved (all criteria met)
- NotReady { unmet_criteria: Vec<usize> }

**Database Schema Addition:**
- daily_stats table needs: is_profitable (BOOLEAN), max_single_trade_loss_pct (REAL)

---

### 16.3 CLI Command Specifications

**From PRD Section 10:** Command-line interface specification

**CLI Framework:** clap with Parser and Subcommand derives

**Commands:**

**1. run** - Execute single strategy (one-off)
- Arguments: --strategy (path to JSON), --dry-run (simulation flag), --max-positions (default 5)
- Use case: Test strategy without running full daemon

**2. daemon** - Start background service with web UI
- Arguments: --config (TOML path, default config/default.toml), --port (default 8420), --strategies (directory path), --mode (simulation|live, default simulation)
- Use case: Main production mode

**3. check-simulation** - Validate exit-to-live criteria
- Arguments: --db (database path, default data/calchas.db)
- Use case: Check if simulation is ready for live trading

**4. export** - Export trade history to CSV
- Arguments: --output (file path), --start-date (YYYY-MM-DD), --end-date (YYYY-MM-DD)
- Use case: Analysis and reporting

**Trading Modes:**
- simulation: Paper trading (mock orders, no real money)
- live: Real trading (actual API calls, real money)

---

### 16.4 Simulation Mode Implementation

**From PRD Section 2.4:** Exit to live criteria

#### Simulation Validator Component

**Module:** trading/simulation_validator.rs

**SimulationValidator Responsibilities:**
- Use MetricsTracker to get simulation performance
- Check exit-to-live criteria against thresholds
- Generate human-readable report with recommendation

**SimulationConfig Thresholds:**
- min_consecutive_profitable_days (default: 7)
- min_net_roi (default: 0.0)
- max_single_day_loss_pct (default: 15.0)

**SimulationReport Structure:**
- metrics: SimulationMetrics
- decision: ExitToLiveDecision (Approved or NotReady)
- recommendation: String (formatted message)

**CLI Integration:**
- Command: `calchas check-simulation`
- Output: ASCII report with metrics checklist, decision, next steps
- Exit codes: 0 if approved, 1 if not ready

---

### 16.5 Market Category Extensibility

**From PRD Section 4.3:** Support for multiple market categories

**MarketCategory Enum:**
- Sports { sport: SportType, game_status: GameStatus }
- Politics
- Economics
- Crypto
- Entertainment
- Weather
- Other

**SportType Sub-Categories:**
- AmericanFootball, Hockey, Soccer, Basketball, Baseball

**GameStatus Options:**
- PreGame, Live, Final, Postponed

**Future Extension:**
- Sport-specific strategy traits (e.g., detect offensive drive momentum)
- Requires additional market metadata from Kalshi
- MVP focuses on generic price-based strategies

---

### 16.6 Updated Module Structure

**New Modules for PRD Compliance:**
- src/trading/metrics_tracker.rs - Track simulation/live metrics
- src/trading/simulation_validator.rs - Check exit-to-live criteria

---


**End of Technical Architecture**

For build progress and implementation roadmap, see: `PROJECT_STATUS.md`
