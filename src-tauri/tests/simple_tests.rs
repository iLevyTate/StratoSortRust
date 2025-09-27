// Simple integration tests to verify core functionality

use stratosort::error::AppError;

#[test]
fn test_error_types() {
    // Test error creation and formatting
    let not_found = AppError::NotFound {
        message: "Test resource".to_string(),
    };
    assert!(not_found.to_string().contains("not found"));

    let validation = AppError::ValidationError {
        message: "Invalid input".to_string(),
    };
    assert!(validation.to_string().contains("Validation error"));

    let security = AppError::SecurityError {
        message: "Access denied".to_string(),
    };
    assert!(security.to_string().contains("Security error"));
}

#[test]
fn test_error_type_names() {
    let errors = vec![
        (AppError::NotFound { message: "test".to_string() }, "NotFound"),
        (AppError::ValidationError { message: "test".to_string() }, "ValidationError"),
        (AppError::SecurityError { message: "test".to_string() }, "SecurityError"),
        (AppError::DatabaseError { message: "test".to_string() }, "DatabaseError"),
        (AppError::ConfigError { message: "test".to_string() }, "ConfigError"),
    ];

    for (error, expected_name) in errors {
        assert_eq!(error.error_type_name(), expected_name);
    }
}

#[test]
fn test_feature_flags() {
    use stratosort::features::flags::{FeatureFlag, FlagValue};

    // Test boolean flag
    let flag = FeatureFlag::boolean("test_flag".to_string(), true);
    assert_eq!(flag.key, "test_flag");
    assert!(flag.is_active());

    // Test flag values
    let bool_val = FlagValue::Boolean(true);
    assert_eq!(bool_val.as_bool(), Some(true));

    let int_val = FlagValue::Integer(42);
    assert_eq!(int_val.as_integer(), Some(42));

    let str_val = FlagValue::String("test".to_string());
    assert_eq!(str_val.as_string(), "test");
}

#[test]
fn test_api_versioning() {
    use stratosort::api::versioning::ApiVersion;

    let v1 = ApiVersion::new(1, 0, 0);
    let v2 = ApiVersion::new(2, 0, 0);
    let v1_5 = ApiVersion::new(1, 5, 0);

    // Test version comparison
    assert!(v2.is_newer_than(&v1));
    assert!(v1_5.is_newer_than(&v1));
    assert!(!v1.is_newer_than(&v2));

    // Test version strings
    assert_eq!(v1.to_string(), "v1.0.0");
    assert_eq!(v2.to_string(), "v2.0.0");
}

#[tokio::test]
async fn test_cache_operations() {
    use stratosort::cache::advanced::{AdvancedCache, CacheConfig};
    use std::sync::Arc;

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

    // Test counter
    let counter = collector.counter("test_counter", HashMap::new()).await;
    counter.increment().await;
    counter.add(5).await;
    assert_eq!(counter.get().await, 6);

    // Test gauge
    let gauge = collector.gauge("test_gauge", HashMap::new()).await;
    gauge.set(100.0).await;
    gauge.decrement(25.0).await;
    assert_eq!(gauge.get().await, 75.0);
}

#[test]
fn test_observability_spans() {
    use stratosort::observability::spans::{Span, SpanKind, SpanStatus};
    use stratosort::observability::tracing::{TraceId, SpanId};

    let trace_id = TraceId::new();
    let span_id = SpanId::new();

    let mut span = Span::new(
        trace_id,
        span_id,
        None,
        "test_operation".to_string(),
        SpanKind::Internal,
    );

    assert_eq!(span.operation_name, "test_operation");
    assert!(!span.is_ended());

    span.end(SpanStatus::Ok);
    assert!(span.is_ended());
}

#[test]
fn test_evaluation_context() {
    use stratosort::features::evaluator::EvaluationContext;

    let context = EvaluationContext::new()
        .with_user("user123".to_string())
        .with_environment("production".to_string())
        .with_segments(vec!["premium".to_string(), "beta".to_string()]);

    assert_eq!(context.user_id, Some("user123".to_string()));
    assert_eq!(context.environment, "production");
    assert_eq!(context.user_segments.len(), 2);
}

// Performance benchmark tests
#[test]
fn test_error_creation_performance() {
    use std::time::Instant;

    let start = Instant::now();
    for _ in 0..10000 {
        let _ = AppError::NotFound {
            message: "Performance test".to_string(),
        };
    }
    let duration = start.elapsed();

    // Should create 10k errors in under 100ms
    assert!(
        duration.as_millis() < 100,
        "Error creation took {:?}",
        duration
    );
}

#[test]
fn test_flag_evaluation_performance() {
    use stratosort::features::flags::FeatureFlag;
    use stratosort::features::evaluator::{FlagEvaluator, EvaluationContext};
    use std::time::Instant;

    let mut evaluator = FlagEvaluator::new();

    // Add 100 flags
    for i in 0..100 {
        let flag = FeatureFlag::boolean(format!("flag_{}", i), true);
        evaluator.add_flag(flag);
    }

    let context = EvaluationContext::new()
        .with_user("perf_test".to_string());

    let start = Instant::now();
    for i in 0..100 {
        let _ = evaluator.evaluate(&format!("flag_{}", i), &context);
    }
    let duration = start.elapsed();

    // Should evaluate 100 flags in under 50ms
    assert!(
        duration.as_millis() < 50,
        "Flag evaluation took {:?}",
        duration
    );
}