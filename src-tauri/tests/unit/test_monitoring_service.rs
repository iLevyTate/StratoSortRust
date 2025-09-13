use stratosort::services::{MonitoringService, MetricType, MetricValue, Alert, AlertSeverity};
use stratosort::error::{AppError, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{timeout, sleep};
use chrono::Utc;

#[cfg(test)]
mod monitoring_service_tests {
    use super::*;

    fn create_test_service() -> Arc<MonitoringService> {
        Arc::new(MonitoringService::new())
    }

    // Basic Metric Recording Tests
    #[tokio::test]
    async fn test_record_counter_metric() {
        let service = create_test_service();
        
        service.record_metric(
            MetricType::Counter,
            "test.counter",
            MetricValue::Integer(5)
        ).await.unwrap();
        
        let value = service.get_metric("test.counter").await.unwrap();
        assert_eq!(value, Some(MetricValue::Integer(5)));
    }

    #[tokio::test]
    async fn test_record_gauge_metric() {
        let service = create_test_service();
        
        service.record_metric(
            MetricType::Gauge,
            "memory.usage",
            MetricValue::Float(75.5)
        ).await.unwrap();
        
        let value = service.get_metric("memory.usage").await.unwrap();
        assert_eq!(value, Some(MetricValue::Float(75.5)));
    }

    #[tokio::test]
    async fn test_record_histogram_metric() {
        let service = create_test_service();
        
        // Record multiple values for histogram
        for i in 1..=10 {
            service.record_metric(
                MetricType::Histogram,
                "response.time",
                MetricValue::Integer(i * 10)
            ).await.unwrap();
        }
        
        let stats = service.get_histogram_stats("response.time").await.unwrap();
        assert!(stats.is_some());
        
        let stats = stats.unwrap();
        assert_eq!(stats.count, 10);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 100.0);
        assert_eq!(stats.mean, 55.0);
    }

    #[tokio::test]
    async fn test_increment_counter() {
        let service = create_test_service();
        
        // Initialize counter
        service.record_metric(
            MetricType::Counter,
            "operations.count",
            MetricValue::Integer(10)
        ).await.unwrap();
        
        // Increment
        service.increment_counter("operations.count", 5).await.unwrap();
        
        let value = service.get_metric("operations.count").await.unwrap();
        assert_eq!(value, Some(MetricValue::Integer(15)));
    }

    #[tokio::test]
    async fn test_decrement_counter() {
        let service = create_test_service();
        
        service.record_metric(
            MetricType::Counter,
            "active.connections",
            MetricValue::Integer(10)
        ).await.unwrap();
        
        service.decrement_counter("active.connections", 3).await.unwrap();
        
        let value = service.get_metric("active.connections").await.unwrap();
        assert_eq!(value, Some(MetricValue::Integer(7)));
    }

    #[tokio::test]
    async fn test_counter_cannot_go_negative() {
        let service = create_test_service();
        
        service.record_metric(
            MetricType::Counter,
            "test.counter",
            MetricValue::Integer(5)
        ).await.unwrap();
        
        // Try to decrement more than current value
        service.decrement_counter("test.counter", 10).await.unwrap();
        
        let value = service.get_metric("test.counter").await.unwrap();
        assert_eq!(value, Some(MetricValue::Integer(0)));
    }

    // Performance Monitoring Tests
    #[tokio::test]
    async fn test_start_and_end_timer() {
        let service = create_test_service();
        
        let timer_id = service.start_timer("operation.duration").await;
        
        // Simulate some work
        sleep(Duration::from_millis(100)).await;
        
        let duration = service.end_timer(timer_id).await.unwrap();
        
        // Duration should be at least 100ms
        assert!(duration >= 100);
        
        // Check histogram was updated
        let stats = service.get_histogram_stats("operation.duration").await.unwrap();
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().count, 1);
    }

    #[tokio::test]
    async fn test_measure_async_operation() {
        let service = create_test_service();
        
        let result = service.measure_async(
            "async.operation",
            async {
                sleep(Duration::from_millis(50)).await;
                42
            }
        ).await;
        
        assert_eq!(result, 42);
        
        // Check timing was recorded
        let stats = service.get_histogram_stats("async.operation.duration").await.unwrap();
        assert!(stats.is_some());
        assert!(stats.unwrap().mean >= 50.0);
    }

    #[tokio::test]
    async fn test_measure_with_error() {
        let service = create_test_service();
        
        let result: Result<()> = service.measure_async(
            "failing.operation",
            async {
                sleep(Duration::from_millis(10)).await;
                Err(AppError::SystemError {
                    message: "Test error".to_string()
                })
            }
        ).await;
        
        assert!(result.is_err());
        
        // Error counter should be incremented
        let errors = service.get_metric("failing.operation.errors").await.unwrap();
        assert_eq!(errors, Some(MetricValue::Integer(1)));
    }

    // Resource Monitoring Tests
    #[tokio::test]
    async fn test_record_memory_usage() {
        let service = create_test_service();
        
        service.record_memory_usage(1024.5).await.unwrap();
        
        let memory = service.get_metric("system.memory.mb").await.unwrap();
        assert_eq!(memory, Some(MetricValue::Float(1024.5)));
    }

    #[tokio::test]
    async fn test_record_cpu_usage() {
        let service = create_test_service();
        
        service.record_cpu_usage(45.7).await.unwrap();
        
        let cpu = service.get_metric("system.cpu.percent").await.unwrap();
        assert_eq!(cpu, Some(MetricValue::Float(45.7)));
    }

    #[tokio::test]
    async fn test_record_disk_usage() {
        let service = create_test_service();
        
        service.record_disk_usage(
            1000000000, // 1GB total
            600000000,  // 600MB used
            400000000   // 400MB free
        ).await.unwrap();
        
        let total = service.get_metric("system.disk.total").await.unwrap();
        assert_eq!(total, Some(MetricValue::Integer(1000000000)));
        
        let used = service.get_metric("system.disk.used").await.unwrap();
        assert_eq!(used, Some(MetricValue::Integer(600000000)));
        
        let free = service.get_metric("system.disk.free").await.unwrap();
        assert_eq!(free, Some(MetricValue::Integer(400000000)));
    }

    // Alert Management Tests
    #[tokio::test]
    async fn test_create_alert() {
        let service = create_test_service();
        
        let alert = service.create_alert(
            AlertSeverity::Warning,
            "High Memory Usage",
            "Memory usage is above 80%"
        ).await.unwrap();
        
        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert_eq!(alert.title, "High Memory Usage");
        assert!(!alert.resolved);
    }

    #[tokio::test]
    async fn test_resolve_alert() {
        let service = create_test_service();
        
        let alert = service.create_alert(
            AlertSeverity::Critical,
            "Database Connection Lost",
            "Cannot connect to database"
        ).await.unwrap();
        
        assert!(!alert.resolved);
        
        service.resolve_alert(alert.id).await.unwrap();
        
        let resolved = service.get_alert(alert.id).await.unwrap();
        assert!(resolved.is_some());
        assert!(resolved.unwrap().resolved);
    }

    #[tokio::test]
    async fn test_get_active_alerts() {
        let service = create_test_service();
        
        // Create multiple alerts
        let alert1 = service.create_alert(
            AlertSeverity::Info,
            "Info Alert",
            "Information"
        ).await.unwrap();
        
        let alert2 = service.create_alert(
            AlertSeverity::Warning,
            "Warning Alert",
            "Warning message"
        ).await.unwrap();
        
        let alert3 = service.create_alert(
            AlertSeverity::Critical,
            "Critical Alert",
            "Critical issue"
        ).await.unwrap();
        
        // Resolve one alert
        service.resolve_alert(alert2.id).await.unwrap();
        
        let active = service.get_active_alerts().await.unwrap();
        assert_eq!(active.len(), 2);
        assert!(active.iter().any(|a| a.id == alert1.id));
        assert!(active.iter().any(|a| a.id == alert3.id));
    }

    #[tokio::test]
    async fn test_get_alerts_by_severity() {
        let service = create_test_service();
        
        // Create alerts of different severities
        for _ in 0..2 {
            service.create_alert(
                AlertSeverity::Info,
                "Info",
                "Info message"
            ).await.unwrap();
        }
        
        for _ in 0..3 {
            service.create_alert(
                AlertSeverity::Warning,
                "Warning",
                "Warning message"
            ).await.unwrap();
        }
        
        service.create_alert(
            AlertSeverity::Critical,
            "Critical",
            "Critical message"
        ).await.unwrap();
        
        let warnings = service.get_alerts_by_severity(AlertSeverity::Warning).await.unwrap();
        assert_eq!(warnings.len(), 3);
        
        let critical = service.get_alerts_by_severity(AlertSeverity::Critical).await.unwrap();
        assert_eq!(critical.len(), 1);
    }

    // Threshold Monitoring Tests
    #[tokio::test]
    async fn test_set_and_check_threshold() {
        let service = create_test_service();
        
        // Set threshold for memory usage
        service.set_threshold(
            "system.memory.mb",
            1000.0,
            AlertSeverity::Warning
        ).await.unwrap();
        
        // Record value below threshold
        service.record_metric(
            MetricType::Gauge,
            "system.memory.mb",
            MetricValue::Float(800.0)
        ).await.unwrap();
        
        let alerts = service.check_thresholds().await.unwrap();
        assert_eq!(alerts.len(), 0);
        
        // Record value above threshold
        service.record_metric(
            MetricType::Gauge,
            "system.memory.mb",
            MetricValue::Float(1200.0)
        ).await.unwrap();
        
        let alerts = service.check_thresholds().await.unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
    }

    #[tokio::test]
    async fn test_multiple_thresholds() {
        let service = create_test_service();
        
        // Set multiple thresholds
        service.set_threshold(
            "cpu.usage",
            80.0,
            AlertSeverity::Warning
        ).await.unwrap();
        
        service.set_threshold(
            "cpu.usage",
            95.0,
            AlertSeverity::Critical
        ).await.unwrap();
        
        // Test warning threshold
        service.record_metric(
            MetricType::Gauge,
            "cpu.usage",
            MetricValue::Float(85.0)
        ).await.unwrap();
        
        let alerts = service.check_thresholds().await.unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Warning);
        
        // Test critical threshold
        service.record_metric(
            MetricType::Gauge,
            "cpu.usage",
            MetricValue::Float(96.0)
        ).await.unwrap();
        
        let alerts = service.check_thresholds().await.unwrap();
        assert!(alerts.iter().any(|a| a.severity == AlertSeverity::Critical));
    }

    // Metric Aggregation Tests
    #[tokio::test]
    async fn test_get_metrics_summary() {
        let service = create_test_service();
        
        // Record various metrics
        service.record_metric(
            MetricType::Counter,
            "requests.total",
            MetricValue::Integer(1000)
        ).await.unwrap();
        
        service.record_metric(
            MetricType::Gauge,
            "connections.active",
            MetricValue::Integer(50)
        ).await.unwrap();
        
        service.record_metric(
            MetricType::Gauge,
            "memory.percent",
            MetricValue::Float(65.5)
        ).await.unwrap();
        
        let summary = service.get_metrics_summary().await.unwrap();
        
        assert!(summary.contains_key("requests.total"));
        assert!(summary.contains_key("connections.active"));
        assert!(summary.contains_key("memory.percent"));
    }

    #[tokio::test]
    async fn test_calculate_rate() {
        let service = create_test_service();
        
        // Record initial counter value
        service.record_metric(
            MetricType::Counter,
            "bytes.transferred",
            MetricValue::Integer(1000)
        ).await.unwrap();
        
        let start_time = Utc::now();
        
        // Wait and record new value
        sleep(Duration::from_secs(1)).await;
        
        service.record_metric(
            MetricType::Counter,
            "bytes.transferred",
            MetricValue::Integer(2000)
        ).await.unwrap();
        
        let rate = service.calculate_rate(
            "bytes.transferred",
            start_time
        ).await.unwrap();
        
        // Rate should be approximately 1000 bytes/second
        assert!(rate >= 900.0 && rate <= 1100.0);
    }

    // Export and Reporting Tests
    #[tokio::test]
    async fn test_export_metrics() {
        let service = create_test_service();
        
        // Record some metrics
        service.record_metric(
            MetricType::Counter,
            "export.test",
            MetricValue::Integer(42)
        ).await.unwrap();
        
        let exported = service.export_metrics().await.unwrap();
        
        assert!(exported.contains("export.test"));
        assert!(exported.contains("42"));
    }

    #[tokio::test]
    async fn test_export_metrics_json() {
        let service = create_test_service();
        
        service.record_metric(
            MetricType::Gauge,
            "json.metric",
            MetricValue::Float(3.14)
        ).await.unwrap();
        
        let json = service.export_metrics_json().await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        
        assert!(parsed["metrics"]["json.metric"].is_number());
    }

    #[tokio::test]
    async fn test_generate_report() {
        let service = create_test_service();
        
        // Set up metrics
        service.record_metric(
            MetricType::Counter,
            "operations.success",
            MetricValue::Integer(950)
        ).await.unwrap();
        
        service.record_metric(
            MetricType::Counter,
            "operations.failure",
            MetricValue::Integer(50)
        ).await.unwrap();
        
        service.create_alert(
            AlertSeverity::Warning,
            "Test Alert",
            "Test warning"
        ).await.unwrap();
        
        let report = service.generate_report().await.unwrap();
        
        assert!(report.contains("operations.success"));
        assert!(report.contains("950"));
        assert!(report.contains("Test Alert"));
        assert!(report.contains("Success Rate: 95.0%"));
    }

    // Cleanup and Shutdown Tests
    #[tokio::test]
    async fn test_clear_metrics() {
        let service = create_test_service();
        
        // Add metrics
        service.record_metric(
            MetricType::Counter,
            "test1",
            MetricValue::Integer(1)
        ).await.unwrap();
        
        service.record_metric(
            MetricType::Counter,
            "test2",
            MetricValue::Integer(2)
        ).await.unwrap();
        
        // Clear all metrics
        service.clear_metrics().await.unwrap();
        
        let summary = service.get_metrics_summary().await.unwrap();
        assert!(summary.is_empty());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let service = create_test_service();
        
        // Add some data
        service.record_metric(
            MetricType::Counter,
            "shutdown.test",
            MetricValue::Integer(1)
        ).await.unwrap();
        
        service.create_alert(
            AlertSeverity::Info,
            "Shutdown Test",
            "Testing shutdown"
        ).await.unwrap();
        
        // Shutdown
        service.shutdown().await;
        
        // Service should still be queryable but empty
        let metrics = service.get_metrics_summary().await.unwrap();
        assert!(metrics.is_empty());
        
        let alerts = service.get_active_alerts().await.unwrap();
        assert!(alerts.is_empty());
    }

    // Concurrent Access Tests
    #[tokio::test]
    async fn test_concurrent_metric_updates() {
        let service = create_test_service();
        
        // Initialize counter
        service.record_metric(
            MetricType::Counter,
            "concurrent.counter",
            MetricValue::Integer(0)
        ).await.unwrap();
        
        // Spawn multiple tasks incrementing the counter
        let handles: Vec<_> = (0..100)
            .map(|_| {
                let service_clone = service.clone();
                tokio::spawn(async move {
                    service_clone.increment_counter("concurrent.counter", 1).await
                })
            })
            .collect();
        
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        let value = service.get_metric("concurrent.counter").await.unwrap();
        assert_eq!(value, Some(MetricValue::Integer(100)));
    }

    #[tokio::test]
    async fn test_concurrent_histogram_updates() {
        let service = create_test_service();
        
        // Spawn tasks recording histogram values
        let handles: Vec<_> = (0..50)
            .map(|i| {
                let service_clone = service.clone();
                tokio::spawn(async move {
                    service_clone.record_metric(
                        MetricType::Histogram,
                        "concurrent.histogram",
                        MetricValue::Integer(i)
                    ).await
                })
            })
            .collect();
        
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        let stats = service.get_histogram_stats("concurrent.histogram").await.unwrap();
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().count, 50);
    }

    // Edge Cases and Error Handling
    #[tokio::test]
    async fn test_invalid_metric_names() {
        let service = create_test_service();
        
        // Empty metric name
        let result = service.record_metric(
            MetricType::Counter,
            "",
            MetricValue::Integer(1)
        ).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nonexistent_metric_retrieval() {
        let service = create_test_service();
        
        let value = service.get_metric("nonexistent.metric").await.unwrap();
        assert!(value.is_none());
    }

    #[tokio::test]
    async fn test_histogram_stats_for_non_histogram() {
        let service = create_test_service();
        
        // Record a counter metric
        service.record_metric(
            MetricType::Counter,
            "not.a.histogram",
            MetricValue::Integer(42)
        ).await.unwrap();
        
        // Try to get histogram stats
        let stats = service.get_histogram_stats("not.a.histogram").await.unwrap();
        assert!(stats.is_none());
    }

    #[tokio::test]
    async fn test_timer_with_invalid_id() {
        let service = create_test_service();
        
        let result = service.end_timer("invalid-timer-id").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_alert() {
        let service = create_test_service();
        
        let result = service.resolve_alert("nonexistent-id").await;
        assert!(result.is_ok()); // Should handle gracefully
    }
}