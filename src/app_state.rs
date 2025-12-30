//! Application state for the trading bot
//!
//! This module contains the AppState struct that holds all the components
//! needed to run the trading loop in simulation mode.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::config::AppConfig;
use crate::kalshi::KalshiClient;
use crate::models::{Position, PositionId, Strategy, StrategyId};
use crate::strategy::loader::StrategyLoader;
use crate::strategy::evaluator::StrategyEvaluator;
use crate::trading::{
    OrderSimulator, RiskManager, ExitManager, OrderExecutor,
    PositionManager, MetricsTracker, PriceTracker, RealOrderbookProvider,
    VolumeTracker, OrderFlowTracker,
};
use crate::arbitrage::{CrossMarketDetector, ArbitrageCalculator, calculator::ArbitrageConfig};
use rust_decimal::Decimal;

/// Time range configuration calculated from all strategies
#[derive(Debug, Clone)]
pub struct TimeRangeConfig {
    /// Minimum time to event in minutes (smallest from all strategies)
    pub min_time_to_event_minutes: u32,
    /// Maximum time to event in minutes (largest from all strategies)
    pub max_time_to_event_minutes: u32,
    /// Whether strategies have conflicting time ranges
    pub has_conflicts: bool,
    /// Details of each strategy's time range (for dashboard display)
    pub strategy_ranges: Vec<(String, u32, u32)>, // (strategy_name, min_minutes, max_minutes)
}

/// Main application state containing all trading components
pub struct AppState {
    /// Kalshi API client for fetching market data
    pub kalshi_client: Arc<KalshiClient>,

    /// Active trading strategies (loaded from JSON files)
    pub strategies: HashMap<StrategyId, Strategy>,

    /// Strategy evaluator for matching markets to strategies
    pub strategy_evaluator: StrategyEvaluator,

    /// Open positions being tracked
    pub positions: HashMap<PositionId, Position>,

    /// Risk manager for enforcing position limits and loss thresholds
    pub risk_manager: RiskManager,

    /// Exit manager for evaluating exit conditions
    pub exit_manager: ExitManager,

    /// Order executor for simulating order fills (shared with position_manager)
    pub order_executor: Arc<Mutex<OrderExecutor>>,

    /// Position manager for coordinating position lifecycle
    pub position_manager: PositionManager,

    /// Metrics tracker for simulation performance
    pub metrics_tracker: MetricsTracker,

    /// Price tracker for momentum analysis
    pub price_tracker: PriceTracker,

    /// Volume tracker for volume spike detection (Phase 1)
    pub volume_tracker: VolumeTracker,

    /// Order flow tracker for orderbook imbalance detection (Phase 2)
    pub order_flow_tracker: OrderFlowTracker,

    /// Orderbook provider for liquidity checks (using real API data)
    pub orderbook_provider: RealOrderbookProvider,

    /// Arbitrage detector for cross-market opportunities
    pub arbitrage_detector: CrossMarketDetector,

    /// Accumulated arbitrage opportunities found (for tracking/analysis)
    pub arbitrage_opportunities_found: Vec<crate::arbitrage::ArbitrageOpportunity>,

    /// Starting capital for ROI calculations
    pub starting_capital: Decimal,

    /// Time range configuration for API filtering (calculated from all strategies)
    pub time_range_config: TimeRangeConfig,
}

