# RUST LEARNING GUIDE
## Learn Rust by Building Calchas

**Learning Philosophy:** Learn concepts when you need them for the component you're building. The product drives what you learn, not arbitrary phases.

**How to Use:**
1. Check `PROJECT_STATUS.md` - What component are you building?
2. Check this file - What Rust concepts does it need?
3. Learn those concepts (resources below)
4. Build the component
5. Check off the concepts

**Architecture Reference:** See `TECHNICAL_ARCHITECTURE.md` for component design details.

---

## 🟢 Core Fundamentals
**Learn these first** - Foundation for everything else

**Used for:** Data models, utility functions, basic code structure

### Variables & Types
- [x] Variables: `let x = 5;`, `let mut y = 10;`
- [x] Scalar types: `i32`, `u64`, `f64`, `bool`, `char`
- [x] Functions: `fn calculate_profit(entry: Decimal, exit: Decimal) -> Decimal`
- [x] Control flow: `if`, `for`, `while`, `loop`

### Ownership & Borrowing
- [x] Ownership rules: Each value has one owner
- [x] Move semantics: `let s2 = s1; // s1 invalid`
- [x] References: `&market` (immutable), `&mut position` (mutable)
- [x] Borrow checker: One mutable OR many immutable references
- [x] Stack vs heap allocation

**Components:** All of them

---

## 🔵 Data Modeling
**Learn when building:** Core data models (Market, Strategy, Position, Order, Trade)

### Structs & Enums
- [x] Define structs with named fields
- [x] Implement methods in `impl` blocks
- [x] Derive traits: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- [x] Enums with variants: `enum OrderStatus { Pending, Filled, ... }`
- [x] Pattern matching: `match order.status { ... }`
- [x] `Option<T>`: Represent optional values (`Some(x)`, `None`)
- [x] `Result<T, E>`: Represent success/failure

### Newtype Pattern
- [x] Type safety: `struct MarketId(String);`
- [x] Prevents mixing up IDs
- [x] Implement `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`

**Files:**
- `src/models/market.rs` → MarketId, Market, MarketCategory, MarketStatus
- `src/models/strategy.rs` → StrategyId, Strategy, StrategyFilters, EntryRules, ExitRules
- `src/models/position.rs` → PositionId, Position, ExitTarget, PositionStatus
- `src/models/order.rs` → OrderId, Order, OrderSide, OrderAction, OrderType, OrderStatus
- `src/models/trade.rs` → TradeId, Trade, ExitReason

**Components:** All data models

---

## 🟡 Practical Rust
**Learn when building:** Strategy loader, business logic, utilities

### Error Handling
- [x] `Result<T, E>` propagation with `?` operator
- [x] Custom error enums: `enum LoaderError { ... }`
- [ ] `thiserror` crate: `#[derive(Error)]`
- [ ] `anyhow` for application errors
- [ ] Error context: `.context("Failed to...")`

### Collections
- [x] `Vec<T>`: Growable array
- [ ] `HashMap<K, V>`: Key-value storage
- [ ] `HashSet<T>`: Unique values

### Iterators
- [ ] Iterator trait: `Iterator::Item`
- [ ] Adapters: `.map()`, `.filter()`, `.filter_map()`, `.fold()`
- [ ] Consuming: `.collect()`, `.count()`, `.sum()`
- [ ] Lazy evaluation
- [ ] Chain iterator methods for data transformation

### File I/O
- [x] `std::fs::read_to_string()`: Read files
- [x] `std::path::Path`, `PathBuf`: File paths
- [x] `std::env::var()`: Environment variables

### JSON Serialization (serde)
- [x] `#[derive(Serialize, Deserialize)]` on structs
- [x] `serde_json::from_str()`, `to_string_pretty()`
- [ ] Customize: `#[serde(rename = "...")]`, `#[serde(default)]`

### Testing
- [x] `#[test]` attribute
- [x] `assert!`, `assert_eq!`, `assert_ne!`
- [x] `#[cfg(test)]` module
- [x] Run: `cargo test`

**Files:**
- `src/strategy/loader.rs` → Load strategy JSON
- `src/config/mod.rs` → Load config from TOML
- `src/utils/*.rs` → Helper functions

**Components:** Strategy loader, config management, utilities

---

## 🟠 Traits & Generics
**Learn when building:** Platform abstraction, reusable code

### Traits
- [ ] Define trait: `trait Exchange { ... }`
- [ ] Implement for type: `impl Exchange for KalshiClient { ... }`
- [ ] Trait bounds: `fn process<T: Exchange>(client: &T)`
- [ ] `impl Trait` syntax
- [ ] Associated types
- [ ] Default implementations

### Generics
- [ ] Generic functions: `fn get_by_id<T>(...) -> Option<&T>`
- [ ] Generic structs: `struct ApiResponse<T> { data: T }`
- [ ] Type parameters with bounds: `<T: Clone + Debug>`

