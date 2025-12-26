# RUST LEARNING CHECKLIST
## Mapped to Calchas Technical Architecture

**Learning Philosophy:** Learn Rust by building Calchas components. Each checkbox = concept learned + code written.

**Progress Tracking:** Check off topics as you complete them. Once checked, you're "free" from that concept - I'll assume you understand it.

**Architecture Reference:** See `TECHNICAL_ARCHITECTURE.md` for component details.

---

## 📚 Recommended Resources

### Books (Read as Needed)
- [ ] **The Rust Programming Language (The Book)** - https://doc.rust-lang.org/book/ (Read chapters 1-10 during Phase 1-2)
- [ ] **Tokio Tutorial** - https://tokio.rs/tokio/tutorial (Essential for Phase 3)
- [ ] **Programming Rust, 2nd Edition** - Deep dive (Phase 4-5)
- [ ] **Rust for Rustaceans** - Advanced patterns (Phase 6-7)

### Online
- [ ] **Rustlings** - Interactive exercises (supplement Phase 1)
- [ ] **Rust by Example** - Quick reference
- [ ] **Jon Gjengset's YouTube** - Deep dives (async, traits, lifetimes)

---

## Phase 1: Foundation (Weeks 1-2)
**Architecture Goal:** Define core data models + Strategy JSON loader
**Milestone:** Load strategy JSON file and print parsed struct

### 1.1 Setup & Project Structure
- [x] Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- [x] Verify: `cargo --version`, `rustc --version`
- [x] Initialize project: `cd ~/Development/calchas && cargo init --name calchas`
- [x] Understand `Cargo.toml` (dependencies, workspace)
- [x] Understand `src/main.rs` vs `src/lib.rs`
- [x] **File:** Create initial `Cargo.toml` with serde, chrono, rust_decimal

### 1.2 Basic Programming Concepts
- [x] Variables: `let x = 5;`, `let mut y = 10;`
- [x] Scalar types: `i32`, `u64`, `f64`, `bool`, `char`
- [x] Functions: `fn calculate_profit_pct(entry: f64, exit: f64) -> f64`
- [x] Control flow: `if price < 20 { ... }`, `for market in markets { ... }`
- [x] **File:** Create `src/utils/mod.rs` and `src/utils/decimal.rs`
- [x] **Implementation:** Write helper functions (PnL calculation, percentage conversion)

### 1.3 Memory & Stack/Heap Concepts
- [x] Understand stack vs heap allocation
- [x] Understand what happens when you do `let s = String::from("hello")`
- [x] Understand why primitives (i32) are on stack, String is on heap
- [x] **Mental Model:** Draw stack/heap diagram for Market struct

### 1.4 Ownership Basics
> **Core Rust concept** - The real entry exam
- [x] Ownership rules: Each value has one owner, owner drops value when out of scope
- [x] Move semantics: `let s1 = String::from("hello"); let s2 = s1; // s1 invalid`
- [x] References and borrowing: `&market` (immutable), `&mut position` (mutable)
- [x] Borrow checker rules: One mutable OR many immutable references
- [x] **File:** `src/models/mod.rs`
- [x] **Implementation:** Pass Market structs between functions using references

### 1.5 Structs & Enums
> **Data modeling before behavior** - Foundation of Calchas
- [ ] Define structs with named fields
- [ ] Implement methods in `impl` blocks
- [ ] Derive traits: `#[derive(Debug, Clone)]`
- [ ] Enums with variants: `enum OrderStatus { Pending, Filled, ... }`
- [ ] `Option<T>` - Represent optional values (`Some(x)`, `None`)
- [ ] `Result<T, E>` - Represent success/failure
- [ ] **Files:**
  - `src/models/market.rs` → `MarketId`, `Market`, `MarketCategory`, `MarketStatus`
  - `src/models/strategy.rs` → `StrategyId`, `Strategy`, `StrategyFilters`, `EntryRules`, `ExitRules`
  - `src/models/position.rs` → `PositionId`, `Position`, `ExitTarget`, `PositionStatus`
  - `src/models/order.rs` → `OrderId`, `Order`, `OrderSide`, `OrderAction`, `OrderType`, `OrderStatus`
  - `src/models/trade.rs` → `TradeId`, `Trade`, `ExitReason`
