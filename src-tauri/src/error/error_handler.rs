use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, warn, debug};

/// Systematic error handling framework
/// Provides consistent error recovery, logging, and user feedback patterns
pub struct ErrorHandler {
    error_counts: std::sync::Arc<std::sync::Mutex<HashMap<String, u32>>>,
    recovery_strategies: HashMap<String, RecoveryStrategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub operation: String,
    pub file_path: Option<String>,
    pub user_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub retry_count: u32,
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Low,      // Non-blocking, user can continue
    Medium,   // Partially blocking, affects some functionality
    High,     // Blocking, requires user attention
    Critical, // System-level failure, requires immediate action
}

#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    Retry { max_attempts: u32, backoff_ms: u64 },
    Fallback { fallback_fn: fn() -> Result<(), AppError> },
    UserPrompt { message: String },
    Ignore,
    FailSafe,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorReport {
    pub error_type: String,
    pub error_message: String,
    pub user_message: String,
    pub severity: ErrorSeverity,
    pub context: ErrorContext,
    pub suggested_actions: Vec<String>,
    pub can_retry: bool,
    pub is_recoverable: bool,
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorHandler {
    pub fn new() -> Self {
        let mut recovery_strategies = HashMap::new();

        // Define systematic recovery strategies for different error types
        recovery_strategies.insert(
            "AI_ERROR".to_string(),
            RecoveryStrategy::Retry { max_attempts: 3, backoff_ms: 1000 }
        );
        recovery_strategies.insert(
            "NETWORK_ERROR".to_string(),
            RecoveryStrategy::Retry { max_attempts: 5, backoff_ms: 2000 }
        );
        recovery_strategies.insert(
            "DATABASE_ERROR".to_string(),
            RecoveryStrategy::Retry { max_attempts: 2, backoff_ms: 500 }
        );
        recovery_strategies.insert(
            "FILE_NOT_FOUND".to_string(),
            RecoveryStrategy::UserPrompt { message: "Please check the file path and try again".to_string() }
        );
        recovery_strategies.insert(
            "RESOURCE_LIMIT_EXCEEDED".to_string(),
            RecoveryStrategy::FailSafe
        );
        recovery_strategies.insert(
            "VALIDATION_ERROR".to_string(),
            RecoveryStrategy::UserPrompt { message: "Please correct the input and try again".to_string() }
        );

        Self {
            error_counts: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            recovery_strategies,
        }
    }

    /// Handle an error systematically with proper logging, recovery, and user feedback
    pub async fn handle_error(
        &self,
        error: AppError,
        context: ErrorContext,
    ) -> Result<ErrorReport, AppError> {
        let error_type = error.error_type();
        let severity = self.determine_severity(&error, &context);

        // Log the error with appropriate level
        match severity {
            ErrorSeverity::Critical => {
                error!("CRITICAL ERROR in {}: {} (retry: {})",
                       context.operation, error, context.retry_count);
            }
            ErrorSeverity::High => {
                error!("HIGH SEVERITY ERROR in {}: {} (retry: {})",
                       context.operation, error, context.retry_count);
            }
            ErrorSeverity::Medium => {
                warn!("ERROR in {}: {} (retry: {})",
                      context.operation, error, context.retry_count);
            }
            ErrorSeverity::Low => {
                debug!("Minor error in {}: {} (retry: {})",
                       context.operation, error, context.retry_count);
            }
        }

        // Track error frequency for pattern detection
        self.increment_error_count(&error_type);

        // Determine recovery strategy
        let recovery_strategy = self.recovery_strategies
            .get(&error_type)
            .cloned()
            .unwrap_or(RecoveryStrategy::Ignore);

        // Generate user-friendly error report
        let error_report = self.create_error_report(error, context, severity, &recovery_strategy);

        // Check for error patterns that might indicate systemic issues
        self.check_error_patterns(&error_type).await;

        Ok(error_report)
    }

    /// Determine error severity based on error type and context
    fn determine_severity(&self, error: &AppError, context: &ErrorContext) -> ErrorSeverity {
        match error {
            AppError::ResourceLimitExceeded { .. } => ErrorSeverity::Critical,
            AppError::StorageFull => ErrorSeverity::Critical,
            AppError::SecurityError { .. } => ErrorSeverity::High,
            AppError::DatabaseError { .. } => {
                if context.retry_count >= 2 {
                    ErrorSeverity::High
                } else {
                    ErrorSeverity::Medium
                }
            }
            AppError::AiError { .. } => {
                if context.retry_count >= 3 {
                    ErrorSeverity::Medium
                } else {
                    ErrorSeverity::Low
                }
            }
            AppError::NetworkError { .. } => ErrorSeverity::Low,
            AppError::FileNotFound { .. } => ErrorSeverity::Medium,
            AppError::InvalidInput { .. } => ErrorSeverity::Low,
            AppError::ValidationError { .. } => ErrorSeverity::Low,
            AppError::ProcessingError { .. } => ErrorSeverity::Medium,
            AppError::Cancelled => ErrorSeverity::Low,
            _ => ErrorSeverity::Medium,
        }
    }

    /// Create a comprehensive error report for frontend consumption
    fn create_error_report(
        &self,
        error: AppError,
        context: ErrorContext,
        severity: ErrorSeverity,
        recovery_strategy: &RecoveryStrategy,
    ) -> ErrorReport {
        let error_type = error.error_type();
        let error_message = error.to_string();
        let user_message = error.user_message();

        let (can_retry, suggested_actions) = match recovery_strategy {
            RecoveryStrategy::Retry { max_attempts, .. } => {
                let can_retry = context.retry_count < *max_attempts;
                let actions = if can_retry {
                    vec!["Try again".to_string(), "Check your connection".to_string()]
                } else {
                    vec!["Contact support".to_string(), "Check system status".to_string()]
                };
                (can_retry, actions)
            }
            RecoveryStrategy::UserPrompt { message } => {
                (false, vec![message.clone(), "Review input data".to_string()])
            }
            RecoveryStrategy::FailSafe => {
                (false, vec!["Free up system resources".to_string(), "Try again later".to_string()])
            }
            RecoveryStrategy::Fallback { .. } => {
                (true, vec!["Use basic mode".to_string(), "Try with reduced settings".to_string()])
            }
            RecoveryStrategy::Ignore => {
                (false, vec!["Continue with other operations".to_string()])
            }
        };

        let is_recoverable = matches!(recovery_strategy,
            RecoveryStrategy::Retry { .. } |
            RecoveryStrategy::Fallback { .. } |
            RecoveryStrategy::UserPrompt { .. }
        );

        ErrorReport {
            error_type,
            error_message,
            user_message,
            severity,
            context,
            suggested_actions,
            can_retry,
            is_recoverable,
        }
    }

    /// Track error frequency for pattern detection
    fn increment_error_count(&self, error_type: &str) {
        if let Ok(mut counts) = self.error_counts.lock() {
            *counts.entry(error_type.to_string()).or_insert(0) += 1;
        }
    }

    /// Check for systematic error patterns that might indicate deeper issues
    async fn check_error_patterns(&self, error_type: &str) {
        if let Ok(counts) = self.error_counts.lock() {
            if let Some(&count) = counts.get(error_type) {
                match error_type {
                    "AI_ERROR" if count >= 10 => {
                        error!("AI service experiencing frequent failures ({} errors) - may need restart", count);
                    }
                    "DATABASE_ERROR" if count >= 5 => {
                        error!("Database experiencing frequent failures ({} errors) - may need maintenance", count);
                    }
                    "NETWORK_ERROR" if count >= 15 => {
                        warn!("Network connectivity issues detected ({} errors)", count);
                    }
                    "RESOURCE_LIMIT_EXCEEDED" if count >= 3 => {
                        error!("System under resource pressure ({} errors) - immediate action needed", count);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Get error statistics for monitoring
    pub fn get_error_statistics(&self) -> HashMap<String, u32> {
        self.error_counts.lock().unwrap().clone()
    }

    /// Reset error counts (useful for testing or after resolving systemic issues)
    pub fn reset_error_counts(&self) {
        if let Ok(mut counts) = self.error_counts.lock() {
            counts.clear();
        }
    }

    /// Execute a function with systematic error handling
    pub async fn with_error_handling<F, T>(
        &self,
        operation: String,
        file_path: Option<String>,
        func: F,
    ) -> Result<T, ErrorReport>
    where
        F: std::future::Future<Output = Result<T, AppError>>,
    {
        let context = ErrorContext {
            operation: operation.clone(),
            file_path,
            user_id: None, // Could be populated from session context
            timestamp: chrono::Utc::now(),
            retry_count: 0,
            severity: ErrorSeverity::Low,
        };

        match func.await {
            Ok(result) => Ok(result),
            Err(error) => {
                let error_report = self.handle_error(error, context).await
                    .map_err(|e| ErrorReport {
                        error_type: "HANDLER_ERROR".to_string(),
                        error_message: e.to_string(),
                        user_message: "An unexpected error occurred in error handling".to_string(),
                        severity: ErrorSeverity::Critical,
                        context: ErrorContext {
                            operation,
                            file_path: None,
                            user_id: None,
                            timestamp: chrono::Utc::now(),
                            retry_count: 0,
                            severity: ErrorSeverity::Critical,
                        },
                        suggested_actions: vec!["Contact support".to_string()],
                        can_retry: false,
                        is_recoverable: false,
                    })?;

                Err(error_report)
            }
        }
    }
}

/// Convenience macro for systematic error handling
#[macro_export]
macro_rules! handle_error {
    ($handler:expr, $operation:expr, $file_path:expr, $result:expr) => {
        match $result {
            Ok(value) => Ok(value),
            Err(error) => {
                let context = $crate::error::error_handler::ErrorContext {
                    operation: $operation.to_string(),
                    file_path: $file_path.map(|s| s.to_string()),
                    user_id: None,
                    timestamp: chrono::Utc::now(),
                    retry_count: 0,
                    severity: $crate::error::error_handler::ErrorSeverity::Medium,
                };

                let error_report = $handler.handle_error(error, context).await?;
                Err($crate::error::AppError::ProcessingError {
                    message: error_report.user_message,
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_error_handler_creation() {
        let handler = ErrorHandler::new();
        assert!(handler.recovery_strategies.contains_key("AI_ERROR"));
        assert!(handler.recovery_strategies.contains_key("DATABASE_ERROR"));
    }

    #[tokio::test]
    async fn test_error_severity_determination() {
        let handler = ErrorHandler::new();
        let context = ErrorContext {
            operation: "test".to_string(),
            file_path: None,
            user_id: None,
            timestamp: chrono::Utc::now(),
            retry_count: 0,
            severity: ErrorSeverity::Low,
        };

        let error = AppError::ResourceLimitExceeded {
            message: "Test".to_string(),
        };
        let severity = handler.determine_severity(&error, &context);
        assert!(matches!(severity, ErrorSeverity::Critical));
    }

    #[tokio::test]
    async fn test_error_count_tracking() {
        let handler = ErrorHandler::new();
        handler.increment_error_count("TEST_ERROR");
        handler.increment_error_count("TEST_ERROR");

        let stats = handler.get_error_statistics();
        assert_eq!(stats.get("TEST_ERROR"), Some(&2));
    }
}