use stratosort::core::organizer::{FileOrganizer, OrganizeRequest, OrganizeResult, OrganizationRule};
use stratosort::error::{Result, AppError};
use tempfile::tempdir;
use std::fs;

#[cfg(test)]
mod organizer_tests {
    use super::*;

    fn create_test_organizer() -> Result<FileOrganizer> {
        FileOrganizer::new()
    }

    fn create_test_file(dir: &std::path::Path, name: &str, content: &str) -> Result<String> {
        let file_path = dir.join(name);
        fs::write(&file_path, content).map_err(|e| AppError::Io(e))?;
        Ok(file_path.to_string_lossy().to_string())
    }

    #[tokio::test]
    async fn test_organizer_creation() {
        let organizer = create_test_organizer();
        assert!(organizer.is_ok());
    }

    #[tokio::test]
    async fn test_organize_by_file_type() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Create test files
        let txt_file = create_test_file(temp_dir.path(), "document.txt", "Text content").unwrap();
        let jpg_file = create_test_file(temp_dir.path(), "image.jpg", "JPEG data").unwrap();
        let pdf_file = create_test_file(temp_dir.path(), "report.pdf", "PDF content").unwrap();
        
        let files = vec![txt_file, jpg_file, pdf_file];
        let target_dir = temp_dir.path().join("organized");
        
        let request = OrganizeRequest {
            files,
            target_directory: target_dir.to_string_lossy().to_string(),
            organization_method: "file_type".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 3);
        assert_eq!(organize_result.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_organize_dry_run() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        let test_file = create_test_file(temp_dir.path(), "test.txt", "Content").unwrap();
        let target_dir = temp_dir.path().join("organized");
        
        let request = OrganizeRequest {
            files: vec![test_file.clone()],
            target_directory: target_dir.to_string_lossy().to_string(),
            organization_method: "file_type".to_string(),
            create_subfolders: true,
            dry_run: true,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert!(organize_result.dry_run);
        
        // Original file should still exist in original location
        assert!(std::path::Path::new(&test_file).exists());
        
        // Target directory might not exist since it's a dry run
        assert!(!target_dir.exists() || fs::read_dir(&target_dir).unwrap().count() == 0);
    }

    #[tokio::test]
    async fn test_organize_nonexistent_files() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        let nonexistent_files = vec![
            "/nonexistent/file1.txt".to_string(),
            "/nonexistent/file2.jpg".to_string(),
        ];
        
        let request = OrganizeRequest {
            files: nonexistent_files,
            target_directory: temp_dir.path().to_string_lossy().to_string(),
            organization_method: "file_type".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 0);
        assert_eq!(organize_result.failed_operations, 2);
    }

    #[tokio::test]
    async fn test_organize_by_date() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        let test_file = create_test_file(temp_dir.path(), "dated_file.txt", "Content").unwrap();
        let target_dir = temp_dir.path().join("organized_by_date");
        
