// Strategy JSON loader
// Loads trading strategies from JSON files

use std::fs;
use std::path::{Path, PathBuf};
use serde_json;

use crate::models::Strategy;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors that can occur when loading strategies
#[derive(Debug)]
pub enum LoaderError {
    /// File not found
    FileNotFound(PathBuf),

    /// Failed to read file
    ReadError(PathBuf, std::io::Error),

    /// Invalid JSON syntax
    JsonError(PathBuf, serde_json::Error),

    /// Strategy validation failed
    ValidationError(String),
}

impl std::fmt::Display for LoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoaderError::FileNotFound(path) => {
                write!(f, "Strategy file not found: {}", path.display())
            }
            LoaderError::ReadError(path, err) => {
                write!(f, "Failed to read strategy file {}: {}", path.display(), err)
            }
            LoaderError::JsonError(path, err) => {
                write!(f, "Invalid JSON in strategy file {}: {}", path.display(), err)
            }
            LoaderError::ValidationError(msg) => {
                write!(f, "Strategy validation failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for LoaderError {}

// =============================================================================
// STRATEGY LOADER
// =============================================================================

/// Loads strategies from JSON files
pub struct StrategyLoader;

impl StrategyLoader {
    /// Load a strategy from a JSON file
    ///
    /// # Arguments
    /// * `path` - Path to the strategy JSON file
    ///
    /// # Returns
    /// * `Ok(Strategy)` - Successfully loaded and validated strategy
    /// * `Err(LoaderError)` - Failed to load or validate strategy
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::strategy::StrategyLoader;
    ///
    /// let strategy = StrategyLoader::load("strategies/underdog_hunter.json")?;
    /// println!("Loaded strategy: {}", strategy.name);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Strategy, LoaderError> {
        let path = path.as_ref();

        // Check if file exists
        if !path.exists() {
            return Err(LoaderError::FileNotFound(path.to_path_buf()));
        }

        // Read file contents
        let contents = fs::read_to_string(path)
            .map_err(|err| LoaderError::ReadError(path.to_path_buf(), err))?;

        // Parse JSON
        let strategy: Strategy = serde_json::from_str(&contents)
            .map_err(|err| LoaderError::JsonError(path.to_path_buf(), err))?;

        // Validate strategy
        Self::validate(&strategy)?;

        Ok(strategy)
    }

    /// Validate a loaded strategy
    ///
    /// Checks for:
    /// - Non-empty name and ID
    /// - Valid percentage ranges (0-100%)
    /// - Positive quantities and limits
    fn validate(strategy: &Strategy) -> Result<(), LoaderError> {
        // Check name and ID
        if strategy.name.is_empty() {
            return Err(LoaderError::ValidationError(
                "Strategy name cannot be empty".to_string()
            ));
        }

        if strategy.id.as_str().is_empty() {
            return Err(LoaderError::ValidationError(
                "Strategy ID cannot be empty".to_string()
            ));
        }

        // Check position size is positive
        if strategy.entry_rules.position_size == 0 {
            return Err(LoaderError::ValidationError(
                "Position size must be greater than 0".to_string()
            ));
        }

        // Validate exit percentages if present
        if let Some(tp) = strategy.exit_rules.take_profit_pct {
            if tp <= rust_decimal::Decimal::ZERO || tp > rust_decimal::Decimal::from(1000) {
                return Err(LoaderError::ValidationError(
                    format!("Take profit percentage must be between 0 and 1000, got {}", tp)
                ));
            }
        }

        if let Some(sl) = strategy.exit_rules.stop_loss_pct {
            if sl <= rust_decimal::Decimal::ZERO || sl > rust_decimal::Decimal::from(100) {
                return Err(LoaderError::ValidationError(
                    format!("Stop loss percentage must be between 0 and 100, got {}", sl)
                ));
            }
        }

        if let Some(ts) = strategy.exit_rules.trailing_stop_pct {
            if ts <= rust_decimal::Decimal::ZERO || ts > rust_decimal::Decimal::from(100) {
                return Err(LoaderError::ValidationError(
                    format!("Trailing stop percentage must be between 0 and 100, got {}", ts)
                ));
            }
        }

        if let Some(max_hold) = strategy.exit_rules.max_hold_time_minutes {
            if max_hold == 0 {
                return Err(LoaderError::ValidationError(
                    "Max hold time must be greater than 0 minutes".to_string()
                ));
            }
        }

        // Validate limit price offset if present
        if let Some(offset) = strategy.entry_rules.limit_price_offset {
            // Offset must be reasonable (within -0.99 to +0.99, since prices are 0-1)
            if offset <= rust_decimal::Decimal::from(-1) || offset >= rust_decimal::Decimal::from(1) {
                return Err(LoaderError::ValidationError(
                    format!("Limit price offset must be between -1.00 and +1.00, got {}", offset)
                ));
            }
        }

        // Check risk limits
        if strategy.risk_limits.max_concurrent_positions == 0 {
            return Err(LoaderError::ValidationError(
                "Max concurrent positions must be greater than 0".to_string()
            ));
        }

        if let Some(max_daily_loss) = strategy.risk_limits.max_daily_loss_usd {
            if max_daily_loss <= rust_decimal::Decimal::ZERO {
                return Err(LoaderError::ValidationError(
                    format!("Max daily loss must be positive, got {}", max_daily_loss)
                ));
            }
        }

        if let Some(max_position_loss) = strategy.risk_limits.max_position_loss_usd {
            if max_position_loss <= rust_decimal::Decimal::ZERO {
                return Err(LoaderError::ValidationError(
                    format!("Max position loss must be positive, got {}", max_position_loss)
                ));
            }
        }

        Ok(())
    }

    /// Load all strategies from a directory
    ///
    /// # Arguments
    /// * `dir_path` - Directory containing strategy JSON files
    ///
    /// # Returns
    /// * `Ok(Vec<Strategy>)` - All successfully loaded strategies
    /// * `Err(LoaderError)` - Failed to read directory or load strategies
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::strategy::StrategyLoader;
    ///
    /// let strategies = StrategyLoader::load_all("strategies")?;
    /// println!("Loaded {} strategies", strategies.len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load_all<P: AsRef<Path>>(dir_path: P) -> Result<Vec<Strategy>, LoaderError> {
        let dir_path = dir_path.as_ref();

        if !dir_path.exists() {
            return Err(LoaderError::FileNotFound(dir_path.to_path_buf()));
        }

        let entries = fs::read_dir(dir_path)
            .map_err(|err| LoaderError::ReadError(dir_path.to_path_buf(), err))?;

        let mut strategies = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|err| {
                LoaderError::ReadError(dir_path.to_path_buf(), err)
            })?;

            let path = entry.path();

            // Only process .json files
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match Self::load(&path) {
                    Ok(strategy) => strategies.push(strategy),
                    Err(err) => {
                        // Log error but continue loading other strategies
                        eprintln!("Warning: Failed to load {}: {}", path.display(), err);
                    }
                }
            }
        }

        Ok(strategies)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_valid_strategy_json() -> String {
        r#"{
            "id": "test-strategy",
            "name": "Test Strategy",
            "description": "A test strategy",
            "version": "1.0.0",
            "enabled": true,
            "filters": {
                "categories": ["Sports"],
                "exclude_categories": null,
                "min_price": "0.10",
                "max_price": "0.90",
                "min_volume": 100,
                "min_open_interest": null,
                "min_time_to_event_minutes": 60,
                "max_time_to_event_minutes": 1440
            },
            "entry_rules": {
                "side": "CheaperSide",
                "position_size": 100,
                "position_size_unit": "Contracts",
                "order_type": "Market",
                "limit_price_offset": null
            },
            "exit_rules": {
                "take_profit_pct": "50.0",
                "stop_loss_pct": "30.0",
                "trailing_stop_pct": null,
                "trailing_stop_activation_pct": null,
                "max_hold_time_minutes": 720,
                "exit_order_type": "Market"
            },
            "risk_limits": {
                "max_concurrent_positions": 5,
                "max_daily_loss_usd": "100.00",
                "max_position_loss_usd": null,
                "loss_cooldown_minutes": null
            }
        }"#.to_string()
    }

    #[test]
    fn test_load_valid_strategy() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_strategy.json");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(create_valid_strategy_json().as_bytes()).unwrap();

        let strategy = StrategyLoader::load(&file_path).unwrap();

        assert_eq!(strategy.name, "Test Strategy");
        assert_eq!(strategy.id.as_str(), "test-strategy");
        assert!(strategy.enabled);
        assert_eq!(strategy.entry_rules.position_size, 100);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = StrategyLoader::load("nonexistent.json");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::FileNotFound(_)));
    }

    #[test]
    fn test_load_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid.json");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"{ invalid json }").unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::JsonError(_, _)));
    }

    #[test]
    fn test_validate_empty_name() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_name.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"Test Strategy\"", "\"\"");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::ValidationError(_)));
    }

    #[test]
    fn test_validate_zero_position_size() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("zero_size.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"position_size\": 100", "\"position_size\": 0");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::ValidationError(_)));
    }

    #[test]
    fn test_load_all_strategies() {
        let temp_dir = TempDir::new().unwrap();

        // Create two valid strategy files
        let file1 = temp_dir.path().join("strategy1.json");
        let mut f1 = fs::File::create(&file1).unwrap();
        f1.write_all(create_valid_strategy_json().as_bytes()).unwrap();

        let file2 = temp_dir.path().join("strategy2.json");
        let mut f2 = fs::File::create(&file2).unwrap();
        let json2 = create_valid_strategy_json().replace("test-strategy", "test-strategy-2");
        f2.write_all(json2.as_bytes()).unwrap();

        // Create a non-JSON file (should be ignored)
        let file3 = temp_dir.path().join("readme.txt");
        let mut f3 = fs::File::create(&file3).unwrap();
        f3.write_all(b"This is not a strategy").unwrap();

        let strategies = StrategyLoader::load_all(temp_dir.path()).unwrap();

        assert_eq!(strategies.len(), 2);
    }

    #[test]
    fn test_validate_negative_trailing_stop() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("negative_trailing.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"trailing_stop_pct\": null", "\"trailing_stop_pct\": \"-10.0\"");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::ValidationError(_)));
    }

    #[test]
    fn test_validate_zero_max_hold_time() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("zero_hold.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"max_hold_time_minutes\": 720", "\"max_hold_time_minutes\": 0");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::ValidationError(_)));
    }

    #[test]
    fn test_validate_negative_max_daily_loss() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("negative_daily_loss.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"max_daily_loss_usd\": \"100.00\"", "\"max_daily_loss_usd\": \"-100.00\"");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::ValidationError(_)));
    }

    #[test]
    fn test_validate_negative_max_position_loss() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("negative_position_loss.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"max_position_loss_usd\": null", "\"max_position_loss_usd\": \"-50.00\"");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::ValidationError(_)));
    }

    #[test]
    fn test_validate_invalid_limit_price_offset() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid_offset.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"limit_price_offset\": null", "\"limit_price_offset\": \"1.50\"");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LoaderError::ValidationError(_)));
    }

    #[test]
    fn test_validate_valid_trailing_stop() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("valid_trailing.json");

        let mut json = create_valid_strategy_json();
        json = json.replace("\"trailing_stop_pct\": null", "\"trailing_stop_pct\": \"20.0\"");

        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        let result = StrategyLoader::load(&file_path);

        assert!(result.is_ok());
    }
}