### Closures
- [ ] `Fn`, `FnMut`, `FnOnce` traits
- [ ] `move` closures
- [ ] Use in iterator chains

**Files:**
- `src/platforms/mod.rs` → Exchange trait
- `src/platforms/kalshi/types.rs` → Generic API types

**Components:** Platform abstraction, extensible strategies

---

## 🔴 Async & Networking
**Learn when building:** Kalshi REST client, WebSocket integration

### Async Basics
- [ ] `async fn` syntax
- [ ] `.await` operator
- [ ] `Future` trait (conceptual)
- [ ] Executor concept (Tokio)

### Tokio Runtime
- [ ] `#[tokio::main]` macro
- [ ] `tokio::spawn()` for concurrent tasks
- [ ] `JoinHandle` to await tasks
- [ ] `tokio::select!` for multiple futures
- [ ] `tokio::join!` for parallel execution

### HTTP Client (reqwest)
- [ ] `reqwest::Client::new()`
- [ ] GET request: `client.get(url).send().await?`
- [ ] POST with JSON: `client.post(url).json(&body).send().await?`
- [ ] Headers: `.header("Authorization", "...")`
- [ ] Deserialize response: `response.json::<T>().await?`

### WebSockets (tokio-tungstenite)
- [ ] Connect: `connect_async(url).await?`
- [ ] Send message: `ws_stream.send(Message::Text(...)).await?`
- [ ] Receive loop: `while let Some(msg) = ws_stream.next().await { ... }`

### Streams
- [ ] `StreamExt` from futures crate
- [ ] `.next().await` to get items
- [ ] `.filter_map()`, `.map()` on streams

### Timeouts & Retries
- [ ] `tokio::time::sleep(Duration)`
- [ ] `tokio::time::timeout(Duration, future)`
- [ ] Exponential backoff retry logic

**Files:**
- `src/platforms/kalshi/client.rs` → REST client
- `src/platforms/kalshi/websocket.rs` → WebSocket client

**Components:** Kalshi integration, real-time price updates

---

## 🟣 Concurrency & Shared State
**Learn when building:** Multi-task daemon, runtime supervisor

### Smart Pointers
- [ ] `Box<T>`: Heap allocation
- [ ] `Rc<T>`: Reference counting (single-threaded)
- [ ] `Arc<T>`: Atomic reference counting (thread-safe)
- [ ] When to use each

### Interior Mutability
- [ ] `Mutex<T>`: Mutual exclusion
- [ ] `RwLock<T>`: Reader-writer lock
- [ ] `Arc<Mutex<T>>` pattern
- [ ] `Arc<RwLock<T>>` for many readers

### Send + Sync Traits
- [ ] `Send`: Safe to move between threads
- [ ] `Sync`: Safe to share references
- [ ] Compiler enforces automatically

### Channels (Tokio)
- [ ] `tokio::sync::mpsc::channel()`: Multi-producer, single-consumer
- [ ] `tokio::sync::broadcast::channel()`: Multi-producer, multi-consumer
- [ ] `tokio::sync::oneshot::channel()`: One-time message
- [ ] Send: `tx.send(value).await?`
- [ ] Receive: `rx.recv().await`

### Actor Pattern
- [ ] Message passing via channels
- [ ] Single task owns mutable state
- [ ] Other tasks send messages

### Graceful Shutdown
- [ ] `tokio::signal::ctrl_c().await`
- [ ] Broadcast shutdown signal
- [ ] `tokio::select!` to listen for shutdown

**Files:**
- `src/runtime/supervisor.rs` → Spawn and manage tasks
- `src/runtime/channels.rs` → Type aliases for channels
- `src/runtime/shutdown.rs` → Shutdown coordination
- `src/trading/position_manager.rs` → Shared position state
- `src/trading/order_executor.rs` → Actor pattern

**Components:** Multi-task runtime, position manager, order executor

---

## ⚫ Production Quality
**Learn when building:** CLI, logging, database, web server

### Logging (tracing)
- [ ] `#[instrument]` macro
- [x] `info!()`, `warn!()`, `error!()` macros
- [x] Structured logging
- [x] Initialize `tracing_subscriber`

### Configuration (config crate)
- [ ] Load TOML files
- [ ] Environment variable overrides
- [ ] Validation

### Database (rusqlite)
- [ ] `Connection::open("db.sqlite")?`
- [ ] Execute SQL: `conn.execute(...)`
- [ ] Query rows: `conn.prepare(...)?, stmt.query_map(...)`
- [ ] `Arc<Mutex<Connection>>` for async access

### CLI (clap)
- [ ] `#[derive(Parser)]` for CLI struct
- [ ] `#[command(subcommand)]` for subcommands
- [ ] Parse args: `Cli::parse()`

