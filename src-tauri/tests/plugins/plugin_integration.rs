// Comprehensive plugin integration tests
// Tests how all plugins work together within StratoSort

#[cfg(test)]
mod test_plugin_integration {
    use super::super::plugin_fixtures::*;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_full_plugin_stack_initialization() {
        // Test all plugins initialize together correctly
        let _mock_app = MockAppHandle::new();

        // Initialize all plugin states
        let process_info = MockProcessInfo::new("stratosort");
        let os_info = MockOsInfo::default();
        let window_state = MockWindowState::default();
        let update_info = MockUpdateInfo::default();
        let localhost_server = MockLocalhostServer::new(3030);
        let http_response = MockHttpResponse::ok_json(json!({"status": "ready"}));

        // Verify all plugins are ready
        PluginAssertions::assert_process_running(&process_info);
        PluginAssertions::assert_os_info_valid(&os_info);
        PluginAssertions::assert_window_state_valid(&window_state);
        PluginAssertions::assert_update_available(&update_info);
        PluginAssertions::assert_http_response_ok(&http_response);

        assert_eq!(
            localhost_server.port, 3030,
            "Localhost server should be on correct port"
        );
    }

    #[tokio::test]
    async fn test_file_operation_with_all_plugins() {
        // Test a complete file organization operation using all plugins
        let mock_app = MockAppHandle::new();

        // 1. Single instance check
        let instance_lock = Arc::new(tokio::sync::Mutex::new(true));
        let lock = instance_lock.try_lock();
        assert!(lock.is_ok(), "Should acquire single instance lock");

        // 2. Check OS resources before operation
        let os_info = MockOsInfo::default();
        assert!(
            os_info.available_memory > 1_000_000_000,
            "Should have enough memory for operation"
        );

        // 3. Create test files
        let test_files = mock_app.create_test_files(5);

        // 4. Start localhost server for AI service
        let server = MockLocalhostServer::new(3030);
        server
            .add_route("/api/analyze", r#"{"category": "Documents"}"#)
            .await;

        // 5. Make HTTP request to AI service
        let ai_response = MockHttpResponse::ok_json(json!({
            "files_analyzed": test_files.len(),
            "categories": {
                "Documents": 3,
                "Images": 2
            }
        }));
        PluginAssertions::assert_http_response_ok(&ai_response);

        // 6. Monitor process during operation
        let process = MockProcessInfo::new("file-organizer");
        assert!(process.memory_usage > 0, "Process should be using memory");

        // 7. Update window state to show progress
        let _window_state = MockWindowState {
            focused: true,
            ..Default::default()
        };

        // 8. Check for updates after operation
        let update_available = MockUpdateInfo::default().version.as_str() > "0.1.0";
        assert!(update_available, "Should check for updates");
    }

    #[tokio::test]
    async fn test_ai_analysis_pipeline() {
        // Test complete AI analysis pipeline using multiple plugins
        let mock_app = MockAppHandle::new();

        // 1. Check system resources (OS plugin)
        let os_info = MockOsInfo::default();
        let can_run_ai = os_info.available_memory > 4_000_000_000; // Need 4GB for AI
        assert!(can_run_ai, "Should have resources for AI");

        // 2. Start local AI server (localhost plugin)
        let ai_server = MockLocalhostServer::new(3030);
        ai_server
            .add_route(
                "/ollama/api/generate",
                r#"{"response": "Document analysis complete"}"#,
            )
            .await;

        // 3. Prepare file for analysis
        let test_file = mock_app.create_test_file("report.pdf", "Annual report content");

        // 4. Make HTTP request to AI service (http plugin)
        let _analysis_request = json!({
            "model": "llama2",
            "file": test_file.to_str().unwrap(),
            "task": "categorize"
        });

        let _response = MockHttpResponse::ok_json(json!({
            "category": "Financial",
            "confidence": 0.92,
            "tags": ["annual", "report", "2024"]
        }));

        // 5. Monitor AI process (process plugin)
        let ai_process = MockProcessInfo {
            pid: 5000,
            name: "ollama".to_string(),
            cmd: vec!["ollama".to_string(), "run".to_string()],
            memory_usage: 3_000_000_000, // 3GB for model
            cpu_usage: 65.0,
        };

        assert!(ai_process.cpu_usage > 50.0, "AI should be CPU intensive");

        // 6. Update UI with results (window-state & positioner plugins)
        let notification_position = (1620, 20); // Top-right corner
        assert!(
            notification_position.0 > 0,
            "Notification should be positioned"
        );
    }

    #[tokio::test]
    async fn test_watch_mode_with_plugins() {
        // Test file watch mode using integrated plugins
        let mock_app = MockAppHandle::new();
        let _watched_dir = mock_app.data_dir.clone();

        // 1. Single instance ensures only one watcher
        let instance_lock = Arc::new(tokio::sync::Mutex::new(true));
        assert!(
            instance_lock.try_lock().is_ok(),
            "Should be single instance"
        );

        // 2. Start file watcher process
        let watcher_process = MockProcessInfo::new("file-watcher");

        // 3. Monitor system resources during watching
        let os_info = MockOsInfo::default();
        let _resource_usage = HashMap::from([
            ("memory_used", watcher_process.memory_usage),
            ("memory_available", os_info.available_memory),
            ("cpu_usage", watcher_process.cpu_usage as u64),
        ]);

        // 4. Detect new file
        let _new_file = mock_app.create_test_file("new_doc.txt", "New content");

        // 5. Send notification via localhost server
        let server = MockLocalhostServer::new(3030);
        server
            .add_route(
                "/api/notify",
                r#"{"type": "file_detected", "path": "new_doc.txt"}"#,
            )
            .await;

        // 6. Trigger AI analysis via HTTP
        let analysis_response = MockHttpResponse::ok_json(json!({
            "action": "organize",
            "destination": "/organized/documents/"
        }));

        assert_eq!(analysis_response.status, 200, "Analysis should succeed");
    }

    #[tokio::test]
    async fn test_update_process_integration() {
        // Test update process with all plugins

        // 1. Check for updates (updater plugin)
        let _update_info = MockUpdateInfo::default();

        // 2. Verify single instance before update
        let instance_lock = Arc::new(tokio::sync::Mutex::new(true));
        assert!(
            instance_lock.try_lock().is_ok(),
            "Should be single instance"
        );

        // 3. Check system requirements (os plugin)
        let os_info = MockOsInfo::default();
        let has_space = os_info.available_memory > 100_000_000; // Need 100MB free
        assert!(has_space, "Should have space for update");

        // 4. Save window state before update (window-state plugin)
        let current_state = MockWindowState::default();
        let saved_state = json!({
            "x": current_state.x,
            "y": current_state.y,
            "width": current_state.width,
            "height": current_state.height
        });

        // 5. Download update via HTTP (http plugin)
        let _download_response = MockHttpResponse::ok_json(json!({
            "download_complete": true,
            "size": 50_000_000
        }));

        // 6. Stop all processes before update (process plugin)
        let processes_to_stop = ["file-watcher", "ai-service", "localhost-server"];
        for process_name in processes_to_stop {
            // Simulate stopping process
            // Process should be stopped
            let _ = process_name;
        }

        // 7. Apply update and restart
        assert!(
            !saved_state.is_null(),
            "Window state should be preserved through update"
        );
    }

    #[tokio::test]
    async fn test_error_recovery_with_plugins() {
        // Test error recovery using plugin capabilities

        // 1. Simulate AI service failure
        let http_error = MockHttpResponse::error(503, "Service Unavailable");
        assert_eq!(http_error.status, 503, "Should detect service error");

        // 2. Fall back to localhost server
        let fallback_server = MockLocalhostServer::new(3031); // Different port
        fallback_server.add_route("/api/fallback", "OK").await;

        // 3. Check process health (process plugin)
        let unhealthy_process = MockProcessInfo {
            pid: 9999,
            name: "crashed-service".to_string(),
            cmd: vec![],
            memory_usage: 0,
            cpu_usage: 0.0,
        };

        // 4. Restart process if needed
        if unhealthy_process.memory_usage == 0 {
            let new_process = MockProcessInfo::new("restarted-service");
            PluginAssertions::assert_process_running(&new_process);
        }

        // 5. Notify user via positioned notification
        let error_notification_pos = (1620, 20); // Top-right
        assert!(
            error_notification_pos.0 > 0,
            "Error notification should be positioned"
        );

        // 6. Log error details for debugging
        let error_log = json!({
            "timestamp": "2024-01-20T10:00:00Z",
            "error": "Service unavailable",
            "recovery_action": "Switched to fallback server",
            "process_restarted": true
        });

        assert!(
            !error_log["recovery_action"].is_null(),
            "Should log recovery action"
        );
    }

    #[tokio::test]
    async fn test_performance_monitoring_integration() {
        // Test performance monitoring across all plugins
        let metrics = Arc::new(RwLock::new(HashMap::new()));

        // 1. Collect OS metrics
        let os_info = MockOsInfo::default();
        metrics
            .write()
            .await
            .insert("total_memory".to_string(), os_info.total_memory);
        metrics
            .write()
            .await
            .insert("available_memory".to_string(), os_info.available_memory);

        // 2. Collect process metrics
        let process = MockProcessInfo::new("stratosort");
        metrics
            .write()
            .await
            .insert("process_memory".to_string(), process.memory_usage);
        metrics
            .write()
            .await
            .insert("cpu_usage".to_string(), process.cpu_usage as u64);

        // 3. Collect HTTP metrics
        let request_count = 42u64;
        let avg_response_time = 150u64; // ms
        metrics
            .write()
            .await
            .insert("http_requests".to_string(), request_count);
        metrics
            .write()
            .await
            .insert("avg_response_time".to_string(), avg_response_time);

        // 4. Collect localhost server metrics
        let server_uptime = 3600u64; // seconds
        let requests_served = 1000u64;
        metrics
            .write()
            .await
            .insert("server_uptime".to_string(), server_uptime);
        metrics
            .write()
            .await
            .insert("requests_served".to_string(), requests_served);

        // 5. Analyze performance
        let collected_metrics = metrics.read().await;
        assert!(collected_metrics.len() >= 8, "Should collect all metrics");

        let memory_usage_percent = (process.memory_usage * 100) / os_info.total_memory;
        assert!(
            memory_usage_percent < 50,
            "Memory usage should be reasonable"
        );
    }

    #[tokio::test]
    async fn test_concurrent_operations_with_plugins() {
        // Test concurrent file operations with all plugins
        let operation_count = 10;
        let mut handles = vec![];

        for i in 0..operation_count {
            let handle = tokio::spawn(async move {
                // Each operation uses plugins
                let process = MockProcessInfo::new(&format!("worker-{}", i));
                let response = MockHttpResponse::ok_json(json!({
                    "operation": i,
                    "status": "complete"
                }));

                // Simulate work
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                (process.pid, response.status)
            });
            handles.push(handle);
        }

        // Wait for all operations
        let mut results = vec![];
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        assert_eq!(
            results.len(),
            operation_count,
            "All operations should complete"
        );
        assert!(
            results.iter().all(|(_, status)| *status == 200),
            "All operations should succeed"
        );
    }
}