- [ ] **Implementation:** Define all core data models from Architecture Section 4

### 1.6 Pattern Matching
> **Backbone of idiomatic Rust** - Master early
- [ ] `match` expressions for exhaustive matching
- [ ] Match on enums: `match order.status { OrderStatus::Filled => ... }`
- [ ] `if let` for single-pattern matching
- [ ] Destructuring: `let Position { id, entry_price, .. } = position;`
- [ ] **File:** `src/strategy/evaluator.rs`
- [ ] **Implementation:** Match on MarketCategory, OrderStatus in strategy evaluation

### 1.7 Slices & Strings
- [ ] String slices (`&str`) vs owned String
- [ ] When to use `&str` (function parameters) vs `String` (owned data)
- [ ] Array slices: `&[T]`
- [ ] **Implementation:** Parse strategy name from JSON, market ticker strings

**Phase 1 Checkpoint:**
- [ ] Can define structs with proper field types
- [ ] Understand when to use `&` vs moving ownership
- [ ] Can pattern match on enums
- [ ] Have working data models in `src/models/`

---

## Phase 2: Practical Rust (Weeks 3-4)
**Architecture Goal:** Strategy JSON loader + Basic Kalshi client (no async yet)
**Milestone:** Load strategy from JSON file, validate, print

### 2.1 Error Handling (Deep Dive)
> **Learn Result<T, E> propagation before error frameworks**
- [ ] `Result<T, E>` propagation with `?` operator
- [ ] Custom error enums: `enum StrategyError { InvalidJson, ValidationFailed, ... }`
- [ ] `thiserror` crate for error boilerplate: `#[derive(Error)]`
- [ ] `anyhow` for application errors (quick prototyping)
- [ ] **Files:**
  - `src/platforms/kalshi/error.rs` → `KalshiError` enum
  - `src/strategy/error.rs` → `StrategyError` enum
  - `src/lib.rs` → `CalchasError` top-level error
- [ ] **Implementation:** Define error types from Architecture Section 9

### 2.2 Collections
- [ ] `Vec<T>` - Growable array
- [ ] `HashMap<K, V>` - Key-value storage
- [ ] `HashSet<T>` - Unique values
- [ ] **File:** `src/strategy/engine.rs`
- [ ] **Implementation:** `HashMap<StrategyId, Strategy>` for loaded strategies

### 2.3 Iterators
> **Learn before async** - Teaches laziness, composition
- [ ] Iterator trait: `Iterator::Item`
- [ ] Adapters: `.map()`, `.filter()`, `.filter_map()`, `.fold()`
- [ ] Consuming: `.collect()`, `.count()`, `.sum()`
- [ ] Lazy evaluation (nothing happens until consumed)
- [ ] **File:** `src/strategy/evaluator.rs`
- [ ] **Implementation:** Chain iterator methods to filter enabled strategies, match filters, and create entry signals

### 2.4 Traits
> **Traits as interfaces** - Core abstraction
- [ ] Define trait: `trait Exchange { fn fetch_markets(&self) -> Result<Vec<Market>>; }`
- [ ] Implement trait for type: `impl Exchange for KalshiClient { ... }`
- [ ] Trait bounds: `fn process<T: Exchange>(client: &T)`
- [ ] `impl Trait` syntax for return types
- [ ] **File:** `src/platforms/mod.rs`
- [ ] **Implementation:** Define `Exchange` trait (Architecture Section 6.1)

### 2.5 Generics
- [ ] Generic functions: `fn get_by_id<T>(id: &str, items: &Vec<T>) -> Option<&T>`
- [ ] Generic structs: `struct ApiResponse<T> { data: T, ... }`
- [ ] Type parameters with bounds: `<T: Clone + Debug>`
- [ ] **File:** `src/platforms/kalshi/types.rs`
- [ ] **Implementation:** Generic API response wrapper