impl AppState {
    /// Initialize application state
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration
    ///
    /// # Returns
    ///
    /// Initialized AppState ready for trading loop
    pub async fn new(config: AppConfig) -> Result<Self, Box<dyn std::error::Error>> {
        tracing::info!("Initializing application state...");

        // DEBUG: Print which API we're using
        tracing::info!("Kalshi API: {} (use_demo={})",
            config.kalshi.base_url(),
            config.kalshi.use_demo
        );

        // Initialize Kalshi client
        let kalshi_client = Arc::new(KalshiClient::from_config(&config.kalshi)?);
        tracing::info!("✓ Kalshi client initialized");

        // Load strategies from directory (hardcoded for now)
        let strategies_dir = std::env::var("CALCHAS_STRATEGIES_DIR")
            .unwrap_or_else(|_| "strategies".to_string());
        let strategies_vec = StrategyLoader::load_all(&strategies_dir)?;
        let strategies: HashMap<StrategyId, Strategy> = strategies_vec
            .into_iter()
            .filter(|s| s.enabled)  // Only load enabled strategies
            .map(|s| (s.id.clone(), s))
            .collect();
        tracing::info!("✓ Loaded {} strategies ({} enabled)", strategies.len(), strategies.len());

        // Calculate time range configuration from all strategies
        let time_range_config = Self::calculate_time_range(&strategies, &config);

        // Initialize trading components
        let strategy_evaluator = StrategyEvaluator;
        let risk_manager = RiskManager::new();
        let exit_manager = ExitManager;  // Unit struct, can use multiple times
        let simulator = OrderSimulator::new(kalshi_client.clone());
        let order_executor = OrderExecutor::new(simulator);
        let order_executor_arc = Arc::new(Mutex::new(order_executor));
        let position_manager = PositionManager::new(
            kalshi_client.clone(),
            ExitManager,  // Use another instance
            order_executor_arc.clone(),
        );

        // Get starting capital from first strategy, or arbitrage config if no strategies
        let starting_capital = strategies
            .values()
            .next()
            .and_then(|s| s.risk_limits.max_daily_loss_usd)
            .unwrap_or(config.arbitrage.starting_capital);

        let metrics_tracker = MetricsTracker::new(starting_capital);
        let price_tracker = PriceTracker::new();
        let volume_tracker = VolumeTracker::new();
        let order_flow_tracker = OrderFlowTracker::new();
        let orderbook_provider = RealOrderbookProvider::new(kalshi_client.clone());

        // Initialize arbitrage detector with configuration from TOML
        let arb_config = ArbitrageConfig {
            min_profit_pct: config.arbitrage.min_profit_pct,
            max_profit_pct: config.arbitrage.max_profit_pct,
            min_total_cost: config.arbitrage.min_total_cost,
            min_quantity: config.arbitrage.min_quantity,
            min_hours_to_settlement: config.arbitrage.min_hours_to_settlement,
            max_capital_per_trade: config.arbitrage.max_capital_per_trade,
        };

        let arb_calculator = ArbitrageCalculator::new(arb_config);
        let arbitrage_detector = CrossMarketDetector::new(
            kalshi_client.clone(),
            arb_calculator,
        );

        tracing::info!("✓ All trading components initialized");
        tracing::info!("✓ Using REAL orderbook data from Kalshi API");
        tracing::info!("✓ Arbitrage scanner initialized (capital: ${}, max/trade: ${}, min profit: {}%)",
            config.arbitrage.starting_capital,
            config.arbitrage.max_capital_per_trade,
            config.arbitrage.min_profit_pct * Decimal::from(100)
        );

        Ok(Self {
            kalshi_client,
            strategies,
            strategy_evaluator,
            positions: HashMap::new(),
            risk_manager,
            exit_manager,
            order_executor: order_executor_arc,
            position_manager,
            metrics_tracker,
            price_tracker,
            volume_tracker,
            order_flow_tracker,
            orderbook_provider,
            arbitrage_detector,
            arbitrage_opportunities_found: Vec::new(),
            starting_capital,
            time_range_config,
        })
    }

    /// Calculate time range configuration from all strategies
    ///
    /// Finds the widest time window across all strategies and detects conflicts
    ///
    /// # Arguments
    ///
    /// * `strategies` - Map of all loaded strategies
    /// * `config` - Application config (provides defaults when strategy doesn't specify)
    fn calculate_time_range(
        strategies: &HashMap<StrategyId, Strategy>,
        config: &AppConfig,
    ) -> TimeRangeConfig {
        let mut min_time = u32::MAX;
        let mut max_time = 0u32;
        let mut strategy_ranges = Vec::new();
        let mut min_values = std::collections::HashSet::new();
        let mut max_values = std::collections::HashSet::new();

        for strategy in strategies.values() {
            let strat_min = strategy.filters.min_time_to_event_minutes
                .unwrap_or(config.runtime.default_min_time_minutes);
            let strat_max = strategy.filters.max_time_to_event_minutes
                .unwrap_or(config.runtime.default_max_time_minutes);

            min_time = min_time.min(strat_min);
            max_time = max_time.max(strat_max);

            strategy_ranges.push((strategy.name.clone(), strat_min, strat_max));
            min_values.insert(strat_min);
            max_values.insert(strat_max);
        }

        // Default to config values if no strategies
        if strategies.is_empty() {
            min_time = config.runtime.default_min_time_minutes;
            max_time = config.runtime.default_max_time_minutes;
        }

        // Conflicts exist if strategies have different min or max values
        let has_conflicts = strategies.len() > 1 && (min_values.len() > 1 || max_values.len() > 1);

        if has_conflicts {
            tracing::warn!("⚠️  Multiple strategies with different time windows detected!");
            tracing::warn!(
                "    Using widest range: {}min - {}min to cover all strategies",
                min_time, max_time
            );
            for (name, min, max) in &strategy_ranges {
                tracing::warn!("    - {}: {}min - {}min", name, min, max);
            }
        } else if !strategies.is_empty() {
            tracing::info!("Time window: {}min - {}min (from {} strategies)", min_time, max_time, strategies.len());
        }

        TimeRangeConfig {
            min_time_to_event_minutes: min_time,
            max_time_to_event_minutes: max_time,
            has_conflicts,
            strategy_ranges,
        }
    }
}
