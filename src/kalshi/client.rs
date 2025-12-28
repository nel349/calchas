// Kalshi API HTTP client
// Handles authenticated requests to Kalshi REST API

use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;

use super::auth::KalshiAuth;
use super::error::KalshiError;
use super::retry::RetryPolicy;
use super::types::{GetMarketsRequest, KalshiMarket, MarketsResponse};
use crate::config::KalshiConfig;

// =============================================================================
// KALSHI HTTP CLIENT
// =============================================================================

/// Kalshi API HTTP client
///
/// Handles:
/// - RSA-PSS signature authentication
/// - Rate limit retries with exponential backoff
/// - Automatic pagination for bulk data fetching
/// - JSON serialization/deserialization
///
/// # Examples
/// ```no_run
/// use calchas::config::AppConfig;
/// use calchas::kalshi::client::KalshiClient;
/// use calchas::kalshi::types::GetMarketsRequest;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = AppConfig::load_default()?;
/// let client = KalshiClient::from_config(&config.kalshi)?;
///
/// let request = GetMarketsRequest {
///     limit: Some(50),
///     status: Some("open".to_string()),
///     ..Default::default()
/// };
///
/// let response = client.get_markets(request).await?;
/// println!("Fetched {} markets", response.markets.len());
/// # Ok(())
/// # }
/// ```
pub struct KalshiClient {
    /// Underlying HTTP client
    http_client: Client,

    /// Authentication handler
    auth: KalshiAuth,

    /// Base URL for API (demo or production)
    base_url: String,

    /// Retry policy for rate limiting
    retry_policy: RetryPolicy,
}

impl KalshiClient {
    /// Create Kalshi client from configuration
    ///
    /// # Arguments
    /// * `config` - Kalshi configuration with API credentials
    ///
    /// # Returns
    /// * `Ok(KalshiClient)` - Ready to make API calls
    /// * `Err(KalshiError)` - If authentication setup fails
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::config::{AppConfig, KalshiConfig};
    /// use calchas::kalshi::client::KalshiClient;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AppConfig::load_default()?;
    /// let client = KalshiClient::from_config(&config.kalshi)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config(config: &KalshiConfig) -> Result<Self, KalshiError> {
        // Create authentication handler
        let auth = KalshiAuth::new(
            config.api_key_id.clone(),
            config.private_key.clone(),
        )?;

