// Kalshi API authentication with RSA-PSS signatures
// Implements the authentication scheme from https://docs.kalshi.com/api-reference/authentication

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use rsa::{RsaPrivateKey, pss};
use sha2::Sha256;

use super::error::KalshiError;

// =============================================================================
// AUTHENTICATION HANDLER
// =============================================================================

/// Kalshi API authentication handler
///
/// Handles RSA-PSS signature generation for authenticated requests.
/// Each request must include:
/// - KALSHI-ACCESS-KEY: API key ID
/// - KALSHI-ACCESS-TIMESTAMP: Unix timestamp in milliseconds
/// - KALSHI-ACCESS-SIGNATURE: RSA-PSS signature of "{timestamp}{method}{path}"
#[derive(Debug)]
pub struct KalshiAuth {
    /// API key ID (from Kalshi dashboard)
    api_key_id: String,

    /// RSA private key for signing requests
    private_key: RsaPrivateKey,
}

impl KalshiAuth {
    /// Create authentication handler from API credentials
    ///
    /// # Arguments
    /// * `api_key_id` - API key ID from Kalshi dashboard
    /// * `private_key_pem` - PEM-encoded RSA private key (PKCS#1 or PKCS#8 format)
    ///
    /// # Returns
    /// * `Ok(KalshiAuth)` - Authentication handler ready to sign requests
    /// * `Err(KalshiError)` - If private key parsing fails
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::kalshi::auth::KalshiAuth;
    ///
    /// let pem = "-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----";
    /// let auth = KalshiAuth::new("my-key-id".to_string(), pem.to_string())?;
    /// # Ok::<(), calchas::kalshi::error::KalshiError>(())
    /// ```
    pub fn new(api_key_id: String, private_key_pem: String) -> Result<Self, KalshiError> {
        // Try PKCS#8 format first (-----BEGIN PRIVATE KEY-----)
        let private_key = match RsaPrivateKey::from_pkcs8_pem(&private_key_pem) {
            Ok(key) => key,
            Err(_) => {
                // If PKCS#8 fails, try PKCS#1 format (-----BEGIN RSA PRIVATE KEY-----)
                // This is what Kalshi provides in their dashboard
                RsaPrivateKey::from_pkcs1_pem(&private_key_pem)
                    .map_err(|e| KalshiError::SignatureError(
                        format!("Failed to parse private key (tried both PKCS#8 and PKCS#1 formats): {}", e)
                    ))?
            }
        };

        Ok(KalshiAuth {
            api_key_id,
            private_key,
        })
    }

    /// Generate RSA-PSS signature for API request
    ///
    /// Creates signature of message: `{timestamp}{method}{path}`
    /// Uses RSA-PSS with SHA256, MGF1, and salt_length=DIGEST_LENGTH
    ///
    /// # Arguments
    /// * `method` - HTTP method ("GET", "POST", "DELETE", etc.)
    /// * `path` - API path (e.g., "/trade-api/v2/markets")
    /// * `timestamp` - Unix timestamp in milliseconds
    ///
    /// # Returns
    /// * `Ok(String)` - Base64-encoded signature
    /// * `Err(KalshiError)` - If signature generation fails
    pub fn sign_request(
        &self,
        method: &str,
        path: &str,
        timestamp: i64,
    ) -> Result<String, KalshiError> {
        // Build signature message: {timestamp}{method}{path}
        let message = format!("{}{}{}", timestamp, method, path);

        // Create PSS signing key with SHA256
        let signing_key = pss::SigningKey::<Sha256>::new(self.private_key.clone());

        // Sign the message with PSS padding
        let mut rng = rand::thread_rng();
        let signature = signing_key.sign_with_rng(&mut rng, message.as_bytes());

        // Base64-encode the signature
        let signature_bytes = signature.to_bytes();
        let signature_b64 = BASE64.encode(&signature_bytes);

        Ok(signature_b64)
    }

