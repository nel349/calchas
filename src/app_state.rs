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
    PositionManager, MetricsTracker,
};
use rust_decimal::Decimal;

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

    /// Starting capital for ROI calculations
    pub starting_capital: Decimal,
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

        // Initialize Kalshi client
        let kalshi_client = Arc::new(KalshiClient::from_config(&config.kalshi)?);
        tracing::info!("✓ Kalshi client initialized");

        // Load strategies from directory (hardcoded for now)
        let strategies_dir = std::env::var("CALCHAS_STRATEGIES_DIR")
            .unwrap_or_else(|_| "strategies".to_string());
        let strategies_vec = StrategyLoader::load_all(&strategies_dir)?;
        let strategies: HashMap<StrategyId, Strategy> = strategies_vec
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();
        tracing::info!("✓ Loaded {} strategies", strategies.len());

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

        // Get starting capital from first strategy (they should all have same risk limits)
        let starting_capital = strategies
            .values()
            .next()
            .and_then(|s| s.risk_limits.max_daily_loss_usd)
            .unwrap_or_else(|| Decimal::from(10000));

        let metrics_tracker = MetricsTracker::new(starting_capital);

        tracing::info!("✓ All trading components initialized");

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
            starting_capital,
        })
    }
}
