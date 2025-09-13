use stratosort::commands::system::*;
use stratosort::error::{AppError, Result};
use stratosort::state::AppState;
use stratosort::config::Config;
use std::sync::Arc;
use tauri::test::{mock_app, mock_context};
use tempfile::tempdir;
use tokio::sync::RwLock;

#[cfg(test)]
mod system_command_tests {
    use super::*;

    async fn create_test_state() -> Arc<AppState> {
        let app = mock_app(mock_context());
        let config = Config::default();
        AppState::new(app.handle().clone(), config)
            .await
            .expect("Failed to create app state")
    }

    // Frontend Ready Command Tests
    #[tokio::test]
    async fn test_frontend_ready_success() {
        let app = mock_app(mock_context());
        let result = frontend_ready(app.handle().clone()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_frontend_ready_window_operations() {
        let app = mock_app(mock_context());
        // Create main window mock
        let result = frontend_ready(app.handle().clone()).await;
        assert!(result.is_ok());
        // Window operations should be attempted even if window doesn't exist
    }

    // System Info Tests
    #[tokio::test]
    async fn test_get_basic_system_info() {
        let app = mock_app(mock_context());
        let result = get_basic_system_info(app.handle().clone()).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        // Validate all fields are populated
        assert!(!info.platform.is_empty());
        assert!(!info.arch.is_empty());
        assert!(!info.version.is_empty());
        assert!(info.total_memory > 0);
        assert!(info.cpu_count > 0);
        assert!(!info.home_dir.is_empty());
        assert!(!info.temp_dir.is_empty());
        assert!(!info.app_version.is_empty());
        assert!(!info.rust_version.is_empty());
    }

    #[tokio::test]
    async fn test_system_info_platform_detection() {
        let app = mock_app(mock_context());
        let result = get_basic_system_info(app.handle().clone()).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        #[cfg(target_os = "windows")]
        assert_eq!(info.platform, "windows");
        
        #[cfg(target_os = "macos")]
        assert_eq!(info.platform, "macos");
        
        #[cfg(target_os = "linux")]
        assert_eq!(info.platform, "linux");
    }

    // Storage Info Tests
    #[tokio::test]
    async fn test_get_storage_info() {
        let state = create_test_state().await;
        let app = mock_app(mock_context());
        
        let result = get_storage_info(
            tauri::State::from(state),
            app.handle().clone()
        ).await;
        
        assert!(result.is_ok());
        let storage = result.unwrap();
        
        // Basic validation
        assert!(storage.total_space >= storage.free_space);
        assert!(storage.used_space <= storage.total_space);
        assert!(storage.cache_size >= 0);
        assert!(storage.database_size >= 0);
    }

    #[tokio::test]
    async fn test_storage_info_calculations() {
        let state = create_test_state().await;
        let app = mock_app(mock_context());
        
        // Add some data to cache
        state.file_cache.set("test_key".to_string(), b"test_data".to_vec());
        
        let result = get_storage_info(
            tauri::State::from(state.clone()),
            app.handle().clone()
        ).await;
        
        assert!(result.is_ok());
        let storage = result.unwrap();
        
        // Cache should have some size
        assert!(storage.cache_size > 0);
        
        // Used space should be calculated correctly
        assert_eq!(storage.used_space, storage.total_space - storage.free_space);
    }

    // Default Folders Tests
    #[tokio::test]
    async fn test_get_default_folders() {
        let app = mock_app(mock_context());
        let result = get_default_folders(app.handle().clone()).await;
        
        assert!(result.is_ok());
        let folders = result.unwrap();
        
        // At least home should be populated
        assert!(!folders.home.is_empty());
        
        // Validate folder structure
        if !folders.documents.is_empty() {
            assert!(folders.documents.contains(&folders.home) || 
                   folders.documents.starts_with("/"));
        }
    }

    // Open Folder Tests
    #[tokio::test]
    async fn test_open_folder_valid_directory() {
        let app = mock_app(mock_context());
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_string_lossy().to_string();
        
        let result = open_folder(dir_path.clone(), app.handle().clone()).await;
        
        // Should succeed or fail gracefully depending on system
        if result.is_err() {
            match result.unwrap_err() {
                AppError::SystemError { .. } => {
                    // Expected on CI/headless systems
                },
                _ => panic!("Unexpected error type"),
            }
        }
    }

    #[tokio::test]
    async fn test_open_folder_nonexistent() {
        let app = mock_app(mock_context());
        let result = open_folder(
            "/nonexistent/path/that/does/not/exist".to_string(),
            app.handle().clone()
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidPath { message } => {
                assert!(message.contains("does not exist"));
            },
            _ => panic!("Expected InvalidPath error"),
        }
    }

    #[tokio::test]
    async fn test_open_folder_file_instead_of_directory() {
        let app = mock_app(mock_context());
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();
        
        let result = open_folder(
            file_path.to_string_lossy().to_string(),
            app.handle().clone()
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidPath { message } => {
                assert!(message.contains("not a directory"));
            },
            _ => panic!("Expected InvalidPath error"),
        }
    }

    #[tokio::test]
    async fn test_open_folder_security_injection_attempts() {
        let app = mock_app(mock_context());
        
        // Test command injection attempts
        let dangerous_paths = vec![
            "test; rm -rf /",
            "test && echo hacked",
            "test | cat /etc/passwd",
            "test`whoami`",
            "$(malicious_command)",
            "test\n\nmalicious",
            "../../../etc/passwd",
        ];
        
        for path in dangerous_paths {
            let result = open_folder(path.to_string(), app.handle().clone()).await;
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::SecurityError { .. } | AppError::InvalidPath { .. } => {
                    // Expected security rejection
                },
                e => panic!("Expected SecurityError or InvalidPath, got: {:?}", e),
            }
        }
    }

    #[tokio::test]
    async fn test_open_folder_executable_patterns() {
        let app = mock_app(mock_context());
        let temp_dir = tempdir().unwrap();
        
        // Create directories with executable-like names
        let dangerous_names = vec![
            "test.exe",
            "script.bat",
            "command.cmd",
            "shell.sh",
        ];
        
        for name in dangerous_names {
            let dir_path = temp_dir.path().join(name);
            std::fs::create_dir_all(&dir_path).unwrap();
            
            let result = open_folder(
                dir_path.to_string_lossy().to_string(),
                app.handle().clone()
            ).await;
            
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::SecurityError { message } => {
                    assert!(message.contains("executable"));
                },
                _ => panic!("Expected SecurityError for executable pattern"),
            }
        }
    }

