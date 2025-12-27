// Kalshi API error types
// Follows existing error handling pattern from src/config/mod.rs

/// Errors that can occur when interacting with the Kalshi API
#[derive(Debug)]
pub enum KalshiError {
    /// Network or HTTP communication error
    NetworkError(String),

    /// Kalshi API returned an error response
    ApiError {
        status_code: u16,
        message: String,
    },

    /// Rate limited by Kalshi API (429 response)
    RateLimited {
        retry_after_ms: Option<u64>,
    },

    /// Authentication failed (invalid credentials, expired token, etc.)
    AuthenticationError(String),

    /// Failed to generate RSA signature
    SignatureError(String),

    /// Failed to parse JSON response
    ParseError(String),

    /// Configuration error (missing credentials, invalid settings, etc.)
    ConfigError(String),

    /// Invalid request parameters
    InvalidRequest(String),
}

impl std::fmt::Display for KalshiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KalshiError::NetworkError(msg) => {
                write!(f, "Network error: {}", msg)
            }
            KalshiError::ApiError { status_code, message } => {
                write!(f, "Kalshi API error ({}): {}", status_code, message)
            }
            KalshiError::RateLimited { retry_after_ms } => {
                match retry_after_ms {
                    Some(ms) => write!(f, "Rate limited, retry after {}ms", ms),
                    None => write!(f, "Rate limited"),
                }
            }
            KalshiError::AuthenticationError(msg) => {
                write!(f, "Authentication error: {}", msg)
            }
            KalshiError::SignatureError(msg) => {
                write!(f, "Signature generation error: {}", msg)
            }
            KalshiError::ParseError(msg) => {
                write!(f, "JSON parse error: {}", msg)
            }
            KalshiError::ConfigError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            KalshiError::InvalidRequest(msg) => {
                write!(f, "Invalid request: {}", msg)
            }
        }
    }
}

impl std::error::Error for KalshiError {}

// =============================================================================
// FROM IMPLEMENTATIONS (Convert external errors to KalshiError)
// =============================================================================

impl From<reqwest::Error> for KalshiError {
    fn from(err: reqwest::Error) -> Self {
        KalshiError::NetworkError(err.to_string())
    }
}

impl From<serde_json::Error> for KalshiError {
    fn from(err: serde_json::Error) -> Self {
        KalshiError::ParseError(err.to_string())
    }
}

impl From<rsa::Error> for KalshiError {
    fn from(err: rsa::Error) -> Self {
        KalshiError::SignatureError(err.to_string())
    }
}

impl From<rsa::pkcs8::Error> for KalshiError {
    fn from(err: rsa::pkcs8::Error) -> Self {
        KalshiError::SignatureError(format!("PKCS8 error: {}", err))
    }
}

impl From<pem::PemError> for KalshiError {
    fn from(err: pem::PemError) -> Self {
        KalshiError::ConfigError(format!("PEM parsing error: {}", err))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_error_display() {
        let err = KalshiError::NetworkError("Connection timeout".to_string());
        assert_eq!(err.to_string(), "Network error: Connection timeout");
    }

    #[test]
    fn test_api_error_display() {
        let err = KalshiError::ApiError {
            status_code: 404,
            message: "Market not found".to_string(),
        };
        assert_eq!(err.to_string(), "Kalshi API error (404): Market not found");
    }

    #[test]
    fn test_rate_limited_with_retry_after() {
        let err = KalshiError::RateLimited {
            retry_after_ms: Some(5000),
        };
        assert_eq!(err.to_string(), "Rate limited, retry after 5000ms");
    }

    #[test]
    fn test_rate_limited_without_retry_after() {
        let err = KalshiError::RateLimited {
            retry_after_ms: None,
        };
        assert_eq!(err.to_string(), "Rate limited");
    }

    #[test]
    fn test_authentication_error_display() {
        let err = KalshiError::AuthenticationError("Invalid API key".to_string());
        assert_eq!(err.to_string(), "Authentication error: Invalid API key");
    }

    #[test]
    fn test_signature_error_display() {
        let err = KalshiError::SignatureError("RSA signing failed".to_string());
        assert_eq!(err.to_string(), "Signature generation error: RSA signing failed");
    }

    #[test]
    fn test_parse_error_display() {
        let err = KalshiError::ParseError("Invalid JSON".to_string());
        assert_eq!(err.to_string(), "JSON parse error: Invalid JSON");
    }

    #[test]
    fn test_config_error_display() {
        let err = KalshiError::ConfigError("Missing API key".to_string());
        assert_eq!(err.to_string(), "Configuration error: Missing API key");
    }

    #[test]
    fn test_invalid_request_display() {
        let err = KalshiError::InvalidRequest("Limit must be 1-1000".to_string());
        assert_eq!(err.to_string(), "Invalid request: Limit must be 1-1000");
    }

    #[test]
    fn test_error_trait_implementation() {
        let err = KalshiError::NetworkError("test".to_string());
        // Should implement std::error::Error trait
        let _: &dyn std::error::Error = &err;
    }
}
