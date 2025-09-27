// Simple library tests to verify functionality

#[cfg(test)]
mod tests {
    use crate::error::AppError;
    use crate::utils::security::validate_file_name;

    #[test]
    fn test_error_handling() {
        // Test error creation
        let error = AppError::NotFound {
            message: "Test resource not found".to_string(),
        };

        // Verify error message
        assert!(error.to_string().contains("not found"));

        // Verify error type
        assert_eq!(error.error_type_name(), "NotFound");

        // Test user message sanitization
        let user_msg = error.user_message();
        assert!(user_msg.contains("resource"));
    }

    #[test]
    fn test_file_validation() {
        // Test valid file names
        assert!(validate_file_name("test.txt").is_ok());
        assert!(validate_file_name("document.pdf").is_ok());
        assert!(validate_file_name("image.png").is_ok());

        // Test invalid file names
        assert!(validate_file_name("").is_err());
        assert!(validate_file_name("..").is_err());
        assert!(validate_file_name(".").is_err());

        // Test Windows reserved names
        #[cfg(windows)]
        {
            assert!(validate_file_name("CON").is_err());
            assert!(validate_file_name("PRN").is_err());
            assert!(validate_file_name("AUX").is_err());
            assert!(validate_file_name("NUL").is_err());
        }
    }

    #[test]
    fn test_error_types() {
        // Test different error variants
        let validation_err = AppError::ValidationError {
            message: "Invalid input".to_string(),
        };
        assert_eq!(validation_err.error_type(), "VALIDATION_ERROR");

        let not_found_err = AppError::NotFound {
            message: "Resource missing".to_string(),
        };
        assert_eq!(not_found_err.error_type(), "NOT_FOUND");

        let security_err = AppError::SecurityError {
            message: "Access denied".to_string(),
        };
        assert_eq!(security_err.error_type(), "SECURITY_ERROR");
    }

    // Path validation requires AppHandle, so skip for now
    // #[test]
    // fn test_path_validation() {
    //     use crate::utils::security::validate_path;
    //     // This would need a mock AppHandle
    // }

    #[test]
    fn test_feature_flags() {
        use crate::features::flags::{FeatureFlag, FlagValue};

        // Create a simple boolean flag
        let flag = FeatureFlag::boolean("test_feature".to_string(), true);
        assert_eq!(flag.key, "test_feature");
        assert!(matches!(flag.default_value, FlagValue::Boolean(true)));
        assert!(flag.is_active());

        // Test flag value conversion
        let bool_value = FlagValue::Boolean(true);
        assert_eq!(bool_value.as_bool(), Some(true));
        assert_eq!(bool_value.as_string(), "true");

        let int_value = FlagValue::Integer(42);
        assert_eq!(int_value.as_integer(), Some(42));
        assert_eq!(int_value.as_float(), Some(42.0));
    }

    // Cache key test removed - CacheKey struct not exposed
    // #[test]
    // fn test_cache_key_generation() {
    //     // CacheKey is internal to cache module
    // }

    #[test]
    fn test_api_version() {
        use crate::api::versioning::ApiVersion;

        // Create versions
        let v1 = ApiVersion::new(1, 0, 0);
        let v2 = ApiVersion::new(2, 0, 0);
        let v1_1 = ApiVersion::new(1, 1, 0);

        // Test version comparison
        assert!(v2.is_newer_than(&v1));
        assert!(v1_1.is_newer_than(&v1));
        assert!(!v1.is_newer_than(&v2));

        // Test version string formatting (Display trait returns without 'v' prefix)
        assert_eq!(v1.to_string(), "1.0.0");
        assert_eq!(v2.to_string(), "2.0.0");
    }

    // Sanitization test removed - function not available
    // #[test]
    // fn test_sanitization() {
    //     // Would need sanitize_input function
    // }
}