    // Open With Default Tests
    #[tokio::test]
    async fn test_open_with_default_valid_file() {
        let app = mock_app(mock_context());
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();
        
        let result = open_with_default(
            file_path.to_string_lossy().to_string(),
            app.handle().clone()
        ).await;
        
        // Should succeed or fail gracefully depending on system
        if result.is_err() {
            match result.unwrap_err() {
                AppError::SystemError { .. } => {
                    // Expected on CI/headless systems
                },
                _ => panic!("Unexpected error type"),
            }
        }
    }

    #[tokio::test]
    async fn test_open_with_default_dangerous_extensions() {
        let app = mock_app(mock_context());
        let temp_dir = tempdir().unwrap();
        
        let dangerous_extensions = vec![
            ".exe", ".bat", ".cmd", ".com", ".scr", 
            ".pif", ".vbs", ".js", ".jar",
        ];
        
        for ext in dangerous_extensions {
            let file_path = temp_dir.path().join(format!("test{}", ext));
            std::fs::write(&file_path, "content").unwrap();
            
            let result = open_with_default(
                file_path.to_string_lossy().to_string(),
                app.handle().clone()
            ).await;
            
            assert!(result.is_err());
            match result.unwrap_err() {
                AppError::SecurityError { message } => {
                    assert!(message.contains("dangerous"));
                },
                _ => panic!("Expected SecurityError for dangerous extension"),
            }
        }
    }

    #[tokio::test]
    async fn test_open_with_default_nonexistent_file() {
        let app = mock_app(mock_context());
        
        let result = open_with_default(
            "/nonexistent/file.txt".to_string(),
            app.handle().clone()
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::FileNotFound { .. } => {},
            _ => panic!("Expected FileNotFound error"),
        }
    }

    // Show in Folder Tests
    #[tokio::test]
    async fn test_show_in_folder_valid_file() {
        let app = mock_app(mock_context());
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();
        
        let result = show_in_folder(
            file_path.to_string_lossy().to_string(),
            app.handle().clone()
        ).await;
        
        // Should succeed or fail gracefully depending on system
        if result.is_err() {
            match result.unwrap_err() {
                AppError::SystemError { .. } => {
                    // Expected on CI/headless systems
                },
                _ => panic!("Unexpected error type"),
            }
        }
    }

    #[tokio::test]
    async fn test_show_in_folder_nonexistent() {
        let app = mock_app(mock_context());
        
        let result = show_in_folder(
            "/nonexistent/file.txt".to_string(),
            app.handle().clone()
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::FileNotFound { .. } => {},
            _ => panic!("Expected FileNotFound error"),
        }
    }

