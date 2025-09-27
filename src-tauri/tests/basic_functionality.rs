// Basic Functionality Tests
// High, mid, and low level tests for core functionality

use stratosort::error::AppError;
use stratosort::utils::security::validate_file_name;

// ====================
// LOW-LEVEL UNIT TESTS
// ====================

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        // Test error type creation
        let error = AppError::NotFound {
            message: "Test not found".to_string(),
        };
        assert_eq!(error.to_string(), "Resource not found: Test not found");
        assert_eq!(error.error_type_name(), "NotFound");
    }

    #[test]
    fn test_validation_error() {
        // Test validation error
        let error = AppError::ValidationError {
            message: "Invalid input".to_string(),
        };
        assert!(error.user_message().contains("Invalid input"));
    }

    #[test]
    fn test_file_name_validation() {
        // Valid file names
        assert!(validate_file_name("test.txt").is_ok());
        assert!(validate_file_name("document.pdf").is_ok());
        assert!(validate_file_name("image.png").is_ok());

        // Invalid file names
        assert!(validate_file_name("").is_err());
        assert!(validate_file_name("..").is_err());
        assert!(validate_file_name("con").is_err()); // Windows reserved
        assert!(validate_file_name("file\0name").is_err()); // Null byte
    }
}

// ======================
// MID-LEVEL MODULE TESTS
// ======================

#[cfg(test)]
mod module_tests {
    use stratosort::features::flags::{FeatureFlag, FlagValue};
    use stratosort::features::evaluator::{FlagEvaluator, EvaluationContext};
    use std::collections::HashMap;

    #[test]
    fn test_feature_flag_creation() {
        // Test feature flag creation
        let flag = FeatureFlag::boolean("test_feature".to_string(), true);
        assert_eq!(flag.key, "test_feature");
        assert!(matches!(flag.default_value, FlagValue::Boolean(true)));
        assert!(flag.is_active());
    }

    #[test]
    fn test_feature_flag_evaluation() {
        // Test feature flag evaluation
        let flag = FeatureFlag::boolean("test_flag".to_string(), true);
        let mut evaluator = FlagEvaluator::new();
        evaluator.add_flag(flag);

        let context = EvaluationContext::new()
            .with_user("user123".to_string())
            .with_environment("test".to_string());

        let result = evaluator.evaluate("test_flag", &context);
        assert!(result.is_ok());

        if let Ok(eval_result) = result {
            assert!(matches!(eval_result.value, FlagValue::Boolean(true)));
        }
    }

    #[test]
    fn test_observability_span_creation() {
        use stratosort::observability::spans::{Span, SpanKind};
        use stratosort::observability::tracing::{TraceId, SpanId};

        let trace_id = TraceId::new();
        let span_id = SpanId::new();
        let span = Span::new(
            trace_id,
            span_id,
            None,
            "test_operation".to_string(),
            SpanKind::Internal,
        );

        assert_eq!(span.operation_name, "test_operation");
        assert!(matches!(span.kind, SpanKind::Internal));
        assert!(!span.is_ended());
    }
}

// =============================
// HIGH-LEVEL INTEGRATION TESTS
// =============================

#[cfg(test)]
mod integration_tests {
    use stratosort::cache::advanced::{AdvancedCache, CacheConfig, InvalidationType};
    use std::sync::Arc;
    use tokio;

    #[tokio::test]
    async fn test_cache_operations() {
        // Create cache with default config
        let config = CacheConfig::default();
        let cache = Arc::new(AdvancedCache::new(config));

        // Test set and get
        cache.set("test_key", "test_value", None).await.unwrap();
        let value: Option<String> = cache.get("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Test invalidation
        cache.invalidate("test_key").await.unwrap();
        let value: Option<String> = cache.get("test_key").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        use stratosort::observability::metrics::MetricsCollector;
        use std::collections::HashMap;

        let collector = MetricsCollector::new();

        // Create counter
        let counter = collector.counter("test_counter", HashMap::new()).await;
        counter.increment().await;
        counter.increment().await;
        assert_eq!(counter.get().await, 2);

        // Create gauge
        let gauge = collector.gauge("test_gauge", HashMap::new()).await;
        gauge.set(42.0).await;
        assert_eq!(gauge.get().await, 42.0);

        // Collect all metrics
        let metrics = collector.collect().await;
        assert!(!metrics.is_empty());
    }
}

// ================================
// SYSTEM-LEVEL INTEGRATION TESTS
// ================================

#[cfg(test)]
mod system_tests {
    use stratosort::api::versioning::{ApiVersion, ApiVersionManager};
    use stratosort::api::documentation::{ApiDocGenerator, DocGeneratorConfig};
    use std::collections::HashMap;

    #[test]
    fn test_api_versioning() {
        let mut manager = ApiVersionManager::new();

        // Register versions
        let v1 = ApiVersion::new(1, 0, 0);
        let v2 = ApiVersion::new(2, 0, 0);

        manager.register_version(v1.clone());
        manager.register_version(v2.clone());
        manager.set_current_version(v2.clone());

        // Check version compatibility
        assert!(manager.is_version_supported(&v1));
        assert!(manager.is_version_supported(&v2));

        let v3 = ApiVersion::new(3, 0, 0);
        assert!(!manager.is_version_supported(&v3));
    }

    #[test]
    fn test_api_documentation_generation() {
        let mut generator = ApiDocGenerator::new(DocGeneratorConfig::default());

        // Generate OpenAPI spec
        let spec = generator.generate_openapi_spec();

        // Verify spec structure
        assert!(spec.contains("openapi"));
        assert!(spec.contains("\"3.0.0\""));
        assert!(spec.contains("paths"));
        assert!(spec.contains("components"));
    }
}

// ==================
// PERFORMANCE TESTS
// ==================

#[cfg(test)]
mod performance_tests {
    use std::time::Instant;

    #[test]
    fn test_error_creation_performance() {
        use stratosort::error::AppError;

        let start = Instant::now();
        for _ in 0..10000 {
            let _ = AppError::NotFound {
                message: "Test".to_string(),
            };
        }
        let duration = start.elapsed();

        // Should create 10k errors in under 100ms
        assert!(duration.as_millis() < 100, "Error creation too slow: {:?}", duration);
    }

    #[test]
    fn test_feature_flag_evaluation_performance() {
        use stratosort::features::flags::FeatureFlag;
        use stratosort::features::evaluator::{FlagEvaluator, EvaluationContext};

        let mut evaluator = FlagEvaluator::new();

        // Add 100 flags
        for i in 0..100 {
            let flag = FeatureFlag::boolean(format!("flag_{}", i), i % 2 == 0);
            evaluator.add_flag(flag);
        }

        let context = EvaluationContext::new()
            .with_user("test_user".to_string());

        let start = Instant::now();
        for i in 0..100 {
            let _ = evaluator.evaluate(&format!("flag_{}", i), &context);
        }
        let duration = start.elapsed();

        // Should evaluate 100 flags in under 10ms
        assert!(duration.as_millis() < 10, "Flag evaluation too slow: {:?}", duration);
    }
}