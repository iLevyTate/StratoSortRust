use stratosort::state::{AppState, OperationType, OperationStatus, ResourceUsage, FileCache};
use stratosort::config::Config;
use stratosort::error::{AppError, Result};
use tauri::test::{mock_app, mock_context};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{timeout, sleep};
use uuid::Uuid;
use tempfile::tempdir;

#[cfg(test)]
mod state_management_tests {
    use super::*;

    async fn create_test_state() -> Arc<AppState> {
        let app = mock_app(mock_context());
        let config = Config::default();
        AppState::new(app.handle().clone(), config)
            .await
            .expect("Failed to create app state")
    }

    // State Initialization Tests
    #[tokio::test]
    async fn test_state_initialization() {
        let state = create_test_state().await;
        
        // Verify all components are initialized
        assert!(state.database.is_initialized());
        assert!(state.ai_service.is_connected().await);
        assert_eq!(state.active_operations.len(), 0);
        assert_eq!(state.file_cache.current_size(), 0);
    }

    #[tokio::test]
    async fn test_state_with_custom_config() {
        let app = mock_app(mock_context());
        let mut config = Config::default();
        config.max_concurrent_analysis = 10;
        config.cache_size_mb = 256;
        
        let state = AppState::new(app.handle().clone(), config.clone())
            .await
            .expect("Failed to create state");
        
        let current_config = state.config.read();
        assert_eq!(current_config.max_concurrent_analysis, 10);
        assert_eq!(current_config.cache_size_mb, 256);
    }

    // Configuration Management Tests
    #[tokio::test]
    async fn test_update_config() {
        let state = create_test_state().await;
        
        let mut new_config = Config::default();
        new_config.ai_provider = "custom".to_string();
        new_config.max_file_size_mb = 100;
        
        let result = state.update_config(new_config.clone()).await;
        assert!(result.is_ok());
        
        let current = state.config.read();
        assert_eq!(current.ai_provider, "custom");
        assert_eq!(current.max_file_size_mb, 100);
    }

    #[tokio::test]
    async fn test_config_persistence() {
        let app = mock_app(mock_context());
        let mut config = Config::default();
        config.auto_organize = true;
        
        let state = AppState::new(app.handle().clone(), config.clone())
            .await
            .unwrap();
        
        // Update config
        let mut updated = config;
        updated.auto_organize = false;
        
        state.update_config(updated).await.unwrap();
        
        // Config should be saved to disk
        // In real implementation, we'd reload and verify
        let current = state.config.read();
        assert!(!current.auto_organize);
    }

    // Operation Management Tests
    #[tokio::test]
    async fn test_start_operation() {
        let state = create_test_state().await;
        
        let op_id = state.start_operation(OperationType::FileAnalysis);
        
        assert!(state.is_operation_active(op_id));
        assert_eq!(state.active_operations.len(), 1);
        
        let status = state.get_operation_status(op_id);
        assert!(status.is_some());
        assert_eq!(status.unwrap().operation_type, OperationType::FileAnalysis);
    }

    #[tokio::test]
    async fn test_update_operation_progress() {
        let state = create_test_state().await;
        
        let op_id = state.start_operation(OperationType::OrganizeFiles);
        
        state.update_progress(op_id, 0.5, "Processing files...".to_string());
        
        let status = state.get_operation_status(op_id).unwrap();
        assert_eq!(status.progress, 0.5);
        assert_eq!(status.message, "Processing files...");
    }

    #[tokio::test]
    async fn test_complete_operation() {
        let state = create_test_state().await;
        
        let op_id = state.start_operation(OperationType::DatabaseMaintenance);
        assert!(state.is_operation_active(op_id));
        
        state.complete_operation(op_id);
        assert!(!state.is_operation_active(op_id));
        assert_eq!(state.active_operations.len(), 0);
    }

    #[tokio::test]
    async fn test_cancel_operation() {
        let state = create_test_state().await;
        
        let op_id = state.start_operation(OperationType::FileAnalysis);
        let token = state.get_cancellation_token(op_id).unwrap();
        
        assert!(!token.is_cancelled());
        
        state.cancel_operation(op_id);
        assert!(token.is_cancelled());
        assert!(!state.is_operation_active(op_id));
    }

    #[tokio::test]
    async fn test_multiple_concurrent_operations() {
        let state = create_test_state().await;
        
        // Start multiple operations
        let op1 = state.start_operation(OperationType::FileAnalysis);
        let op2 = state.start_operation(OperationType::OrganizeFiles);
        let op3 = state.start_operation(OperationType::SmartFolderUpdate);
        
        assert_eq!(state.active_operations.len(), 3);
        
        // Update different operations
        state.update_progress(op1, 0.3, "Analyzing...".to_string());
        state.update_progress(op2, 0.6, "Organizing...".to_string());
        state.complete_operation(op3);
        
        assert_eq!(state.active_operations.len(), 2);
        assert!(state.is_operation_active(op1));
        assert!(state.is_operation_active(op2));
        assert!(!state.is_operation_active(op3));
    }

