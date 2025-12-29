// Retry logic with exponential backoff for Kalshi API rate limiting
//
// Kalshi API returns 429 status codes when rate limited.
// This module implements exponential backoff retry strategy.

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

use super::error::KalshiError;

// =============================================================================
// RETRY POLICY
// =============================================================================

/// Retry policy for handling rate limits and transient failures
///
/// Implements exponential backoff:
/// - Attempt 0: base_delay_ms
/// - Attempt 1: base_delay_ms * 2
/// - Attempt 2: base_delay_ms * 4
/// - etc., capped at max_delay_ms
///
/// Only retries on RateLimited errors. All other errors fail immediately.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (not counting initial attempt)
    max_retries: u32,

    /// Base delay in milliseconds (first retry waits this long)
    base_delay_ms: u64,

    /// Maximum delay in milliseconds (cap for exponential backoff)
    max_delay_ms: u64,
}

impl RetryPolicy {
    /// Create a new retry policy with custom settings
    ///
    /// # Arguments
    /// * `max_retries` - Maximum retry attempts (default: 3)
    /// * `base_delay_ms` - Initial delay in ms (default: 1000)
    /// * `max_delay_ms` - Maximum delay cap in ms (default: 30000)
    ///
    /// # Examples
    /// ```
    /// use calchas::kalshi::retry::RetryPolicy;
    ///
    /// let policy = RetryPolicy::new(3, 1000, 30000);
    /// ```
    pub fn new(max_retries: u32, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        RetryPolicy {
            max_retries,
            base_delay_ms,
            max_delay_ms,
        }
    }

    /// Create retry policy with default settings
    ///
    /// Defaults:
    /// - max_retries: 3
    /// - base_delay_ms: 1000 (1 second)
    /// - max_delay_ms: 30000 (30 seconds)
    ///
    /// # Examples
    /// ```
    /// use calchas::kalshi::retry::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default();
    /// ```
    pub fn default() -> Self {
        RetryPolicy::new(3, 1000, 30000)
    }

    /// Calculate delay for a given retry attempt
    ///
    /// Uses exponential backoff: base * 2^attempt, capped at max
    ///
    /// # Arguments
    /// * `attempt` - Retry attempt number (0 = first retry, 1 = second, etc.)
    ///
    /// # Returns
    /// Duration to wait before this retry attempt
    ///
    /// # Examples
    /// ```
    /// use calchas::kalshi::retry::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::new(3, 1000, 30000);
    ///
    /// assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(1000));  // 1s
    /// assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(2000));  // 2s
    /// assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(4000));  // 4s
    /// assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(8000));  // 8s
    /// ```
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        // Exponential backoff: base * 2^attempt
        // Protect against overflow by capping shift at 63
        let shift = attempt.min(63);
        let delay_ms = self.base_delay_ms.saturating_mul(1u64 << shift);

        // Cap at max_delay
        let capped_delay_ms = delay_ms.min(self.max_delay_ms);

