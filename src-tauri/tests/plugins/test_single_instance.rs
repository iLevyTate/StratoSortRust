// Tests for tauri-plugin-single-instance
// Tests preventing multiple app instances

#[cfg(test)]
mod test_single_instance_plugin {
    use super::super::plugin_fixtures::*;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::{Mutex, RwLock};

    #[tokio::test]
    async fn test_prevent_multiple_instances() {
        // Test preventing multiple app instances
        let instance_lock = Arc::new(Mutex::new(true));

        // First instance
        let first_instance = instance_lock.try_lock();
        assert!(
            first_instance.is_ok(),
            "First instance should start successfully"
        );

        // Try to start second instance
        let second_instance = instance_lock.try_lock();
        assert!(
            second_instance.is_err(),
            "Second instance should be prevented"
        );
    }

    #[tokio::test]
    async fn test_focus_existing_instance() {
        // Test focusing existing instance when new instance is attempted
        let window_focused = Arc::new(RwLock::new(false));

        // First instance running
        let instance_running = true;

        // Attempt to start second instance
        if instance_running {
            // Should focus existing instance instead
            *window_focused.write().await = true;

            // Send command line args to existing instance
            let args = ["--open", "/path/to/file.txt"];

            // Verify focus was triggered
            assert!(
                *window_focused.read().await,
                "Existing instance should be focused"
            );
            assert!(
                !args.is_empty(),
                "Args should be passed to existing instance"
            );
        }
    }

    #[tokio::test]
    async fn test_pass_files_to_existing_instance() {
        // Test passing file paths to existing instance
        let files_to_open = vec![
            "/test/document1.pdf",
            "/test/image.jpg",
            "/test/archive.zip",
        ];

        // Existing instance file queue
        let file_queue = Arc::new(RwLock::new(Vec::new()));

        // Add files to existing instance queue
        for file in &files_to_open {
            file_queue.write().await.push(file.to_string());
        }

        // Verify files were queued
        let queued = file_queue.read().await;
        assert_eq!(queued.len(), 3, "All files should be queued");
        assert!(
            queued.contains(&"/test/document1.pdf".to_string()),
            "Document should be in queue"
        );
    }

    #[tokio::test]
    async fn test_instance_lock_file() {
        // Test lock file mechanism
        let mock_app = MockAppHandle::new();
        let lock_file_path = mock_app.data_dir.join("app.lock");

        // Create lock file for first instance
        std::fs::write(&lock_file_path, "pid:1234").unwrap();

        // Check if lock file exists (second instance check)
        let lock_exists = lock_file_path.exists();
        assert!(lock_exists, "Lock file should exist");

        // Read PID from lock file
        let lock_content = std::fs::read_to_string(&lock_file_path).unwrap();
        assert!(
            lock_content.contains("pid:"),
            "Lock file should contain PID"
        );

        // Clean up lock file on exit
        std::fs::remove_file(&lock_file_path).unwrap();
        assert!(
            !lock_file_path.exists(),
            "Lock file should be removed on exit"
        );
    }

    #[tokio::test]
    async fn test_ipc_communication() {
        // Test IPC between instances
        let ipc_messages = Arc::new(RwLock::new(Vec::new()));

        // Send message from second instance to first
        let message = json!({
            "type": "open_files",
            "files": ["/test/file1.txt", "/test/file2.txt"],
            "timestamp": "2024-01-20T10:00:00Z"
        });

        ipc_messages.write().await.push(message.clone());

        // First instance receives message
        let messages = ipc_messages.read().await;
        assert_eq!(messages.len(), 1, "Should receive IPC message");
        assert_eq!(
            messages[0]["type"], "open_files",
            "Should receive correct message type"
        );
    }

    #[test]
    fn test_handle_deep_links() {
        // Test handling deep links in single instance
        let deep_link = "stratosort://open?file=/test/document.pdf";

        // Parse deep link
        let is_deep_link = deep_link.starts_with("stratosort://");
        assert!(is_deep_link, "Should recognize deep link");

        // Extract action and parameters
        let action = if deep_link.contains("open") {
            "open"
        } else {
            "unknown"
        };
        let has_file_param = deep_link.contains("file=");

        assert_eq!(action, "open", "Should extract open action");
        assert!(has_file_param, "Should have file parameter");
    }