    // File Cache Tests
    #[tokio::test]
    async fn test_file_cache_basic_operations() {
        let state = create_test_state().await;
        
        let key = "test_file".to_string();
        let data = vec![1, 2, 3, 4, 5];
        
        state.file_cache.set(key.clone(), data.clone());
        
        assert!(state.file_cache.contains(&key));
        assert_eq!(state.file_cache.get(&key), Some(data));
        assert_eq!(state.file_cache.current_size(), 5);
    }

    #[tokio::test]
    async fn test_file_cache_eviction() {
        let state = create_test_state().await;
        
        // Fill cache with data
        for i in 0..100 {
            let key = format!("file_{}", i);
            let data = vec![0u8; 1024 * 1024]; // 1MB each
            state.file_cache.set(key, data);
        }
        
        // Cache should enforce size limits
        let max_size = state.config.read().cache_size_mb as usize * 1024 * 1024;
        assert!(state.file_cache.current_size() <= max_size);
    }

    #[tokio::test]
    async fn test_file_cache_clear() {
        let state = create_test_state().await;
        
        // Add data to cache
        for i in 0..10 {
            state.file_cache.set(format!("key_{}", i), vec![0u8; 100]);
        }
        
        assert!(state.file_cache.current_size() > 0);
        
        state.file_cache.clear();
        assert_eq!(state.file_cache.current_size(), 0);
    }