    // Clear Cache Tests
    #[tokio::test]
    async fn test_clear_cache_empty() {
        let state = create_test_state().await;
        
        let result = clear_cache(tauri::State::from(state)).await;
        
        assert!(result.is_ok());
        let clear_result = result.unwrap();
        assert!(clear_result.success);
        assert_eq!(clear_result.freed_bytes, 0);
    }

    #[tokio::test]
    async fn test_clear_cache_with_data() {
        let state = create_test_state().await;
        
        // Add data to cache
        state.file_cache.set("key1".to_string(), vec![1, 2, 3, 4, 5]);
        state.file_cache.set("key2".to_string(), vec![6, 7, 8, 9, 10]);
        
        let before_size = state.file_cache.current_size();
        assert!(before_size > 0);
        
        let result = clear_cache(tauri::State::from(state.clone())).await;
        
        assert!(result.is_ok());
        let clear_result = result.unwrap();
        assert!(clear_result.success);
        assert_eq!(clear_result.freed_bytes, before_size);
        
        // Cache should be empty
        assert_eq!(state.file_cache.current_size(), 0);
    }

    // App Logs Tests
    #[tokio::test]
    async fn test_get_app_logs_nonexistent() {
        let app = mock_app(mock_context());
        
        let result = get_app_logs(app.handle().clone(), None).await;
        
        assert!(result.is_ok());
        let logs = result.unwrap();
        assert!(logs.is_empty());
    }

    #[tokio::test]
    async fn test_get_app_logs_with_limit() {
        let app = mock_app(mock_context());
        let log_dir = app.path().app_log_dir().unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();
        
        let log_file = log_dir.join("stratosort.log");
        let log_content = (0..100)
            .map(|i| format!("Log line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&log_file, log_content).unwrap();
        
        // Get last 10 lines
        let result = get_app_logs(app.handle().clone(), Some(10)).await;
        
        assert!(result.is_ok());
        let logs = result.unwrap();
        assert_eq!(logs.len(), 10);
        assert!(logs[0].contains("Log line 90"));
        assert!(logs[9].contains("Log line 99"));
    }

    // Update Check Tests
    #[tokio::test]
    async fn test_check_for_updates() {
        let app = mock_app(mock_context());
        
        let result = check_for_updates(app.handle().clone()).await;
        
        assert!(result.is_ok());
        let update_info = result.unwrap();
        
        // Should at least return current version
        assert!(!update_info.current_version.is_empty());
        assert!(!update_info.latest_version.is_empty());
    }

    #[tokio::test]
    async fn test_version_comparison() {
        // Test the is_newer_version function indirectly
        let app = mock_app(mock_context());
        
        let result = check_for_updates(app.handle().clone()).await;
        assert!(result.is_ok());
        
        let info = result.unwrap();
        if info.update_available {
            // If update is available, latest should be newer
            assert_ne!(info.current_version, info.latest_version);
        } else {
            // Otherwise versions should match or current is newer
            // This is a basic check as we can't control GitHub API response
            assert!(!info.current_version.is_empty());
        }
    }

    // Shutdown Tests
    #[tokio::test]
    async fn test_shutdown_application() {
        let state = create_test_state().await;
        let app = mock_app(mock_context());
        
        // Add some active operations
        let op_id = state.start_operation(
            stratosort::state::OperationType::FileAnalysis
        );
        
        let result = shutdown_application(
            tauri::State::from(state.clone()),
            app.handle().clone()
        ).await;
        
        // Function should return before actual exit
        assert!(result.is_ok());
        assert!(result.unwrap().contains("shutdown initiated"));
        
        // Operations should be cancelled
        assert!(!state.is_operation_active(op_id));
    }

    #[tokio::test]
    async fn test_force_shutdown() {
        let app = mock_app(mock_context());
        
        let result = force_shutdown(app.handle().clone()).await;
        
        // Function should return before actual exit
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Force shutdown initiated"));
    }

    // Resource Usage Tests
    #[tokio::test]
    async fn test_get_resource_usage() {
        let state = create_test_state().await;
        
        let result = get_resource_usage(tauri::State::from(state)).await;
        
        assert!(result.is_ok());
        let usage = result.unwrap();
        
        assert!(usage.memory_mb >= 0.0);
        assert!(usage.cpu_percent >= 0.0);
        assert!(usage.active_operations >= 0);
        assert!(usage.cache_size_mb >= 0.0);
    }

    #[tokio::test]
    async fn test_resource_usage_with_operations() {
        let state = create_test_state().await;
        
        // Start some operations
        state.start_operation(stratosort::state::OperationType::FileAnalysis);
        state.start_operation(stratosort::state::OperationType::OrganizeFiles);
        
        // Add cache data
        state.file_cache.set("test".to_string(), vec![0; 1024 * 1024]); // 1MB
        
        let result = get_resource_usage(tauri::State::from(state)).await;
        
        assert!(result.is_ok());
        let usage = result.unwrap();
        
        assert_eq!(usage.active_operations, 2);
        assert!(usage.cache_size_mb >= 1.0);
    }

    // Edge Cases and Error Scenarios
    #[tokio::test]
    async fn test_path_traversal_attempts() {
        let app = mock_app(mock_context());
        
        let malicious_paths = vec![
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32",
            "test/../../sensitive",
            "./../../root",
        ];
        
        for path in malicious_paths {
            // Test with open_folder
            let result = open_folder(path.to_string(), app.handle().clone()).await;
            assert!(result.is_err());
            
            // Test with open_with_default
            let result = open_with_default(path.to_string(), app.handle().clone()).await;
            assert!(result.is_err());
            
            // Test with show_in_folder
            let result = show_in_folder(path.to_string(), app.handle().clone()).await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_unicode_path_handling() {
        let app = mock_app(mock_context());
        let temp_dir = tempdir().unwrap();
        
        // Create file with unicode characters
        let unicode_file = temp_dir.path().join("测试文件_тест_テスト.txt");
        std::fs::write(&unicode_file, "content").unwrap();
        
        let result = show_in_folder(
            unicode_file.to_string_lossy().to_string(),
            app.handle().clone()
        ).await;
        
        // Should handle unicode paths gracefully
        if result.is_err() {
            match result.unwrap_err() {
                AppError::SystemError { .. } => {
                    // Expected on some systems
                },
                _ => panic!("Unexpected error for unicode path"),
            }
        }
    }

    #[tokio::test]
    async fn test_very_long_path_handling() {
        let app = mock_app(mock_context());
        
        // Create a very long path
        let long_path = format!("{}/{}", 
            "/test",
            "a".repeat(500) // Very long filename
        );
        
        let result = open_folder(long_path, app.handle().clone()).await;
        
        assert!(result.is_err());
        // Should reject or handle gracefully
    }

    #[tokio::test]
    async fn test_concurrent_cache_operations() {
        let state = create_test_state().await;
        
        // Spawn multiple tasks that interact with cache
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    state_clone.file_cache.set(
                        format!("key_{}", i),
                        vec![i as u8; 100]
                    );
                })
            })
            .collect();
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Clear cache
        let result = clear_cache(tauri::State::from(state.clone())).await;
        assert!(result.is_ok());
        
        // Cache should be empty
        assert_eq!(state.file_cache.current_size(), 0);
    }

