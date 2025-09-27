use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// CSRF token store for validating tokens from frontend
pub struct CsrfStore {
    tokens: Mutex<HashMap<String, u64>>, // token -> expiry timestamp
}

impl CsrfStore {
    /// Create a new CSRF store
    pub fn new() -> Self {
        CsrfStore {
            tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Store a new token with expiry
    pub fn store_token(&self, token: String) -> crate::error::Result<()> {
        let mut tokens = self.tokens.lock().map_err(|_| crate::error::AppError::SystemError {
            message: "Failed to acquire CSRF token lock".to_string()
        })?;
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_secs() + 1800; // 30 minutes
        tokens.insert(token, expiry);
        Ok(())
    }

    /// Validate a CSRF token
    pub fn validate_token(&self, token: &str) -> bool {
        let tokens = match self.tokens.lock() {
            Ok(tokens) => tokens,
            Err(_) => return false,
        };

        if let Some(expiry) = tokens.get(token) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();
            return now < *expiry;
        }
        false
    }

    /// Clean up expired tokens
    pub fn cleanup_expired(&self) {
        if let Ok(mut tokens) = self.tokens.lock() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();
            tokens.retain(|_, expiry| now < *expiry);
        }
    }
}

impl Default for CsrfStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract CSRF token from command arguments
pub fn extract_csrf_token(args: &Value) -> Option<String> {
    args.as_object()
        .and_then(|obj| obj.get("__csrf_token"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Validate CSRF token from arguments
pub fn validate_csrf(store: &CsrfStore, args: &Value) -> Result<(), String> {
    // Allow commands that don't modify state to pass without CSRF
    // This can be configured based on your needs

    if let Some(token) = extract_csrf_token(args) {
        if store.validate_token(&token) {
            Ok(())
        } else {
            Err("Invalid or expired CSRF token".into())
        }
    } else {
        // CSRF token is required for security
        Err("CSRF token required".into())
    }
}

/// Macro to add CSRF validation to commands
#[macro_export]
macro_rules! validate_csrf {
    ($store:expr, $args:expr) => {{
        if let Err(e) = $crate::middleware::csrf::validate_csrf($store, &serde_json::to_value($args).unwrap_or_else(|_| serde_json::Value::Null)) {
            return Err($crate::error::AppError::SecurityError { message: e });
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_csrf_store() {
        let store = CsrfStore::new();
        let token = "test-token-123".to_string();

        // Store token
        store.store_token(token.clone()).expect("Failed to store test token");

        // Validate token
        assert!(store.validate_token(&token));

        // Invalid token
        assert!(!store.validate_token("invalid-token"));
    }

    #[test]
    fn test_extract_csrf_token() {
        let args = json!({
            "__csrf_token": "test-token",
            "other_param": "value"
        });

        let token = extract_csrf_token(&args);
        assert_eq!(token, Some("test-token".to_string()));
    }

    #[test]
    fn test_validate_csrf() {
        let store = CsrfStore::new();
        let token = "test-token".to_string();
        store.store_token(token.clone()).expect("Failed to store test token");

        let args = json!({
            "__csrf_token": token,
        });

        assert!(validate_csrf(&store, &args).is_ok());

        let invalid_args = json!({
            "__csrf_token": "invalid",
        });

        assert!(validate_csrf(&store, &invalid_args).is_err());
    }
}