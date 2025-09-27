use serde::Serialize;
use once_cell::sync::Lazy;
use regex::Regex;

pub mod error_handler;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Resource not found: {message}")]
    NotFound { message: String },

    #[error("Access denied: {path}")]
    AccessDenied { path: String },

    #[error("Invalid path: {message}")]
    InvalidPath { message: String },

    #[error("AI service error: {message}")]
    AiError { message: String },

    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Resource limit exceeded: {message}")]
    ResourceLimitExceeded { message: String },

    #[error("Rate limit exceeded for {endpoint}. Retry after {retry_after_seconds} seconds")]
    RateLimitExceeded {
        retry_after_seconds: u32,
        endpoint: String,
    },

    #[error("File too large: {path} ({size} bytes exceeds maximum {max_size} bytes)")]
    FileTooLarge {
        path: String,
        size: u64,
        max_size: u64,
    },

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Processing error: {message}")]
    ProcessingError { message: String },

    #[error("Operation error: {message}")]
    OperationError { message: String },

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Resource not available: {resource}")]
    ResourceNotAvailable { resource: String },

    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Invalid operation: {message}")]
    InvalidOperation { message: String },

    #[error("Security error: {message}")]
    SecurityError { message: String },

    #[error("System error: {message}")]
    SystemError { message: String },

    #[error("Storage full")]
    StorageFull,

    #[error("Model not found: {model}")]
    ModelNotFound { model: String },

    #[error("Operation timed out: {message}")]
    Timeout { message: String },

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Assertion failed: {message}")]
    AssertionError { message: String },

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("IO error: {message}")]
    IoError { message: String },

    #[error("External service error - {service}: {message}")]
    ExternalServiceError { service: String, message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),

    #[error(transparent)]
    SqlxError(#[from] sqlx::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    NotifyError(#[from] notify::Error),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        // Create a sanitized error response
        let error_response = ErrorResponse {
            error_type: self.error_type_name().to_string(),
            message: self.user_message(),
            recoverable: self.is_recoverable(),
        };

        error_response.serialize(serializer)
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error_type: String,
    message: String,
    recoverable: bool,
}

impl AppError {
    /// Returns a user-friendly error message with sensitive information sanitized
    pub fn user_message(&self) -> String {
        match self {
            Self::FileNotFound { .. } => "The requested file could not be found".to_string(),
            Self::NotFound { .. } => "The requested resource was not found".to_string(),
            Self::AccessDenied { .. } => "Access to this resource is denied".to_string(),
            Self::InvalidPath { .. } => "The provided path is invalid".to_string(),
            Self::AiError { .. } => "AI service is temporarily unavailable. Please check your Ollama connection.".to_string(),
            Self::DatabaseError { .. } => "Database operation failed. Please try again or restart the application.".to_string(),
            Self::ConfigError { .. } => "Configuration is invalid. Please check your settings.".to_string(),
            Self::NetworkError { .. } => "Network connection failed. Please check your internet connection.".to_string(),
            Self::ParseError { .. } => "Failed to parse the data. The file format may be unsupported.".to_string(),
            Self::ProcessingError { message } => Self::sanitize_message(message),
            Self::Cancelled => "Operation was cancelled".to_string(),
            Self::ResourceNotAvailable { resource } => {
                // Sanitize resource name to prevent exposing internal paths
                let safe_resource = Self::sanitize_resource_name(resource);
                format!("{} is not available", safe_resource)
            },
            Self::InvalidInput { message } => Self::sanitize_message(message),
            Self::SecurityError { .. } => "A security check failed. Operation was blocked for safety.".to_string(),
            Self::SystemError { .. } => "System operation failed. Please check system permissions.".to_string(),
            Self::StorageFull => "Storage space is full. Please free up some disk space.".to_string(),
            Self::ModelNotFound { model } => {
                // Only show model name, not full path or internal details
                let safe_model = Self::sanitize_model_name(model);
                format!("AI model '{}' is not installed. Please install it via Ollama.", safe_model)
            },
            Self::Io(_) => "File operation failed. Please check file permissions and disk space.".to_string(),
            Self::Tauri(_) => "Application error occurred. Please restart the application.".to_string(),
            Self::SqlxError(_) => "Database error occurred. Please restart the application.".to_string(),
            Self::SerdeJson(_) => "Data processing error. The data format may be invalid.".to_string(),
            Self::NotifyError(_) => "File monitoring error occurred. File watching may be temporarily disabled.".to_string(),
            Self::ReqwestError(_) => "Network request failed. Please check your connection and try again.".to_string(),
            Self::Other(_) => "An unexpected error occurred. Please try again or restart the application.".to_string(),
            Self::ResourceLimitExceeded { message } => Self::sanitize_message(message),
            Self::Timeout { .. } => "Operation timed out. Please try again with a smaller selection.".to_string(),
            Self::ValidationError { message } => Self::sanitize_message(message),
            Self::OperationError { message } => Self::sanitize_message(message),
            Self::InvalidOperation { message } => Self::sanitize_message(message),
            Self::RateLimitExceeded { endpoint, .. } => {
                format!("Rate limit exceeded for endpoint: {}", endpoint)
            }
            Self::FileTooLarge { path, size, max_size } => {
                format!("File {} is too large: {} bytes (max: {} bytes)",
                    Self::sanitize_message(path), size, max_size)
            }
            Self::AssertionError { message } => Self::sanitize_message(message),
            Self::SerializationError { message } => Self::sanitize_message(message),
            Self::IoError { message } => Self::sanitize_message(message),
            Self::ExternalServiceError { service, message } => {
                format!("{} service error: {}", service, Self::sanitize_message(message))
            }
        }
    }