    /// Generate authentication headers for API request
    ///
    /// Creates the three required headers:
    /// - KALSHI-ACCESS-KEY: API key ID
    /// - KALSHI-ACCESS-TIMESTAMP: Current timestamp (milliseconds)
    /// - KALSHI-ACCESS-SIGNATURE: RSA-PSS signature
    ///
    /// # Arguments
    /// * `method` - HTTP method ("GET", "POST", etc.)
    /// * `path` - API path (e.g., "/trade-api/v2/markets")
    ///
    /// # Returns
    /// * `Ok(HeaderMap)` - Headers ready to add to HTTP request
    /// * `Err(KalshiError)` - If signature generation fails
    ///
    /// # Examples
    /// ```no_run
    /// use calchas::kalshi::auth::KalshiAuth;
    ///
    /// # let pem = "-----BEGIN PRIVATE KEY-----\nMIIE...\n-----END PRIVATE KEY-----";
    /// # let auth = KalshiAuth::new("key-id".to_string(), pem.to_string())?;
    /// let headers = auth.auth_headers("GET", "/trade-api/v2/markets")?;
    /// # Ok::<(), calchas::kalshi::error::KalshiError>(())
    /// ```
    pub fn auth_headers(&self, method: &str, path: &str) -> Result<HeaderMap, KalshiError> {
        // Get current timestamp in milliseconds
        let timestamp = Utc::now().timestamp_millis();

        // Generate signature
        let signature = self.sign_request(method, path, timestamp)?;

        // Build header map
        let mut headers = HeaderMap::new();

        // KALSHI-ACCESS-KEY
        headers.insert(
            HeaderName::from_static("kalshi-access-key"),
            HeaderValue::from_str(&self.api_key_id)
                .map_err(|e| KalshiError::AuthenticationError(format!("Invalid API key ID: {}", e)))?,
        );

        // KALSHI-ACCESS-TIMESTAMP
        headers.insert(
            HeaderName::from_static("kalshi-access-timestamp"),
            HeaderValue::from_str(&timestamp.to_string())
                .map_err(|e| KalshiError::AuthenticationError(format!("Invalid timestamp: {}", e)))?,
        );

        // KALSHI-ACCESS-SIGNATURE
        headers.insert(
            HeaderName::from_static("kalshi-access-signature"),
            HeaderValue::from_str(&signature)
                .map_err(|e| KalshiError::AuthenticationError(format!("Invalid signature: {}", e)))?,
        );

        Ok(headers)
    }

