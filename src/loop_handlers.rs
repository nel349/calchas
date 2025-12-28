//! Loop handler functions for the trading bot
//!
//! This module contains helper functions for each step of the main trading loop.

use crate::app_state::AppState;
use crate::kalshi::{KalshiClient, GetMarketsRequest};
use crate::models::{Market, ExitReason};
use crate::strategy::signals::EntrySignal;
use crate::trading::{RiskDecision, TradingError};
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;

/// Fetch active markets from Kalshi (for entry scanning when we have capacity)
///
/// Paginates through markets in 7-day window to find opportunities.
/// Fetches up to 5000 markets (5 pages) to ensure variety.
///
/// # Arguments
///
/// * `kalshi_client` - Kalshi API client
///
/// # Returns
///
/// Vector of active markets
pub async fn fetch_all_markets(
    kalshi_client: &Arc<KalshiClient>,
) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
    use chrono::Utc;
    let now = Utc::now();
    tracing::info!("Scanning markets at {}", now.format("%H:%M:%S"));

    // Fetch markets closing in next 7 days
    let min_close = now + chrono::Duration::minutes(30);
    let max_close = now + chrono::Duration::days(7);

    let mut all_markets = Vec::new();
    let mut cursor: Option<String> = None;
    let max_pages = 5;  // Fetch up to 5000 markets
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
/// Uses the same 7-day window as scanning and filters client-side.
///
/// # Arguments
///
/// * `kalshi_client` - Kalshi API client
/// * `market_ids` - Market IDs to fetch
///
/// # Returns
///
/// Vector of markets (may be smaller than input if some markets closed)
pub async fn fetch_markets_by_ids(
    kalshi_client: &Arc<KalshiClient>,
    market_ids: &[crate::models::MarketId],
) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
    use chrono::Utc;
    let now = Utc::now();
    tracing::info!("Fetching {} position markets at {}", market_ids.len(), now.format("%H:%M:%S"));

    // Use broad time window (positions could be in markets closing anytime)
    let min_close = now + chrono::Duration::minutes(30);
    let max_close = now + chrono::Duration::days(7);

    let request = GetMarketsRequest {
        limit: Some(1000),
        cursor: None,
        status: Some("open".to_string()),
        series_ticker: None,
        min_close_ts: Some(min_close.timestamp()),
        max_close_ts: Some(max_close.timestamp()),
    };

    let response = kalshi_client.get_markets(request).await?;

    // Filter to only our position markets
    let market_id_set: std::collections::HashSet<_> = market_ids.iter().collect();
    let found: Vec<Market> = response.markets
        .into_iter()
        .map(|km| Into::<Market>::into(km))
        .filter(|m| market_id_set.contains(&m.id))
        .collect();

    tracing::info!("  Found {}/{} position markets", found.len(), market_ids.len());
    Ok(found)
}

/// Evaluate strategies against markets to generate entry signals
///
/// # Arguments
///
/// * `state` - Application state
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

        match crate::strategy::evaluator::StrategyEvaluator::evaluate(markets, strategy) {
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

/// Process an entry signal (risk check → execute → open position)
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
            tracing::warn!(
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

    tracing::info!("--- Price Updates ({} positions) ---", state.positions.len());

    // Build market lookup map for efficient price updates
    let market_map: std::collections::HashMap<_, _> = markets
        .iter()
        .map(|m| (m.id.clone(), m))
        .collect();

    // Get positions to check (need to collect to avoid borrow issues)
    let position_ids: Vec<_> = state.positions.keys().cloned().collect();

    tracing::info!("Checking {} markets in market_map", market_map.len());

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
            crate::models::PositionSide::No => "NO",
        };

        let price_change = current_price - position.current_price;
        let change_symbol = if price_change > rust_decimal::Decimal::ZERO {
            "↑"
        } else if price_change < rust_decimal::Decimal::ZERO {
            "↓"
        } else {
            "="
        };

        tracing::info!(
            "  {} ({}) Entry:${:.4} → Current:${:.4} {} = P&L:${:.2} (TP:${:.4}, SL:${:.4})",
            market.ticker,
            side_str,
            position.entry_price,
            current_price,
            change_symbol,
            updated_position.unrealized_pnl,
            position.exit_target.take_profit_price.unwrap_or_default(),
            position.exit_target.stop_loss_price.unwrap_or_default()
        );

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
        } else {
            // No exit, just update the position in state
            state.positions.insert(position_id, updated_position);
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
    _exit_reason: ExitReason,  // Not used - close_position determines it internally
) -> Result<(), Box<dyn std::error::Error>> {
    // Close position via position manager
    let trade = state.position_manager.close_position(position_id).await?;

    tracing::info!(
        "✓ Position closed: {} (Net P&L: ${:.2}, Return: {:.2}%)",
        position_id.0,
        trade.net_pnl,
        trade.return_pct
    );

    // Record trade in metrics tracker
    state.metrics_tracker.record_trade(&trade);

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

    tracing::info!(
        "═══ Status: {} open positions | {} trades completed | Win: {:.1}% | ROI: {:.2}% ═══",
        active_positions,
        metrics.total_trades,
        metrics.win_rate,
        metrics.net_roi
    );
}
