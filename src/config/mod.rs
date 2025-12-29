// Configuration management
// Loads from TOML files with environment variable overrides

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Configuration loading errors
#[derive(Debug)]
pub enum ConfigLoadError {
    /// Config file not found
    FileNotFound(PathBuf),

    /// Failed to parse TOML
    ParseError(String),

    /// Invalid configuration (validation failed)
    ValidationError(String),

    /// Environment variable error
    EnvError(String),
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLoadError::FileNotFound(path) => {
                write!(f, "Config file not found: {}", path.display())
            }
            ConfigLoadError::ParseError(msg) => {
                write!(f, "Failed to parse config: {}", msg)
            }
            ConfigLoadError::ValidationError(msg) => {
                write!(f, "Invalid configuration: {}", msg)
            }
            ConfigLoadError::EnvError(msg) => {
                write!(f, "Environment variable error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ConfigLoadError {}

impl From<ConfigError> for ConfigLoadError {
    fn from(err: ConfigError) -> Self {
        match err {
            ConfigError::NotFound(_) => {
                ConfigLoadError::ParseError(err.to_string())
            }
            ConfigError::PathParse { cause } => {
                ConfigLoadError::ParseError(format!("Path parse error: {}", cause))
            }
            ConfigError::FileParse { cause, .. } => {
                ConfigLoadError::ParseError(cause.to_string())
            }
            ConfigError::Type { .. } => {
                ConfigLoadError::ParseError(err.to_string())
            }
            ConfigError::Message(msg) => {
                ConfigLoadError::ParseError(msg)
            }
            ConfigError::Foreign(err) => {
                ConfigLoadError::ParseError(err.to_string())
            }
            _ => {
                // Handle any other variants (ConfigError is non-exhaustive)
                ConfigLoadError::ParseError(err.to_string())
            }
        }
    }
}

// =============================================================================
// CONFIG STRUCTURES
// =============================================================================

/// Application configuration (Phase 1-2: basic + Kalshi API)
///
/// This will be extended in later phases to include:
/// - [database] - SQLite settings (Phase 5)
/// - [web] - Web server settings (Phase 7)
/// - [logging] - Log configuration (currently using RUST_LOG env var)
/// - [risk] - Risk management limits (Phase 4)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Runtime configuration
    pub runtime: RuntimeConfig,

    /// Kalshi API configuration (Phase 2)
    pub kalshi: KalshiConfig,

    /// Arbitrage configuration
    #[serde(default)]
    pub arbitrage: ArbitrageConfig,
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Directory containing strategy JSON files
    /// Can be overridden with: CALCHAS_RUNTIME__STRATEGY_DIR
    pub strategy_dir: String,

    /// Default minimum time to event in minutes (when strategy doesn't specify)
    /// Can be overridden with: CALCHAS__RUNTIME__DEFAULT_MIN_TIME_MINUTES
    #[serde(default = "default_min_time_minutes")]
    pub default_min_time_minutes: u32,

    /// Default maximum time to event in minutes (when strategy doesn't specify)
    /// Can be overridden with: CALCHAS__RUNTIME__DEFAULT_MAX_TIME_MINUTES
    #[serde(default = "default_max_time_minutes")]
    pub default_max_time_minutes: u32,
}

/// Default minimum time: 0 minutes (allow live events)
fn default_min_time_minutes() -> u32 {
    0
}

/// Default maximum time: 30 days
fn default_max_time_minutes() -> u32 {
    43200  // 30 days * 24 hours * 60 minutes
}

/// Kalshi API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalshiConfig {
    /// Use demo API (true) or production API (false)
    /// Can be overridden with: CALCHAS__KALSHI__USE_DEMO
    #[serde(default = "default_use_demo")]
    pub use_demo: bool,

    /// API key ID from Kalshi dashboard
    /// MUST be set via environment variable: CALCHAS__KALSHI__API_KEY_ID
    /// Never commit this to version control!
    pub api_key_id: String,

    /// RSA private key in PEM format
    /// MUST be set via environment variable: CALCHAS__KALSHI__PRIVATE_KEY
    /// Never commit this to version control!
    ///
    /// Example format in .env file:
    /// CALCHAS__KALSHI__PRIVATE_KEY="-----BEGIN PRIVATE KEY-----\nMIIEv...\n-----END PRIVATE KEY-----"
    pub private_key: String,
}

/// Default value for use_demo (true for safety)
fn default_use_demo() -> bool {
    true
}

impl KalshiConfig {
    /// Get the base URL for Kalshi API based on environment
    ///
    /// # Returns
    /// * Demo API: `https://demo-api.kalshi.co/trade-api/v2`
    /// * Production API: `https://api.elections.kalshi.com/trade-api/v2`
    pub fn base_url(&self) -> &str {
        if self.use_demo {
            "https://demo-api.kalshi.co/trade-api/v2"
        } else {
            "https://api.elections.kalshi.com/trade-api/v2"
        }
    }
}

/// Arbitrage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageConfig {
    /// Starting capital for arbitrage trading
    #[serde(default = "default_starting_capital")]
    pub starting_capital: rust_decimal::Decimal,