    // Get error type name for tracing
    pub fn error_type_name(&self) -> &'static str {
        match self {
            Self::FileNotFound { .. } => "FileNotFound",
            Self::NotFound { .. } => "NotFound",
            Self::AccessDenied { .. } => "AccessDenied",
            Self::InvalidPath { .. } => "InvalidPath",
            Self::AiError { .. } => "AiError",
            Self::DatabaseError { .. } => "DatabaseError",
            Self::ConfigError { .. } => "ConfigError",
            Self::NetworkError { .. } => "NetworkError",
            Self::ParseError { .. } => "ParseError",
            Self::ProcessingError { .. } => "ProcessingError",
            Self::Cancelled => "Cancelled",
            Self::ResourceNotAvailable { .. } => "ResourceNotAvailable",
            Self::InvalidInput { .. } => "InvalidInput",
            Self::SecurityError { .. } => "SecurityError",
            Self::SystemError { .. } => "SystemError",
            Self::StorageFull => "StorageFull",
            Self::ModelNotFound { .. } => "ModelNotFound",
            Self::Io(_) => "IoError",
            Self::Tauri(_) => "TauriError",
            Self::SqlxError(_) => "SqlxError",
            Self::SerdeJson(_) => "SerdeJsonError",
            Self::NotifyError(_) => "NotifyError",
            Self::ReqwestError(_) => "ReqwestError",
            Self::Other(_) => "Other",
            Self::ResourceLimitExceeded { .. } => "ResourceLimitExceeded",
            Self::Timeout { .. } => "Timeout",
            Self::ValidationError { .. } => "ValidationError",
            Self::OperationError { .. } => "OperationError",
            Self::InvalidOperation { .. } => "InvalidOperation",
            Self::RateLimitExceeded { .. } => "RateLimitExceeded",
            Self::FileTooLarge { .. } => "FileTooLarge",
            Self::AssertionError { .. } => "AssertionError",
            Self::SerializationError { .. } => "SerializationError",
            Self::IoError { .. } => "IoError",
            Self::ExternalServiceError { .. } => "ExternalServiceError",
        }
    }

    /// Sanitize error messages to remove sensitive information
    fn sanitize_message(message: &str) -> String {
        // Compile regex patterns once using lazy static initialization
        // These patterns are known to be valid, but we handle errors gracefully
        static WIN_PATH_REGEX: Lazy<Option<Regex>> = Lazy::new(|| {
            Regex::new(r#"[A-Z]:[\\\/][^\s"']+"#).ok()
        });

        static UNIX_PATH_REGEX: Lazy<Option<Regex>> = Lazy::new(|| {
            Regex::new(r#"\/[^\s"']+"#).ok()
        });

        static URL_REGEX: Lazy<Option<Regex>> = Lazy::new(|| {
            Regex::new(r"https?://[^@\s]+@[^\s]+").ok()
        });

        static IP_REGEX: Lazy<Option<Regex>> = Lazy::new(|| {
            Regex::new(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b").ok()
        });

        static PORT_REGEX: Lazy<Option<Regex>> = Lazy::new(|| {
            Regex::new(r":[0-9]{2,5}\b").ok()
        });

        // Remove absolute paths (Windows and Unix)
        let mut sanitized = message.to_string();

        // Apply sanitization using the pre-compiled regex patterns
        // If any regex failed to compile (highly unlikely), skip that sanitization step
        if let Some(ref regex) = *WIN_PATH_REGEX {
            sanitized = regex.replace_all(&sanitized, "[path]").to_string();
        }

        if let Some(ref regex) = *UNIX_PATH_REGEX {
            sanitized = regex.replace_all(&sanitized, "[path]").to_string();
        }

        if let Some(ref regex) = *URL_REGEX {
            sanitized = regex.replace_all(&sanitized, "[url]").to_string();
        }

        if let Some(ref regex) = *IP_REGEX {
            sanitized = regex.replace_all(&sanitized, "[ip]").to_string();
        }

        if let Some(ref regex) = *PORT_REGEX {
            sanitized = regex.replace_all(&sanitized, ":[port]").to_string();
        }

        // Limit message length to prevent verbose error dumps
        if sanitized.len() > 200 {
            sanitized.truncate(197);
            sanitized.push_str("...");
        }

        sanitized
    }

    /// Sanitize resource names to prevent path disclosure
    fn sanitize_resource_name(resource: &str) -> String {
        // Extract just the filename if it's a path
        std::path::Path::new(resource)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("resource")
            .to_string()
    }

    /// Sanitize model names to prevent internal detail disclosure
    fn sanitize_model_name(model: &str) -> String {
        // Remove version numbers and technical suffixes
        model.split(':').next()
            .unwrap_or("model")
            .split('/').last()
            .unwrap_or("model")
            .to_string()
    }

    /// Returns the error type for frontend handling
    pub fn error_type(&self) -> String {
        match self {
            Self::FileNotFound { .. } => "FILE_NOT_FOUND",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::AccessDenied { .. } => "ACCESS_DENIED",
            Self::InvalidPath { .. } => "INVALID_PATH",
            Self::AiError { .. } => "AI_ERROR",
            Self::DatabaseError { .. } => "DATABASE_ERROR",
            Self::ConfigError { .. } => "CONFIG_ERROR",
            Self::NetworkError { .. } => "NETWORK_ERROR",
            Self::ParseError { .. } => "PARSE_ERROR",
            Self::ProcessingError { .. } => "PROCESSING_ERROR",
            Self::Cancelled => "CANCELLED",
            Self::ResourceNotAvailable { .. } => "RESOURCE_NOT_AVAILABLE",
            Self::InvalidInput { .. } => "INVALID_INPUT",
            Self::SecurityError { .. } => "SECURITY_ERROR",
            Self::SystemError { .. } => "SYSTEM_ERROR",
            Self::StorageFull => "STORAGE_FULL",
            Self::ModelNotFound { .. } => "MODEL_NOT_FOUND",
            Self::Timeout { .. } => "TIMEOUT",
            Self::Io(_) => "IO_ERROR",
            Self::Tauri(_) => "TAURI_ERROR",
            Self::SqlxError(_) => "DATABASE_ERROR",
            Self::SerdeJson(_) => "PARSE_ERROR",
            Self::NotifyError(_) => "FILE_WATCHER_ERROR",
            Self::ReqwestError(_) => "NETWORK_ERROR",
            Self::Other(_) => "UNKNOWN_ERROR",
            Self::ResourceLimitExceeded { .. } => "RESOURCE_LIMIT_EXCEEDED",
            Self::ValidationError { .. } => "VALIDATION_ERROR",
            Self::OperationError { .. } => "OPERATION_ERROR",
            Self::InvalidOperation { .. } => "INVALID_OPERATION",
            Self::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED",
            Self::FileTooLarge { .. } => "FILE_TOO_LARGE",
            Self::AssertionError { .. } => "ASSERTION_ERROR",
            Self::SerializationError { .. } => "SERIALIZATION_ERROR",
            Self::IoError { .. } => "IO_ERROR",
            Self::ExternalServiceError { .. } => "EXTERNAL_SERVICE_ERROR",
        }
        .to_string()
    }

    /// Indicates if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        !matches!(
            self,
            Self::StorageFull
                | Self::AccessDenied { .. }
                | Self::ConfigError { .. }
                | Self::SecurityError { .. }
        )
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
