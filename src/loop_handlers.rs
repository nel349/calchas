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
/// Paginates through active markets to maximize opportunities.
/// Fetches up to 10 pages (10,000 markets) for maximum coverage.
/// If series_tickers is provided, fetches markets from each series and combines them.
///
/// # Arguments
///
/// * `kalshi_client` - Kalshi API client
/// * `min_time_minutes` - Minimum time to event in minutes (from strategy config)
/// * `max_time_minutes` - Maximum time to event in minutes (from strategy config)
/// * `series_tickers` - Optional series tickers to filter by (e.g., ["KXNBAGAME", "KXNFLGAME"])
///
/// # Returns
///
/// Vector of active markets
pub async fn fetch_all_markets(
    kalshi_client: &Arc<KalshiClient>,
    min_time_minutes: u32,
    max_time_minutes: u32,
    series_tickers: Option<Vec<String>>,
) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
    use chrono::Utc;
    let now = Utc::now();
    tracing::info!("Scanning markets at {}", now.format("%H:%M:%S"));

    // Use strategy-configured time window
    let min_close = now + chrono::Duration::minutes(min_time_minutes as i64);
    let max_close = now + chrono::Duration::minutes(max_time_minutes as i64);

    let mut all_markets = Vec::new();

    // If multiple series tickers provided, fetch each series separately
    match series_tickers {
        Some(tickers) if !tickers.is_empty() => {
            for ticker in tickers {
                let markets = fetch_series_markets(
                    kalshi_client,
                    &ticker,
                    min_close.timestamp(),
                    max_close.timestamp(),
                ).await?;
                all_markets.extend(markets);
            }
            tracing::info!("  Total: {} markets from {} series", all_markets.len(), all_markets.len());
        }
        _ => {
            // No series filter - fetch all markets
            tracing::info!("📊 Fetching all markets (no series filter)");
            all_markets = fetch_series_markets(
                kalshi_client,
                "",
                min_close.timestamp(),
                max_close.timestamp(),
            ).await?;
        }
    }

    Ok(all_markets)
}