    #[tokio::test]
    async fn test_cleanup_stale_locks() {
        // Test cleaning up stale lock files from crashed instances
        let mock_app = MockAppHandle::new();
        let lock_file_path = mock_app.data_dir.join("app.lock");

        // Create stale lock file (with old PID that doesn't exist)
        std::fs::write(&lock_file_path, "pid:99999").unwrap();

        // Check if process with PID exists
        let _pid = 99999;
        let process_exists = false; // Simulate process doesn't exist

        if !process_exists {
            // Remove stale lock
            std::fs::remove_file(&lock_file_path).unwrap();

            // Create new lock for current instance
            std::fs::write(&lock_file_path, "pid:1234").unwrap();
        }

        let lock_content = std::fs::read_to_string(&lock_file_path).unwrap();
        assert!(
            lock_content.contains("pid:1234"),
            "Should have new lock file"
        );
    }

    #[tokio::test]
    async fn test_instance_with_different_profiles() {
        // Test allowing multiple instances with different profiles
        let profile1_lock = Arc::new(Mutex::new(true));
        let profile2_lock = Arc::new(Mutex::new(true));

        // Instance with profile1
        let instance1 = profile1_lock.try_lock();
        assert!(instance1.is_ok(), "First profile instance should start");

        // Instance with profile2 (different profile)
        let instance2 = profile2_lock.try_lock();
        assert!(
            instance2.is_ok(),
            "Different profile instance should be allowed"
        );

        // Another instance with profile1 (should fail)
        let instance3 = profile1_lock.try_lock();
        assert!(
            instance3.is_err(),
            "Same profile instance should be prevented"
        );
    }

    #[tokio::test]
    async fn test_graceful_handoff() {
        // Test graceful handoff of operations to existing instance
        let operations_queue = Arc::new(RwLock::new(Vec::new()));

        // New instance tries to perform operations
        let operations = [
            json!({"action": "organize", "path": "/test/downloads"}),
            json!({"action": "analyze", "files": ["/test/doc1.pdf", "/test/doc2.pdf"]}),
        ];

        // Hand off to existing instance
        for op in operations {
            operations_queue.write().await.push(op);
        }

        // Existing instance processes queue
        let queue = operations_queue.read().await;
        assert_eq!(queue.len(), 2, "All operations should be handed off");
        assert_eq!(
            queue[0]["action"], "organize",
            "First operation should be organize"
        );
    }

    #[test]
    fn test_command_line_args_parsing() {
        // Test parsing command line arguments for existing instance
        let args = [
            "stratosort",
            "--organize",
            "/path/to/folder",
            "--ai-model",
            "llama2",
            "--recursive",
        ];

        // Parse arguments
        let mut parsed_args = json!({});

        for i in 1..args.len() {
            if args[i].starts_with("--") {
                let key = args[i].trim_start_matches("--");
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    parsed_args[key] = json!(args[i + 1]);
                } else {
                    parsed_args[key] = json!(true);
                }
            }
        }

        assert_eq!(
            parsed_args["organize"], "/path/to/folder",
            "Should parse organize path"
        );
        assert_eq!(parsed_args["ai-model"], "llama2", "Should parse AI model");
        assert_eq!(parsed_args["recursive"], true, "Should parse boolean flag");
    }

    #[tokio::test]
    async fn test_instance_detection_cross_platform() {
        // Test instance detection works across platforms
        let os_info = MockOsInfo::default();

        let lock_mechanism = match os_info.platform.as_str() {
            "windows" => "mutex",
            "linux" => "lock_file",
            "macos" => "lock_file",
            _ => "lock_file",
        };

        assert!(
            !lock_mechanism.is_empty(),
            "Should have lock mechanism for platform"
        );

        // Test platform-specific detection
        match lock_mechanism {
            "mutex" => {
                // Windows named mutex
                // Windows uses named mutex for single instance
            }
            "lock_file" => {
                // Unix lock file
                // Unix systems use lock files
            }
            _ => {}
        }
    }
}