    /// Maximum capital to deploy per trade
    #[serde(default = "default_max_capital_per_trade")]
    pub max_capital_per_trade: rust_decimal::Decimal,

    /// Minimum profit percentage (e.g., 0.04 = 4%)
    #[serde(default = "default_min_profit_pct")]
    pub min_profit_pct: rust_decimal::Decimal,

    /// Minimum hours before settlement
    #[serde(default = "default_min_hours_to_settlement")]
    pub min_hours_to_settlement: i64,

    /// Maximum days to settlement
    #[serde(default = "default_max_days_to_settlement")]
    pub max_days_to_settlement: i64,

    /// Minimum quantity (contracts) required
    #[serde(default = "default_min_quantity")]
    pub min_quantity: u64,
}

fn default_starting_capital() -> rust_decimal::Decimal {
    rust_decimal::Decimal::from(500)
}

fn default_max_capital_per_trade() -> rust_decimal::Decimal {
    rust_decimal::Decimal::from(75)
}

fn default_min_profit_pct() -> rust_decimal::Decimal {
    rust_decimal::Decimal::new(4, 2) // 0.04 = 4%
}

fn default_min_hours_to_settlement() -> i64 {
    12
}

fn default_max_days_to_settlement() -> i64 {
    7
}

fn default_min_quantity() -> u64 {
    40
}

impl Default for ArbitrageConfig {
    fn default() -> Self {
        ArbitrageConfig {
            starting_capital: default_starting_capital(),
            max_capital_per_trade: default_max_capital_per_trade(),
            min_profit_pct: default_min_profit_pct(),
            min_hours_to_settlement: default_min_hours_to_settlement(),
            max_days_to_settlement: default_max_days_to_settlement(),
            min_quantity: default_min_quantity(),
        }
    }
}

// =============================================================================
// CONFIG LOADER
// =============================================================================

impl AppConfig {
    /// Load configuration from TOML file with environment variable overrides
    ///
    /// # Loading Order (later sources override earlier):
    /// 1. Load from TOML file
    /// 2. Merge environment variables with CALCHAS_ prefix
    ///    - Use double underscore for nested: CALCHAS_RUNTIME__STRATEGY_DIR
    ///
    /// # Validation
    /// - Checks that strategy_dir exists and is a directory
    /// - Fails fast if required paths are missing
    ///
    /// # Arguments
    /// * `config_path` - Path to config.toml file
    ///
    /// # Returns
    /// * `Ok(AppConfig)` - Loaded and validated configuration
    /// * `Err(ConfigLoadError)` - If loading or validation fails
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::config::AppConfig;
    ///
    /// let config = AppConfig::load("config/config.toml")?;
    /// println!("Strategy dir: {}", config.runtime.strategy_dir);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load<P: AsRef<Path>>(config_path: P) -> Result<Self, ConfigLoadError> {
        let config_path = config_path.as_ref();

        // Check if config file exists
        if !config_path.exists() {
            return Err(ConfigLoadError::FileNotFound(config_path.to_path_buf()));
        }

        // Build config from file + environment
        let settings = Config::builder()
            // Load from TOML file
            .add_source(File::from(config_path))
            // Merge environment variables with CALCHAS_ prefix
            // Example: CALCHAS__RUNTIME__STRATEGY_DIR=./strategies
            // Double underscore __ for nesting, single _ stays in field name
            // So CALCHAS__RUNTIME__STRATEGY_DIR becomes runtime.strategy_dir
            .add_source(
                Environment::with_prefix("CALCHAS")
                    .prefix_separator("__")
                    .separator("__")  // Use __ for nested levels
            )
            .build()?;

        // Deserialize into AppConfig
        let config: AppConfig = settings.try_deserialize()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration
    ///
    /// Checks:
    /// - strategy_dir exists and is a directory
    ///
    /// # Returns
    /// * `Ok(())` - Configuration is valid
    /// * `Err(ConfigLoadError)` - Validation failed
    fn validate(&self) -> Result<(), ConfigLoadError> {
        // Validate strategy_dir exists
        let strategy_path = Path::new(&self.runtime.strategy_dir);

        if !strategy_path.exists() {
            return Err(ConfigLoadError::ValidationError(
                format!(
                    "Strategy directory does not exist: {}",
                    strategy_path.display()
                )
            ));
        }

        if !strategy_path.is_dir() {
            return Err(ConfigLoadError::ValidationError(
                format!(
                    "Strategy path is not a directory: {}",
                    strategy_path.display()
                )
            ));
        }

        Ok(())
    }