    #[tokio::test]
    async fn test_directory_size_calculation_with_symlinks() {
        let temp_dir = tempdir().unwrap();
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");
        std::fs::create_dir_all(&dir1).unwrap();
        std::fs::create_dir_all(&dir2).unwrap();
        
        // Create some files
        std::fs::write(dir1.join("file1.txt"), "content1").unwrap();
        std::fs::write(dir2.join("file2.txt"), "content2").unwrap();
        
        // Create symlink (may fail on Windows without admin rights)
        #[cfg(unix)]
        {
            let link_path = dir1.join("link_to_dir2");
            let _ = std::os::unix::fs::symlink(&dir2, link_path);
        }
        
        // Calculate size should not follow symlinks (prevent infinite loops)
        let size_result = super::calculate_dir_size(&dir1.to_path_buf()).await;
        assert!(size_result.is_ok());
        
        let size = size_result.unwrap();
        // Size should only include file1.txt, not the linked directory
        assert!(size < 1000); // Should be small, just the one file
    }

    #[tokio::test]
    async fn test_memory_pressure_scenarios() {
        let state = create_test_state().await;
        
        // Simulate high memory usage in cache
        for i in 0..100 {
            state.file_cache.set(
                format!("large_key_{}", i),
                vec![0; 1024 * 1024] // 1MB each
            );
        }
        
        let initial_size = state.file_cache.current_size();
        assert!(initial_size > 100 * 1024 * 1024); // Should be ~100MB
        
        // Clear cache should handle large amounts of data
        let result = clear_cache(tauri::State::from(state.clone())).await;
        assert!(result.is_ok());
        
        let clear_result = result.unwrap();
        assert!(clear_result.success);
        assert_eq!(clear_result.freed_bytes, initial_size);
    }
}