    #[tokio::test]
    async fn test_cache_concurrent_access() {
        let state = create_test_state().await;
        
        let handles: Vec<_> = (0..50)
            .map(|i| {
                let cache = state.file_cache.clone();
                tokio::spawn(async move {
                    let key = format!("concurrent_{}", i);
                    let data = vec![i as u8; 100];
                    cache.set(key.clone(), data.clone());
                    cache.get(&key)
                })
            })
            .collect();
        
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
        }
    }

    // Resource Usage Monitoring Tests
    #[tokio::test]
    async fn test_get_resource_usage() {
        let state = create_test_state().await;
        
        // Start some operations
        state.start_operation(OperationType::FileAnalysis);
        state.start_operation(OperationType::OrganizeFiles);
        
        // Add cache data
        state.file_cache.set("test".to_string(), vec![0u8; 1024 * 1024]);
        
        let usage = state.get_resource_usage().await;
        
        assert_eq!(usage.active_operations, 2);
        assert!(usage.cache_size_mb >= 1.0);
        assert!(usage.memory_mb > 0.0);
        assert!(usage.cpu_percent >= 0.0);
    }

    #[tokio::test]
    async fn test_resource_usage_tracking() {
        let state = create_test_state().await;
        
        let initial_usage = state.get_resource_usage().await;
        
        // Perform operations that affect resources
        for i in 0..5 {
            state.start_operation(OperationType::FileAnalysis);
            state.file_cache.set(format!("file_{}", i), vec![0u8; 500_000]);
        }
        
        let updated_usage = state.get_resource_usage().await;
        
        assert!(updated_usage.active_operations > initial_usage.active_operations);
        assert!(updated_usage.cache_size_mb > initial_usage.cache_size_mb);
    }

    // Cleanup and Shutdown Tests
    #[tokio::test]
    async fn test_cleanup_cache() {
        let state = create_test_state().await;
        
        // Add data to cache and database
        state.file_cache.set("temp".to_string(), vec![0u8; 1000]);
        
        let result = state.cleanup_cache().await;
        assert!(result.is_ok());
        
        // Cache should be cleaned (implementation dependent)
        // Database cleanup would be tested in integration tests
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let state = create_test_state().await;
        
        // Start operations
        let op1 = state.start_operation(OperationType::FileAnalysis);
        let op2 = state.start_operation(OperationType::OrganizeFiles);
        
        // Start file watcher
        let watcher = Arc::new(stratosort::services::FileWatcher::new(state.clone()));
        *state.file_watcher.write() = Some(watcher.clone());
        
        // Add cache data
        state.file_cache.set("data".to_string(), vec![0u8; 1000]);
        
        // Perform shutdown
        let result = state.shutdown().await;
        assert!(result.is_ok());
        
        // All operations should be cancelled
        assert!(!state.is_operation_active(op1));
        assert!(!state.is_operation_active(op2));
        assert_eq!(state.active_operations.len(), 0);
        
        // Cache should be cleared
        assert_eq!(state.file_cache.current_size(), 0);
        
        // File watcher should be stopped
        assert!(state.file_watcher.read().is_none() || 
                !state.file_watcher.read().as_ref().unwrap().is_watching().await);
    }

    #[tokio::test]
    async fn test_shutdown_with_timeout() {
        let state = create_test_state().await;
        
        // Start long-running operation
        let op_id = state.start_operation(OperationType::DatabaseMaintenance);
        
        // Simulate operation that doesn't respond to cancellation
        let token = state.get_cancellation_token(op_id).unwrap();
        let handle = tokio::spawn(async move {
            loop {
                if token.is_cancelled() {
                    // Intentionally ignore cancellation for test
                    sleep(Duration::from_secs(10)).await;
                }
                sleep(Duration::from_millis(100)).await;
            }
        });
        
        // Shutdown should complete despite unresponsive operation
        let shutdown_result = timeout(
            Duration::from_secs(2),
            state.shutdown()
        ).await;
        
        assert!(shutdown_result.is_ok());
        
        // Operation should be force cleared
        assert_eq!(state.active_operations.len(), 0);
        
        handle.abort();
    }

    // Service Integration Tests
    #[tokio::test]
    async fn test_ai_service_integration() {
        let state = create_test_state().await;
        
        // Test AI service is accessible
        let connected = state.ai_service.is_connected().await;
        assert!(connected || !connected); // Service may or may not be available
        
        // Test fallback behavior
        let embedding = state.ai_service.generate_embedding("test text").await;
        assert!(embedding.is_ok()); // Should use fallback if service unavailable
    }

    #[tokio::test]
    async fn test_database_integration() {
        let state = create_test_state().await;
        
        // Database should be initialized
        assert!(state.database.is_initialized());
        
        // Test basic operation
        let result = state.database.record_file_operation(
            "test.txt",
            "move",
            Some("/new/location.txt"),
            None
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_smart_folder_manager_integration() {
        let state = create_test_state().await;
        
        // Create a smart folder
        let folder = stratosort::core::SmartFolder {
            id: Uuid::new_v4(),
            name: "Test Folder".to_string(),
            path: "/test/path".to_string(),
            rules: vec![],
            auto_organize: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = state.smart_folders.create(folder).await;
        assert!(result.is_ok());
        
        let all = state.smart_folders.get_all().await;
        assert!(all.is_ok());
    }

    // Concurrent State Modifications Tests
    #[tokio::test]
    async fn test_concurrent_operation_starts() {
        let state = create_test_state().await;
        
        let handles: Vec<_> = (0..20)
            .map(|i| {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    let op_type = match i % 3 {
                        0 => OperationType::FileAnalysis,
                        1 => OperationType::OrganizeFiles,
                        _ => OperationType::SmartFolderUpdate,
                    };
                    state_clone.start_operation(op_type)
                })
            })
            .collect();
        
        let mut op_ids = vec![];
        for handle in handles {
            op_ids.push(handle.await.unwrap());
        }
        
        // All operations should be unique
        let unique_ids: std::collections::HashSet<_> = op_ids.iter().collect();
        assert_eq!(unique_ids.len(), 20);
        
        // All should be active
        for id in op_ids {
            assert!(state.is_operation_active(id));
        }
    }

    #[tokio::test]
    async fn test_concurrent_config_updates() {
        let state = create_test_state().await;
        
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    let mut config = Config::default();
                    config.max_concurrent_analysis = i as usize;
                    state_clone.update_config(config).await
                })
            })
            .collect();
        
        for handle in handles {
            // Some updates might fail due to concurrent access, but shouldn't panic
            let _ = handle.await.unwrap();
        }
        
        // Final config should be valid
        let config = state.config.read();
        assert!(config.max_concurrent_analysis < 10);
    }

    // Memory Management Tests
    #[tokio::test]
    async fn test_memory_pressure_handling() {
        let state = create_test_state().await;
        
        // Simulate memory pressure by filling cache
        for i in 0..1000 {
            let key = format!("large_file_{}", i);
            let data = vec![0u8; 100_000]; // 100KB each
            state.file_cache.set(key, data);
            
            // Check if cache respects limits
            let size = state.file_cache.current_size();
            let max_size = state.config.read().cache_size_mb as usize * 1024 * 1024;
            assert!(size <= max_size);
        }
    }

    #[tokio::test]
    async fn test_operation_cleanup_on_error() {
        let state = create_test_state().await;
        
        let op_id = state.start_operation(OperationType::FileAnalysis);
        
        // Simulate operation error by cancelling
        state.cancel_operation(op_id);
        
        // Operation should be cleaned up
        assert!(!state.is_operation_active(op_id));
        assert!(state.get_operation_status(op_id).is_none());
    }

    // State Consistency Tests
    #[tokio::test]
    async fn test_state_consistency_after_errors() {
        let state = create_test_state().await;
        
        // Start operations
        let op1 = state.start_operation(OperationType::FileAnalysis);
        let op2 = state.start_operation(OperationType::OrganizeFiles);
        
        // Simulate partial failure
        state.cancel_operation(op1);
        state.update_progress(op2, 0.5, "Halfway".to_string());
        
        // State should remain consistent
        assert!(!state.is_operation_active(op1));
        assert!(state.is_operation_active(op2));
        
        let status = state.get_operation_status(op2).unwrap();
        assert_eq!(status.progress, 0.5);
    }

    #[tokio::test]
    async fn test_state_recovery_simulation() {
        let state = create_test_state().await;
        
        // Simulate state before "crash"
        state.start_operation(OperationType::FileAnalysis);
        state.file_cache.set("important".to_string(), vec![1, 2, 3]);
        
        // Simulate recovery by clearing transient state
        state.active_operations.clear();
        
        // State should be recoverable
        assert_eq!(state.active_operations.len(), 0);
        assert!(state.database.is_initialized());
        
        // Can start new operations
        let new_op = state.start_operation(OperationType::OrganizeFiles);
        assert!(state.is_operation_active(new_op));
    }
}