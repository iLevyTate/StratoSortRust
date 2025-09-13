use stratosort::error::{AppError, Result};
use serde_json;

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_file_not_found_error() {
        let error = AppError::FileNotFound {
            path: "/test/path.txt".to_string(),
        };
        
        assert_eq!(error.to_string(), "File not found: /test/path.txt");
        assert_eq!(error.error_type(), "FILE_NOT_FOUND");
        assert_eq!(error.user_message(), "The requested file could not be found");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_access_denied_error() {
        let error = AppError::AccessDenied {
            path: "/restricted/path".to_string(),
        };
        
        assert_eq!(error.to_string(), "Access denied: /restricted/path");
        assert_eq!(error.error_type(), "ACCESS_DENIED");
        assert_eq!(error.user_message(), "Access to this resource is denied");
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_security_error() {
        let error = AppError::SecurityError {
            message: "Path traversal detected".to_string(),
        };
        
        assert_eq!(error.to_string(), "Security error: Path traversal detected");
        assert_eq!(error.error_type(), "SECURITY_ERROR");
        assert_eq!(error.user_message(), "Security violation detected");
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_resource_limit_exceeded() {
        let error = AppError::ResourceLimitExceeded {
            message: "Too many concurrent operations".to_string(),
        };
        
        assert_eq!(error.to_string(), "Resource limit exceeded: Too many concurrent operations");
        assert_eq!(error.error_type(), "RESOURCE_LIMIT_EXCEEDED");
        assert_eq!(error.user_message(), "Too many concurrent operations");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_cancelled_error() {
        let error = AppError::Cancelled;
        
        assert_eq!(error.to_string(), "Operation cancelled");
        assert_eq!(error.error_type(), "CANCELLED");
        assert_eq!(error.user_message(), "Operation was cancelled");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_ai_error() {
        let error = AppError::AiError {
            message: "Model not responding".to_string(),
        };
        
        assert_eq!(error.to_string(), "AI service error: Model not responding");
        assert_eq!(error.error_type(), "AI_ERROR");
        assert_eq!(error.user_message(), "AI service is temporarily unavailable");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_database_error() {
        let error = AppError::DatabaseError {
            message: "Connection failed".to_string(),
        };
        
        assert_eq!(error.to_string(), "Database error: Connection failed");
        assert_eq!(error.error_type(), "DATABASE_ERROR");
        assert_eq!(error.user_message(), "Database operation failed");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_timeout_error() {
        let error = AppError::Timeout {
            message: "Request timed out after 30s".to_string(),
        };
        
        assert_eq!(error.to_string(), "Operation timed out: Request timed out after 30s");
        assert_eq!(error.error_type(), "TIMEOUT");
        assert_eq!(error.user_message(), "Request timed out after 30s");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_storage_full_error() {
        let error = AppError::StorageFull;
        
        assert_eq!(error.to_string(), "Storage full");
        assert_eq!(error.error_type(), "STORAGE_FULL");
        assert_eq!(error.user_message(), "Storage space is full");
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_model_not_found_error() {
        let error = AppError::ModelNotFound {
            model: "llama3.2".to_string(),
        };
        
        assert_eq!(error.to_string(), "Model not found: llama3.2");
        assert_eq!(error.error_type(), "MODEL_NOT_FOUND");
        assert_eq!(error.user_message(), "Model 'llama3.2' is not installed");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_error_serialization() {
        let error = AppError::FileNotFound {
            path: "/test/file.txt".to_string(),
        };
        
        let serialized = serde_json::to_string(&error).expect("Failed to serialize error");
        let parsed: serde_json::Value = serde_json::from_str(&serialized).expect("Failed to parse JSON");
        
        assert_eq!(parsed["error_type"], "FILE_NOT_FOUND");
        assert_eq!(parsed["message"], "The requested file could not be found");
        assert_eq!(parsed["recoverable"], true);
    }

    #[test]
    fn test_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let app_error = AppError::from(io_error);
        
        match app_error {
            AppError::Io(_) => {
                assert_eq!(app_error.error_type(), "IO_ERROR");
                assert_eq!(app_error.user_message(), "File operation failed");
                assert!(app_error.is_recoverable());
            }
            _ => panic!("Expected IO error"),
        }
    }

    #[test]
    fn test_result_type_ok() {
        let result: Result<String> = Ok("success".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_result_type_error() {
        let result: Result<String> = Err(AppError::Cancelled);
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Cancelled => (),
            _ => panic!("Expected Cancelled error"),
        }
    }

    #[test]
    fn test_config_error_non_recoverable() {
        let error = AppError::ConfigError {
            message: "Invalid configuration".to_string(),
        };
        
        assert!(!error.is_recoverable());
        assert_eq!(error.error_type(), "CONFIG_ERROR");
    }

    #[test]
    fn test_invalid_input_error() {
        let error = AppError::InvalidInput {
            message: "Parameter cannot be empty".to_string(),
        };
        
        assert_eq!(error.user_message(), "Parameter cannot be empty");
        assert_eq!(error.error_type(), "INVALID_INPUT");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_network_error() {
        let error = AppError::NetworkError {
            message: "Connection refused".to_string(),
        };
        
        assert_eq!(error.error_type(), "NETWORK_ERROR");
        assert_eq!(error.user_message(), "Network connection failed");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_resource_not_available() {
        let error = AppError::ResourceNotAvailable {
            resource: "GPU".to_string(),
        };
        
        assert_eq!(error.user_message(), "GPU is not available");
        assert_eq!(error.error_type(), "RESOURCE_NOT_AVAILABLE");
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_parse_error() {
        let error = AppError::ParseError {
            message: "Invalid JSON format".to_string(),
        };
        
        assert_eq!(error.error_type(), "PARSE_ERROR");
        assert_eq!(error.user_message(), "Failed to parse the data");
        assert!(error.is_recoverable());
    }
}