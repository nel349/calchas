# What's Next: Runtime Integration (Phase 4 Final Components)

## 📊 Current Status

**Phase 4 Progress:** 79% complete (11/14 components)

**✅ Complete:**
1. Order Simulator
2. Risk Manager
3. Exit Manager
4. Order Executor
5. Position Manager
6. Metrics Tracker
7. Price Tracker
8. Volume Tracker (Phase 1)
9. Order Flow Tracker (Phase 2)
10. Filter Integration
11. **LIVE Game Prioritization** ← Just finished!

**❌ Still Need:**
12. **MAIN APP** (`src/main.rs`) - Entry point, supervisor pattern
13. **Channels** (`src/runtime/channels.rs`) - Communication setup
14. **Tasks** (`src/runtime/tasks/`) - 4 concurrent async tasks

---

## 🎯 Next: Build Runtime Integration

### Step 1: Channels (Communication Layer)

**File:** `src/runtime/channels.rs`

**What:** Define communication channels between tasks

**Channel Types:**
```rust
pub struct Channels {
    // WebSocket → Strategy + Position tasks
    pub price_updates: broadcast::Sender<PriceUpdate>,

    // Strategy → Executor task
    pub entry_signals: mpsc::Sender<EntrySignal>,

    // Position → Executor task
    pub exit_commands: mpsc::Sender<ExitCommand>,

    // Supervisor → All tasks
    pub shutdown: broadcast::Sender<()>,
}
```

**Reference:** TECHNICAL_ARCHITECTURE.md Section 7.3

---

### Step 2: Tasks (Concurrent Workers)

**Directory:** `src/runtime/tasks/`

**Files to Create:**
1. `websocket_task.rs` - Receive Kalshi price updates → broadcast
2. `strategy_task.rs` - Evaluate markets → send entry signals
3. `position_task.rs` - Monitor exits → send exit commands
4. `executor_task.rs` - Execute orders, manage positions

**Reference:** TECHNICAL_ARCHITECTURE.md Section 7.2

---

### Step 3: Main App (Supervisor)

**File:** `src/main.rs`

**What:** Orchestrate everything

**Flow:**
```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 1. Load config
    let config = AppConfig::load("config/default.toml")?;

    // 2. Initialize shared state
    let kalshi_client = Arc::new(KalshiClient::new(&config.kalshi));
    let strategies = Arc::new(RwLock::new(load_strategies("strategies/")?));
    let positions = Arc::new(RwLock::new(HashMap::new()));

    // 3. Create channels
    let channels = Channels::new();

    // 4. Spawn tasks
    let ws_handle = tokio::spawn(websocket_task(...));
    let strategy_handle = tokio::spawn(strategy_task(...));
    let position_handle = tokio::spawn(position_task(...));
    let executor_handle = tokio::spawn(executor_task(...));

    // 5. Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    // 6. Graceful shutdown
    channels.shutdown.send(())?;
    ws_handle.await?;
    strategy_handle.await?;
    position_handle.await?;
    executor_handle.await?;

    Ok(())
}
```

**Reference:** TECHNICAL_ARCHITECTURE.md Section 7.1

---

## 📋 Implementation Checklist

### Phase 4 Final Push (3 components)

**Component 12: Channels** (Est: 1-2 hours)
- [ ] Create `src/runtime/channels.rs`
- [ ] Define channel types (broadcast, mpsc)
- [ ] Set buffer sizes (price: 1000, signals: 100)
- [ ] Write basic tests (send/receive)

**Component 13: Tasks** (Est: 4-6 hours)
- [ ] Create `src/runtime/tasks/mod.rs`
- [ ] Implement `websocket_task.rs`
- [ ] Implement `strategy_task.rs`
- [ ] Implement `position_task.rs`
- [ ] Implement `executor_task.rs`
- [ ] Wire up all channel communication

**Component 14: Main App** (Est: 2-3 hours)
- [ ] Update `src/main.rs` with supervisor pattern
- [ ] Add shutdown signal handling (Ctrl+C)
- [ ] Test full end-to-end flow
- [ ] Run simulation mode test

---

## 🎉 Phase 4 Milestone

**When these 3 components are done:**

```bash
$ cargo run
```

**Expected behavior:**
1. Bot starts, loads strategies
2. Connects to Kalshi WebSocket
3. Receives price updates
4. Evaluates strategies (with LIVE prioritization!)
5. Opens simulated position
6. Monitors exit conditions
7. Closes position profitably
8. Logs all actions

**Success criteria:**
- ✅ Bot runs without crashing
- ✅ Opens at least 1 position
- ✅ Hits exit target (take profit or stop loss)
- ✅ Closes position successfully
- ✅ All logs show correct flow

---

## 📖 Key References

**Architecture:**
- `docs/TECHNICAL_ARCHITECTURE.md` Section 7 - Concurrency Model (THE FULL DESIGN)

**Examples:**
- Section 7.1: Main function flow
- Section 7.2: Task breakdown (4 tasks)
- Section 7.3: Channel types & usage

**Data Flow:**
- Section 8.1: Entry flow (opening position)
- Section 8.2: Exit flow (closing position)

---

## ⚡ Why This Matters

**All the trading logic is DONE:**
- Risk management ✅
- Exit conditions ✅
- Order execution ✅
- Position tracking ✅
- Indicators (volume, order flow) ✅
- LIVE game prioritization ✅

**What's missing:** Wiring it all together!

The runtime integration is the **glue** that makes everything work as a cohesive system.

---

## 🚀 Next Steps

1. **Read TECHNICAL_ARCHITECTURE.md Section 7** (THE DESIGN ALREADY EXISTS)
2. **Start with channels** (simplest component)
3. **Build tasks one by one** (websocket → strategy → position → executor)
4. **Wire up main.rs** (supervisor pattern)
5. **Test end-to-end** (cargo run)

**Estimated time:** 8-12 hours of focused work

**Reward:** A FULLY WORKING TRADING BOT! 🎉