        // Create HTTP client
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| KalshiError::NetworkError(e.to_string()))?;

        Ok(KalshiClient {
            http_client,
            auth,
            base_url: config.base_url().to_string(),
            retry_policy: RetryPolicy::default(),
        })
    }

    /// Create Kalshi client with custom retry policy
    ///
    /// # Arguments
    /// * `config` - Kalshi configuration
    /// * `retry_policy` - Custom retry policy for rate limiting
    pub fn with_retry_policy(
        config: &KalshiConfig,
        retry_policy: RetryPolicy,
    ) -> Result<Self, KalshiError> {
        let mut client = Self::from_config(config)?;
        client.retry_policy = retry_policy;
        Ok(client)
    }

    /// Fetch a single page of markets
    ///
    /// # Arguments
    /// * `request` - Market query parameters (limit, cursor, status, etc.)
    ///
    /// # Returns
    /// * `Ok(MarketsResponse)` - Page of markets with optional cursor for next page
    /// * `Err(KalshiError)` - If request fails
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::config::AppConfig;
    /// use calchas::kalshi::client::KalshiClient;
    /// use calchas::kalshi::types::GetMarketsRequest;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = AppConfig::load_default()?;
    /// # let client = KalshiClient::from_config(&config.kalshi)?;
    /// let request = GetMarketsRequest {
    ///     limit: Some(100),
    ///     status: Some("open".to_string()),
    ///     ..Default::default()
    /// };
    ///
    /// let response = client.get_markets(request).await?;
    /// for market in &response.markets {
    ///     println!("{}: {}", market.ticker, market.title);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_markets(
        &self,
        request: GetMarketsRequest,
    ) -> Result<MarketsResponse, KalshiError> {
        // Build query parameters
        let mut query_params = Vec::new();

        if let Some(limit) = request.limit {
            query_params.push(("limit", limit.to_string()));
        }

        if let Some(cursor) = &request.cursor {
            query_params.push(("cursor", cursor.clone()));
        }

        if let Some(status) = &request.status {
            query_params.push(("status", status.clone()));
        }

        if let Some(series_ticker) = &request.series_ticker {
            query_params.push(("series_ticker", series_ticker.clone()));
        }

        if let Some(min_close_ts) = request.min_close_ts {
            query_params.push(("min_close_ts", min_close_ts.to_string()));
        }

        if let Some(max_close_ts) = request.max_close_ts {
            query_params.push(("max_close_ts", max_close_ts.to_string()));
        }

        // Convert to &str references
        let query_params_ref: Vec<(&str, &str)> = query_params
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        // Make authenticated request
        let query_option = if query_params_ref.is_empty() {
            None
        } else {
            Some(query_params_ref.as_slice())
        };

        self.request(Method::GET, "/markets", query_option).await
    }

    /// Fetch all markets with automatic pagination
    ///
    /// Repeatedly calls `get_markets` and follows cursors until all pages are fetched.
    ///
    /// # Arguments
    /// * `request` - Market query parameters (limit is used per page, cursor is ignored)
    ///
    /// # Returns
    /// * `Ok(Vec<KalshiMarket>)` - All markets matching the query
    /// * `Err(KalshiError)` - If any request fails
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::config::AppConfig;
    /// use calchas::kalshi::client::KalshiClient;
    /// use calchas::kalshi::types::GetMarketsRequest;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = AppConfig::load_default()?;
    /// # let client = KalshiClient::from_config(&config.kalshi)?;
    /// // Fetch ALL open markets (may take multiple requests)
    /// let request = GetMarketsRequest {
    ///     limit: Some(1000),  // Max per page
    ///     status: Some("open".to_string()),
    ///     ..Default::default()
    /// };
    ///
    /// let all_markets = client.get_all_markets(request).await?;
    /// println!("Total markets: {}", all_markets.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_all_markets(
        &self,
        mut request: GetMarketsRequest,
    ) -> Result<Vec<KalshiMarket>, KalshiError> {
        let mut all_markets = Vec::new();
        let mut cursor = None;

        loop {
            // Set cursor from previous page
            request.cursor = cursor;

            // Fetch page
            let response = self.get_markets(request.clone()).await?;

            // Append markets
            all_markets.extend(response.markets);

            // Check if there's a next page
            match response.cursor {
                Some(next_cursor) if !next_cursor.is_empty() => {
                    cursor = Some(next_cursor);
                }
                _ => break,  // No more pages
            }
        }

        Ok(all_markets)
    }

    /// Make authenticated HTTP request to Kalshi API
    ///
    /// Internal helper that:
    /// 1. Builds full URL
    /// 2. Generates authentication headers
    /// 3. Makes HTTP request
    /// 4. Handles errors (rate limiting, auth failures, etc.)
    /// 5. Deserializes JSON response
    ///
    /// Automatically retries on rate limits using the retry policy.
    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<&[(&str, &str)]>,
    ) -> Result<T, KalshiError> {
        // Retry wrapper
        self.retry_policy
            .execute(|| async {
                // Build URL
                let url = format!("{}{}", self.base_url, path);

                // Generate auth headers
                let auth_headers = self.auth.auth_headers(method.as_str(), path)?;

                // Build request
                let mut request_builder = self.http_client.request(method.clone(), &url);

                // Add query parameters
                if let Some(query_params) = query {
                    request_builder = request_builder.query(query_params);
                }

                // Add auth headers
                for (key, value) in auth_headers.iter() {
                    request_builder = request_builder.header(key, value);
                }

                // Send request
                let response = request_builder
                    .send()
                    .await
                    .map_err(|e| KalshiError::NetworkError(e.to_string()))?;

                // Check status code
                let status = response.status();

                match status {
                    StatusCode::OK => {
                        // Success - deserialize JSON
                        let body = response
                            .json::<T>()
                            .await
                            .map_err(|e| KalshiError::ParseError(e.to_string()))?;

                        Ok(body)
                    }

                    StatusCode::TOO_MANY_REQUESTS => {
                        // Rate limited - extract retry_after if available
                        let retry_after_ms = response
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(|seconds| seconds * 1000);  // Convert to milliseconds

                        Err(KalshiError::RateLimited { retry_after_ms })
                    }

                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                        // Authentication error
                        let error_msg = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Authentication failed".to_string());

                        Err(KalshiError::AuthenticationError(error_msg))
                    }

                    StatusCode::BAD_REQUEST => {
                        // Invalid request
                        let error_msg = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Bad request".to_string());

                        Err(KalshiError::InvalidRequest(error_msg))
                    }

                    _ => {
                        // Other API error
                        let error_msg = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());

                        Err(KalshiError::ApiError {
                            status_code: status.as_u16(),
                            message: error_msg,
                        })
                    }
                }
            })
            .await
    }

    /// Get the base URL being used (for debugging)
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the API key ID being used (for debugging)
    pub fn api_key_id(&self) -> &str {
        self.auth.api_key_id()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test with fake credentials (won't make real API calls)
    fn create_test_config() -> KalshiConfig {
        KalshiConfig {
            use_demo: true,
            api_key_id: "test-key-id".to_string(),
            private_key: r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC+wwxqGkTth05A
Hqs/bSLERUCm+DkooMhRtftBtJeCDGg4mgc9p9D8fNOnGppBockShtVk8vWc01C6
0oxK+PdK0kmcZJrruP4eQlKn80XHp+xRZnnwXlWuYUZQY5WNpoLXEmcVI0Aeq/f8
TB88C6jW2kwjp7vvDNWorazV+GoL8MWCuJyElSPM9x4QHWjuJBYe/TdDED49nluZ
jfZvytvsnnO3sWOp0JKVNzS4D0EBgAv3OMsNec7UZvMscBzW/8ZIr0yVYzyGQkJF
F5ubd5nqNYcyoH4qEL+AOKfvLKXpiDMLOBPNGtxZRQkZ897PxHXNyEnCynQlNMpu
HedrDx3nAgMBAAECggEARncFMHJYsMcQ1CWgC1NUityr96Fsd8IAjRJkaA5+Asdz
fikDcLZ7P7Eae7kbbxAElsfgrqQCrzXttb0NnqodqvFHyLHu+hEBKYtFPg3iYlB4
vk7Uz0IBc2MyVoKanVL7NNfy5P968XmDppo11XfXG9pSUr9kb/a1O9Q/qmBTR+o5
9Wtf1gwbdpex2yhKQ6Jxq1ZpeX1kFDk6KPnbW1cFOYE0/xwVAADtF50hI9lLdWLR
SD0an72dHJkRMejSP/Ef9FEWFCN14z0jCjEoFEth1vovuOCni0c4SfNZFMH7MClO
4MHYM7Gbygr9AHCjWaGlMcXudpI4SBJB8zwWnU9JFQKBgQDz3FX1fBgF2lT7sJyq
NO6uPN4iuJwnwDMM3XilTLJs7+Mcm744KrtdC4QPF/nLEsrYblQPosINlUFUUwED
xj2zDcAG6MFHzccXc6IXBmwtP+wQKITAOSSkY07NNhmlm11ylGMgVBcCGm0YCduN
5HSnq5DW4TN5GPo1BT92em/DZQKBgQDIQgr5X+lD7NDGY8YJhm0dq/Ijq0uARLa/
2IuPkZD9qCfAylUvwdTt4JnkRJzN5Ec0rR0F8GmWZ+qL7Q4q6bZE+t9B+zq60k31
AT6W619VhhLDxltgaxW46d9uvhi6X2jEbdXrSZQZJee8EWpUxrD2h75wV93waWcp
VdV4VTH1WwKBgEZ2v1vczLA8Q1wqz0obW3B7ZBCSWYTe86FfCXJyNAhoVK66jf96
0YL0Red6nRJBzt01HBMci4gTPbpY9a0ahk+LxJX6gYb2/fVX01ll4LI+iz6sBpfo
qx7ZFzcSz9xbhWgLWo1H3xIbgrR0fL2GavLcD1EX56CxR/M0Hf9lJ5BFAoGBAIj/
/Yf7IJcylOWUbnAnwdBxyJa0YlOfLrLyjw+qE1olRwTypvKkFWqjpERw2CFXEYus
/tUIUwPtlZ0ikPW0q9hnFIOMPvJ+W4zIzCvtXGwi7AV5VxwQRm0ZupyFel9OVFtF
lPqBfMrzjqSv+WGECJ6v4Q30XsZRJZ02tnK7PhFPAoGADG37fSIeTs5M7FhtHtug
DzeJY7yT15Mz7DOXHszUX6OsMH58mCBsKb6QGH+bdspza7j8vPoDcE/l2Fd3uMME
6dYtkUkCoKSuFOcdNPix9pVK4HE2ZR6MEgaW0c2kmZ7qKTCuXuCYKzLT7nh3kxCe
YoM11+R+b4hcoWWA8nr+S50=
-----END PRIVATE KEY-----"#.to_string(),
        }
    }

    #[test]
    fn test_client_creation() {
        let config = create_test_config();
        let client = KalshiClient::from_config(&config);

        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.api_key_id(), "test-key-id");
        assert_eq!(client.base_url(), "https://demo-api.kalshi.co/trade-api/v2");
    }

    #[test]
    fn test_client_with_production_url() {
        let mut config = create_test_config();
        config.use_demo = false;

        let client = KalshiClient::from_config(&config).unwrap();
        assert_eq!(client.base_url(), "https://api.elections.kalshi.com/trade-api/v2");
    }

    #[test]
    fn test_client_with_custom_retry_policy() {
        let config = create_test_config();
        let retry_policy = RetryPolicy::new(5, 500, 10000);

        let client = KalshiClient::with_retry_policy(&config, retry_policy);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_invalid_private_key() {
        let mut config = create_test_config();
        config.private_key = "invalid key".to_string();

        let result = KalshiClient::from_config(&config);
        assert!(result.is_err());
    }

    // Note: Real API integration tests should be marked with #[ignore]
    // and require valid credentials in .env file
}
