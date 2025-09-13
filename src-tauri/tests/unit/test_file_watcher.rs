use stratosort::services::{FileWatcher, FileEvent, UserAction, UserActionType, WatchModeConfig};
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::{AppError, Result};
use tauri::test::{mock_app, mock_context};
use tempfile::tempdir;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

#[cfg(test)]
mod file_watcher_tests {
    use super::*;

    async fn create_test_watcher() -> (Arc<FileWatcher>, tempfile::TempDir) {
        let app = mock_app(mock_context());
        let config = Config::default();
        let state = Arc::new(
            AppState::new(app.handle().clone(), config)
                .await
                .expect("Failed to create app state")
        );
        
        let watcher = Arc::new(FileWatcher::new(state));
        let temp_dir = tempdir().unwrap();
        
        (watcher, temp_dir)
    }

    // Basic Start/Stop Tests
    #[tokio::test]
    async fn test_start_and_stop_watcher() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        let result = watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await;
        
        assert!(result.is_ok());
        assert!(watcher.is_watching().await);
        
        let stop_result = watcher.stop().await;
        assert!(stop_result.is_ok());
        assert!(!watcher.is_watching().await);
    }

    #[tokio::test]
    async fn test_start_with_multiple_directories() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");
        std::fs::create_dir_all(&dir1).unwrap();
        std::fs::create_dir_all(&dir2).unwrap();
        
        let result = watcher.start(vec![
            dir1.to_string_lossy().to_string(),
            dir2.to_string_lossy().to_string(),
        ]).await;
        
        assert!(result.is_ok());
        
        let watched = watcher.get_watched_directories().await;
        assert_eq!(watched.len(), 2);
    }

    #[tokio::test]
    async fn test_start_with_nonexistent_directory() {
        let (watcher, _) = create_test_watcher().await;
        
        let result = watcher.start(vec![
            "/nonexistent/directory/path".to_string()
        ]).await;
        
        assert!(result.is_err());
        assert!(!watcher.is_watching().await);
    }

    #[tokio::test]
    async fn test_double_start() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        let path = temp_dir.path().to_string_lossy().to_string();
        
        // First start
        assert!(watcher.start(vec![path.clone()]).await.is_ok());
        
        // Second start should fail
        let result = watcher.start(vec![path]).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::StateError { .. } => {},
            _ => panic!("Expected StateError for double start"),
        }
    }

    #[tokio::test]
    async fn test_stop_without_start() {
        let (watcher, _) = create_test_watcher().await;
        
        let result = watcher.stop().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::StateError { .. } => {},
            _ => panic!("Expected StateError for stop without start"),
        }
    }

    // File Event Detection Tests
    #[tokio::test]
    async fn test_detect_file_creation() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Set up event receiver
        let mut event_rx = watcher.subscribe_events().await;
        
        // Start watching
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Create a file
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, "content").await.unwrap();
        
        // Wait for event
        let event = timeout(Duration::from_secs(5), event_rx.recv())
            .await
            .expect("Timeout waiting for event")
            .expect("Failed to receive event");
        
        assert_eq!(event.event_type, "create");
        assert!(event.path.contains("test.txt"));
        assert_eq!(event.extension, Some(".txt".to_string()));
    }

    #[tokio::test]
    async fn test_detect_file_modification() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Create file first
        let test_file = temp_dir.path().join("modify.txt");
        std::fs::write(&test_file, "initial").unwrap();
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Modify the file
        tokio::time::sleep(Duration::from_millis(100)).await;
        tokio::fs::write(&test_file, "modified content").await.unwrap();
        
        // Wait for modify event
        let event = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(evt) = event_rx.recv().await {
                    if evt.event_type == "modify" {
                        return evt;
                    }
                }
            }
        })
        .await
        .expect("Timeout waiting for modify event");
        
        assert_eq!(event.event_type, "modify");
        assert!(event.path.contains("modify.txt"));
    }

    #[tokio::test]
    async fn test_detect_file_deletion() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Create file first
        let test_file = temp_dir.path().join("delete.txt");
        std::fs::write(&test_file, "content").unwrap();
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Delete the file
        tokio::time::sleep(Duration::from_millis(100)).await;
        tokio::fs::remove_file(&test_file).await.unwrap();
        
        // Wait for delete event
        let event = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(evt) = event_rx.recv().await {
                    if evt.event_type == "remove" {
                        return evt;
                    }
                }
            }
        })
        .await
        .expect("Timeout waiting for delete event");
        
        assert_eq!(event.event_type, "remove");
        assert!(event.path.contains("delete.txt"));
    }

    #[tokio::test]
    async fn test_detect_file_rename() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Create file first
        let old_path = temp_dir.path().join("old_name.txt");
        let new_path = temp_dir.path().join("new_name.txt");
        std::fs::write(&old_path, "content").unwrap();
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Rename the file
        tokio::time::sleep(Duration::from_millis(100)).await;
        tokio::fs::rename(&old_path, &new_path).await.unwrap();
        
        // Collect events (rename typically generates remove + create)
        let mut events = vec![];
        let _ = timeout(Duration::from_secs(2), async {
            while let Some(evt) = event_rx.recv().await {
                events.push(evt);
                if events.len() >= 2 {
                    break;
                }
            }
        }).await;
        
        // Should have detected the rename operation
        assert!(events.iter().any(|e| e.path.contains("old_name.txt")));
        assert!(events.iter().any(|e| e.path.contains("new_name.txt")));
    }

    // Watch Mode Configuration Tests
    #[tokio::test]
    async fn test_update_watch_config() {
        let (watcher, _) = create_test_watcher().await;
        
        let config = WatchModeConfig {
            enabled: true,
            watch_directories: vec!["/test".to_string()],
            auto_organize_delay_ms: 3000,
            learning_enabled: false,
            confidence_threshold: 0.8,
            max_auto_organize_count: 20,
            excluded_extensions: vec![".log".to_string()],
            excluded_directories: vec!["temp".to_string()],
        };
        
        watcher.update_config(config.clone()).await.unwrap();
        
        let retrieved = watcher.get_config().await;
        assert_eq!(retrieved.auto_organize_delay_ms, 3000);
        assert_eq!(retrieved.confidence_threshold, 0.8);
        assert!(!retrieved.learning_enabled);
    }

    #[tokio::test]
    async fn test_excluded_extensions() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Configure exclusions
        let mut config = WatchModeConfig::default();
        config.excluded_extensions = vec![".tmp".to_string(), ".log".to_string()];
        watcher.update_config(config).await.unwrap();
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Create excluded files
        tokio::fs::write(temp_dir.path().join("test.tmp"), "temp").await.unwrap();
        tokio::fs::write(temp_dir.path().join("test.log"), "log").await.unwrap();
        
        // Create included file
        tokio::fs::write(temp_dir.path().join("test.txt"), "text").await.unwrap();
        
        // Should only get event for .txt file
        let event = timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("Timeout")
            .expect("No event");
        
        assert!(event.path.contains("test.txt"));
        assert!(!event.path.contains(".tmp"));
        assert!(!event.path.contains(".log"));
    }

    #[tokio::test]
    async fn test_excluded_directories() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Create directories
        let excluded_dir = temp_dir.path().join("node_modules");
        let included_dir = temp_dir.path().join("src");
        std::fs::create_dir_all(&excluded_dir).unwrap();
        std::fs::create_dir_all(&included_dir).unwrap();
        
        // Configure exclusions
        let mut config = WatchModeConfig::default();
        config.excluded_directories = vec!["node_modules".to_string()];
        watcher.update_config(config).await.unwrap();
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Create files in both directories
        tokio::fs::write(excluded_dir.join("package.json"), "{}").await.unwrap();
        tokio::fs::write(included_dir.join("main.rs"), "fn main()").await.unwrap();
        
        // Should only get event for src directory
        let event = timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("Timeout")
            .expect("No event");
        
        assert!(event.path.contains("main.rs"));
        assert!(!event.path.contains("node_modules"));
    }

    // User Action Learning Tests
    #[tokio::test]
    async fn test_record_user_action() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        let action = UserAction {
            action_type: UserActionType::MoveFile,
            timestamp: chrono::Utc::now().timestamp(),
            file_path: temp_dir.path().join("source.txt").to_string_lossy().to_string(),
            destination_path: Some(temp_dir.path().join("dest.txt").to_string_lossy().to_string()),
            folder_created: None,
            rename_pattern: None,
            confidence: 0.9,
        };
        
        watcher.record_user_action(action.clone()).await.unwrap();
        
        let actions = watcher.get_recent_user_actions(10).await.unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].file_path, action.file_path);
    }

    #[tokio::test]
    async fn test_learn_from_user_actions() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Enable learning
        let mut config = WatchModeConfig::default();
        config.learning_enabled = true;
        watcher.update_config(config).await.unwrap();
        
        // Record multiple similar actions
        for i in 0..5 {
            let action = UserAction {
                action_type: UserActionType::MoveFile,
                timestamp: chrono::Utc::now().timestamp(),
                file_path: temp_dir.path().join(format!("report_{}.pdf", i))
                    .to_string_lossy().to_string(),
                destination_path: Some(temp_dir.path().join("reports/archive")
                    .to_string_lossy().to_string()),
                folder_created: None,
                rename_pattern: None,
                confidence: 0.95,
            };
            watcher.record_user_action(action).await.unwrap();
        }
        
        // Check if pattern was learned
        let suggestion = watcher.suggest_action_for_file(
            &temp_dir.path().join("report_new.pdf").to_string_lossy()
        ).await.unwrap();
        
        assert!(suggestion.is_some());
        let suggested = suggestion.unwrap();
        assert!(suggested.destination_path.unwrap().contains("reports/archive"));
        assert!(suggested.confidence > 0.7);
    }

    // Pending Files and Auto-Organization Tests
    #[tokio::test]
    async fn test_pending_file_detection() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Configure auto-organize with delay
        let mut config = WatchModeConfig::default();
        config.auto_organize_delay_ms = 1000;
        config.enabled = true;
        watcher.update_config(config).await.unwrap();
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Create a new file
        let test_file = temp_dir.path().join("pending.txt");
        tokio::fs::write(&test_file, "content").await.unwrap();
        
        // File should be pending
        tokio::time::sleep(Duration::from_millis(500)).await;
        let pending = watcher.get_pending_files().await;
        assert!(pending.iter().any(|p| p.path == test_file));
        
        // After delay, file should be processed
        tokio::time::sleep(Duration::from_millis(600)).await;
        let pending_after = watcher.get_pending_files().await;
        assert!(!pending_after.iter().any(|p| p.path == test_file));
    }

    #[tokio::test]
    async fn test_auto_organize_trigger() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Set up auto-organize
        let mut config = WatchModeConfig::default();
        config.enabled = true;
        config.auto_organize_delay_ms = 500;
        config.confidence_threshold = 0.5;
        watcher.update_config(config).await.unwrap();
        
        // Record pattern for .log files
        let action = UserAction {
            action_type: UserActionType::MoveFile,
            timestamp: chrono::Utc::now().timestamp(),
            file_path: temp_dir.path().join("test.log").to_string_lossy().to_string(),
            destination_path: Some(temp_dir.path().join("logs").to_string_lossy().to_string()),
            folder_created: None,
            rename_pattern: None,
            confidence: 0.9,
        };
        watcher.record_user_action(action).await.unwrap();
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Create a new .log file
        let new_log = temp_dir.path().join("new.log");
        tokio::fs::write(&new_log, "log content").await.unwrap();
        
        // Wait for auto-organize event
        let event = timeout(Duration::from_secs(3), async {
            loop {
                if let Some(evt) = event_rx.recv().await {
                    if !evt.is_user_action {
                        return evt;
                    }
                }
            }
        }).await;
        
        // Should have triggered auto-organization
        assert!(event.is_ok());
    }

    // Event Deduplication Tests
    #[tokio::test]
    async fn test_event_deduplication() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Rapidly create and modify same file
        let test_file = temp_dir.path().join("rapid.txt");
        for i in 0..5 {
            tokio::fs::write(&test_file, format!("content {}", i)).await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Collect events with timeout
        let mut events = vec![];
        let _ = timeout(Duration::from_secs(2), async {
            while let Some(evt) = event_rx.recv().await {
                events.push(evt);
            }
        }).await;
        
        // Should have deduplicated rapid changes
        assert!(events.len() < 5);
    }

    // Operation Grouping Tests
    #[tokio::test]
    async fn test_operation_grouping() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        let operation_id = Uuid::new_v4().to_string();
        
        // Record related events
        for i in 0..3 {
            let event = FileEvent {
                event_type: "move".to_string(),
                path: temp_dir.path().join(format!("file{}.txt", i))
                    .to_string_lossy().to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                file_name: Some(format!("file{}.txt", i)),
                extension: Some(".txt".to_string()),
                source_path: None,
                destination_path: Some(temp_dir.path().join("organized")
                    .to_string_lossy().to_string()),
                is_user_action: true,
                operation_id: Some(operation_id.clone()),
            };
            
            watcher.record_event(event).await.unwrap();
        }
        
        // Get grouped operations
        let operations = watcher.get_recent_operations().await.unwrap();
        
        let grouped = operations.get(&operation_id);
        assert!(grouped.is_some());
        assert_eq!(grouped.unwrap().len(), 3);
    }

    // Error Handling Tests
    #[tokio::test]
    async fn test_watch_invalid_path() {
        let (watcher, _) = create_test_watcher().await;
        
        let result = watcher.start(vec![
            "".to_string() // Empty path
        ]).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_watch_file_instead_of_directory() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "content").unwrap();
        
        let result = watcher.start(vec![
            file_path.to_string_lossy().to_string()
        ]).await;
        
        // Should handle gracefully (some systems allow watching files)
        if result.is_err() {
            match result.unwrap_err() {
                AppError::InvalidPath { .. } => {},
                _ => panic!("Expected InvalidPath error"),
            }
        }
    }

    // Concurrent Operations Tests
    #[tokio::test]
    async fn test_concurrent_event_processing() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Create files concurrently
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let path = temp_dir.path().join(format!("concurrent_{}.txt", i));
                tokio::spawn(async move {
                    tokio::fs::write(path, format!("content {}", i)).await
                })
            })
            .collect();
        
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        // Give time for events to be processed
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Should have processed all events without issues
        assert!(watcher.is_watching().await);
    }

    #[tokio::test]
    async fn test_memory_pressure_with_many_events() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Generate many events
        for i in 0..100 {
            let file = temp_dir.path().join(format!("stress_{}.txt", i));
            tokio::fs::write(&file, format!("content {}", i)).await.unwrap();
            
            if i % 10 == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        
        // System should handle the load
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(watcher.is_watching().await);
        
        // Clean shutdown
        assert!(watcher.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_recursive_directory_watching() {
        let (watcher, temp_dir) = create_test_watcher().await;
        
        // Create nested directory structure
        let deep_dir = temp_dir.path().join("level1/level2/level3");
        std::fs::create_dir_all(&deep_dir).unwrap();
        
        let mut event_rx = watcher.subscribe_events().await;
        
        watcher.start(vec![
            temp_dir.path().to_string_lossy().to_string()
        ]).await.unwrap();
        
        // Create file in deep directory
        let deep_file = deep_dir.join("deep.txt");
        tokio::fs::write(&deep_file, "deep content").await.unwrap();
        
        // Should detect file in nested directory
        let event = timeout(Duration::from_secs(5), event_rx.recv())
            .await
            .expect("Timeout")
            .expect("No event");
        
        assert!(event.path.contains("deep.txt"));
        assert!(event.path.contains("level3"));
    }
}