### 2.6 Lifetimes
> **Learn borrowing rules before lifetime syntax**
- [ ] Lifetime annotations: `fn longest<'a>(x: &'a str, y: &'a str) -> &'a str`
- [ ] Lifetime elision rules (when you don't need to write them)
- [ ] Struct lifetimes: `struct StrategyRef<'a> { strategy: &'a Strategy }`
- [ ] **File:** `src/strategy/engine.rs`
- [ ] **Implementation:** References to strategies in evaluation (if needed)

### 2.7 Modules & Crate Structure
- [ ] `mod` keyword to declare modules
- [ ] `pub` for public visibility
- [ ] `use` for imports
- [ ] File-based modules: `src/models/market.rs` becomes `mod models::market`
- [ ] **Files:** Organize all modules according to Architecture Section 5.1
- [ ] **Implementation:** Create module tree with models/, platforms/, strategy/, trading/, storage/, runtime/, web/, utils/ directories

### 2.8 Testing
- [ ] `#[test]` attribute for unit tests
- [ ] `assert!`, `assert_eq!`, `assert_ne!` macros
- [ ] `#[cfg(test)]` module for test-only code
- [ ] Run tests: `cargo test`
- [ ] **File:** `src/models/position.rs` (inline tests)
- [ ] **Implementation:** Write test for calculating unrealized PnL based on entry and current price

### 2.9 JSON Serialization (serde)
- [ ] Add `serde = { version = "1.0", features = ["derive"] }` to Cargo.toml
- [ ] Add `serde_json = "1.0"` for JSON support
- [ ] `#[derive(Serialize, Deserialize)]` on structs
- [ ] Customize with `#[serde(rename = "...")]`, `#[serde(default)]`
- [ ] **File:** `src/strategy/loader.rs`
- [ ] **Implementation:** Load strategy JSON file, deserialize to Strategy struct using serde_json

### 2.10 Standard Library Before Crates
> **stdlib is powerful** - Use before reaching for external crates
- [ ] `std::fs::read_to_string()` - Read files
- [ ] `std::path::Path`, `PathBuf` - File paths
- [ ] `std::env::var()` - Environment variables
- [ ] **File:** `src/config/mod.rs`
- [ ] **Implementation:** Load config from TOML file

**Phase 2 Checkpoint:**
- [ ] Can load strategy JSON and parse into Rust struct
- [ ] Understand how `?` operator works with Result
- [ ] Can write unit tests for data models
- [ ] Have working `src/strategy/loader.rs` module

---

## Phase 3: Async & Networking (Weeks 5-6)
**Architecture Goal:** Kalshi REST + WebSocket clients
**Milestone:** Connect to Kalshi, fetch markets, subscribe to price updates

### 3.1 Threads & Channels (Synchronous First)
> **Learn blocking concurrency before non-blocking**
- [ ] `std::thread::spawn()` to create threads
- [ ] `std::sync::mpsc::channel()` for message passing
- [ ] `Send` and `Sync` traits (what they mean)
- [ ] **Exercise:** Write a multi-threaded market scanner (before async)

### 3.2 Async Basics
- [ ] `async fn` - Async function syntax
- [ ] `.await` - Wait for async operation
- [ ] `Future` trait (conceptual - what async returns)
- [ ] Executor concept (Tokio is the executor)
- [ ] **Mental Model:** Async is cooperative multitasking, not parallelism

### 3.3 Tokio Runtime
- [ ] Add `tokio = { version = "1", features = ["full"] }` to Cargo.toml
- [ ] `#[tokio::main]` macro for async main function
- [ ] `tokio::spawn()` to spawn concurrent tasks
- [ ] `JoinHandle` to wait for tasks
- [ ] **File:** `src/main.rs`
- [ ] **Implementation:** Add tokio::main attribute to main function to enable async runtime

### 3.4 HTTP Clients (reqwest)
- [ ] Add `reqwest = { version = "0.11", features = ["json"] }` to Cargo.toml
- [ ] Create client: `let client = reqwest::Client::new();`
- [ ] GET request: `client.get(url).send().await?`
- [ ] POST with JSON: `client.post(url).json(&body).send().await?`
- [ ] Headers: `.header("Authorization", "Bearer token")`
- [ ] **File:** `src/platforms/kalshi/client.rs`
- [ ] **Implementation:** KalshiClient struct with http_client, base_url, auth_token (RwLock) - implement login, get_markets, place_order methods

### 3.5 JSON Serialization (Async Context)
- [ ] Deserialize JSON response: `response.json::<Vec<Market>>().await?`
- [ ] Handle API-specific types (Kalshi responses)
- [ ] **File:** `src/platforms/kalshi/types.rs`
- [ ] **Implementation:** Define Kalshi API response types

### 3.6 WebSockets (tokio-tungstenite)
- [ ] Add `tokio-tungstenite = "0.21"` to Cargo.toml
- [ ] Connect: `let (ws_stream, _) = connect_async(url).await?`
- [ ] Send message: `ws_stream.send(Message::Text(...)).await?`
- [ ] Receive messages in loop: `while let Some(msg) = ws_stream.next().await { ... }`
- [ ] **File:** `src/platforms/kalshi/websocket.rs`
- [ ] **Implementation:** KalshiWebSocket struct with ws_stream and subscriptions (HashSet) - implement connect, subscribe, next_update methods

### 3.7 Channels (Async - Tokio)
- [ ] `tokio::sync::mpsc::channel()` - Multi-producer, single-consumer
- [ ] `tokio::sync::broadcast::channel()` - Multi-producer, multi-consumer
- [ ] `tokio::sync::oneshot::channel()` - One-time message
- [ ] Send: `tx.send(value).await?`
- [ ] Receive: `rx.recv().await`
- [ ] **File:** `src/runtime/channels.rs`
- [ ] **Implementation:** Define type aliases for PriceUpdateSender/Receiver (broadcast), EntrySignalSender/Receiver (mpsc)

### 3.8 Async Streams
- [ ] Add `futures = "0.3"` to Cargo.toml
- [ ] `use futures::stream::StreamExt;`
- [ ] `.next().await` to get next item from stream
- [ ] `.filter_map()`, `.map()` on streams
- [ ] **File:** `src/platforms/kalshi/websocket.rs`
- [ ] **Implementation:** Process WebSocket messages as stream

### 3.9 Timeouts & Delays
- [ ] `tokio::time::sleep(Duration::from_secs(5)).await` - Delay
- [ ] `tokio::time::timeout(Duration::from_secs(10), future).await` - Timeout
- [ ] **File:** `src/platforms/kalshi/client.rs`
- [ ] **Implementation:** Add retry logic with exponential backoff (Architecture Section 9.2)

**Phase 3 Checkpoint:**
- [ ] Can connect to Kalshi API (login, fetch markets)
- [ ] Can subscribe to WebSocket and receive price updates
- [ ] Understand async/await and when to use `.await`
- [ ] Have working `src/platforms/kalshi/` module

---

## Phase 4: Concurrency & State (Weeks 7-8)
**Architecture Goal:** Multi-task runtime (WebSocket + Strategy + Position + Executor tasks)
**Milestone:** Real-time price monitoring with concurrent strategy evaluation

### 4.1 Smart Pointers
> **Learn ownership-friendly APIs before smart pointers**
- [ ] `Box<T>` - Heap allocation (single owner)
- [ ] `Rc<T>` - Reference counting (single-threaded)
- [ ] `Arc<T>` - Atomic reference counting (thread-safe)
- [ ] `Weak<T>` - Break reference cycles
- [ ] **File:** `src/runtime/supervisor.rs`
- [ ] **Implementation:** Wrap KalshiClient in Arc, clone to share across spawned tasks

### 4.2 Interior Mutability
- [ ] `RefCell<T>` - Runtime borrow checking (single-threaded)
- [ ] `Mutex<T>` - Mutual exclusion (thread-safe)
- [ ] `RwLock<T>` - Reader-writer lock (multiple readers OR one writer)
- [ ] **File:** `src/trading/position_manager.rs`
- [ ] **Implementation:** PositionManager with Arc<RwLock<HashMap<PositionId, Position>>> - use read() for queries, write() for updates

### 4.3 Send + Sync Traits
- [ ] `Send` - Safe to move between threads
- [ ] `Sync` - Safe to share references between threads
- [ ] Why `Arc<Mutex<T>>` requires `T: Send`
- [ ] Compiler enforces these automatically
- [ ] **Mental Model:** Rust prevents data races at compile time

### 4.4 Spawning Concurrent Tasks
- [ ] `tokio::spawn()` for independent tasks
- [ ] Tasks run concurrently on Tokio runtime
- [ ] Return `JoinHandle<T>` to await task result
- [ ] **File:** `src/runtime/supervisor.rs`
- [ ] **Implementation:** Spawn 5 tasks (websocket, strategy, position, executor, web) and store JoinHandles

### 4.5 Select & Join
- [ ] `tokio::select!` - Wait for first completion
- [ ] `tokio::join!` - Wait for all to complete
- [ ] **File:** `src/runtime/supervisor.rs`
- [ ] **Implementation:** Use tokio::select! to handle either price updates or periodic interval ticks

### 4.6 Graceful Shutdown
- [ ] `tokio::signal::ctrl_c().await` - Wait for Ctrl+C
- [ ] Broadcast shutdown signal to all tasks
- [ ] `tokio::select!` to listen for shutdown
- [ ] **File:** `src/runtime/shutdown.rs`
- [ ] **Implementation:** Use tokio::select! to handle shutdown signal or normal work, break loop on shutdown

### 4.7 Lazy Initialization
- [ ] `std::sync::OnceLock` - Thread-safe lazy initialization
- [ ] Initialize logger once
- [ ] **File:** `src/utils/logging.rs`
- [ ] **Implementation:** Global logger setup

### 4.8 Actor Pattern
- [ ] Message passing via channels
- [ ] Single task owns mutable state
- [ ] Other tasks send messages to request changes
- [ ] **File:** `src/trading/order_executor.rs`
- [ ] **Implementation:** Order executor task receives EntrySignal and ExitCommand via channels, uses tokio::select! to handle both

**Phase 4 Checkpoint:**
- [ ] Can spawn multiple concurrent tasks
- [ ] Understand Arc<RwLock<T>> for shared mutable state
- [ ] Can communicate between tasks using channels
- [ ] Have working multi-task runtime in `src/runtime/supervisor.rs`

---

## Phase 5: Advanced Patterns (Weeks 9-10)
**Architecture Goal:** Extensible strategy system + Risk management
**Milestone:** Multiple strategies running concurrently with risk limits

### 5.1 Advanced Traits
- [ ] Associated types: `trait Exchange { type Market; }`
- [ ] Default implementations
- [ ] Trait inheritance: `trait A: B + C`
- [ ] **File:** `src/platforms/mod.rs`
- [ ] **Implementation:** Advanced Exchange trait variations

### 5.2 Trait Objects
- [ ] `dyn Trait` for runtime polymorphism
- [ ] `Box<dyn Trait>` - Owned trait object
- [ ] `Arc<dyn Trait>` - Shared trait object
- [ ] Object safety rules
- [ ] **File:** `src/strategy/engine.rs`
- [ ] **Implementation:** (Future) Dynamic strategy loading

### 5.3 Closures
- [ ] `Fn` - Immutable capture
- [ ] `FnMut` - Mutable capture
- [ ] `FnOnce` - Consume captured values
- [ ] `move` closures - Take ownership
- [ ] **File:** `src/strategy/evaluator.rs`
- [ ] **Implementation:** Use closures in iterator chains to filter markets by liquidity and status

### 5.4 Macros (Declarative)
- [ ] `macro_rules!` basics
- [ ] Pattern matching in macros
- [ ] **File:** `src/utils/logging.rs`
- [ ] **Implementation:** Custom logging macro (optional)

### 5.5 Builder Pattern
- [ ] Fluent API design
- [ ] Typestate pattern (compile-time guarantees)
- [ ] **File:** `src/models/strategy.rs`
- [ ] **Implementation:** (Optional) StrategyBuilder

### 5.6 Newtype Pattern
- [ ] Type safety with wrapper types: `struct MarketId(String);`
- [ ] Prevents mixing up IDs
- [ ] **Already implemented in Phase 1!** (MarketId, PositionId, OrderId)

### 5.7 Type Aliases
- [ ] `type Result<T> = std::result::Result<T, CalchasError>;`
- [ ] Consistent error types across codebase
- [ ] **File:** `src/lib.rs`
- [ ] **Implementation:** Define CalchasResult type alias

### 5.8 Zero-Cost Abstractions
- [ ] Understand monomorphization (generics compile to concrete types)
- [ ] Iterators compile to same code as loops
- [ ] **Mental Model:** Abstractions have no runtime cost in Rust

**Phase 5 Checkpoint:**
- [ ] Can use closures with iterator chains
- [ ] Understand when to use trait objects vs generics
- [ ] Have extensible strategy system

---

## Phase 6: Production Quality (Weeks 11-12)
**Architecture Goal:** CLI + Logging + Database + Web UI
**Milestone:** Production-ready daemon with monitoring dashboard

### 6.1 Logging (tracing)
- [ ] Add `tracing = "0.1"`, `tracing-subscriber = "0.3"` to Cargo.toml
- [ ] `#[instrument]` macro for automatic span creation
- [ ] `info!()`, `warn!()`, `error!()` macros
- [ ] Structured logging: `info!(position_id = %id, pnl = %pnl, "Position closed")`
- [ ] **File:** `src/utils/logging.rs`
- [ ] **Implementation:** Initialize tracing_subscriber with fmt formatter, enable thread IDs, file names, line numbers

### 6.2 Configuration Management
- [ ] Add `config = "0.13"`, `toml = "0.8"` to Cargo.toml
- [ ] Load TOML files
- [ ] Environment variable overrides
- [ ] **File:** `src/config/mod.rs`
- [ ] **Implementation:** AppConfig from Architecture Section 10

### 6.3 Database Integration (rusqlite)
- [ ] Add `rusqlite = { version = "0.30", features = ["bundled"] }` to Cargo.toml
- [ ] Create connection: `let conn = Connection::open("calchas.db")?;`
- [ ] Execute SQL: `conn.execute("INSERT INTO ...", params![])?`
- [ ] Query rows: `let mut stmt = conn.prepare("SELECT * FROM ...")?;`
- [ ] **File:** `src/storage/sqlite.rs`
- [ ] **Implementation:** SqliteDatabase struct with Arc<Mutex<Connection>> - implement insert_trade, get_active_positions methods

### 6.4 CLI Argument Parsing (clap)
- [ ] Add `clap = { version = "4.4", features = ["derive"] }` to Cargo.toml
- [ ] `#[derive(Parser)]` for CLI struct
- [ ] `#[command(subcommand)]` for subcommands
- [ ] **File:** `src/main.rs`
- [ ] **Implementation:** Cli struct with Commands enum - Run (strategy, dry_run), Daemon (config, port, mode), CheckSimulation (db), Export (output)

### 6.5 Error Context (anyhow)
- [ ] `.context("Failed to fetch market")` for error wrapping
- [ ] Rich error messages in logs
- [ ] **File:** Throughout codebase
- [ ] **Implementation:** Add context to all Result returns

### 6.6 Testing Strategies
- [ ] Integration tests in `tests/` directory
- [ ] Mock Kalshi API with `wiremock` crate
- [ ] **File:** `tests/integration/kalshi_client_test.rs`
- [ ] **Implementation:** Use wiremock::MockServer to simulate Kalshi API responses, test KalshiClient methods

### 6.7 Profiling & Benchmarking
- [ ] Add `criterion = "0.5"` to `[dev-dependencies]`
- [ ] `cargo bench` to run benchmarks
- [ ] **File:** `benches/strategy_eval.rs`
- [ ] **Implementation:** Benchmark strategy evaluation speed

### 6.8 Documentation
- [ ] `///` doc comments for public functions
- [ ] `//!` module-level docs
- [ ] `cargo doc --open` to view docs
- [ ] **File:** All public modules
- [ ] **Implementation:** Document `src/platforms/kalshi/mod.rs`, `src/strategy/mod.rs`

### 6.9 Web Server (Axum)
- [ ] Add `axum = "0.7"` to Cargo.toml
- [ ] Define routes: `Router::new().route("/api/positions", get(get_positions))`
- [ ] Shared state: `Extension<Arc<AppState>>`
- [ ] WebSocket support: `axum::extract::ws::WebSocket`
- [ ] **File:** `src/web/server.rs`
- [ ] **Implementation:** Create Router with routes for /api/positions, /api/trades, /ws - add Extension layer for shared state - bind and serve

### 6.10 React Frontend
- [ ] Set up Vite project in `frontend/`
- [ ] `npm create vite@latest frontend -- --template react-ts`
- [ ] Install dependencies: `cd frontend && npm install`
- [ ] **Files:** `frontend/src/components/PositionTracker.tsx`, `MarketScanner.tsx`, etc.
- [ ] **Implementation:** Components from Architecture Section 5.1

**Phase 6 Checkpoint:**
- [ ] Can run `calchas daemon --port 8420` and access web UI
- [ ] Can run `calchas check-simulation` to see metrics
- [ ] Have structured logging throughout codebase
- [ ] Have integration tests for Kalshi client

---

## Phase 7: Expert Topics (Optional - Weeks 13+)
**Architecture Goal:** Performance optimization, advanced features
**Milestone:** Production deployment, custom optimizations

### 7.1 Advanced Type System
- [ ] Higher-ranked trait bounds (HRTBs)
- [ ] Generic Associated Types (GATs)
- [ ] Variance (covariance, contravariance)
- [ ] Type-state programming for compile-time guarantees

### 7.2 Procedural Macros
- [ ] Derive macros
- [ ] Attribute macros
- [ ] Custom derives for strategy validation

### 7.3 Unsafe Rust
> **Learn unsafe theory before unsafe code**
- [ ] Understand undefined behavior (UB)
- [ ] Raw pointers (`*const T`, `*mut T`)
- [ ] `unsafe` blocks and functions
- [ ] **When needed:** FFI, custom allocators (rarely for Calchas)

### 7.4 Advanced Memory & Performance
- [ ] Custom allocators (jemalloc)
- [ ] Arena allocators
- [ ] Cache-friendly data structures
- [ ] SIMD for calculations

### 7.5 Advanced Concurrency
- [ ] Lock-free data structures
- [ ] Custom async executors
- [ ] Actor models (Actix-style)

### 7.6 System Design
- [ ] High-throughput services (100k+ RPS)
- [ ] Backpressure handling
- [ ] Horizontal scaling

### 7.7 WebAssembly
- [ ] Compile to wasm32-unknown-unknown
- [ ] Run strategies in browser (backtesting UI)

### 7.8 Open Source Contributions
- [ ] Contribute to Tokio, Serde, Axum
- [ ] Write your own crate
- [ ] Share Calchas strategies with community

**Phase 7 Checkpoint:**
- [ ] Can optimize performance with profiling
- [ ] Can write unsafe code when necessary
- [ ] Ready to contribute to open source Rust projects

---

## 🎯 Architecture Component Mapping

| Component | Phase | Files | Rust Concepts |
|-----------|-------|-------|---------------|
| **Data Models** | 1 | `src/models/*.rs` | Structs, enums, newtypes |
| **Strategy Loader** | 2 | `src/strategy/loader.rs` | serde, Result, File I/O |
| **Kalshi REST Client** | 3 | `src/platforms/kalshi/client.rs` | async/await, reqwest, error handling |
| **Kalshi WebSocket** | 3 | `src/platforms/kalshi/websocket.rs` | tokio-tungstenite, streams |
| **Strategy Engine** | 3-4 | `src/strategy/engine.rs` | Iterators, closures, RwLock |
| **Position Manager** | 4 | `src/trading/position_manager.rs` | Arc<RwLock>, async tasks |
| **Order Executor** | 4 | `src/trading/order_executor.rs` | Actor pattern, channels |
| **Runtime Supervisor** | 4 | `src/runtime/supervisor.rs` | tokio::spawn, shutdown |
| **SQLite Integration** | 6 | `src/storage/sqlite.rs` | rusqlite, SQL |
| **CLI** | 6 | `src/main.rs` | clap, subcommands |
| **Web Server** | 6 | `src/web/server.rs` | Axum, WebSocket |
| **Metrics Tracker** | 6 | `src/trading/metrics_tracker.rs` | Database queries, aggregation |

---

## ✅ How to Use This Checklist

1. **Work sequentially** - Complete Phase 1 before Phase 2
2. **Check off items as you implement them** - Once checked, I assume you understand
3. **Ask for help when stuck** - "I'm stuck on async/await, can you explain?"
4. **Reference architecture** - Each component references `TECHNICAL_ARCHITECTURE.md`
5. **Code every day** - Consistency beats intensity

**Current Phase:** Phase 1 - Foundation (Week 1)
**Current Component:** Utility helpers (completed 1.1, 1.2)
**Next Milestone:** Define core data models (Phase 1.5)

---

**Version:** 2.0 (Architecture-Mapped)
**Last Updated:** December 2025
**Status:** Ready to build Calchas
