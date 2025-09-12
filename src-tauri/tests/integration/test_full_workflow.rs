use stratosort::commands::{files, ai, organize, settings, system};
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::Result;
use tauri::test::{mock_app, mock_context, MockRuntime};
use std::sync::Arc;
use tempfile::tempdir;

#[cfg(test)]
mod full_workflow_tests {
    use super::*;

    async fn setup_complete_test_environment() -> Result<(tauri::App<MockRuntime>, Arc<AppState>)> {
        let context = mock_context();
        let app = mock_app(context);
        
        let mut config = Config::default();
        config.debug_mode = true;
        config.max_concurrent_analysis = 2;
        config.auto_analyze_on_add = true;
        
        let app_state = AppState::new(&config)?;
        
        Ok((app, app_state))
    }

    #[tokio::test]
    async fn test_complete_file_organization_workflow() {
        let (app, app_state) = setup_complete_test_environment().await.unwrap();
        let temp_dir = tempdir().unwrap();
        
        // === SETUP PHASE ===
        println!("Setting up test environment...");
        
        // Create a realistic file structure
        let documents = vec![
            ("contracts/service_agreement.pdf", "SERVICE AGREEMENT\nThis contract outlines the terms of service..."),
            ("contracts/nda.docx", "NON-DISCLOSURE AGREEMENT\nConfidential information protection..."),
            ("invoices/invoice_001.pdf", "INVOICE #001\nBill to: Company Inc.\nAmount: $5,000.00"),
            ("invoices/invoice_002.txt", "Invoice 002\nServices rendered: $2,500.00"),
            ("reports/monthly_report.xlsx", "Monthly Performance Report\nQ1 2024 Results"),
            ("reports/annual_summary.pdf", "Annual Report 2023\nCompany performance summary"),
            ("misc/readme.txt", "Project documentation and setup instructions"),
            ("misc/presentation.pptx", "Quarterly business review presentation"),
        ];
        
        let media_files = vec![
            ("photos/vacation_2023.jpg", "JPEG binary data..."),
            ("photos/family_reunion.png", "PNG binary data..."),
            ("videos/conference_recording.mp4", "MP4 video binary data..."),
            ("audio/podcast_episode.mp3", "MP3 audio binary data..."),
        ];
        
        let mut all_file_paths = Vec::new();
        
        // Create document files
        for (path, content) in documents {
            let full_path = temp_dir.path().join(path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&full_path, content).unwrap();
            all_file_paths.push(full_path.to_string_lossy().to_string());
        }
        
        // Create media files
        for (path, content) in media_files {
            let full_path = temp_dir.path().join(path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&full_path, content).unwrap();
            all_file_paths.push(full_path.to_string_lossy().to_string());
        }
        
        println!("Created {} test files", all_file_paths.len());
        
        // === DISCOVERY PHASE ===
        println!("Starting file discovery...");
        
        // Scan the entire directory structure
        let scan_result = files::scan_directory(
            temp_dir.path().to_string_lossy().to_string(),
            true, // recursive
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(scan_result.is_ok());
        let discovered_files = scan_result.unwrap();
        assert!(discovered_files.len() >= 12); // All created files + possible directories
        
        println!("Discovered {} files and directories", discovered_files.len());
        
        // === ANALYSIS PHASE ===
        println!("Starting file analysis...");
        
        // Analyze all files in batches to simulate realistic usage
        let batch_size = 4;
        let mut all_analyses = Vec::new();
        
        for batch in all_file_paths.chunks(batch_size) {
            let batch_result = files::analyze_files(
                batch.to_vec(),
                tauri::State::from(app_state.clone()),
                app.handle().clone(),
            ).await;
            
            assert!(batch_result.is_ok());
            let mut batch_analyses = batch_result.unwrap();
            all_analyses.append(&mut batch_analyses);
            
            // Small delay to simulate realistic user behavior
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        assert_eq!(all_analyses.len(), all_file_paths.len());
        println!("Analyzed {} files", all_analyses.len());
        
        // Verify analysis quality
        let contract_analysis = all_analyses.iter()
            .find(|a| a.path.contains("service_agreement"))
            .expect("Should find contract analysis");
        assert!(contract_analysis.tags.contains(&"contract".to_string()));
        assert!(contract_analysis.tags.contains(&"legal".to_string()));
        
        let invoice_analysis = all_analyses.iter()
            .find(|a| a.path.contains("invoice_001"))
            .expect("Should find invoice analysis");
        assert!(invoice_analysis.tags.contains(&"invoice".to_string()));
        assert!(invoice_analysis.tags.contains(&"financial".to_string()));
        
        // === AI-POWERED ORGANIZATION SUGGESTIONS ===
        println!("Generating organization suggestions...");
        
        let suggestion_result = ai::suggest_organization(
            all_file_paths.clone(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(suggestion_result.is_ok());
        let suggestions = suggestion_result.unwrap();
        assert_eq!(suggestions.len(), all_file_paths.len());
        
        // Verify suggestions make sense
        let media_suggestions: Vec<_> = suggestions.iter()
            .filter(|s| s.target_folder == "Images" || s.target_folder == "Videos" || s.target_folder == "Audio")
            .collect();
        assert!(media_suggestions.len() >= 4); // Should categorize media files correctly
        
        let document_suggestions: Vec<_> = suggestions.iter()
            .filter(|s| s.target_folder == "Documents")
            .collect();
        assert!(document_suggestions.len() >= 6); // Should categorize document files correctly
        
        println!("Generated {} organization suggestions", suggestions.len());
        
        // === SMART FOLDER CREATION ===
        println!("Creating smart folders...");
        
        // Create smart folders for different categories
        let smart_folders = vec![
            ("Financial Documents", "category:Documents AND (tags:invoice OR tags:financial)", "/organized/financial"),
            ("Legal Documents", "category:Documents AND (tags:contract OR tags:legal)", "/organized/legal"),
            ("Media Files", "category:Images OR category:Videos OR category:Audio", "/organized/media"),
            ("Reports", "category:Documents AND tags:report", "/organized/reports"),
        ];
        
        let mut created_folder_ids = Vec::new();
        
        for (name, query, target_path) in smart_folders {
            let folder_request = organize::SmartFolderRequest {
                name: name.to_string(),
                description: format!("Auto-organize {}", name.to_lowercase()),
                query: query.to_string(),
                auto_organize: true,
                target_path: target_path.to_string(),
            };
            
            let create_result = organize::create_smart_folder(
                folder_request,
                tauri::State::from(app_state.clone()),
            ).await;
            
            assert!(create_result.is_ok());
            let created_folder = create_result.unwrap();
            created_folder_ids.push(created_folder.id);
            println!("Created smart folder: {}", created_folder.name);
        }
        
        // Verify smart folders were created
        let list_result = organize::list_smart_folders(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(list_result.is_ok());
        let folders = list_result.unwrap();
        assert_eq!(folders.len(), 4);
        
        // === FILE ORGANIZATION EXECUTION ===
        println!("Executing file organization...");
        
        let organized_dir = temp_dir.path().join("final_organization");
        
        let organize_result = organize::organize_files_by_type(
            all_file_paths.clone(),
            organized_dir.to_string_lossy().to_string(),
            true, // create_subfolders
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(organize_result.is_ok());
        let org_result = organize_result.unwrap();
        assert_eq!(org_result.processed_files, all_file_paths.len());
        assert_eq!(org_result.failed_operations, 0);
        assert!(!org_result.created_folders.is_empty());
        
        println!("Organized {} files into {} folders", 
                org_result.processed_files, 
                org_result.created_folders.len());
        
        // === VERIFICATION PHASE ===
        println!("Verifying organization results...");
        
        // Verify the organized directory structure
        assert!(organized_dir.exists());
        
        // Check that appropriate subfolders were created
        let created_folders: Vec<_> = std::fs::read_dir(&organized_dir)
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
        
        assert!(!created_folders.is_empty());
        
        // Should have folders for different file types
        let expected_categories = vec!["Documents", "Images", "Videos", "Audio"];
        for category in expected_categories {
            if created_folders.iter().any(|f| f.contains(category)) {
                println!("✓ Found {} folder", category);
            }
        }
        
        // === SYSTEM HEALTH CHECK ===
        println!("Performing final system health check...");
        
        let health_result = system::health_check(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(health_result.is_ok());
        let health = health_result.unwrap();
        assert!(health.database_connected);
        
        let metrics_result = system::get_performance_metrics(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(metrics_result.is_ok());
        let metrics = metrics_result.unwrap();
        assert!(metrics.memory_usage > 0.0);
        
        // === CLEANUP ===
        println!("Cleaning up test resources...");
        
        // Delete created smart folders
        for folder_id in created_folder_ids {
            let delete_result = organize::delete_smart_folder(
                folder_id.to_string(),
                tauri::State::from(app_state.clone()),
            ).await;
            assert!(delete_result.is_ok());
        }
        
        // Verify cleanup
        let final_list_result = organize::list_smart_folders(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(final_list_result.is_ok());
        let final_folders = final_list_result.unwrap();
        assert!(final_folders.is_empty());
        
        println!("✓ Complete workflow test passed successfully!");
    }

    #[tokio::test]
    async fn test_real_world_scenario_mixed_content() {
        let (app, app_state) = setup_complete_test_environment().await.unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Simulate a typical user's Downloads folder
        let mixed_files = vec![
            ("Resume_John_Doe.pdf", "RESUME\nJohn Doe\nSoftware Engineer\nExperience: 5 years..."),
            ("invoice_electricity_march.pdf", "ELECTRICITY BILL\nAccount: 123456\nAmount Due: $89.50"),
            ("photo_birthday_party.jpg", "JPEG image data"),
            ("video_tutorial_rust.mp4", "MP4 video data"),
            ("contract_freelance_work.docx", "FREELANCE CONTRACT\nProject scope and deliverables"),
            ("receipt_grocery_shopping.png", "PNG receipt image"),
            ("backup_documents.zip", "ZIP archive data"),
            ("meeting_notes_march.txt", "Meeting Notes - March 15\n- Discuss project timeline\n- Review requirements"),
            ("presentation_quarterly_review.pptx", "PowerPoint presentation data"),
            ("song_favorite_music.mp3", "MP3 audio data"),
            ("tax_documents_2023.pdf", "TAX RETURN 2023\nPersonal income tax documents"),
            ("screenshot_bug_report.png", "PNG screenshot data"),
        ];
        
        let mut file_paths = Vec::new();
        
        // Create all files
        for (filename, content) in &mixed_files {
            let file_path = temp_dir.path().join(filename);
            std::fs::write(&file_path, content).unwrap();
            file_paths.push(file_path.to_string_lossy().to_string());
        }
        
        println!("Created mixed content scenario with {} files", file_paths.len());
        
        // === ANALYSIS WITH REALISTIC BATCHING ===
        let batch_results = files::analyze_files(
            file_paths.clone(),
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(batch_results.is_ok());
        let analyses = batch_results.unwrap();
        assert_eq!(analyses.len(), mixed_files.len());
        
        // Verify specific categorizations
        let financial_files = analyses.iter()
            .filter(|a| a.tags.contains(&"invoice".to_string()) || 
                        a.tags.contains(&"financial".to_string()) ||
                        a.path.contains("tax"))
            .count();
        assert!(financial_files >= 2); // Should find invoice and tax docs
        
        let legal_files = analyses.iter()
            .filter(|a| a.tags.contains(&"contract".to_string()) || 
                        a.tags.contains(&"legal".to_string()))
            .count();
        assert!(legal_files >= 1); // Should find contract
        
        // === ORGANIZATION WITH CONFLICT RESOLUTION ===
        let target_dir = temp_dir.path().join("organized_mixed");
        
        // First organization attempt
        let org_result1 = organize::organize_files_by_type(
            file_paths.clone(),
            target_dir.to_string_lossy().to_string(),
            true,
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(org_result1.is_ok());
        let result1 = org_result1.unwrap();
        assert_eq!(result1.processed_files, file_paths.len());
        
        // Verify organization structure
        assert!(target_dir.exists());
        
        // Check that files were properly categorized
        let documents_dir = target_dir.join("Documents");
        let images_dir = target_dir.join("Images");
        let videos_dir = target_dir.join("Videos");
        let audio_dir = target_dir.join("Audio");
        
        // At least some of these directories should exist
        let created_dirs = vec![&documents_dir, &images_dir, &videos_dir, &audio_dir]
            .into_iter()
            .filter(|dir| dir.exists())
            .count();
        
        assert!(created_dirs > 0, "Should create at least one category directory");
        
        println!("✓ Mixed content scenario completed successfully");
    }

    #[tokio::test]
    async fn test_large_scale_operation() {
        let (app, app_state) = setup_complete_test_environment().await.unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Create a larger number of files to test scalability
        let file_count = 50;
        let mut file_paths = Vec::new();
        
        // Create various file types in bulk
        for i in 0..file_count {
            let file_type = match i % 6 {
                0 => ("txt", "Text document content"),
                1 => ("pdf", "PDF document content"),
                2 => ("jpg", "JPEG image data"),
                3 => ("mp4", "MP4 video data"),
                4 => ("mp3", "MP3 audio data"),
                _ => ("docx", "Word document content"),
            };
            
            let filename = format!("file_{:03}.{}", i, file_type.0);
            let file_path = temp_dir.path().join(&filename);
            std::fs::write(&file_path, format!("{} - File number {}", file_type.1, i)).unwrap();
            file_paths.push(file_path.to_string_lossy().to_string());
        }
        
        println!("Created {} files for large scale test", file_count);
        
        // Process in realistic batches
        let batch_size = 10;
        let mut total_processed = 0;
        
        for (batch_num, batch) in file_paths.chunks(batch_size).enumerate() {
            println!("Processing batch {} of {}", batch_num + 1, 
                    (file_count + batch_size - 1) / batch_size);
            
            let batch_result = files::analyze_files(
                batch.to_vec(),
                tauri::State::from(app_state.clone()),
                app.handle().clone(),
            ).await;
            
            assert!(batch_result.is_ok());
            let analyses = batch_result.unwrap();
            total_processed += analyses.len();
            
            // Small delay between batches
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        
        assert_eq!(total_processed, file_count);
        
        // Organize all files
        let target_dir = temp_dir.path().join("large_scale_organized");
        
        let org_result = organize::organize_files_by_type(
            file_paths,
            target_dir.to_string_lossy().to_string(),
            true,
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(org_result.is_ok());
        let result = org_result.unwrap();
        assert_eq!(result.processed_files, file_count);
        
        // Verify system performance after large operation
        let metrics = system::get_performance_metrics(
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(metrics.is_ok());
        let perf_metrics = metrics.unwrap();
        
        // System should still be responsive
        assert!(perf_metrics.memory_usage < 90.0); // Less than 90% memory usage
        
        println!("✓ Large scale operation completed successfully");
    }

    #[tokio::test]
    async fn test_configuration_persistence_workflow() {
        let (_, app_state) = setup_complete_test_environment().await.unwrap();
        
        // Test configuration changes persist across operations
        let original_config = settings::get_config(
            tauri::State::from(app_state.clone()),
        ).await.unwrap();
        
        // Modify configuration
        let mut modified_config = original_config.clone();
        modified_config.max_concurrent_analysis = 1;
        modified_config.auto_analyze_on_add = false;
        modified_config.show_notifications = false;
        
        // Apply changes
        let update_result = settings::update_config(
            modified_config.clone(),
            tauri::State::from(app_state.clone()),
        ).await;
        
        assert!(update_result.is_ok());
        
        // Perform some operations with new config
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("config_test.txt");
        std::fs::write(&test_file, "Configuration test content").unwrap();
        
        let analysis_result = files::analyze_files(
            vec![test_file.to_string_lossy().to_string()],
            tauri::State::from(app_state.clone()),
            app.handle().clone(),
        ).await;
        
        assert!(analysis_result.is_ok());
        
        // Verify configuration is still applied
        let current_config = settings::get_config(
            tauri::State::from(app_state.clone()),
        ).await.unwrap();
        
        assert_eq!(current_config.max_concurrent_analysis, 1);
        assert!(!current_config.auto_analyze_on_add);
        assert!(!current_config.show_notifications);
        
        println!("✓ Configuration persistence test passed");
    }
}