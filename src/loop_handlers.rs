//! Loop handler functions for the trading bot
//!
//! This module contains helper functions for each step of the main trading loop.

use crate::app_state::AppState;
use crate::kalshi::{KalshiClient, GetMarketsRequest};
use crate::models::{Market, ExitReason};
use crate::strategy::signals::{EntrySignal, SignalSide};
use crate::trading::{RiskDecision, TradingError, OrderbookProvider};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::sync::Arc;

/// Fetch active markets from Kalshi (for entry scanning when we have capacity)
///
/// Paginates through ALL active markets to maximize opportunities.
/// Fetches up to 10 pages (10,000 markets) for maximum coverage.
///
/// # Arguments
///
/// * `kalshi_client` - Kalshi API client
/// * `min_time_minutes` - Minimum time to event in minutes (from strategy config)
/// * `max_time_minutes` - Maximum time to event in minutes (from strategy config)
///
/// # Returns
///
/// Vector of active markets
pub async fn fetch_all_markets(
    kalshi_client: &Arc<KalshiClient>,
    min_time_minutes: u32,
    max_time_minutes: u32,
) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
    use chrono::Utc;
    let now = Utc::now();
    tracing::info!("Scanning markets at {}", now.format("%H:%M:%S"));

    // Use strategy-configured time window
    let min_close = now + chrono::Duration::minutes(min_time_minutes as i64);
    let max_close = now + chrono::Duration::minutes(max_time_minutes as i64);

    let mut all_markets = Vec::new();
    let mut cursor: Option<String> = None;
    let max_pages = 10;  // Fetch up to 10,000 markets for maximum coverage
    let mut page_count = 0;

    loop {
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: None,
            min_close_ts: Some(min_close.timestamp()),
            max_close_ts: Some(max_close.timestamp()),
        };

        let response = kalshi_client.get_markets(request).await?;

        let batch: Vec<Market> = response.markets
            .into_iter()
            .map(|km| km.into())
            .collect();

        let batch_size = batch.len();
        tracing::info!("  Page {}: {} markets", page_count + 1, batch_size);
        all_markets.extend(batch);
        page_count += 1;

        // Stop conditions
        if batch_size == 0 || page_count >= max_pages {
            break;
        }

        // Get next cursor
        if let Some(next_cursor) = response.cursor {
            if !next_cursor.is_empty() {
                cursor = Some(next_cursor);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    tracing::info!("  Total: {} markets from {} pages", all_markets.len(), page_count);
    Ok(all_markets)
}

/// Fetch specific markets by ID (for updating existing positions)
///
/// Uses very broad time window and paginates to find position markets.
///
/// # Arguments
///
/// * `kalshi_client` - Kalshi API client
/// * `market_ids` - Market IDs to fetch
/// * `min_time_minutes` - Minimum time to event in minutes (from strategy config)
/// * `max_time_minutes` - Maximum time to event in minutes (from strategy config)
///
/// # Returns
///
/// Vector of markets (may be smaller than input if some markets closed)
pub async fn fetch_markets_by_ids(
    kalshi_client: &Arc<KalshiClient>,
    market_ids: &[crate::models::MarketId],
    min_time_minutes: u32,
    max_time_minutes: u32,
) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
    use chrono::Utc;
    let now = Utc::now();
    tracing::info!("Fetching {} position markets...", market_ids.len());

    // Use same time window as fetch_all_markets for consistency
    let min_close = now + chrono::Duration::minutes(min_time_minutes as i64);
    let max_close = now + chrono::Duration::minutes(max_time_minutes as i64);

    tracing::debug!("  Time window: {} to {}",
        min_close.format("%Y-%m-%d %H:%M"),
        max_close.format("%Y-%m-%d %H:%M")
    );

    let market_id_set: std::collections::HashSet<_> = market_ids.iter().cloned().collect();
    let mut found_markets = Vec::new();
    let mut cursor: Option<String> = None;
    let max_pages = 30;  // Search up to 30k markets (supports up to 1000 positions)
    let mut page_count = 0;

    loop {
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: None,
            min_close_ts: Some(min_close.timestamp()),
            max_close_ts: Some(max_close.timestamp()),
        };

        let response = kalshi_client.get_markets(request).await?;

        let total_in_batch = response.markets.len();
        tracing::debug!("    Page {}: API returned {} markets", page_count + 1, total_in_batch);

        // If API returns 0 markets, stop early
        if total_in_batch == 0 {
            tracing::warn!("    API returned 0 markets - time window might be wrong!");
            break;
        }

        // Filter as we fetch
        let batch: Vec<Market> = response.markets
            .into_iter()
            .map(|km| Into::<Market>::into(km))
            .filter(|m| market_id_set.contains(&m.id))
            .collect();

        if !batch.is_empty() {
            tracing::debug!("    -> Matched {} of our position markets!", batch.len());
        }
        found_markets.extend(batch);
        page_count += 1;

        // Stop if found all or hit limit
        if found_markets.len() >= market_ids.len() || page_count >= max_pages {
            break;
        }

        if let Some(next_cursor) = response.cursor {
            if !next_cursor.is_empty() {
                cursor = Some(next_cursor);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    tracing::info!("  Found {}/{} position markets", found_markets.len(), market_ids.len());
    Ok(found_markets)
}

/// Evaluate strategies against markets to generate entry signals
///
/// Note: Prices are recorded in the main loop before calling this function.
///
/// # Arguments
///
/// * `state` - Application state (read-only access)
/// * `markets` - Markets to evaluate
///
/// # Returns
///
/// Vector of (EntrySignal, Market) tuples with market data attached
pub fn evaluate_strategies(
    state: &AppState,
    markets: &[Market],
) -> Vec<(EntrySignal, Market)> {
    let mut signal_market_pairs = Vec::new();

    // Build market lookup map
    let market_map: std::collections::HashMap<_, _> = markets
        .iter()
        .map(|m| (m.id.clone(), m.clone()))
        .collect();

    for strategy in state.strategies.values() {
        tracing::info!("Evaluating strategy: {} against {} markets", strategy.name, markets.len());

        match crate::strategy::evaluator::StrategyEvaluator::evaluate(markets, strategy, Some(&state.price_tracker)) {
            Ok(strategy_signals) => {
                tracing::info!("  Generated {} signals from {}", strategy_signals.len(), strategy.name);
                // Attach market data to each signal
                for signal in strategy_signals {
                    if let Some(market) = market_map.get(&signal.market_id) {
                        signal_market_pairs.push((signal, market.clone()));
                    }
                }
            }
            Err(e) => tracing::warn!("Failed to evaluate strategy {}: {:?}", strategy.id.0, e),
        }
    }

    if signal_market_pairs.is_empty() {
        tracing::warn!("⚠️  No signals generated from {} markets", markets.len());
    }
    signal_market_pairs
}

/// Check orderbook for acceptable spread and liquidity
///
/// # Arguments
///
/// * `orderbook_provider` - Provider for fetching orderbook data
/// * `signal` - Entry signal being evaluated
/// * `max_spread` - Maximum spread in cents (optional)
/// * `min_quantity` - Minimum liquidity required (optional)
///
/// # Returns
///
/// Ok(true) if orderbook passes checks or checks are disabled
/// Ok(false) if orderbook fails checks
async fn check_orderbook_acceptable(
    orderbook_provider: &impl OrderbookProvider,
    signal: &EntrySignal,
    max_spread: Option<Decimal>,
    min_quantity: Option<u64>,
) -> Result<bool, Box<dyn std::error::Error>> {
    // If no orderbook filters configured, pass
    if max_spread.is_none() && min_quantity.is_none() {
        return Ok(true);
    }

    // Fetch orderbook
    let orderbook = match orderbook_provider.get_orderbook(&signal.market_id).await? {
        Some(ob) => ob,
        None => {
            tracing::warn!("  Orderbook not available for {}", signal.market_id.as_str());
            return Ok(true); // Allow if orderbook unavailable (simulation fallback)
        }
    };

    // Check spread
    if let Some(max_spread_cents) = max_spread {
        if let Some(spread) = orderbook.spread() {
            if spread > max_spread_cents {
                tracing::warn!(
                    "  ✗ Spread too wide: ${:.2} > ${:.2}",
                    spread,
                    max_spread_cents
                );
                return Ok(false);
            }
        }
    }

    // Check liquidity (side-specific)
    if let Some(min_qty) = min_quantity {
        let available_quantity = match signal.side {
            SignalSide::Yes => orderbook.yes_best_ask_quantity(),
            SignalSide::No => orderbook.no_best_ask_quantity(),
        };

        if available_quantity < min_qty {
            tracing::warn!(
                "  ✗ Insufficient liquidity: {} contracts < {} required",
                available_quantity,
                min_qty
            );
            return Ok(false);
        }
    }

    tracing::debug!("  ✓ Orderbook check passed for {}", signal.market_id.as_str());
    Ok(true)
}

/// Process an entry signal (orderbook check → risk check → execute → open position)
///
/// # Arguments
///
/// * `state` - Application state (mutable)
/// * `signal` - Entry signal to process
///
/// # Returns
///
/// Ok(()) if position opened, Err if rejected or failed
pub async fn process_entry_signal(
    state: &mut AppState,
    signal: EntrySignal,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get strategy for this signal
    let strategy = state.strategies.get(&signal.strategy_id)
        .ok_or("Strategy not found for signal")?;

    // Orderbook check (if configured in strategy)
    let orderbook_ok = check_orderbook_acceptable(
        &state.orderbook_provider,
        &signal,
        strategy.filters.max_spread_cents,
        strategy.filters.min_best_price_quantity,
    ).await?;

    if !orderbook_ok {
        tracing::warn!("  Orderbook check failed for {}", signal.market_id.as_str());
        return Ok(()); // Not an error, just rejected
    }

    // Risk check
    let risk_decision = state.risk_manager.check_entry(&signal, &state.positions, strategy);
    match risk_decision {
        RiskDecision::Approved => {
            tracing::info!(
                "✓ Risk check APPROVED for {} ({:?})",
                signal.market_id.0,
                signal.side
            );
        }
        RiskDecision::Rejected(reason) => {
            tracing::debug!(
                "✗ Risk check REJECTED for {} ({:?})",
                signal.market_id.0,
                reason
            );
            return Ok(()); // Not an error, just rejected
        }
    }

    // Execute entry order (simulated)
    // For Phase 4: Use signal's recommended price directly to avoid slow API calls
    // In Phase 6, we'll use real-time WebSocket prices
    let filled_order = {
        // Lock the executor (not using it in Phase 4, but keep lock for consistency)
        let _executor = state.order_executor.lock()
            .map_err(|_| TradingError::LockError)?;

        // Create order from signal (without executing through simulator)
        use crate::models::{Order, OrderId, OrderSide, OrderAction, OrderType};
        use rust_decimal::Decimal;
        use rust_decimal_macros::dec;

        let order_id = OrderId::new(format!("sim_{}", uuid::Uuid::new_v4()));
        let side = match signal.side {
            crate::strategy::signals::SignalSide::Yes => OrderSide::Yes,
            crate::strategy::signals::SignalSide::No => OrderSide::No,
        };

        // Calculate actual position size based on position_size_unit
        let contracts = match strategy.entry_rules.position_size_unit {
            crate::models::strategy::PositionSizeUnit::Contracts => {
                // Use position_size directly
                signal.position_size
            }
            crate::models::strategy::PositionSizeUnit::Dollars => {
                // Calculate contracts from dollar amount including fees
                let dollar_amount = Decimal::from(signal.position_size);

                // Determine fee based on order type
                let fee_per_contract = match signal.order_type {
                    crate::models::strategy::OrderType::Market => dec!(0.007),  // Taker fee
                    crate::models::strategy::OrderType::Limit => dec!(-0.001),  // Maker rebate
                };

                // Total cost per contract = price + fee
                let cost_per_contract = signal.recommended_price + fee_per_contract;

                // Calculate contracts: dollar_amount / cost_per_contract
                let contracts_decimal = dollar_amount / cost_per_contract;

                // Round down to get whole contracts
                contracts_decimal.floor().to_u64().unwrap_or(0)
            }
        };

        let mut order = Order::new(
            order_id,
            signal.market_id.clone(),
            None,  // position_id (will be set by PositionManager)
            side,
            OrderAction::Buy,
            OrderType::Market,
            None,  // limit_price (market order)
            contracts,
        );

        // Simulate instant fill at recommended price
        order.update_fill(contracts, signal.recommended_price);

        order
    };

    // Open position
    let position_id = state.position_manager.open_position(filled_order.clone(), strategy)?;

    // Store position in AppState
    if let Some(position) = state.position_manager.get_position(&position_id) {
        state.positions.insert(position_id.clone(), position.clone());

        let side_str = match position.side {
            crate::models::PositionSide::Yes => "YES",
            crate::models::PositionSide::No => "NO",
        };

        tracing::info!(
            "✓ POSITION OPENED: {} ({} side) @ ${:.2} (qty: {}) | TP: ${:.2} | SL: ${:.2}",
            signal.market_ticker,
            side_str,
            position.entry_price,
            position.quantity,
            position.exit_target.take_profit_price.unwrap_or_default(),
            position.exit_target.stop_loss_price.unwrap_or_default()
        );
    }

    Ok(())
}

/// Update position prices and check for exits
///
/// # Arguments
///
/// * `state` - Application state (mutable)
/// * `markets` - Current market data for price updates
///
/// # Returns
///
/// Ok(()) if successful
pub async fn update_and_check_positions(
    state: &mut AppState,
    markets: &[Market],
) -> Result<(), Box<dyn std::error::Error>> {
    if state.positions.is_empty() {
        return Ok(());
    }

    tracing::info!("");
    tracing::info!("╔═══════════════════════════════════════════════════════════════╗");
    tracing::info!("║  POSITION UPDATES ({} active)                                   ", state.positions.len());
    tracing::info!("╚═══════════════════════════════════════════════════════════════╝");

    // Build market lookup map for efficient price updates
    let market_map: std::collections::HashMap<_, _> = markets
        .iter()
        .map(|m| (m.id.clone(), m))
        .collect();

    // Get positions to check (need to collect to avoid borrow issues)
    let position_ids: Vec<_> = state.positions.keys().cloned().collect();

    for position_id in position_ids {
        // Get position (safely)
        let position = match state.positions.get(&position_id) {
            Some(p) => p.clone(),
            None => continue,
        };

        // Find current market data
        tracing::debug!("Looking for market {} in market_map", position.market_id.0);
        let market = match market_map.get(&position.market_id) {
            Some(m) => m,
            None => {
                tracing::warn!(
                    "⚠️  Market {} not found in current data (map has {} markets), skipping price update for position {}",
                    position.market_id.0,
                    market_map.len(),
                    position_id.0
                );
                continue;
            }
        };

        // Get current price based on position side
        // In simulation, we use the market's current price
        // (In production, we'd use bid for exits to account for spread)
        let current_price = match position.side {
            crate::models::PositionSide::Yes => market.yes_price,  // Current Yes price
            crate::models::PositionSide::No => market.no_price,    // Current No price
        };

        tracing::debug!(
            "Market {} found - Entry: ${:.2}, Current: ${:.2}, Old position price: ${:.2}",
            position.market_id.0,
            position.entry_price,
            current_price,
            position.current_price
        );

        // Update position with current price
        let updated_position = {
            let mut updated = position.clone();
            updated.update_price(current_price);
            updated
        };

        let side_str = match position.side {
            crate::models::PositionSide::Yes => "YES",
            crate::models::PositionSide::No => "NO ",
        };

        let price_change = current_price - position.current_price;
        let change_symbol = if price_change > rust_decimal::Decimal::ZERO {
            "↑"
        } else if price_change < rust_decimal::Decimal::ZERO {
            "↓"
        } else {
            "→"
        };

        let pnl_color = if updated_position.unrealized_pnl > rust_decimal::Decimal::ZERO {
            "+"
        } else {
            ""
        };

        tracing::info!(
            "  [{}/{}] {} | Entry ${:.2} {} ${:.2} | P&L: {}{:.2} | TP: ${:.2} SL: ${:.2}",
            side_str,
            position.quantity,
            market.ticker,
            position.entry_price,
            change_symbol,
            current_price,
            pnl_color,
            updated_position.unrealized_pnl,
            position.exit_target.take_profit_price.unwrap_or_default(),
            position.exit_target.stop_loss_price.unwrap_or_default()
        );

        // Always update the position in AppState with new price
        state.positions.insert(position_id.clone(), updated_position.clone());

        // Also update in PositionManager (it has its own HashMap)
        if let Some(pm_position) = state.position_manager.get_position_mut(&position_id) {
            pm_position.update_price(updated_position.current_price);
        }

        // Check if exit condition met
        if state.exit_manager.should_exit(&updated_position) {
            if let Some(exit_reason) = state.exit_manager.determine_exit_reason(&updated_position) {
                tracing::info!(
                    "✓ EXIT TRIGGERED: {} ({:?}, P&L: ${:.2})",
                    position_id.0,
                    exit_reason,
                    updated_position.unrealized_pnl
                );

                // Execute exit
                match execute_exit(state, &position_id, exit_reason).await {
                    Ok(()) => {
                        // Position closed successfully
                        state.positions.remove(&position_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to execute exit for {}: {}", position_id.0, e);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Execute exit for a position
///
/// # Arguments
///
/// * `state` - Application state
/// * `position_id` - Position to exit
/// * `exit_reason` - Why we're exiting
///
/// # Returns
///
/// Ok(()) if successful
async fn execute_exit(
    state: &mut AppState,
    position_id: &crate::models::PositionId,
    exit_reason: ExitReason,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::models::{Order, OrderId, OrderSide, OrderAction, OrderType};
    use chrono::Utc;

    // Get position from PositionManager (it has the updated price)
    let position = state.position_manager.get_position(position_id)
        .ok_or("Position not found")?
        .clone();

    // Create exit order directly (bypass slow simulator market fetch)
    // We already have current price from the position update loop
    let order_id = OrderId::new(format!("sim_exit_{}", uuid::Uuid::new_v4()));

    // Convert PositionSide to OrderSide
    let order_side = match position.side {
        crate::models::PositionSide::Yes => OrderSide::Yes,
        crate::models::PositionSide::No => OrderSide::No,
    };

    let mut exit_order = Order::new(
        order_id,
        position.market_id.clone(),
        Some(position_id.clone()),
        order_side,
        OrderAction::Sell,
        OrderType::Market,
        None,  // market order
        position.quantity,
    );

    // Simulate instant fill at current price (we already updated it in the main loop)
    exit_order.update_fill(position.quantity, position.current_price);

    // Get exit price
    let exit_price = exit_order.average_fill_price
        .ok_or("Exit order not filled")?;

    // Get mutable position to update it
    let pm_position = state.position_manager.get_position_mut(position_id)
        .ok_or("Position not found in manager")?;

    // Mark position as closed
    pm_position.mark_exit_pending(exit_order.id.clone());
    pm_position.mark_closed();

    // Calculate fees (taker fees for simulation)
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    let entry_fees = (dec!(0.007) * pm_position.entry_price) * Decimal::from(pm_position.quantity);
    let exit_fees = (dec!(0.007) * exit_price) * Decimal::from(pm_position.quantity);
    let total_fees = entry_fees + exit_fees;

    // Create trade record using Trade::new()
    let trade = crate::models::Trade::new(
        position_id.clone(),
        pm_position.market_id.clone(),
        pm_position.strategy_id.clone(),
        pm_position.entry_order_id.clone(),
        pm_position.entry_price,
        pm_position.quantity,
        pm_position.entry_timestamp,
        exit_order.id.clone(),
        exit_price,
        pm_position.quantity,
        Utc::now(),
        exit_reason,
        total_fees,
    );

    tracing::info!(
        "✓ Position closed: {} (Net P&L: ${:.2}, Return: {:.2}%)",
        position_id.0,
        trade.net_pnl,
        trade.return_pct
    );

    // Record trade in metrics tracker
    state.metrics_tracker.record_trade(&trade);

    // Debug: Check metrics after recording
    let metrics_after = state.metrics_tracker.calculate_metrics();
    tracing::info!(
        "📊 Metrics after trade: Trades={}, Net P&L=${:.4}",
        metrics_after.total_trades,
        metrics_after.net_pnl
    );

    Ok(())
}

/// Print current status
///
/// # Arguments
///
/// * `state` - Application state
pub fn print_status(state: &AppState) {
    let active_positions = state.positions.len();
    let metrics = state.metrics_tracker.calculate_metrics();

    // Calculate total unrealized P&L from open positions
    use rust_decimal::Decimal;
    let total_unrealized_pnl: Decimal = state.positions.values()
        .map(|p| p.unrealized_pnl)
        .sum();

    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("  Positions: {} open | Unrealized P&L: ${:.2} | Trades: {} | Win Rate: {:.1}% | Realized P&L: ${:.2}",
        active_positions,
        total_unrealized_pnl,
        metrics.total_trades,
        metrics.win_rate,
        metrics.net_pnl
    );
    tracing::info!("═══════════════════════════════════════════════════════════════");
}
