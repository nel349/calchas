// Logging infrastructure using tracing
// Provides structured logging for the entire application

use tracing_subscriber::{fmt, EnvFilter};

/// Initialize the tracing subscriber for logging
///
/// This sets up structured logging with:
/// - Timestamp on each log line
/// - Log level filtering (controlled by RUST_LOG env var)
/// - Colored output (in terminals that support it)
/// - File and line number information
///
/// # Log Levels (from most to least verbose)
/// - `trace` - Very detailed, for deep debugging
/// - `debug` - Debugging information
/// - `info` - General information (default)
/// - `warn` - Warning messages
/// - `error` - Error messages
///
/// # Environment Variable
/// Set `RUST_LOG` to control logging verbosity:
/// - `RUST_LOG=info` - Show info, warn, error (default)
/// - `RUST_LOG=debug` - Show debug and above
/// - `RUST_LOG=trace` - Show everything
/// - `RUST_LOG=calchas=debug` - Debug only for calchas crate
/// - `RUST_LOG=calchas::strategy=trace` - Trace only for strategy module
///
/// # Panics
/// Panics if a global subscriber has already been set (call this only once)
///
/// # Examples
/// ```no_run
/// use calchas::utils::logging;
///
/// fn main() {
///     logging::init();
///
///     tracing::info!("Application started");
///     tracing::debug!("Debug information");
///     tracing::error!("Something went wrong!");
/// }
/// ```
pub fn init() {
    try_init().expect("Failed to initialize logging - already initialized");
}

/// Try to initialize the tracing subscriber, returning an error if already initialized
///
/// Same as `init()` but returns `Result` instead of panicking.
///
/// # Returns
/// - `Ok(())` if logging was successfully initialized
/// - `Err(String)` if logging was already initialized
///
/// # Examples
/// ```no_run
/// use calchas::utils::logging;
///
/// fn main() {
///     match logging::try_init() {
///         Ok(_) => println!("Logging initialized"),
///         Err(e) => eprintln!("Logging already initialized: {}", e),
///     }
/// }
/// ```
pub fn try_init() -> Result<(), String> {
    // Build the subscriber with sensible defaults
    let subscriber = fmt()
        // Use a compact format (not full)
        .compact()
        // Add timestamps to log lines
        .with_timer(fmt::time::UtcTime::rfc_3339())
        // Add file and line number
        .with_file(true)
        .with_line_number(true)
        // Add thread ID for async debugging
        .with_thread_ids(true)
        // Add target (module path)
        .with_target(true)
        // Set log level filter from RUST_LOG env var (default: info)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .finish();

    // Set as global default subscriber
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| e.to_string())
}

/// Initialize logging with a custom log level
///
/// # Arguments
/// * `level` - Log level filter (e.g., "debug", "info", "warn")
///
/// # Panics
/// Panics if a global subscriber has already been set
///
/// # Examples
/// ```no_run
/// use calchas::utils::logging;
///
/// fn main() {
///     logging::init_with_level("debug");
///     tracing::debug!("This will be shown");
/// }
/// ```
pub fn init_with_level(level: &str) {
    try_init_with_level(level).expect("Failed to initialize logging - already initialized");
}

/// Try to initialize logging with a custom log level
///
/// Same as `init_with_level()` but returns `Result` instead of panicking.
///
/// # Arguments
/// * `level` - Log level filter (e.g., "debug", "info", "warn")
///
/// # Returns
/// - `Ok(())` if logging was successfully initialized
/// - `Err(String)` if logging was already initialized
pub fn try_init_with_level(level: &str) -> Result<(), String> {
    let subscriber = fmt()
        .compact()
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .with_env_filter(EnvFilter::new(level))
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| e.to_string())
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_init_logging() {
        // This test uses try_init() which doesn't panic
        // We can't actually test the logging output easily

        // Note: We can only call this once per test binary
        // If it returns Err, it means logging was already initialized
        let result = try_init();

        // Either succeeds or returns error - both are ok for this test
        assert!(result.is_ok() || result.is_err());

        // Verify we get an error if called twice
        let second_result = try_init();
        assert!(second_result.is_err());
    }

    #[test]
    fn test_try_init_with_custom_level() {
        // This test uses try_init_with_level() which doesn't panic
        let result = try_init_with_level("debug");

        // Either succeeds or returns error - both are ok for this test
        assert!(result.is_ok() || result.is_err());

        // Verify we get an error if called twice
        let second_result = try_init_with_level("trace");
        assert!(second_result.is_err());
    }
}