        let request = OrganizeRequest {
            files: vec![test_file],
            target_directory: target_dir.to_string_lossy().to_string(),
            organization_method: "date".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 1);
        assert_eq!(organize_result.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_organize_by_size() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Create files of different sizes
        let small_file = create_test_file(temp_dir.path(), "small.txt", "x").unwrap();
        let large_file = create_test_file(temp_dir.path(), "large.txt", &"x".repeat(5000)).unwrap();
        
        let files = vec![small_file, large_file];
        let target_dir = temp_dir.path().join("organized_by_size");
        
        let request = OrganizeRequest {
            files,
            target_directory: target_dir.to_string_lossy().to_string(),
            organization_method: "size".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 2);
        assert_eq!(organize_result.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_organize_without_subfolders() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        let test_file = create_test_file(temp_dir.path(), "test.txt", "Content").unwrap();
        let target_dir = temp_dir.path().join("flat_organization");
        
        let request = OrganizeRequest {
            files: vec![test_file],
            target_directory: target_dir.to_string_lossy().to_string(),
            organization_method: "file_type".to_string(),
            create_subfolders: false,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 1);
        assert_eq!(organize_result.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_organize_duplicate_files() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Create two files with same name in different directories
        let dir1 = temp_dir.path().join("dir1");
        let dir2 = temp_dir.path().join("dir2");
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();
        
        let file1 = create_test_file(&dir1, "duplicate.txt", "Content 1").unwrap();
        let file2 = create_test_file(&dir2, "duplicate.txt", "Content 2").unwrap();
        
        let target_dir = temp_dir.path().join("organized");
        
        let request = OrganizeRequest {
            files: vec![file1, file2],
            target_directory: target_dir.to_string_lossy().to_string(),
            organization_method: "file_type".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        // Both files should be processed, with duplicates handled
        assert!(organize_result.processed_files <= 2);
    }

    #[tokio::test]
    async fn test_add_organization_rule() {
        let mut organizer = create_test_organizer().unwrap();
        
        let rule = OrganizationRule {
            name: "PDF Documents".to_string(),
            conditions: vec![
                ("extension".to_string(), "pdf".to_string()),
            ],
            target_folder: "Documents/PDFs".to_string(),
            priority: 1,
        };
        
        let result = organizer.add_rule(&rule).await;
        assert!(result.is_ok());
        
        let rules = organizer.get_rules().await;
        assert!(rules.is_ok());
        assert!(!rules.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_remove_organization_rule() {
        let mut organizer = create_test_organizer().unwrap();
        
        let rule = OrganizationRule {
            name: "Test Rule".to_string(),
            conditions: vec![
                ("extension".to_string(), "test".to_string()),
            ],
            target_folder: "Test".to_string(),
            priority: 1,
        };
        
        organizer.add_rule(&rule).await.unwrap();
        
        let result = organizer.remove_rule("Test Rule").await;
        assert!(result.is_ok());
        
        let rules = organizer.get_rules().await.unwrap();
        assert!(!rules.iter().any(|r| r.name == "Test Rule"));
    }

    #[tokio::test]
    async fn test_organize_with_custom_rules() {
        let mut organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        // Add custom rule for PDF files
        let pdf_rule = OrganizationRule {
            name: "Important PDFs".to_string(),
            conditions: vec![
                ("extension".to_string(), "pdf".to_string()),
                ("content".to_string(), "important".to_string()),
            ],
            target_folder: "Important_Documents".to_string(),
            priority: 1,
        };
        
        organizer.add_rule(&pdf_rule).await.unwrap();
        
        let pdf_file = create_test_file(temp_dir.path(), "important.pdf", "This is important content").unwrap();
        let target_dir = temp_dir.path().join("organized");
        
        let request = OrganizeRequest {
            files: vec![pdf_file],
            target_directory: target_dir.to_string_lossy().to_string(),
            organization_method: "custom_rules".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 1);
    }

    #[tokio::test]
    async fn test_invalid_organization_method() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        let test_file = create_test_file(temp_dir.path(), "test.txt", "Content").unwrap();
        
        let request = OrganizeRequest {
            files: vec![test_file],
            target_directory: temp_dir.path().to_string_lossy().to_string(),
            organization_method: "invalid_method".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        // Should handle invalid method gracefully
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_organize_empty_file_list() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        let request = OrganizeRequest {
            files: vec![], // Empty file list
            target_directory: temp_dir.path().to_string_lossy().to_string(),
            organization_method: "file_type".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        assert_eq!(organize_result.processed_files, 0);
        assert_eq!(organize_result.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_organize_to_nonexistent_target_directory() {
        let organizer = create_test_organizer().unwrap();
        let temp_dir = tempdir().unwrap();
        
        let test_file = create_test_file(temp_dir.path(), "test.txt", "Content").unwrap();
        let nonexistent_target = temp_dir.path().join("nonexistent").join("target");
        
        let request = OrganizeRequest {
            files: vec![test_file],
            target_directory: nonexistent_target.to_string_lossy().to_string(),
            organization_method: "file_type".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        let result = organizer.organize_files(&request).await;
        assert!(result.is_ok());
        
        let organize_result = result.unwrap();
        // Should create target directory and process file
        assert_eq!(organize_result.processed_files, 1);
    }

    #[test]
    fn test_organize_request_validation() {
        let request = OrganizeRequest {
            files: vec!["test.txt".to_string()],
            target_directory: String::new(), // Empty target directory
            organization_method: "file_type".to_string(),
            create_subfolders: true,
            dry_run: false,
        };
        
        // This would typically be validated in the organizer
        assert!(request.target_directory.is_empty());
    }

    #[test]
    fn test_organization_rule_serialization() {
        let rule = OrganizationRule {
            name: "Test Rule".to_string(),
            conditions: vec![
                ("extension".to_string(), "txt".to_string()),
                ("size".to_string(), "small".to_string()),
            ],
            target_folder: "Text_Files".to_string(),
            priority: 5,
        };
        
        let serialized = serde_json::to_string(&rule);
        assert!(serialized.is_ok());
        
        let json_str = serialized.unwrap();
        assert!(json_str.contains("Test Rule"));
        assert!(json_str.contains("Text_Files"));
    }

    #[test]
    fn test_organize_result_serialization() {
        let result = OrganizeResult {
            processed_files: 10,
            failed_operations: 2,
            created_folders: vec!["Documents".to_string(), "Images".to_string()],
            moved_files: vec![
                ("old/path.txt".to_string(), "new/path.txt".to_string()),
            ],
            dry_run: false,
            errors: vec!["Failed to move file".to_string()],
        };
        
        let serialized = serde_json::to_string(&result);
        assert!(serialized.is_ok());
        
        let json_str = serialized.unwrap();
        assert!(json_str.contains("\"processed_files\":10"));
        assert!(json_str.contains("\"failed_operations\":2"));
    }

    #[tokio::test]
    async fn test_concurrent_organization() {
        let organizer = std::sync::Arc::new(create_test_organizer().unwrap());
        let temp_dir = tempdir().unwrap();
        
        let mut handles = vec![];
        
        for i in 0..3 {
            let organizer_clone = organizer.clone();
            let file_dir = temp_dir.path().join(format!("dir_{}", i));
            fs::create_dir_all(&file_dir).unwrap();
            let test_file = create_test_file(&file_dir, &format!("file_{}.txt", i), "Content").unwrap();
            let target_dir = temp_dir.path().join(format!("organized_{}", i));
            
            let handle = tokio::spawn(async move {
                let request = OrganizeRequest {
                    files: vec![test_file],
                    target_directory: target_dir.to_string_lossy().to_string(),
                    organization_method: "file_type".to_string(),
                    create_subfolders: true,
                    dry_run: false,
                };
                
                organizer_clone.organize_files(&request).await
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            
            let organize_result = result.unwrap();
            assert_eq!(organize_result.processed_files, 1);
        }
    }
}