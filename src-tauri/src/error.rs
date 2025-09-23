use serde::Serialize;

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

    #[error("Validation error for {field}: {message}")]
    ValidationError { field: String, message: String },

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
            error_type: self.error_type(),
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
            Self::ValidationError { field: _, message } => Self::sanitize_message(message),
            Self::OperationError { message } => Self::sanitize_message(message),
            Self::InvalidOperation { message } => Self::sanitize_message(message),
        }
    }

    /// Sanitize error messages to remove sensitive information
    fn sanitize_message(message: &str) -> String {
        // Remove absolute paths (Windows and Unix)
        let mut sanitized = message.to_string();

        // Remove Windows absolute paths
        let win_path_regex = regex::Regex::new(r#"[A-Z]:[\\\/][^\s"']+"#).unwrap();
        sanitized = win_path_regex.replace_all(&sanitized, "[path]").to_string();

        // Remove Unix absolute paths
        let unix_path_regex = regex::Regex::new(r#"\/[^\s"']+"#).unwrap();
        sanitized = unix_path_regex.replace_all(&sanitized, "[path]").to_string();

        // Remove potential URLs with credentials
        let url_regex = regex::Regex::new(r"https?://[^@\s]+@[^\s]+").unwrap();
        sanitized = url_regex.replace_all(&sanitized, "[url]").to_string();

        // Remove IP addresses
        let ip_regex = regex::Regex::new(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b").unwrap();
        sanitized = ip_regex.replace_all(&sanitized, "[ip]").to_string();

        // Remove port numbers
        let port_regex = regex::Regex::new(r":[0-9]{2,5}\b").unwrap();
        sanitized = port_regex.replace_all(&sanitized, ":[port]").to_string();

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