    /// Load configuration from default path (config/config.toml)
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::config::AppConfig;
    ///
    /// let config = AppConfig::load_default()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load_default() -> Result<Self, ConfigLoadError> {
        Self::load("config/config.toml")
    }

    /// Load configuration from .env file first, then from TOML file
    ///
    /// This is the recommended way to load config in production.
    /// It loads environment variables from .env file before loading the config,
    /// allowing .env to override TOML values.
    ///
    /// # Loading Order
    /// 1. Load .env file (if exists) using dotenvy
    /// 2. Load TOML config file
    /// 3. Merge environment variables (from .env or shell)
    /// 4. Validate configuration
    ///
    /// # Arguments
    /// * `env_path` - Path to .env file (e.g., ".env")
    /// * `config_path` - Path to config.toml file (e.g., "config/config.toml")
    ///
    /// # Returns
    /// * `Ok(AppConfig)` - Loaded and validated configuration
    /// * `Err(ConfigLoadError)` - If loading or validation fails
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::config::AppConfig;
    ///
    /// // Load .env first, then config.toml
    /// let config = AppConfig::load_with_env(".env", "config/config.toml")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Note
    /// If .env file doesn't exist, this silently continues (not an error).
    /// Only TOML file is required.
    pub fn load_with_env<P1: AsRef<Path>, P2: AsRef<Path>>(
        env_path: P1,
        config_path: P2,
    ) -> Result<Self, ConfigLoadError> {
        let env_path = env_path.as_ref();

        // Load .env file if it exists (ignore if missing)
        if env_path.exists() {
            dotenvy::from_path(env_path)
                .map_err(|e| ConfigLoadError::EnvError(e.to_string()))?;
        }

        // Now load config (env vars from .env are available)
        Self::load(config_path)
    }

    /// Load configuration with .env from default paths
    ///
    /// Loads from .env and config/config.toml
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::config::AppConfig;
    ///
    /// let config = AppConfig::load_with_env_default()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load_with_env_default() -> Result<Self, ConfigLoadError> {
        Self::load_with_env(".env", "config/config.toml")
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config(temp_dir: &TempDir, strategy_dir: &str) -> PathBuf {
        let config_path = temp_dir.path().join("config.toml");
        let config_content = format!(
            r#"
[runtime]
strategy_dir = "{}"

[kalshi]
use_demo = true
api_key_id = "test-key-id"
private_key = "test-private-key"
"#,
            strategy_dir
        );

        fs::write(&config_path, config_content).unwrap();
        config_path
    }

    #[test]
    #[serial]  // Run serially - env vars can affect this test
    fn test_load_valid_config() {
        // Clear any env vars from previous tests
        unsafe {
            std::env::remove_var("CALCHAS__RUNTIME__STRATEGY_DIR");
        }

        let temp_dir = TempDir::new().unwrap();

        // Create strategy directory
        let strategy_dir = temp_dir.path().join("strategies");
        fs::create_dir(&strategy_dir).unwrap();

        // Create config file
        let config_path = create_test_config(&temp_dir, strategy_dir.to_str().unwrap());

        // Load config
        let config = AppConfig::load(&config_path).unwrap();

        assert_eq!(config.runtime.strategy_dir, strategy_dir.to_str().unwrap());
    }

    #[test]
    fn test_config_file_not_found() {
        let result = AppConfig::load("nonexistent.toml");

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigLoadError::FileNotFound(_) => {}
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[test]
    #[serial]  // Run serially - env vars can affect this test
    fn test_validation_strategy_dir_missing() {
        // Clear any env vars from previous tests
        unsafe {
            std::env::remove_var("CALCHAS__RUNTIME__STRATEGY_DIR");
        }

        let temp_dir = TempDir::new().unwrap();

        // Create config pointing to non-existent directory
        let strategy_dir = temp_dir.path().join("missing_strategies");
        let config_path = create_test_config(&temp_dir, strategy_dir.to_str().unwrap());

        // Should fail validation
        let result = AppConfig::load(&config_path);

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigLoadError::ValidationError(msg) => {
                assert!(msg.contains("does not exist"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    #[serial]  // Run serially - env vars can affect this test
    fn test_validation_strategy_path_not_directory() {
        // Clear any env vars from previous tests
        unsafe {
            std::env::remove_var("CALCHAS__RUNTIME__STRATEGY_DIR");
        }

        let temp_dir = TempDir::new().unwrap();

        // Create a file instead of directory
        let strategy_file = temp_dir.path().join("strategies.txt");
        fs::write(&strategy_file, "not a directory").unwrap();

        let config_path = create_test_config(&temp_dir, strategy_file.to_str().unwrap());

        // Should fail validation
        let result = AppConfig::load(&config_path);

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigLoadError::ValidationError(msg) => {
                assert!(msg.contains("not a directory"));
            }
            _ => panic!("Expected ValidationError"),
        }
    }

    #[test]
    #[serial]  // Run serially - modifies global env vars
    fn test_env_var_override() {
        let temp_dir = TempDir::new().unwrap();

        // Create two strategy directories
        let strategy_dir_config = temp_dir.path().join("strategies_config");
        let strategy_dir_env = temp_dir.path().join("strategies_env");
        fs::create_dir(&strategy_dir_config).unwrap();
        fs::create_dir(&strategy_dir_env).unwrap();

        // Set environment variable to override BEFORE creating config
        // (unsafe in Rust 1.83+)
        // Note: Using double underscore for nesting - CALCHAS__RUNTIME__STRATEGY_DIR
        unsafe {
            std::env::set_var("CALCHAS__RUNTIME__STRATEGY_DIR", strategy_dir_env.to_str().unwrap());
        }

        // Create config file
        let config_path = create_test_config(&temp_dir, strategy_dir_config.to_str().unwrap());

        // Load config (should use env var, not file value)
        let config = AppConfig::load(&config_path).unwrap();

        // Verify env var override worked
        assert_eq!(config.runtime.strategy_dir, strategy_dir_env.to_str().unwrap());

        // Clean up env var (unsafe in Rust 1.83+)
        unsafe {
            std::env::remove_var("CALCHAS__RUNTIME__STRATEGY_DIR");
        }
    }

    #[test]
    fn test_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("bad_config.toml");

        // Write invalid TOML
        fs::write(&config_path, "this is not valid toml [[").unwrap();

        let result = AppConfig::load(&config_path);

        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigLoadError::ParseError(_) => {}
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    #[serial]  // Run serially - modifies env vars
    fn test_load_with_env_file() {
        // Clear any env vars from previous tests
        unsafe {
            std::env::remove_var("CALCHAS__RUNTIME__STRATEGY_DIR");
        }

        let temp_dir = TempDir::new().unwrap();

        // Create two strategy directories
        let strategy_dir_toml = temp_dir.path().join("strategies_from_toml");
        let strategy_dir_env = temp_dir.path().join("strategies_from_env");
        fs::create_dir(&strategy_dir_toml).unwrap();
        fs::create_dir(&strategy_dir_env).unwrap();

        // Create config.toml with one path
        let config_path = create_test_config(&temp_dir, strategy_dir_toml.to_str().unwrap());

        // Create .env file with different path
        let env_path = temp_dir.path().join(".env");
        let env_content = format!(
            "CALCHAS__RUNTIME__STRATEGY_DIR={}",
            strategy_dir_env.to_str().unwrap()
        );
        fs::write(&env_path, env_content).unwrap();

        // Load config with .env file
        let config = AppConfig::load_with_env(&env_path, &config_path).unwrap();

        // Should use .env value, not TOML value
        assert_eq!(config.runtime.strategy_dir, strategy_dir_env.to_str().unwrap());

        // Clean up
        unsafe {
            std::env::remove_var("CALCHAS__RUNTIME__STRATEGY_DIR");
        }
    }

    #[test]
    #[serial]  // Run serially - modifies env vars
    fn test_load_with_env_missing_file() {
        // Clear any env vars from previous tests
        unsafe {
            std::env::remove_var("CALCHAS__RUNTIME__STRATEGY_DIR");
        }

        let temp_dir = TempDir::new().unwrap();

        // Create strategy directory
        let strategy_dir = temp_dir.path().join("strategies");
        fs::create_dir(&strategy_dir).unwrap();

        // Create config.toml
        let config_path = create_test_config(&temp_dir, strategy_dir.to_str().unwrap());

        // .env file doesn't exist
        let env_path = temp_dir.path().join(".env");

        // Should still work (silently ignore missing .env)
        let config = AppConfig::load_with_env(&env_path, &config_path).unwrap();

        // Should use TOML value
        assert_eq!(config.runtime.strategy_dir, strategy_dir.to_str().unwrap());
    }
}