        Duration::from_millis(capped_delay_ms)
    }

    /// Execute a function with retry logic
    ///
    /// Retries on:
    /// - KalshiError::RateLimited (uses server retry_after or exponential backoff)
    /// - KalshiError::NetworkError (transient network failures, exponential backoff)
    ///
    /// All other errors (auth, parse, etc.) fail immediately without retry.
    ///
    /// # Type Parameters
    /// * `F` - Function that returns a Future
    /// * `Fut` - Future type returned by F
    /// * `T` - Success value type
    ///
    /// # Arguments
    /// * `f` - Async function to execute (will be called up to max_retries + 1 times)
    ///
    /// # Returns
    /// * `Ok(T)` - Success value from f
    /// * `Err(KalshiError)` - Final error after all retries exhausted
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::kalshi::retry::RetryPolicy;
    /// use calchas::kalshi::error::KalshiError;
    ///
    /// # async fn example() -> Result<(), KalshiError> {
    /// let policy = RetryPolicy::default();
    ///
    /// let result = policy.execute(|| async {
    ///     // Your API call here
    ///     Ok::<_, KalshiError>("success")
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute<F, Fut, T>(&self, mut f: F) -> Result<T, KalshiError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, KalshiError>>,
    {
        let mut attempt = 0;

        loop {
            // Execute the function
            match f().await {
                Ok(value) => return Ok(value),

                Err(KalshiError::RateLimited { retry_after_ms }) => {
                    // Check if we've exhausted retries
                    if attempt >= self.max_retries {
                        return Err(KalshiError::RateLimited { retry_after_ms });
                    }

                    // Calculate delay
                    let delay = if let Some(retry_after) = retry_after_ms {
                        // Use server-provided retry_after if available
                        Duration::from_millis(retry_after)
                    } else {
                        // Use exponential backoff
                        self.delay_for_attempt(attempt)
                    };

                    // Log retry attempt (optional - could use tracing here)
                    // For now, retrying silently to avoid dependencies

                    // Wait before retry
                    sleep(delay).await;

                    attempt += 1;
                }

                Err(KalshiError::NetworkError(ref msg)) => {
                    // Retry transient network errors (connection drops, timeouts, etc.)
                    if attempt >= self.max_retries {
                        return Err(KalshiError::NetworkError(msg.clone()));
                    }

                    // Use exponential backoff for network errors
                    let delay = self.delay_for_attempt(attempt);

                    tracing::warn!(
                        "Network error (attempt {}/{}): {}. Retrying in {:.1}s...",
                        attempt + 1,
                        self.max_retries + 1,
                        msg,
                        delay.as_secs_f64()
                    );

                    // Wait before retry
                    sleep(delay).await;

                    attempt += 1;
                }

                Err(other_error) => {
                    // Don't retry auth, parse, or other non-transient errors
                    return Err(other_error);
                }
            }
        }
    }

    /// Get maximum number of retries
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Get base delay in milliseconds
    pub fn base_delay_ms(&self) -> u64 {
        self.base_delay_ms
    }

    /// Get maximum delay in milliseconds
    pub fn max_delay_ms(&self) -> u64 {
        self.max_delay_ms
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_retry_policy_creation() {
        let policy = RetryPolicy::new(5, 500, 10000);
        assert_eq!(policy.max_retries(), 5);
        assert_eq!(policy.base_delay_ms(), 500);
        assert_eq!(policy.max_delay_ms(), 10000);
    }

    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries(), 3);
        assert_eq!(policy.base_delay_ms(), 1000);
        assert_eq!(policy.max_delay_ms(), 30000);
    }

    #[test]
    fn test_exponential_backoff_delay() {
        let policy = RetryPolicy::new(5, 1000, 30000);

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(1000));   // 1s
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(2000));   // 2s
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(4000));   // 4s
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(8000));   // 8s
        assert_eq!(policy.delay_for_attempt(4), Duration::from_millis(16000));  // 16s
        assert_eq!(policy.delay_for_attempt(5), Duration::from_millis(30000));  // 32s capped at 30s
    }

    #[test]
    fn test_delay_cap() {
        let policy = RetryPolicy::new(10, 1000, 5000);

        // Should cap at 5000ms
        assert_eq!(policy.delay_for_attempt(10), Duration::from_millis(5000));
        assert_eq!(policy.delay_for_attempt(100), Duration::from_millis(5000));
    }

    #[test]
    fn test_delay_overflow_protection() {
        let policy = RetryPolicy::new(100, 1000, u64::MAX);

        // Should not panic on large attempt numbers
        let delay = policy.delay_for_attempt(63);  // 2^63 would overflow
        assert!(delay.as_millis() > 0);
    }

    #[tokio::test]
    async fn test_execute_success_first_try() {
        let policy = RetryPolicy::default();

        let result = policy.execute(|| async {
            Ok::<_, KalshiError>(42)
        }).await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_execute_non_retryable_error_no_retry() {
        // Test that non-retryable errors (auth, parse) fail immediately without retry
        let policy = RetryPolicy::default();
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = policy.execute(|| {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                // AuthenticationError should NOT be retried (not transient)
                Err::<(), _>(KalshiError::AuthenticationError("Invalid credentials".to_string()))
            }
        }).await;

        // Should fail immediately without retry
        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);  // Only called once
    }

    #[tokio::test]
    async fn test_execute_network_error_with_retry() {
        // Test that network errors ARE retried (transient failures)
        let policy = RetryPolicy::new(3, 10, 100);  // Fast retries for testing
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = policy.execute(|| {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    // Fail first 2 attempts with network error
                    Err(KalshiError::NetworkError("Connection closed".to_string()))
                } else {
                    // Succeed on 3rd attempt
                    Ok(42)
                }
            }
        }).await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);  // Called 3 times (retried twice)
    }

    #[tokio::test]
    async fn test_execute_rate_limit_with_retry() {
        let policy = RetryPolicy::new(3, 10, 100);  // Fast retries for testing
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = policy.execute(|| {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    // Fail first 2 attempts with rate limit
                    Err(KalshiError::RateLimited { retry_after_ms: None })
                } else {
                    // Succeed on 3rd attempt
                    Ok(42)
                }
            }
        }).await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);  // Called 3 times
    }

    #[tokio::test]
    async fn test_execute_rate_limit_max_retries_exceeded() {
        let policy = RetryPolicy::new(2, 10, 100);  // Only 2 retries
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = policy.execute(|| {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(KalshiError::RateLimited { retry_after_ms: None })
            }
        }).await;

        // Should fail after max_retries + 1 attempts
        assert!(result.is_err());
        match result.unwrap_err() {
            KalshiError::RateLimited { .. } => {},
            _ => panic!("Expected RateLimited error"),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 3);  // Initial + 2 retries
    }

    #[tokio::test]
    async fn test_execute_uses_retry_after_ms() {
        let policy = RetryPolicy::new(3, 1000, 10000);
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let start = std::time::Instant::now();

        let result = policy.execute(|| {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 1 {
                    // Server says retry after 50ms
                    Err(KalshiError::RateLimited { retry_after_ms: Some(50) })
                } else {
                    Ok(42)
                }
            }
        }).await;

        let elapsed = start.elapsed();

        assert_eq!(result.unwrap(), 42);
        // Should have waited ~50ms (not 1000ms from base_delay)
        assert!(elapsed.as_millis() >= 50);
        assert!(elapsed.as_millis() < 500);  // Generous upper bound
    }

    #[tokio::test]
    async fn test_execute_exponential_backoff_timing() {
        let policy = RetryPolicy::new(2, 50, 1000);  // 50ms base
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let start = std::time::Instant::now();

        let _result = policy.execute(|| {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err::<(), _>(KalshiError::RateLimited { retry_after_ms: None })
                } else {
                    Ok(())
                }
            }
        }).await;

        let elapsed = start.elapsed();

        // Should wait 50ms + 100ms = 150ms total
        assert!(elapsed.as_millis() >= 150);
        assert!(elapsed.as_millis() < 500);  // Generous upper bound
    }

    #[test]
    fn test_clone_retry_policy() {
        let policy1 = RetryPolicy::new(5, 2000, 20000);
        let policy2 = policy1.clone();

        assert_eq!(policy1.max_retries(), policy2.max_retries());
        assert_eq!(policy1.base_delay_ms(), policy2.base_delay_ms());
        assert_eq!(policy1.max_delay_ms(), policy2.max_delay_ms());
    }
}