    /// Get API key ID
    pub fn api_key_id(&self) -> &str {
        &self.api_key_id
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test RSA private key (2048-bit, PKCS#8 format)
    // Generated for testing only - DO NOT use in production
    const TEST_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
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
-----END PRIVATE KEY-----"#;

    fn create_test_auth() -> KalshiAuth {
        KalshiAuth::new(
            "test-key-id".to_string(),
            TEST_PRIVATE_KEY.to_string(),
        ).expect("Failed to create test auth")
    }

    #[test]
    fn test_auth_creation() {
        let auth = create_test_auth();
        assert_eq!(auth.api_key_id(), "test-key-id");
    }

    #[test]
    fn test_invalid_private_key() {
        let result = KalshiAuth::new(
            "test-key".to_string(),
            "not a valid PEM key".to_string(),
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            KalshiError::SignatureError(msg) => {
                assert!(msg.contains("Failed to parse private key"));
            }
            _ => panic!("Expected SignatureError"),
        }
    }

    #[test]
    fn test_sign_request_deterministic_message() {
        let auth = create_test_auth();

        let timestamp = 1234567890123i64;
        let method = "GET";
        let path = "/trade-api/v2/markets";

        // Sign twice with same inputs
        let signature1 = auth.sign_request(method, path, timestamp).unwrap();
        let signature2 = auth.sign_request(method, path, timestamp).unwrap();

        // RSA-PSS uses random salt, so signatures will differ
        // But they should both be valid base64 strings
        assert!(BASE64.decode(&signature1).is_ok());
        assert!(BASE64.decode(&signature2).is_ok());

        // Signatures should be non-empty and have reasonable length
        assert!(!signature1.is_empty());
        assert!(signature1.len() > 100);  // RSA-2048 signature is ~256 bytes base64-encoded
    }

    #[test]
    fn test_sign_request_different_methods() {
        let auth = create_test_auth();
        let timestamp = 1234567890123i64;
        let path = "/trade-api/v2/markets";

        let sig_get = auth.sign_request("GET", path, timestamp).unwrap();
        let sig_post = auth.sign_request("POST", path, timestamp).unwrap();

        // Different methods should produce different signature messages
        // (though signatures themselves are random due to PSS)
        assert_ne!(sig_get, sig_post);
    }

    #[test]
    fn test_sign_request_different_paths() {
        let auth = create_test_auth();
        let timestamp = 1234567890123i64;

        let sig1 = auth.sign_request("GET", "/trade-api/v2/markets", timestamp).unwrap();
        let sig2 = auth.sign_request("GET", "/trade-api/v2/portfolio", timestamp).unwrap();

        // Different paths should produce different signatures
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_auth_headers_structure() {
        let auth = create_test_auth();

        let headers = auth.auth_headers("GET", "/trade-api/v2/markets").unwrap();

        // Check all three required headers are present
        assert!(headers.contains_key("kalshi-access-key"));
        assert!(headers.contains_key("kalshi-access-timestamp"));
        assert!(headers.contains_key("kalshi-access-signature"));

        // Check values
        let key = headers.get("kalshi-access-key").unwrap().to_str().unwrap();
        assert_eq!(key, "test-key-id");

        let timestamp = headers.get("kalshi-access-timestamp").unwrap().to_str().unwrap();
        assert!(timestamp.parse::<i64>().is_ok());

        let signature = headers.get("kalshi-access-signature").unwrap().to_str().unwrap();
        assert!(BASE64.decode(signature).is_ok());
    }

    #[test]
    fn test_auth_headers_timestamp_is_current() {
        let auth = create_test_auth();

        let before = Utc::now().timestamp_millis();
        let headers = auth.auth_headers("GET", "/trade-api/v2/markets").unwrap();
        let after = Utc::now().timestamp_millis();

        let timestamp_str = headers.get("kalshi-access-timestamp").unwrap().to_str().unwrap();
        let timestamp = timestamp_str.parse::<i64>().unwrap();

        // Timestamp should be between before and after
        assert!(timestamp >= before);
        assert!(timestamp <= after);
    }

    #[test]
    fn test_signature_message_format() {
        let auth = create_test_auth();

        // Test that signature message follows {timestamp}{method}{path} format
        let timestamp = 1234567890123i64;
        let method = "POST";
        let path = "/trade-api/v2/orders";

        // Should create message: "1234567890123POST/trade-api/v2/orders"
        let signature = auth.sign_request(method, path, timestamp);
        assert!(signature.is_ok());
    }

    #[test]
    fn test_base64_encoding() {
        let auth = create_test_auth();

        let signature = auth.sign_request("GET", "/test", 123456).unwrap();

        // Verify it's valid base64
        let decoded = BASE64.decode(&signature);
        assert!(decoded.is_ok());

        // RSA-2048 signature should be 256 bytes
        let decoded_bytes = decoded.unwrap();
        assert_eq!(decoded_bytes.len(), 256);
    }

    #[test]
    fn test_multiple_header_generation() {
        let auth = create_test_auth();

        // Generate headers twice
        let headers1 = auth.auth_headers("GET", "/markets").unwrap();
        let headers2 = auth.auth_headers("GET", "/markets").unwrap();

        // Keys should be the same
        assert_eq!(
            headers1.get("kalshi-access-key"),
            headers2.get("kalshi-access-key")
        );

        // Timestamps should be different (or very close)
        let ts1 = headers1.get("kalshi-access-timestamp").unwrap();
        let ts2 = headers2.get("kalshi-access-timestamp").unwrap();
        // They might be equal if generated in same millisecond, which is fine

        // Signatures should be different (due to different timestamps + random salt)
        // Unless timestamps are identical (unlikely but possible in tests)
        if ts1 != ts2 {
            assert_ne!(
                headers1.get("kalshi-access-signature"),
                headers2.get("kalshi-access-signature")
            );
        }
    }
}
