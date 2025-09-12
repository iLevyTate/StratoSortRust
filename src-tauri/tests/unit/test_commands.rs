use stratosort::commands::{files, ai, organize, settings, system, history};
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::{Result, AppError};
use tauri::test::{mock_app, mock_context};
use std::sync::Arc;
use tempfile::tempdir;

#[cfg(test)]
mod command_tests {
    use super::*;

    fn create_mock_app_state() -> Result<Arc<AppState>> {
        let config = Config::default();
        AppState::new(&config)
    }

    fn create_mock_app() -> tauri::App<tauri::Wry> {
        let context = mock_context();
        mock_app(context)
    }

    // File Commands Tests
    #[tokio::test]
    async fn test_scan_directory_command() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        
        // Create some test files
        std::fs::write(temp_dir.path().join("test1.txt"), "content1").unwrap();
        std::fs::write(temp_dir.path().join("test2.pdf"), "content2").unwrap();
        
        let result = files::scan_directory(
            temp_dir.path().to_string_lossy().to_string(),
            false,
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_ok());
        let files = result.unwrap();
        assert!(files.len() >= 2);
    }

    #[tokio::test]
    async fn test_scan_directory_nonexistent() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        
        let result = files::scan_directory(
            "/nonexistent/path".to_string(),
            false,
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::FileNotFound { .. } => (),
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_file_content_command() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        
        let test_content = "This is test file content";
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, test_content).unwrap();
        
        let result = files::get_file_content(
            test_file.to_string_lossy().to_string(),
            Some("user123".to_string()),
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content, test_content);
    }

    #[tokio::test]
    async fn test_get_file_preview_command() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        
        let test_content = "x".repeat(5000); // 5KB content
        let test_file = temp_dir.path().join("large.txt");
        std::fs::write(&test_file, &test_content).unwrap();
        
        let result = files::get_file_preview(
            test_file.to_string_lossy().to_string(),
            1000, // 1KB preview limit
            app.handle().clone(),
        ).await;
        
        assert!(result.is_ok());
        let preview = result.unwrap();
        assert!(preview.truncated);
        assert!(preview.content.len() <= 1000);
        assert_eq!(preview.total_size as usize, test_content.len());
    }

    #[tokio::test]
    async fn test_analyze_files_command() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        
        // Create test files
        let file1 = temp_dir.path().join("doc1.txt");
        let file2 = temp_dir.path().join("doc2.txt");
        std::fs::write(&file1, "This is a test document").unwrap();
        std::fs::write(&file2, "Another test document").unwrap();
        
        let paths = vec![
            file1.to_string_lossy().to_string(),
            file2.to_string_lossy().to_string(),
        ];
        
        let result = files::analyze_files(
            paths,
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_ok());
        let analyses = result.unwrap();
        assert_eq!(analyses.len(), 2);
    }

    #[tokio::test]
    async fn test_analyze_files_empty_list() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        
        let result = files::analyze_files(
            vec![], // Empty list
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidPath { message } => {
                assert!(message.contains("No paths provided"));
            },
            _ => panic!("Expected InvalidPath error"),
        }
    }

    #[tokio::test]
    async fn test_move_files_command() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        
        // Create source file
        let source_file = temp_dir.path().join("source.txt");
        std::fs::write(&source_file, "content").unwrap();
        
        let dest_dir = temp_dir.path().join("destination");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let dest_file = dest_dir.join("moved.txt");
        
        let operations = vec![files::MoveOperation {
            source: source_file.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
        }];
        
        let result = files::move_files(
            operations,
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[tokio::test]
    async fn test_get_recent_files_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = files::get_recent_files(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let recent_files = result.unwrap();
        // Should return empty list initially
        assert!(recent_files.is_empty());
    }

    // Settings Commands Tests
    #[tokio::test]
    async fn test_get_config_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = settings::get_config(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.ai_provider, "ollama");
    }

    #[tokio::test]
    async fn test_update_config_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let mut new_config = Config::default();
        new_config.max_concurrent_analysis = 5;
        
        let result = settings::update_config(
            new_config,
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_reset_config_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = settings::reset_config(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_export_config_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = settings::export_config(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let config_json = result.unwrap();
        assert!(!config_json.is_empty());
        assert!(config_json.contains("ai_provider"));
    }

    #[tokio::test]
    async fn test_import_config_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let config = Config::default();
        let config_json = config.export();
        
        let result = settings::import_config(
            config_json,
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
    }

    // System Commands Tests
    #[tokio::test]
    async fn test_get_system_info_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = system::get_system_info(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let system_info = result.unwrap();
        assert!(!system_info.os.is_empty());
        assert!(system_info.total_memory > 0);
    }

    #[tokio::test]
    async fn test_get_app_info_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = system::get_app_info(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let app_info = result.unwrap();
        assert!(!app_info.version.is_empty());
        assert!(!app_info.name.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = system::health_check(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let health = result.unwrap();
        assert!(health.database_connected);
        // AI service might not be connected in tests
    }

    #[tokio::test]
    async fn test_get_performance_metrics_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = system::get_performance_metrics(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert!(metrics.memory_usage >= 0.0);
        assert!(metrics.cpu_usage >= 0.0);
    }

    // AI Commands Tests
    #[tokio::test]
    async fn test_test_ai_connection_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = ai::test_ai_connection(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        // Connection might fail in test environment, but command should not error
    }

    #[tokio::test]
    async fn test_generate_embeddings_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = ai::generate_embeddings(
            "This is a test text for embedding generation".to_string(),
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert!(!embeddings.is_empty());
    }

    #[tokio::test]
    async fn test_suggest_organization_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let files = vec![
            "/test/document.pdf".to_string(),
            "/test/image.jpg".to_string(),
            "/test/video.mp4".to_string(),
        ];
        
        let result = ai::suggest_organization(
            files,
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let suggestions = result.unwrap();
        assert_eq!(suggestions.len(), 3);
    }

    // History Commands Tests
    #[tokio::test]
    async fn test_get_operation_history_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = history::get_operation_history(
            10,
            0,
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let history = result.unwrap();
        // Should return empty list initially
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_undo_operation_command() {
        let app_state = create_mock_app_state().unwrap();
        let temp_dir = tempdir().unwrap();
        
        // This would typically require a previous operation to exist
        let result = history::undo_operation(
            uuid::Uuid::new_v4().to_string(),
            tauri::State::from(app_state)
        ).await;
        
        // Should handle non-existent operation gracefully
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_clear_history_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = history::clear_history(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
    }

    // Organize Commands Tests
    #[tokio::test]
    async fn test_organize_files_by_type_command() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        
        // Create test files
        let file1 = temp_dir.path().join("test1.txt");
        let file2 = temp_dir.path().join("test2.pdf");
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();
        
        let files = vec![
            file1.to_string_lossy().to_string(),
            file2.to_string_lossy().to_string(),
        ];
        
        let target_dir = temp_dir.path().join("organized");
        
        let result = organize::organize_files_by_type(
            files,
            target_dir.to_string_lossy().to_string(),
            true,
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_ok());
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 2);
    }

    #[tokio::test]
    async fn test_create_smart_folder_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let smart_folder = organize::SmartFolderRequest {
            name: "Test Smart Folder".to_string(),
            description: "A test smart folder".to_string(),
            query: "category:document".to_string(),
            auto_organize: true,
            target_path: "/test/smart_folder".to_string(),
        };
        
        let result = organize::create_smart_folder(
            smart_folder,
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let created_folder = result.unwrap();
        assert_eq!(created_folder.name, "Test Smart Folder");
    }

    #[tokio::test]
    async fn test_list_smart_folders_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = organize::list_smart_folders(
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let folders = result.unwrap();
        // Should return empty list initially
        assert!(folders.is_empty());
    }

    // Cancel Operation Tests
    #[tokio::test]
    async fn test_cancel_operation_command() {
        let app_state = create_mock_app_state().unwrap();
        
        let operation_id = uuid::Uuid::new_v4().to_string();
        
        let result = stratosort::commands::cancel_operation(
            operation_id,
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_ok());
        let cancelled = result.unwrap();
        // Should return false since operation doesn't exist
        assert!(!cancelled);
    }

    #[tokio::test]
    async fn test_cancel_operation_invalid_uuid() {
        let app_state = create_mock_app_state().unwrap();
        
        let result = stratosort::commands::cancel_operation(
            "invalid-uuid".to_string(),
            tauri::State::from(app_state)
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidInput { message } => {
                assert!(message.contains("Invalid UUID"));
            },
            _ => panic!("Expected InvalidInput error"),
        }
    }

    // Error Handling Tests
    #[tokio::test]
    async fn test_command_with_invalid_path() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        
        let result = files::get_file_content(
            "".to_string(), // Empty path
            None,
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_command_with_large_file_list() {
        let app_state = create_mock_app_state().unwrap();
        let app = create_mock_app();
        
        // Create a list with too many files
        let large_file_list: Vec<String> = (0..2000)
            .map(|i| format!("/fake/file{}.txt", i))
            .collect();
        
        let result = files::analyze_files(
            large_file_list,
            tauri::State::from(app_state),
            app.handle().clone(),
        ).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::SecurityError { message } => {
                assert!(message.contains("Too many files"));
            },
            _ => panic!("Expected SecurityError"),
        }
    }
}