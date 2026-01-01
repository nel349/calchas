-- Phase 5: Database Persistence - Initial Schema
-- Trades table (immutable historical record)
CREATE TABLE trades (
    id TEXT PRIMARY KEY,
    position_id TEXT NOT NULL,
    market_id TEXT NOT NULL,
    strategy_id TEXT NOT NULL,

    -- Entry details
    entry_order_id TEXT NOT NULL,
    entry_price TEXT NOT NULL,        -- Store Decimal as TEXT to preserve precision
    entry_quantity INTEGER NOT NULL,
    entry_timestamp TEXT NOT NULL,    -- ISO 8601 format

    -- Exit details
    exit_order_id TEXT NOT NULL,
    exit_price TEXT NOT NULL,
    exit_quantity INTEGER NOT NULL,
    exit_timestamp TEXT NOT NULL,
    exit_reason TEXT NOT NULL,        -- "TakeProfit", "StopLoss", etc.

    -- Performance metrics
    gross_pnl TEXT NOT NULL,
    fees TEXT NOT NULL,
    net_pnl TEXT NOT NULL,
    return_pct TEXT NOT NULL,
    hold_duration_seconds INTEGER NOT NULL,

    -- Metadata
    notes TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Index for analytics queries
CREATE INDEX idx_trades_strategy_exit ON trades(strategy_id, exit_timestamp);
CREATE INDEX idx_trades_exit_reason ON trades(exit_reason);
CREATE INDEX idx_trades_date ON trades(DATE(exit_timestamp));
CREATE INDEX idx_trades_strategy_date ON trades(strategy_id, DATE(exit_timestamp));

-- Daily performance aggregates
CREATE TABLE daily_stats (
    date TEXT NOT NULL,
    strategy_id TEXT NOT NULL,
    trade_count INTEGER NOT NULL DEFAULT 0,
    wins INTEGER NOT NULL DEFAULT 0,
    losses INTEGER NOT NULL DEFAULT 0,
    total_win_amount TEXT NOT NULL,     -- Decimal as TEXT
    total_loss_amount TEXT NOT NULL,
    net_pnl TEXT NOT NULL,
    is_profitable INTEGER NOT NULL,     -- Boolean as 0/1
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (date, strategy_id)
);

-- Index for time-series queries
CREATE INDEX idx_daily_stats_strategy_date ON daily_stats(strategy_id, date);

-- Open positions (for crash recovery)
CREATE TABLE positions (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL,
    strategy_id TEXT NOT NULL,

    -- Entry details
    side TEXT NOT NULL,               -- "Yes" or "No"
    entry_price TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    entry_timestamp TEXT NOT NULL,
    entry_order_id TEXT NOT NULL,

    -- Current state
    current_price TEXT NOT NULL,
    unrealized_pnl TEXT NOT NULL,
    peak_pnl TEXT NOT NULL,

    -- Exit targets
    take_profit_price TEXT,
    stop_loss_price TEXT,
    trailing_stop_distance TEXT,
    trailing_stop_activation_pct TEXT,
    expiry_time TEXT,

    -- Status
    status TEXT NOT NULL,             -- "Active", "ExitPending", "Closed"
    exit_order_id TEXT,

    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_positions_status ON positions(status);
CREATE INDEX idx_positions_market ON positions(market_id);

-- Markets cache (optional - for offline analytics)
CREATE TABLE markets (
    id TEXT PRIMARY KEY,
    ticker TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    event_ticker TEXT NOT NULL,
    category TEXT NOT NULL,
    sub_category TEXT,
    status TEXT NOT NULL,

    -- Pricing
    yes_price TEXT NOT NULL,
    no_price TEXT NOT NULL,
    yes_bid TEXT NOT NULL,
    yes_ask TEXT NOT NULL,
    no_bid TEXT NOT NULL,
    no_ask TEXT NOT NULL,

    -- Liquidity
    volume INTEGER NOT NULL,
    volume_24h INTEGER NOT NULL,
    open_interest INTEGER NOT NULL,

    -- Timing
    event_time TEXT NOT NULL,
    close_time TEXT NOT NULL,

    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_markets_ticker ON markets(ticker);
CREATE INDEX idx_markets_status ON markets(status);