### Web Server (Axum)
- [ ] Define routes: `Router::new().route(...)`
- [ ] Handlers: `async fn handler(Extension(state): Extension<Arc<State>>)`
- [ ] JSON responses: `Json(data)`
- [ ] WebSocket support: `axum::extract::ws::WebSocket`
- [ ] Shared state: `Extension<Arc<AppState>>`

### Testing Strategies
- [ ] Integration tests in `tests/` directory
- [ ] Mock external APIs with `wiremock`
- [ ] `#[tokio::test]` for async tests

### Documentation
- [x] `///` doc comments
- [x] `//!` module-level docs
- [ ] `cargo doc --open`

**Files:**
- `src/utils/logging.rs` → Logging setup
- `src/config/mod.rs` → Config management
- `src/storage/sqlite.rs` → Database integration
- `src/main.rs` → CLI parsing
- `src/web/server.rs` → Web server
- `tests/integration/*.rs` → Integration tests

**Components:** CLI, logging, database, web dashboard, testing

---

## 🔷 Advanced (Optional)
**Learn when needed:** Performance optimization, advanced patterns

### Lifetimes
- [ ] Lifetime annotations: `<'a>`
- [ ] Lifetime elision rules
- [ ] Struct lifetimes

### Advanced Traits
- [ ] Trait objects: `Box<dyn Trait>`
- [ ] Object safety
- [ ] Trait inheritance

### Modules & Visibility
- [x] `mod` keyword
- [x] `pub`, `pub(crate)`, `pub(super)`
- [x] File-based modules
- [x] Re-exports: `pub use`

### Type Aliases
- [ ] `type Result<T> = std::result::Result<T, CalchasError>;`

### Macros (Declarative)
- [ ] `macro_rules!` basics
- [ ] Pattern matching in macros

### Performance
- [ ] Profiling with `cargo flamegraph`
- [ ] Benchmarking with `criterion`
- [ ] Zero-cost abstractions

**Use when:** Optimization needed, complex abstractions required

---

## 🎯 Component → Concept Mapping

Quick reference: What Rust concepts does each component need?

| Component | Concepts Needed |
|-----------|----------------|
| **Data Models** | Structs, enums, newtypes, derives, Option, Result |
| **Strategy Loader** | serde, File I/O, error handling, Result |
| **Kalshi REST Client** | async/await, reqwest, error handling, retry logic |
| **Kalshi WebSocket** | async, tokio-tungstenite, streams, channels |
| **Strategy Engine** | Iterators, closures, RwLock, HashMap |
| **Position Manager** | Arc<RwLock>, async tasks, error handling |
| **Order Executor** | Actor pattern, channels, tokio::select |
| **Runtime Supervisor** | tokio::spawn, shutdown, JoinHandle |
| **SQLite Integration** | rusqlite, Arc<Mutex>, SQL |
| **CLI** | clap, subcommands |
| **Web Server** | Axum, WebSocket, Extension, JSON |
| **Logging** | tracing, structured logging |

---

## 📚 Learning Resources

### Official Docs
- [The Rust Book](https://doc.rust-lang.org/book/) - Start here (Chapters 1-10)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) - Quick reference
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial) - Essential for async
- [serde.rs](https://serde.rs/) - JSON serialization guide

### Interactive
- [Rustlings](https://github.com/rust-lang/rustlings) - Practice exercises
- [Exercism Rust Track](https://exercism.org/tracks/rust) - Mentored exercises

### Video
- [Jon Gjengset's YouTube](https://www.youtube.com/@jonhoo) - Deep dives
- [No Boilerplate](https://www.youtube.com/@NoBoilerplate) - Quick concepts

### Books (Advanced)
- **Programming Rust, 2nd Edition** - Comprehensive reference
- **Rust for Rustaceans** - Advanced patterns

### Community
- [r/rust](https://www.reddit.com/r/rust/) - Reddit community
- [Rust Discord](https://discord.gg/rust-lang) - Real-time help
- [users.rust-lang.org](https://users.rust-lang.org/) - Forums

---

## ✅ Workflow

**Building a component:**
1. Check `PROJECT_STATUS.md` → What am I building?
2. Check `TECHNICAL_ARCHITECTURE.md` → How should it work?
3. Check this file → What Rust concepts do I need?
4. Learn concepts (if not checked off yet)
5. Build the component
6. Check off concepts in this file
7. Check off component in `PROJECT_STATUS.md`

**Example:**
- Building: Strategy loader
- Need: serde, File I/O, error handling
- Learn: Read "The Book" Ch. 9 (Error Handling) + serde docs
- Build: `src/strategy/loader.rs`
- Check off: serde, File I/O, error handling concepts
- Check off: Strategy loader component

---

**Version:** 3.0 (Concept-Based, Product-Driven)
**Last Updated:** December 25, 2024
