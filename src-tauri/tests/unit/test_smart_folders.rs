use stratosort::core::{SmartFolderManager, SmartFolder};
use stratosort::commands::organization::{OrganizationRule, RuleType};
use stratosort::storage::Database;
use stratosort::error::{AppError, Result};
use tauri::test::{mock_app, mock_context};
use tempfile::tempdir;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(test)]
mod smart_folder_tests {
    use super::*;

    async fn create_test_manager() -> Arc<SmartFolderManager> {
        let app = mock_app(mock_context());
        let db = Arc::new(Database::new(&app.handle()).await.unwrap());
        Arc::new(SmartFolderManager::new(db))
    }

    // Basic CRUD Operations
    #[tokio::test]
    async fn test_create_smart_folder() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Test Folder".to_string(),
            path: temp_dir.path().to_string_lossy().to_string(),
            rules: vec![
                OrganizationRule {
                    rule_type: RuleType::Extension,
                    value: ".pdf".to_string(),
                    operator: "equals".to_string(),
                },
            ],
            auto_organize: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = manager.create(folder.clone()).await;
        assert!(result.is_ok());
        
        let created = result.unwrap();
        assert_eq!(created.name, folder.name);
        assert_eq!(created.path, folder.path);
        assert_eq!(created.rules.len(), 1);
    }

    #[tokio::test]
    async fn test_create_folder_duplicate_name() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Duplicate".to_string(),
            path: temp_dir.path().to_string_lossy().to_string(),
            rules: vec![],
            auto_organize: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // Create first folder
        assert!(manager.create(folder.clone()).await.is_ok());
        
        // Try to create duplicate
        let mut duplicate = folder;
        duplicate.id = Uuid::new_v4();
        
        let result = manager.create(duplicate).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::DatabaseError { .. } => {},
            _ => panic!("Expected DatabaseError for duplicate name"),
        }
    }

    #[tokio::test]
    async fn test_get_smart_folder() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Get Test".to_string(),
            path: temp_dir.path().to_string_lossy().to_string(),
            rules: vec![],
            auto_organize: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let created = manager.create(folder.clone()).await.unwrap();
        
        // Get by ID
        let retrieved = manager.get(created.id).await.unwrap();
        assert!(retrieved.is_some());
        
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.name, created.name);
    }

    #[tokio::test]
    async fn test_get_nonexistent_folder() {
        let manager = create_test_manager().await;
        
        let result = manager.get(Uuid::new_v4()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_all_smart_folders() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create multiple folders
        for i in 0..3 {
            let folder = SmartFolder {
                id: Uuid::new_v4(),
                name: format!("Folder {}", i),
                path: temp_dir.path().join(format!("folder{}", i))
                    .to_string_lossy().to_string(),
                rules: vec![],
                auto_organize: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            manager.create(folder).await.unwrap();
        }
        
        let all = manager.get_all().await.unwrap();
        assert_eq!(all.len(), 3);
        
        // Verify they're ordered by creation time
        for i in 0..2 {
            assert!(all[i].created_at <= all[i+1].created_at);
        }
    }

    #[tokio::test]
    async fn test_update_smart_folder() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Original".to_string(),
            path: temp_dir.path().to_string_lossy().to_string(),
            rules: vec![],
            auto_organize: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let created = manager.create(folder).await.unwrap();
        
        // Update the folder
        let mut updated = created.clone();
        updated.name = "Updated".to_string();
        updated.auto_organize = true;
        updated.rules = vec![
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".txt".to_string(),
                operator: "equals".to_string(),
            },
        ];
        
        let result = manager.update(updated.clone()).await;
        assert!(result.is_ok());
        
        // Verify update
        let retrieved = manager.get(created.id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "Updated");
        assert!(retrieved.auto_organize);
        assert_eq!(retrieved.rules.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_smart_folder() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "To Delete".to_string(),
            path: temp_dir.path().to_string_lossy().to_string(),
            rules: vec![],
            auto_organize: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let created = manager.create(folder).await.unwrap();
        
        // Delete the folder
        let result = manager.delete(created.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // Verify deletion
        let retrieved = manager.get(created.id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_folder() {
        let manager = create_test_manager().await;
        
        let result = manager.delete(Uuid::new_v4()).await;
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false for nonexistent
    }

    // Rule Matching Tests
    #[tokio::test]
    async fn test_matches_extension_rule() {
        let manager = create_test_manager().await;
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".pdf".to_string(),
                operator: "equals".to_string(),
            },
        ];
        
        assert!(manager.matches_rules("/test/document.pdf", &rules, "AND").await);
        assert!(!manager.matches_rules("/test/document.txt", &rules, "AND").await);
        assert!(!manager.matches_rules("/test/document", &rules, "AND").await);
    }

    #[tokio::test]
    async fn test_matches_filename_contains_rule() {
        let manager = create_test_manager().await;
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::FileName,
                value: "report".to_string(),
                operator: "contains".to_string(),
            },
        ];
        
        assert!(manager.matches_rules("/test/annual_report.pdf", &rules, "AND").await);
        assert!(manager.matches_rules("/test/report_2024.txt", &rules, "AND").await);
        assert!(!manager.matches_rules("/test/document.pdf", &rules, "AND").await);
    }

    #[tokio::test]
    async fn test_matches_size_rules() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create test files with different sizes
        let small_file = temp_dir.path().join("small.txt");
        std::fs::write(&small_file, "small").unwrap(); // ~5 bytes
        
        let large_file = temp_dir.path().join("large.txt");
        std::fs::write(&large_file, "x".repeat(2_000_000)).unwrap(); // ~2MB
        
        let rules_gt = vec![
            OrganizationRule {
                rule_type: RuleType::Size,
                value: "1000000".to_string(), // 1MB
                operator: "greater_than".to_string(),
            },
        ];
        
        assert!(manager.matches_rules(
            &large_file.to_string_lossy(), 
            &rules_gt, 
            "AND"
        ).await);
        assert!(!manager.matches_rules(
            &small_file.to_string_lossy(), 
            &rules_gt, 
            "AND"
        ).await);
        
        let rules_lt = vec![
            OrganizationRule {
                rule_type: RuleType::Size,
                value: "1000".to_string(), // 1KB
                operator: "less_than".to_string(),
            },
        ];
        
        assert!(manager.matches_rules(
            &small_file.to_string_lossy(), 
            &rules_lt, 
            "AND"
        ).await);
        assert!(!manager.matches_rules(
            &large_file.to_string_lossy(), 
            &rules_lt, 
            "AND"
        ).await);
    }

    #[tokio::test]
    async fn test_matches_date_rules() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "content").unwrap();
        
        // File was just created, so it should be newer than 1 day ago
        let one_day_ago = (chrono::Utc::now() - chrono::Duration::days(1))
            .timestamp()
            .to_string();
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::DateModified,
                value: one_day_ago,
                operator: "after".to_string(),
            },
        ];
        
        assert!(manager.matches_rules(
            &test_file.to_string_lossy(), 
            &rules, 
            "AND"
        ).await);
        
        // Test with future date
        let future = (chrono::Utc::now() + chrono::Duration::days(1))
            .timestamp()
            .to_string();
        
        let future_rules = vec![
            OrganizationRule {
                rule_type: RuleType::DateModified,
                value: future,
                operator: "after".to_string(),
            },
        ];
        
        assert!(!manager.matches_rules(
            &test_file.to_string_lossy(), 
            &future_rules, 
            "AND"
        ).await);
    }

    #[tokio::test]
    async fn test_matches_multiple_rules_and() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let test_file = temp_dir.path().join("report_2024.pdf");
        std::fs::write(&test_file, "content").unwrap();
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".pdf".to_string(),
                operator: "equals".to_string(),
            },
            OrganizationRule {
                rule_type: RuleType::FileName,
                value: "report".to_string(),
                operator: "contains".to_string(),
            },
        ];
        
        // Both rules should match
        assert!(manager.matches_rules(
            &test_file.to_string_lossy(), 
            &rules, 
            "AND"
        ).await);
        
        // Change extension rule to not match
        let wrong_rules = vec![
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".txt".to_string(),
                operator: "equals".to_string(),
            },
            OrganizationRule {
                rule_type: RuleType::FileName,
                value: "report".to_string(),
                operator: "contains".to_string(),
            },
        ];
        
        // Should fail with AND logic
        assert!(!manager.matches_rules(
            &test_file.to_string_lossy(), 
            &wrong_rules, 
            "AND"
        ).await);
    }

    #[tokio::test]
    async fn test_matches_multiple_rules_or() {
        let manager = create_test_manager().await;
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".pdf".to_string(),
                operator: "equals".to_string(),
            },
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".doc".to_string(),
                operator: "equals".to_string(),
            },
        ];
        
        // Should match PDF files
        assert!(manager.matches_rules("/test/document.pdf", &rules, "OR").await);
        
        // Should match DOC files
        assert!(manager.matches_rules("/test/document.doc", &rules, "OR").await);
        
        // Should not match other extensions
        assert!(!manager.matches_rules("/test/document.txt", &rules, "OR").await);
    }

    #[tokio::test]
    async fn test_matches_empty_rules() {
        let manager = create_test_manager().await;
        
        let rules = vec![];
        
        // Empty rules should not match anything
        assert!(!manager.matches_rules("/test/file.txt", &rules, "AND").await);
        assert!(!manager.matches_rules("/test/file.txt", &rules, "OR").await);
    }

    #[tokio::test]
    async fn test_matches_invalid_file_path() {
        let manager = create_test_manager().await;
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".txt".to_string(),
                operator: "equals".to_string(),
            },
        ];
        
        // Nonexistent file should not match
        assert!(!manager.matches_rules(
            "/nonexistent/path/file.txt", 
            &rules, 
            "AND"
        ).await);
    }

    // Auto-Organization Tests
    #[tokio::test]
    async fn test_get_auto_organize_folders() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create mix of auto-organize and manual folders
        for i in 0..5 {
            let folder = SmartFolder {
                id: Uuid::new_v4(),
                name: format!("Folder {}", i),
                path: temp_dir.path().join(format!("folder{}", i))
                    .to_string_lossy().to_string(),
                rules: vec![],
                auto_organize: i % 2 == 0, // Even numbers are auto-organize
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            manager.create(folder).await.unwrap();
        }
        
        let auto_folders = manager.get_auto_organize_folders().await.unwrap();
        assert_eq!(auto_folders.len(), 3); // Should have 3 auto-organize folders
        
        for folder in auto_folders {
            assert!(folder.auto_organize);
        }
    }

    #[tokio::test]
    async fn test_find_matching_folder_single_match() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let pdf_folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "PDF Files".to_string(),
            path: temp_dir.path().join("pdfs").to_string_lossy().to_string(),
            rules: vec![
                OrganizationRule {
                    rule_type: RuleType::Extension,
                    value: ".pdf".to_string(),
                    operator: "equals".to_string(),
                },
            ],
            auto_organize: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let created = manager.create(pdf_folder).await.unwrap();
        
        let matched = manager.find_matching_folder("/test/document.pdf").await.unwrap();
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, created.id);
    }

    #[tokio::test]
    async fn test_find_matching_folder_multiple_matches() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create two folders that could match PDFs
        let general_docs = SmartFolder {
            id: Uuid::new_v4(),
            name: "Documents".to_string(),
            path: temp_dir.path().join("docs").to_string_lossy().to_string(),
            rules: vec![
                OrganizationRule {
                    rule_type: RuleType::Extension,
                    value: ".pdf".to_string(),
                    operator: "equals".to_string(),
                },
            ],
            auto_organize: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let created_first = manager.create(general_docs).await.unwrap();
        
        // Create more specific folder
        let reports = SmartFolder {
            id: Uuid::new_v4(),
            name: "Reports".to_string(),
            path: temp_dir.path().join("reports").to_string_lossy().to_string(),
            rules: vec![
                OrganizationRule {
                    rule_type: RuleType::Extension,
                    value: ".pdf".to_string(),
                    operator: "equals".to_string(),
                },
                OrganizationRule {
                    rule_type: RuleType::FileName,
                    value: "report".to_string(),
                    operator: "contains".to_string(),
                },
            ],
            auto_organize: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        manager.create(reports).await.unwrap();
        
        // Should match first created folder (general docs)
        let matched = manager.find_matching_folder("/test/document.pdf").await.unwrap();
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, created_first.id);
    }

    #[tokio::test]
    async fn test_find_matching_folder_no_match() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create folder that only matches PDFs
        let pdf_folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "PDF Files".to_string(),
            path: temp_dir.path().join("pdfs").to_string_lossy().to_string(),
            rules: vec![
                OrganizationRule {
                    rule_type: RuleType::Extension,
                    value: ".pdf".to_string(),
                    operator: "equals".to_string(),
                },
            ],
            auto_organize: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        manager.create(pdf_folder).await.unwrap();
        
        // Try to match a TXT file
        let matched = manager.find_matching_folder("/test/document.txt").await.unwrap();
        assert!(matched.is_none());
    }

    // Edge Cases and Error Handling
    #[tokio::test]
    async fn test_invalid_rule_operators() {
        let manager = create_test_manager().await;
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::Extension,
                value: ".pdf".to_string(),
                operator: "invalid_operator".to_string(),
            },
        ];
        
        // Should handle invalid operator gracefully (default to false)
        assert!(!manager.matches_rules("/test/document.pdf", &rules, "AND").await);
    }

    #[tokio::test]
    async fn test_malformed_size_values() {
        let manager = create_test_manager().await;
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::Size,
                value: "not_a_number".to_string(),
                operator: "greater_than".to_string(),
            },
        ];
        
        // Should handle parse errors gracefully
        assert!(!manager.matches_rules("/test/file.txt", &rules, "AND").await);
    }

    #[tokio::test]
    async fn test_malformed_date_values() {
        let manager = create_test_manager().await;
        
        let rules = vec![
            OrganizationRule {
                rule_type: RuleType::DateModified,
                value: "not_a_timestamp".to_string(),
                operator: "after".to_string(),
            },
        ];
        
        // Should handle parse errors gracefully
        assert!(!manager.matches_rules("/test/file.txt", &rules, "AND").await);
    }

    #[tokio::test]
    async fn test_concurrent_folder_operations() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Spawn multiple tasks creating folders
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let manager_clone = manager.clone();
                let path = temp_dir.path().join(format!("folder{}", i))
                    .to_string_lossy().to_string();
                
                tokio::spawn(async move {
                    let folder = SmartFolder {
                        id: Uuid::new_v4(),
                        name: format!("Concurrent {}", i),
                        path,
                        rules: vec![],
                        auto_organize: false,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };
                    manager_clone.create(folder).await
                })
            })
            .collect();
        
        // Wait for all to complete
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
        
        // Verify all were created
        let all = manager.get_all().await.unwrap();
        assert_eq!(all.len(), 10);
    }

    #[tokio::test]
    async fn test_folder_with_complex_rules() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create a test file
        let test_file = temp_dir.path().join("quarterly_report_2024.pdf");
        std::fs::write(&test_file, "x".repeat(500_000)).unwrap(); // ~500KB
        
        let complex_folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Complex Rules".to_string(),
            path: temp_dir.path().join("complex").to_string_lossy().to_string(),
            rules: vec![
                OrganizationRule {
                    rule_type: RuleType::Extension,
                    value: ".pdf".to_string(),
                    operator: "equals".to_string(),
                },
                OrganizationRule {
                    rule_type: RuleType::FileName,
                    value: "report".to_string(),
                    operator: "contains".to_string(),
                },
                OrganizationRule {
                    rule_type: RuleType::Size,
                    value: "100000".to_string(), // 100KB
                    operator: "greater_than".to_string(),
                },
                OrganizationRule {
                    rule_type: RuleType::Size,
                    value: "10000000".to_string(), // 10MB
                    operator: "less_than".to_string(),
                },
            ],
            auto_organize: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        manager.create(complex_folder).await.unwrap();
        
        // File should match all rules
        let matched = manager.find_matching_folder(
            &test_file.to_string_lossy()
        ).await.unwrap();
        assert!(matched.is_some());
        
        // Create file that doesn't match size constraint
        let small_file = temp_dir.path().join("small_report.pdf");
        std::fs::write(&small_file, "small").unwrap();
        
        let matched_small = manager.find_matching_folder(
            &small_file.to_string_lossy()
        ).await.unwrap();
        assert!(matched_small.is_none());
    }

    #[tokio::test]
    async fn test_folder_path_normalization() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create folder with non-normalized path
        let folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Path Test".to_string(),
            path: format!("{}//subfolder//..//final", 
                temp_dir.path().to_string_lossy()),
            rules: vec![],
            auto_organize: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = manager.create(folder).await;
        assert!(result.is_ok());
        
        // Path should be stored as-is (normalization happens elsewhere)
        let created = result.unwrap();
        assert!(created.path.contains("//"));
    }

    #[tokio::test]
    async fn test_unicode_in_folder_names_and_paths() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "文档_документы_ドキュメント".to_string(),
            path: temp_dir.path().join("文档").to_string_lossy().to_string(),
            rules: vec![
                OrganizationRule {
                    rule_type: RuleType::FileName,
                    value: "文档".to_string(),
                    operator: "contains".to_string(),
                },
            ],
            auto_organize: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = manager.create(folder).await;
        assert!(result.is_ok());
        
        let created = result.unwrap();
        assert!(created.name.contains("文档"));
        
        // Test matching with unicode
        assert!(manager.matches_rules(
            "/test/文档_report.pdf",
            &created.rules,
            "AND"
        ).await);
    }
}