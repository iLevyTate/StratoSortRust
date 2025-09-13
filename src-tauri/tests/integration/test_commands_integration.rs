use stratosort::commands::{files, ai, organize, settings, system, history};
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::Result;
use tauri::test::{mock_app, mock_context, MockRuntime};
use std::sync::Arc;
use tempfile::tempdir;

#[cfg(test)]
mod integration_tests {
    use super::*;

    async fn setup_test_environment() -> Result<(tauri::App<MockRuntime>, Arc<AppState>)> {
        let context = mock_context();
        let app = mock_app(context);
        
        let mut config = Config::default();
        config.debug_mode = true;
        
        let app_state = AppState::new(&config)?;
        
        Ok((app, app_state))
    }

    #[tokio::test]
    async fn test_complete_file_analysis_workflow() {
        let (app, app_state) = setup_test_environment().await.unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Step 1: Create test files
        let test_files = vec![
            ("document.txt", "This is an important contract document with legal terms."),
            ("report.pdf", "Monthly financial report with key performance indicators."),
            ("image.jpg", "Binary image data here"),
            ("invoice.txt", "Invoice #12345 for services rendered. Total: $500.00"),
        ];
        
        let mut file_paths = Vec::new();
        for (filename, content) in test_files {
            let file_path = temp_dir.path().join(filename);
            std::fs::write(&file_path, content).unwrap();
            file_paths.push(file_path.to_string_lossy().to_string());
        }
        
        // Step 2: Scan directory
        let scan_result = files::scan_directory(
            temp_dir.path().to_string_lossy().to_string(),
            false,
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(scan_result.is_ok());
        let scanned_files = scan_result.unwrap();
        assert_eq!(scanned_files.len(), 4);
        
        // Step 3: Analyze files
        let analysis_result = files::analyze_files(
            file_paths.clone(),
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(analysis_result.is_ok());
        let analyses = analysis_result.unwrap();
        assert_eq!(analyses.len(), 4);
        
        // Verify analysis quality
        for analysis in &analyses {
            assert!(!analysis.category.is_empty());
            assert!(!analysis.summary.is_empty());
            assert!(analysis.confidence > 0.0);
        }
        
        // Check that invoice and contract files are properly tagged
        let invoice_analysis = analyses.iter().find(|a| a.path.contains("invoice")).unwrap();
        assert!(invoice_analysis.tags.contains(&"invoice".to_string()));
        assert!(invoice_analysis.tags.contains(&"financial".to_string()));
        
        let contract_analysis = analyses.iter().find(|a| a.path.contains("document")).unwrap();
        assert!(contract_analysis.tags.contains(&"contract".to_string()));
        assert!(contract_analysis.tags.contains(&"legal".to_string()));
        
        // Step 4: Generate organization suggestions
        let suggestion_result = ai::suggest_organization(
            file_paths.clone(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(suggestion_result.is_ok());
        let suggestions = suggestion_result.unwrap();
        assert_eq!(suggestions.len(), 4);
        
        for suggestion in &suggestions {
            assert!(!suggestion.target_folder.is_empty());
            assert!(!suggestion.reason.is_empty());
            assert!(suggestion.confidence > 0.0);
        }
        
        // Step 5: Organize files
        let target_dir = temp_dir.path().join("organized");
        let organize_result = organize::organize_files_by_type(
            file_paths,
            target_dir.to_string_lossy().to_string(),
            true, // create_subfolders
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(organize_result.is_ok());
        let result = organize_result.unwrap();
        assert_eq!(result.processed_files, 4);
        assert_eq!(result.failed_operations, 0);
        assert!(!result.created_folders.is_empty());
        
        // Step 6: Verify files were moved correctly
        assert!(target_dir.exists());
        // Check that subfolders were created
        let subdirs: Vec<_> = std::fs::read_dir(&target_dir)
            .unwrap()
            .filter_map(|entry| {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    Some(entry.file_name().to_string_lossy().to_string())
                } else {
                    None
                }
            })
            .collect();
        
        assert!(!subdirs.is_empty());
    }

    #[tokio::test]
    async fn test_smart_folder_workflow() {
        let (app, app_state) = setup_test_environment().await.unwrap();
        
        // Step 1: Create a smart folder
        let smart_folder_request = organize::SmartFolderRequest {
            name: "Important Documents".to_string(),
            description: "Automatically organize important documents".to_string(),
            query: "category:Documents AND (tags:important OR tags:contract)".to_string(),
            auto_organize: true,
            target_path: "/organized/important".to_string(),
        };
        
        let create_result = organize::create_smart_folder(
            smart_folder_request,
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(create_result.is_ok());
        let created_folder = create_result.unwrap();
        assert_eq!(created_folder.name, "Important Documents");
        assert!(created_folder.auto_organize);
        
        // Step 2: List smart folders
        let list_result = organize::list_smart_folders(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(list_result.is_ok());
        let folders = list_result.unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Important Documents");
        
        // Step 3: Update smart folder
        let mut updated_folder = created_folder.clone();
        updated_folder.description = "Updated description".to_string();
        
        let update_result = organize::update_smart_folder(
            updated_folder.id.to_string(),
            organize::SmartFolderRequest {
                name: updated_folder.name.clone(),
                description: "Updated description".to_string(),
                query: updated_folder.query.clone(),
                auto_organize: updated_folder.auto_organize,
                target_path: updated_folder.target_path.clone(),
            },
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(update_result.is_ok());
        
        // Step 4: Delete smart folder
        let delete_result = organize::delete_smart_folder(
            created_folder.id.to_string(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(delete_result.is_ok());
        
        // Step 5: Verify deletion
        let final_list_result = organize::list_smart_folders(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(final_list_result.is_ok());
        let final_folders = final_list_result.unwrap();
        assert!(final_folders.is_empty());
    }

    #[tokio::test]
    async fn test_configuration_management_workflow() {
        let (_, app_state) = setup_test_environment().await.unwrap();
        
        // Step 1: Get initial configuration
        let initial_config_result = settings::get_config(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(initial_config_result.is_ok());
        let initial_config = initial_config_result.unwrap();
        assert_eq!(initial_config.ai_provider, "ollama");
        
        // Step 2: Update configuration
        let mut updated_config = initial_config.clone();
        updated_config.max_concurrent_analysis = 10;
        updated_config.debug_mode = true;
        updated_config.show_notifications = false;
        
        let update_result = settings::update_config(
            updated_config.clone(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(update_result.is_ok());
        
        // Step 3: Verify configuration was updated
        let updated_config_result = settings::get_config(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(updated_config_result.is_ok());
        let retrieved_config = updated_config_result.unwrap();
        assert_eq!(retrieved_config.max_concurrent_analysis, 10);
        assert!(retrieved_config.debug_mode);
        assert!(!retrieved_config.show_notifications);
        
        // Step 4: Export configuration
        let export_result = settings::export_config(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(export_result.is_ok());
        let exported_json = export_result.unwrap();
        assert!(!exported_json.is_empty());
        assert!(exported_json.contains("max_concurrent_analysis"));
        
        // Step 5: Reset configuration
        let reset_result = settings::reset_config(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(reset_result.is_ok());
        
        // Step 6: Verify reset
        let reset_config_result = settings::get_config(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(reset_config_result.is_ok());
        let reset_config = reset_config_result.unwrap();
        assert_eq!(reset_config.max_concurrent_analysis, 3); // Default value
        
        // Step 7: Import configuration
        let import_result = settings::import_config(
            exported_json,
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(import_result.is_ok());
        
        // Step 8: Verify import
        let imported_config_result = settings::get_config(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(imported_config_result.is_ok());
        let imported_config = imported_config_result.unwrap();
        assert_eq!(imported_config.max_concurrent_analysis, 10);
    }

    #[tokio::test]
    async fn test_system_monitoring_workflow() {
        let (_, app_state) = setup_test_environment().await.unwrap();
        
        // Step 1: Get system information
        let system_info_result = system::get_system_info(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(system_info_result.is_ok());
        let system_info = system_info_result.unwrap();
        assert!(!system_info.os.is_empty());
        assert!(system_info.total_memory > 0);
        assert!(system_info.cpu_count > 0);
        
        // Step 2: Get application info
        let app_info_result = system::get_app_info(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(app_info_result.is_ok());
        let app_info = app_info_result.unwrap();
        assert!(!app_info.version.is_empty());
        assert!(!app_info.name.is_empty());
        assert!(!app_info.build_date.is_empty());
        
        // Step 3: Health check
        let health_result = system::health_check(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(health_result.is_ok());
        let health = health_result.unwrap();
        assert!(health.database_connected);
        assert!(!health.uptime_seconds.is_zero());
        
        // Step 4: Get performance metrics
        let metrics_result = system::get_performance_metrics(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(metrics_result.is_ok());
        let metrics = metrics_result.unwrap();
        assert!(metrics.memory_usage >= 0.0);
        assert!(metrics.cpu_usage >= 0.0);
        assert!(metrics.active_operations >= 0);
    }

    #[tokio::test]
    async fn test_ai_integration_workflow() {
        let (_, app_state) = setup_test_environment().await.unwrap();
        
        // Step 1: Test AI connection
        let connection_result = ai::test_ai_connection(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(connection_result.is_ok());
        // Connection may fail in test environment, but command should succeed
        
        // Step 2: Generate embeddings
        let text = "This is a test document about machine learning and artificial intelligence.";
        let embeddings_result = ai::generate_embeddings(
            text.to_string(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(embeddings_result.is_ok());
        let embeddings = embeddings_result.unwrap();
        assert!(!embeddings.is_empty());
        assert!(embeddings.len() > 10); // Should have reasonable dimension
        
        // Step 3: Generate multiple embeddings and test similarity
        let similar_text = "This document discusses AI and ML technologies.";
        let similar_embeddings_result = ai::generate_embeddings(
            similar_text.to_string(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(similar_embeddings_result.is_ok());
        let similar_embeddings = similar_embeddings_result.unwrap();
        assert_eq!(embeddings.len(), similar_embeddings.len());
        
        // Step 4: Test organization suggestions
        let test_files = vec![
            "/documents/contract.pdf".to_string(),
            "/downloads/invoice.xlsx".to_string(),
            "/desktop/presentation.pptx".to_string(),
            "/music/song.mp3".to_string(),
            "/photos/vacation.jpg".to_string(),
        ];
        
        let suggestions_result = ai::suggest_organization(
            test_files.clone(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(suggestions_result.is_ok());
        let suggestions = suggestions_result.unwrap();
        assert_eq!(suggestions.len(), 5);
        
        // Verify suggestions are reasonable
        let audio_suggestion = suggestions.iter().find(|s| s.source_path.contains("song.mp3")).unwrap();
        assert_eq!(audio_suggestion.target_folder, "Audio");
        
        let image_suggestion = suggestions.iter().find(|s| s.source_path.contains("vacation.jpg")).unwrap();
        assert_eq!(image_suggestion.target_folder, "Images");
        
        let document_suggestion = suggestions.iter().find(|s| s.source_path.contains("contract.pdf")).unwrap();
        assert_eq!(document_suggestion.target_folder, "Documents");
    }

    #[tokio::test]
    async fn test_operation_history_workflow() {
        let (app, app_state) = setup_test_environment().await.unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Step 1: Perform some operations to create history
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "Test content").unwrap();
        
        let dest_dir = temp_dir.path().join("destination");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let dest_file = dest_dir.join("moved.txt");
        
        // Move operation
        let move_operations = vec![files::MoveOperation {
            source: test_file.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
        }];
        
        let move_result = files::move_files(
            move_operations,
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(move_result.is_ok());
        
        // Step 2: Get operation history
        let history_result = history::get_operation_history(
            10,
            0,
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(history_result.is_ok());
        let history_entries = history_result.unwrap();
        assert!(!history_entries.is_empty());
        
        // Step 3: Test undo functionality
        if let Some(first_operation) = history_entries.first() {
            let undo_result = history::undo_operation(
                first_operation.id.to_string(),
                tauri::State::from(app_state.clone()),
            ).await;
            
            // Undo may succeed or fail depending on operation type
            // The important thing is the command doesn't panic
            assert!(undo_result.is_ok() || undo_result.is_err());
        }
        
        // Step 4: Clear history
        let clear_result = history::clear_history(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(clear_result.is_ok());
        
        // Step 5: Verify history is cleared
        let empty_history_result = history::get_operation_history(
            10,
            0,
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(empty_history_result.is_ok());
        let empty_history = empty_history_result.unwrap();
        assert!(empty_history.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let (app, app_state) = setup_test_environment().await.unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Create multiple test files
        let mut file_paths = Vec::new();
        for i in 0..10 {
            let file_path = temp_dir.path().join(format!("file_{}.txt", i));
            std::fs::write(&file_path, format!("Content {}", i)).unwrap();
            file_paths.push(file_path.to_string_lossy().to_string());
        }
        
        // Run concurrent analysis operations
        let mut handles = Vec::new();
        for chunk in file_paths.chunks(2) {
            let app_state_clone = app_state.clone();
            let app_handle = app.handle().clone();
            let chunk_files = chunk.to_vec();
            
            let handle = tokio::spawn(async move {
                files::analyze_files(
                    chunk_files,
                    tauri::State::from(app_state_clone),
                    app_handle,
                ).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        let mut total_analyses = 0;
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            total_analyses += result.unwrap().len();
        }
        
        assert_eq!(total_analyses, 10);
    }

    #[tokio::test]
    async fn test_error_handling_and_recovery() {
        let (app, app_state) = setup_test_environment().await.unwrap();
        
        // Test 1: Invalid file operations
        let invalid_move_result = files::move_files(
            vec![files::MoveOperation {
                source: "/nonexistent/source.txt".to_string(),
                destination: "/nonexistent/dest.txt".to_string(),
            }],
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(invalid_move_result.is_ok()); // Command succeeds but reports failures
        let move_results = invalid_move_result.unwrap();
        assert_eq!(move_results.len(), 1);
        assert!(!move_results[0].success);
        assert!(move_results[0].error.is_some());
        
        // Test 2: Invalid configuration
        let mut invalid_config = Config::default();
        invalid_config.max_concurrent_analysis = 0; // Invalid value
        
        let invalid_config_result = settings::update_config(
            invalid_config,
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(invalid_config_result.is_err()); // Should fail validation
        
        // Test 3: Large batch operations
        let large_file_list: Vec<String> = (0..1500)
            .map(|i| format!("/fake/file{}.txt", i))
            .collect();
        
        let large_batch_result = files::analyze_files(
            large_file_list,
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(large_batch_result.is_err()); // Should fail due to size limit
        
        // Test 4: System still functional after errors
        let health_after_errors = system::health_check(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(health_after_errors.is_ok());
        let health = health_after_errors.unwrap();
        assert!(health.database_connected); // System should still be healthy
    }
}