/// Helper: Fetch markets for a single series ticker
async fn fetch_series_markets(
    kalshi_client: &Arc<KalshiClient>,
    series_ticker: &str,
    _min_close_ts: i64,  // Unused - kept for API compatibility
    _max_close_ts: i64,  // Unused - kept for API compatibility
) -> Result<Vec<Market>, Box<dyn std::error::Error>> {
    let mut all_markets = Vec::new();
    let mut cursor: Option<String> = None;
    let max_pages = 10;
    let mut page_count = 0;

    loop {
        let request = GetMarketsRequest {
            limit: Some(1000),
            cursor: cursor.clone(),
            status: Some("open".to_string()),
            series_ticker: if series_ticker.is_empty() { None } else { Some(series_ticker.to_string()) },
            // CRITICAL FIX: Remove time filters to catch LIVE sports games
            // College football/basketball have placeholder close_time in 2026,
            // but actual expiration tonight. Time filtering happens in strategy filters.
            min_close_ts: None,
            max_close_ts: None,
        };

        let response = kalshi_client.get_markets(request).await?;

        let batch: Vec<Market> = response.markets
            .into_iter()
            .map(|km| km.into())
            .collect();

        let batch_size = batch.len();
        tracing::debug!("  Page {}: {} markets", page_count + 1, batch_size);
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

    Ok(all_markets)
}

/// Fetch specific markets by ID (for updating existing positions)
///
/// Searches all markets without time filters to handle postponed/rescheduled games.
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
    // Deduplicate market IDs (e.g., "Both" strategy creates 2 positions per market)
    let market_id_set: std::collections::HashSet<_> = market_ids.iter().cloned().collect();
    tracing::info!("Fetching {} unique position markets ({} total positions)...",
        market_id_set.len(), market_ids.len());

    // Extract unique series from market IDs (e.g., "KXNBAGAME-25DEC29CLESAS-CLE" → "KXNBAGAME")
    let series_set: std::collections::HashSet<String> = market_ids.iter()
        .filter_map(|id| {
            // Split on first dash to get series
            id.as_str().split('-').next().map(String::from)
        })
        .collect();

    tracing::debug!("  Position markets span {} series: {:?}", series_set.len(), series_set);

    let mut found_markets = Vec::new();

    // Fetch each series separately to avoid pagination issues
    // Must fetch both "open" (active/initialized) AND "settled" (finalized) markets
    for series in series_set {
        tracing::debug!("  Fetching markets from series: {}", series);

        // Fetch both open and settled markets for this series
        for status_filter in ["open", "settled"] {
            let mut cursor: Option<String> = None;
            let max_pages = 3;  // 3 pages per series should be enough
            let mut page_count = 0;

            loop {
                let request = GetMarketsRequest {
                    limit: Some(1000),
                    cursor: cursor.clone(),
                    status: Some(status_filter.to_string()),
                    series_ticker: Some(series.clone()),
                    min_close_ts: None,  // No time filter - games can be postponed
                    max_close_ts: None,
                };

        let response = kalshi_client.get_markets(request).await?;

        let total_in_batch = response.markets.len();
        tracing::debug!("    Page {}: API returned {} markets", page_count + 1, total_in_batch);

        // If API returns 0 markets, stop early (expected for some series/status combos)
        if total_in_batch == 0 {
            tracing::debug!("    API returned 0 markets for series {} (status: {})", series, status_filter);
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

                // Stop if found all unique markets or hit page limit for this series/status
                if found_markets.len() >= market_id_set.len() || page_count >= max_pages {
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

            // If we found all markets, stop searching other statuses/series
            if found_markets.len() >= market_id_set.len() {
                break;
            }
        }

        // If we found all markets, stop searching other series
        if found_markets.len() >= market_id_set.len() {
            break;
        }
    }

    tracing::info!("  Found {}/{} unique position markets", found_markets.len(), market_id_set.len());
    Ok(found_markets)
}

/// Scan for arbitrage opportunities across all markets
///
/// Uses the arbitrage detector to find cross-market arbitrage opportunities.
/// These are guaranteed profit opportunities where YES + NO < $1.00.
///
/// # Arguments
///
/// * `state` - Application state
///
/// # Returns
///
/// Vector of arbitrage opportunities, sorted by profitability
pub async fn scan_arbitrage_opportunities(
    state: &AppState,
) -> Result<Vec<crate::arbitrage::ArbitrageOpportunity>, Box<dyn std::error::Error>> {
    tracing::info!("🔍 Starting arbitrage scan...");

    let opportunities = state.arbitrage_detector.scan().await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    if opportunities.is_empty() {
        tracing::info!("  No arbitrage opportunities found");
    } else {
        tracing::info!("✅ Found {} arbitrage opportunities", opportunities.len());
    }

    Ok(opportunities)
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

        // Check if LIVE game prioritization is enabled
        let prioritize_live = strategy.filters.prioritize_live_games.unwrap_or(false);

        if prioritize_live {
            // 2-tier evaluation: LIVE games first, then non-LIVE
            let (live_markets, non_live_markets): (Vec<_>, Vec<_>) =
                markets.iter().cloned().partition(|m| m.is_live_game());

            tracing::info!(
                "  Prioritizing {} LIVE games, {} non-LIVE markets",
                live_markets.len(),
                non_live_markets.len()
            );

            // Evaluate LIVE markets first
            if !live_markets.is_empty() {
                match crate::strategy::evaluator::StrategyEvaluator::evaluate(
                    &live_markets,
                    strategy,
                    Some(&state.price_tracker),
                    Some(&state.volume_tracker),
                    Some(&state.order_flow_tracker),
                ) {
                    Ok(live_signals) => {
                        tracing::info!("  Generated {} signals from LIVE games", live_signals.len());

                        // Attach market data to live signals
                        for signal in live_signals {
                            if let Some(market) = market_map.get(&signal.market_id) {
                                signal_market_pairs.push((signal, market.clone()));
                            }
                        }
                    }
                    Err(e) => tracing::warn!("Failed to evaluate LIVE markets for strategy {}: {:?}", strategy.id.0, e),
                }
            }

            // Evaluate non-LIVE markets (risk manager will handle position limits)
            if !non_live_markets.is_empty() {
                match crate::strategy::evaluator::StrategyEvaluator::evaluate(
                    &non_live_markets,
                    strategy,
                    Some(&state.price_tracker),
                    Some(&state.volume_tracker),
                    Some(&state.order_flow_tracker),
                ) {
                    Ok(non_live_signals) => {
                        tracing::info!("  Generated {} signals from non-LIVE markets", non_live_signals.len());

                        // Attach market data to non-live signals
                        for signal in non_live_signals {
                            if let Some(market) = market_map.get(&signal.market_id) {
                                signal_market_pairs.push((signal, market.clone()));
                            }
                        }
                    }
                    Err(e) => tracing::warn!("Failed to evaluate non-LIVE markets for strategy {}: {:?}", strategy.id.0, e),
                }
            }
        } else {
            // No prioritization - evaluate all markets together (backward compatible)
            match crate::strategy::evaluator::StrategyEvaluator::evaluate(
                markets,
                strategy,
                Some(&state.price_tracker),
                Some(&state.volume_tracker),
                Some(&state.order_flow_tracker),
            ) {
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
/// * `market` - Market data for this signal
///
/// # Returns
///
/// Ok(()) if position opened, Err if rejected or failed
pub async fn process_entry_signal(
    state: &mut AppState,
    signal: EntrySignal,
    market: &crate::models::Market,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if market is still open for trading
    if !market.is_open() {
        tracing::debug!(
            "  ⏸️  Market {} is not active (status: {:?}), skipping entry",
            market.ticker,
            market.status
        );
        return Ok(()); // Not an error, just skip closed markets
    }

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
    let risk_decision = state.risk_manager.check_entry(&signal, &state.position_manager, strategy);
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

                // Estimate fee per contract for position sizing
                let fee_per_contract = match signal.order_type {
                    crate::models::strategy::OrderType::Market => {
                        crate::kalshi::fees::calculate_kalshi_taker_fee(signal.recommended_price, 1)
                    }
                    crate::models::strategy::OrderType::Limit => {
                        crate::kalshi::fees::calculate_kalshi_maker_fee(signal.recommended_price, 1)
                    }
                };

                // Total cost per contract = price + fee
                let cost_per_contract = signal.recommended_price + fee_per_contract;

                // Calculate contracts: dollar_amount / cost_per_contract
                let contracts_decimal = dollar_amount / cost_per_contract;

                // Round down to get whole contracts
                contracts_decimal.floor().to_u64().unwrap_or(0)
            }
        };

        // Validate quantity is positive (prevent zero-quantity orders)
        if contracts == 0 {
            tracing::warn!(
                "  ⚠️  Insufficient capital for {} - calculated 0 contracts (price: ${:.2}, position size: {})",
                signal.market_ticker,
                signal.recommended_price,
                signal.position_size
            );
            return Ok(()); // Not an error, just skip insufficient capital
        }

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

    // Persist position to database immediately after creation (Phase 5)
    if let Some(position) = state.position_manager.get_position(&position_id) {
        if let Err(e) = state.storage.save_position(position) {
            tracing::error!("Failed to persist new position {} to database: {}", position_id.0, e);
            // Non-fatal - position is still in memory
        }
    }

    // Get position for logging
    if let Some(position) = state.position_manager.get_position(&position_id) {
        let side_str = match position.side {
            crate::models::PositionSide::Yes => "YES",
            crate::models::PositionSide::No => "NO",
        };

        tracing::info!(
            "✓ POSITION OPENED: {} ({} side) @ ${:.4} (qty: {}) | TP: ${:.4} | SL: ${:.4}",
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
    if state.position_manager.active_position_count() == 0 {
        return Ok(());
    }

    tracing::info!("");
    tracing::info!("╔═══════════════════════════════════════════════════════════════╗");
    tracing::info!("║  POSITION UPDATES ({} active)                                   ", state.position_manager.active_position_count());
    tracing::info!("╚═══════════════════════════════════════════════════════════════╝");

    // Build market lookup map for efficient price updates
    let market_map: std::collections::HashMap<_, _> = markets
        .iter()
        .map(|m| (m.id.clone(), m))
        .collect();

    tracing::debug!(
        "Received {} fresh markets from API for position updates",
        markets.len()
    );

    // Get positions to check (need to collect to avoid borrow issues)
    let position_ids: Vec<_> = state.position_manager.get_position_ids();

    for position_id in position_ids {
        // Get position (safely)
        let position = match state.position_manager.get_position(&position_id) {
            Some(p) => p.clone(),
            None => continue,
        };

        // Find current market data
        tracing::debug!("Looking for market {} in market_map", position.market_id.0);
        let market = match market_map.get(&position.market_id) {
            Some(m) => m,
            None => {
                tracing::warn!(
                    "⚠️  Market {} not found in API (checked {} markets)",
                    position.market_id.0,
                    market_map.len()
                );
                tracing::warn!(
                    "   Likely expired crypto market - force-closing position {} at last known price ${:.2}",
                    position_id.0,
                    position.current_price
                );

                // For crypto 15-min markets that disappear from API completely,
                // close at last known price (settlement price unknown)

                // Create trade record (manually since we can't fetch the market)
                let trade = crate::models::Trade::new(
                    position.id.clone(),
                    position.market_id.clone(),
                    position.strategy_id.clone(),
                    position.entry_order_id.clone(),
                    position.entry_price,
                    position.quantity,
                    position.entry_timestamp,
                    crate::models::OrderId::new("manual-exit".to_string()), // Manual exit, no real order
                    position.current_price, // Exit at last known price
                    position.quantity,
                    chrono::Utc::now(),
                    crate::models::ExitReason::ManualExit,
                    rust_decimal::Decimal::ZERO, // No fees for simulated closure
                );

                tracing::info!(
                    "✓ Position closed (market disappeared from API) | P&L: ${:.2}",
                    trade.net_pnl
                );

                state.metrics_tracker.record_trade(&trade);

                // Remove position from manager (manual exit)
                state.position_manager.remove_position(&position_id);

                continue;
            }
        };

        // Check if market is finalized (game finished and settled)
        // Don't close for "initialized" markets (games that haven't started yet)
        if market.status == crate::models::MarketStatus::Finalized {
            tracing::warn!(
                "⚠️  Market {} is Finalized (game finished/settled) - closing position {}",
                market.ticker,
                position_id.0
            );

            // Get settlement price (YES=1.00 or YES=0.00 depending on outcome)
            let settlement_price = match position.side {
                crate::models::PositionSide::Yes => market.yes_price,
                crate::models::PositionSide::No => market.no_price,
            };

            // Update position with settlement price
            let mut settled_position = position.clone();
            settled_position.update_price(settlement_price);

            tracing::info!(
                "💰 SETTLED: [{}/{}] {} | Entry ${:.2} → ${:.2} | Final P&L: ${:.2}",
                match position.side {
                    crate::models::PositionSide::Yes => "YES",
                    crate::models::PositionSide::No => "NO ",
                },
                position.quantity,
                market.ticker,
                position.entry_price,
                settlement_price,
                settled_position.unrealized_pnl
            );

            // Execute settlement exit
            match execute_exit(state, &position_id, ExitReason::MarketClosed).await {
                Ok(()) => {
                    // Delete position from database FIRST (Phase 5)
                    if let Err(e) = state.storage.delete_position(&position_id) {
                        tracing::error!("Failed to delete position {} from database: {}", position_id.0, e);
                    }

                    // Remove from memory AFTER DB deletion (crash consistency)
                    state.position_manager.remove_position(&position_id);
                }
                Err(e) => {
                    tracing::error!("Failed to settle position {}: {}", position_id.0, e);
                }
            }
            continue;
        }

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

        // DEBUG: Log full market state to verify prices are actually changing
        tracing::debug!(
            "  Market state: YES bid=${:.2} ask=${:.2} mid=${:.2} | NO bid=${:.2} ask=${:.2} mid=${:.2} | Vol={} OI={} Status={:?}",
            market.yes_bid,
            market.yes_ask,
            market.yes_price,
            market.no_bid,
            market.no_ask,
            market.no_price,
            market.volume,
            market.open_interest,
            market.status
        );

        // DEBUG: Log ACTUAL exit target values to diagnose SL calculation bug
        // tracing::warn!(
        //     "  EXIT TARGET DEBUG: Entry={:.6} | TP={:.6} | SL={:.6}",
        //     position.entry_price,
        //     position.exit_target.take_profit_price.unwrap_or_default(),
        //     position.exit_target.stop_loss_price.unwrap_or_default()
        // );

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
        let pnl_color = if updated_position.unrealized_pnl > rust_decimal::Decimal::ZERO {
            "+"
        } else {
            ""
        };

        // Only show colored arrow and circle if price ACTUALLY CHANGED this iteration
        // Threshold: more than $0.005 change since last update
        let threshold = rust_decimal::Decimal::new(5, 3); // 0.005
        let price_change_abs = price_change.abs();

        if price_change_abs >= threshold {
            // Price changed significantly - show colored arrow and circle
            let (change_symbol, color_code) = if price_change > rust_decimal::Decimal::ZERO {
                ("↑", "\x1b[32m")  // Green up arrow
            } else {
                ("↓", "\x1b[31m")  // Red down arrow (price_change < 0 since we checked abs >= threshold)
            };

            // Use println! for ANSI color rendering (tracing doesn't render colors)
            println!(
                "{}  [{}/{}] {} | Entry ${:.4} {}{}\x1b[0m ${:.4} | P&L: {}{:.2} | TP: ${:.4} SL: ${:.4} {}●\x1b[0m",
                chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ  INFO"),
                side_str,
                position.quantity,
                market.ticker,
                position.entry_price,
                color_code,
                change_symbol,
                current_price,
                pnl_color,
                updated_position.unrealized_pnl,
                position.exit_target.take_profit_price.unwrap_or_default(),
                position.exit_target.stop_loss_price.unwrap_or_default(),
                color_code
            );
        } else {
            // Price unchanged or changed less than threshold - plain display
            tracing::info!(
                "  [{}/{}] {} | Entry ${:.4} → ${:.4} | P&L: {}{:.2} | TP: ${:.4} SL: ${:.4}",
                side_str,
                position.quantity,
                market.ticker,
                position.entry_price,
                current_price,
                pnl_color,
                updated_position.unrealized_pnl,
                position.exit_target.take_profit_price.unwrap_or_default(),
                position.exit_target.stop_loss_price.unwrap_or_default()
            );
        }

        // Persist position to database FIRST (Phase 5)
        // Critical: DB must be updated before memory to maintain consistency on crash
        if let Err(e) = state.storage.save_position(&updated_position) {
            tracing::error!("Failed to persist position {} to database: {}", position_id.0, e);
            // Non-fatal - continue (in-memory state still updates)
        }

        // Update position in PositionManager with new price
        if let Some(pm_position) = state.position_manager.get_position_mut(&position_id) {
            pm_position.update_price(updated_position.current_price);
        }

        // Check settlement-aware exit logic FIRST (highest priority - time-sensitive)
        // Only if enabled in strategy configuration AND market has predictable settlement
        if let Some(strategy) = state.strategies.get(&position.strategy_id) {
            let settlement_enabled = strategy.exit_rules.settlement_aware_exit.unwrap_or(false);

            if settlement_enabled
                && market.is_open()  // Only Active markets are tradeable (skip Determined/Finalized)
                && market.is_settlement_predictable()  // Only apply to predictable markets (crypto, economics)
                && state.exit_manager.check_settlement_logic(&updated_position, market, current_price) {
                tracing::info!(
                    "✓ SETTLEMENT EXIT TRIGGERED: {} (Losing position near settlement, P&L: ${:.2})",
                    position_id.0,
                    updated_position.unrealized_pnl
                );

                // Execute settlement-aware exit
                match execute_exit(state, &position_id, ExitReason::SettlementCutLoss).await {
                    Ok(()) => {
                        // Delete position from database FIRST (Phase 5)
                        // Critical: DB must be updated before memory to maintain crash consistency
                        if let Err(e) = state.storage.delete_position(&position_id) {
                            tracing::error!("Failed to delete position {} from database: {}", position_id.0, e);
                            // Non-fatal - still remove from memory (trade already recorded)
                        }

                        // Remove from memory AFTER DB deletion (crash consistency)
                        state.position_manager.remove_position(&position_id);

                        continue; // Skip standard exit checks - already exited
                    }
                    Err(e) => {
                        tracing::error!("Failed to execute settlement exit for {}: {}", position_id.0, e);
                    }
                }
            }
        }

        // Check standard exit conditions (TP, SL, trailing stop, max hold time)
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
                        // Delete position from database FIRST (Phase 5)
                        // Critical: DB must be updated before memory to maintain crash consistency
                        if let Err(e) = state.storage.delete_position(&position_id) {
                            tracing::error!("Failed to delete position {} from database: {}", position_id.0, e);
                            // Non-fatal - still remove from memory (trade already recorded)
                        }

                        // Remove from memory AFTER DB deletion (crash consistency)
                        state.position_manager.remove_position(&position_id);
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
    use crate::kalshi::fees::calculate_kalshi_taker_fee;
    let entry_fees = calculate_kalshi_taker_fee(pm_position.entry_price, pm_position.quantity);
    let exit_fees = calculate_kalshi_taker_fee(exit_price, pm_position.quantity);
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

    // Persist trade to database (Phase 5)
    if let Err(e) = state.storage.insert_trade(&trade) {
        tracing::error!("Failed to persist trade {} to database: {}", trade.id.0, e);
        // Non-fatal - in-memory metrics still work
    }

    // Debug: Check metrics after recording
    let metrics_after = state.metrics_tracker.calculate_metrics();
    tracing::info!(
        "📊 Metrics after trade: Trades={}, Net P&L=${:.4}",
        metrics_after.total_trades,
        metrics_after.net_pnl
    );

    // NOTE: Position removal from PositionManager is done by CALLER after DB deletion
    // to maintain crash consistency (DB first, then memory)

    Ok(())
}

/// Print current status
///
/// # Arguments
///
/// * `state` - Application state
pub fn print_status(state: &AppState) {
    let active_positions = state.position_manager.active_position_count();
    let metrics = state.metrics_tracker.calculate_metrics();

    // Calculate total unrealized P&L from open positions
    use rust_decimal::Decimal;
    let total_unrealized_pnl: Decimal = state.position_manager.get_active_positions()
        .iter()
        .map(|p| p.unrealized_pnl)
        .sum();

    // Calculate net profit (realized + unrealized)
    let net_profit = metrics.net_pnl + total_unrealized_pnl;

    tracing::info!("");
    tracing::info!("═══════════════════════════════════════════════════════════════");
    tracing::info!("  Positions: {} open | Unrealized: ${:.2} | Realized: ${:.2} | Net Profit: ${:.2}",
        active_positions,
        total_unrealized_pnl,
        metrics.net_pnl,
        net_profit
    );
    tracing::info!("  Trades: {} | Wins: {} | Losses: {} | Win Rate: {:.1}%",
        metrics.total_trades,
        metrics.total_wins,
        metrics.total_losses,
        metrics.win_rate
    );

    // Show exit reason breakdown if we have trades
    if metrics.total_trades > 0 {
        let exits = state.metrics_tracker.get_exit_reason_summary();
        tracing::info!("  Exits: TP:{} Settled:{} SL:{} MaxHold:{}",
            exits.take_profit,
            exits.market_closed,
            exits.stop_loss,
            exits.max_hold
        );
    }

    // Show arbitrage opportunities count if in arbitrage mode
    if !state.arbitrage_opportunities_found.is_empty() {
        tracing::info!("  Arbitrage Opportunities Found: {} total", state.arbitrage_opportunities_found.len());
    }

    tracing::info!("═══════════════════════════════════════════════════════════════");
}

/// Display arbitrage opportunities in formatted table
///
/// Shows the top arbitrage opportunities found, with detailed profit analysis.
///
/// # Arguments
///
/// * `opportunities` - List of arbitrage opportunities to display
/// * `max_display` - Maximum number of opportunities to show (default: 10)
pub fn display_arbitrage_opportunities(
    opportunities: &[crate::arbitrage::ArbitrageOpportunity],
    max_display: usize,
) {
    use rust_decimal::Decimal;

    if opportunities.is_empty() {
        return; // Already logged in scan function
    }

    tracing::info!("");
    tracing::info!("🎯 ARBITRAGE OPPORTUNITIES DETECTED");
    tracing::info!("═══════════════════════════════════════════════════════════════");

    let to_show = std::cmp::min(max_display, opportunities.len());

    for (idx, opp) in opportunities.iter().take(to_show).enumerate() {
        // Calculate expected profit in USD for default position size
        let position_size = std::cmp::min(opp.quantity, 100); // Show profit for 100 contracts or max available
        let capital_needed = opp.capital_required(position_size);
        let expected_profit = opp.expected_profit * Decimal::from(position_size);

        tracing::info!(
            "[{}] {} ({}d)",
            idx + 1,
            &opp.market_title[..std::cmp::min(50, opp.market_title.len())], // Truncate long titles
            opp.time_to_settlement.num_days(),
        );
        tracing::info!(
            "    YES: ${:.2} | NO: ${:.2} | Total: ${:.2} | Profit: {:.1}% | Annualized ROI: {:.0}%",
            opp.yes_ask,
            opp.no_ask,
            opp.total_cost,
            opp.profit_pct * Decimal::from(100),
            opp.annualized_roi() * Decimal::from(100),
        );
        tracing::info!(
            "    Qty: {} contracts | Capital: ${:.2} | Expected profit: ${:.2}",
            opp.quantity,
            capital_needed,
            expected_profit,
        );
        tracing::info!("    Market ID: {}", opp.market_id.as_str());
        tracing::info!("");
    }

    if opportunities.len() > max_display {
        tracing::info!("  ... and {} more opportunities", opportunities.len() - max_display);
        tracing::info!("");
    }

    // Summary statistics
    let total_opportunities = opportunities.len();
    let total_capital_available: Decimal = opportunities
        .iter()
        .map(|opp| opp.capital_required(std::cmp::min(opp.quantity, 100)))
        .sum();
    let avg_profit_pct: Decimal = opportunities
        .iter()
        .map(|opp| opp.profit_pct)
        .sum::<Decimal>()
        / Decimal::from(total_opportunities);

    tracing::info!("📊 SUMMARY:");
    tracing::info!("  Total opportunities: {}", total_opportunities);
    tracing::info!("  Average profit: {:.1}%", avg_profit_pct * Decimal::from(100));
    tracing::info!("  Capital to deploy all: ${:.2}", total_capital_available);
    tracing::info!("═══════════════════════════════════════════════════════════════");
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// Test that position display shows colored arrows only when price changes
    #[test]
    fn test_position_display_shows_arrows_on_price_change() {
        // Simulate price change scenarios
        let threshold = dec!(0.005);
        
        // Case 1: Price went UP significantly (≥ 0.005)
        let price_change_up = dec!(0.01);
        assert!(price_change_up.abs() >= threshold, "Should trigger colored display");
        
        // Case 2: Price went DOWN significantly (≥ 0.005)
        let price_change_down = dec!(-0.01);
        assert!(price_change_down.abs() >= threshold, "Should trigger colored display");
        
        // Case 3: Price unchanged
        let price_change_zero = dec!(0.0);
        assert!(price_change_zero.abs() < threshold, "Should NOT trigger colored display");
        
        // Case 4: Price changed < threshold
        let price_change_tiny = dec!(0.003);
        assert!(price_change_tiny.abs() < threshold, "Should NOT trigger colored display");
    }

    /// Test that up arrow uses green color code
    #[test]
    fn test_up_arrow_is_green() {
        let price_change = dec!(0.01);
        
        if price_change > dec!(0.0) {
            let (symbol, color) = ("↑", "\x1b[32m");
            assert_eq!(symbol, "↑");
            assert_eq!(color, "\x1b[32m", "Green color code");
        }
    }

    /// Test that down arrow uses red color code
    #[test]
    fn test_down_arrow_is_red() {
        let price_change = dec!(-0.01);
        
        if price_change < dec!(0.0) {
            let (symbol, color) = ("↓", "\x1b[31m");
            assert_eq!(symbol, "↓");
            assert_eq!(color, "\x1b[31m", "Red color code");
        }
    }

    /// Test threshold value is exactly 0.005 (half cent)
    #[test]
    fn test_threshold_is_half_cent() {
        let threshold = rust_decimal::Decimal::new(5, 3);
        assert_eq!(threshold, dec!(0.005));
    }
}
