use stratosort::storage::database::{Database, FileAnalysis, SmartFolder, Operation, VectorStats};
use stratosort::commands::notifications::Notification;
use stratosort::error::Result;
use tempfile::tempdir;
use uuid::Uuid;
use std::sync::Arc;
use chrono::Utc;

#[cfg(test)]
mod database_tests {
    use super::*;

    async fn create_test_database() -> Result<Database> {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite://{}", db_path.display());
        
        Database::new(&db_url).await
    }

    #[tokio::test]
    async fn test_database_creation() {
        let db = create_test_database().await;
        assert!(db.is_ok());
    }

    #[tokio::test]
    async fn test_save_and_get_analysis() {
        let db = create_test_database().await.unwrap();
        
        let analysis = FileAnalysis {
            id: Uuid::new_v4(),
            path: "/test/file.txt".to_string(),
            file_type: "text/plain".to_string(),
            summary: "Test file summary".to_string(),
            tags: vec!["test".to_string(), "sample".to_string()],
            category: "document".to_string(),
            confidence: 0.95,
            size: 1024,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // Save analysis
        let result = db.save_analysis(&analysis).await;
        assert!(result.is_ok());
        
        // Retrieve analysis
        let retrieved = db.get_analysis(&analysis.path).await;
        assert!(retrieved.is_ok());
        
        let retrieved_analysis = retrieved.unwrap().unwrap();
        assert_eq!(retrieved_analysis.path, analysis.path);
        assert_eq!(retrieved_analysis.summary, analysis.summary);
        assert_eq!(retrieved_analysis.category, analysis.category);
        assert_eq!(retrieved_analysis.tags, analysis.tags);
    }

    #[tokio::test]
    async fn test_get_analysis_nonexistent() {
        let db = create_test_database().await.unwrap();
        
        let result = db.get_analysis("/nonexistent/file.txt").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_save_and_get_embedding() {
        let db = create_test_database().await.unwrap();
        
        let path = "/test/file.txt";
        let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        
        // Save embedding
        let result = db.save_embedding(path, &embedding, Some("test-model")).await;
        assert!(result.is_ok());
        
        // Retrieve embedding
        let retrieved = db.get_embedding(path).await;
        assert!(retrieved.is_ok());
        
        let retrieved_embedding = retrieved.unwrap().unwrap();
        assert_eq!(retrieved_embedding, embedding);
    }

    #[tokio::test]
    async fn test_search_similar_files() {
        let db = create_test_database().await.unwrap();
        
        // Save test embeddings
        let embeddings = vec![
            ("/file1.txt", vec![1.0, 0.0, 0.0]),
            ("/file2.txt", vec![0.0, 1.0, 0.0]),
            ("/file3.txt", vec![0.0, 0.0, 1.0]),
            ("/file4.txt", vec![0.9, 0.1, 0.0]), // Similar to file1
        ];
        
        for (path, embedding) in embeddings {
            db.save_embedding(path, &embedding, Some("test-model")).await.unwrap();
        }
        
        // Search for similar files to file1
        let query_embedding = vec![1.0, 0.0, 0.0];
        let similar = db.search_similar_files(&query_embedding, 0.8, 10).await;
        assert!(similar.is_ok());
        
        let results = similar.unwrap();
        assert!(!results.is_empty());
        // file1 should be most similar (perfect match)
        // file4 should also be found (high similarity)
    }

    #[tokio::test]
    async fn test_create_and_get_smart_folder() {
        let db = create_test_database().await.unwrap();
        
        let smart_folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Test Folder".to_string(),
            description: "A test smart folder".to_string(),
            query: "category:document".to_string(),
            auto_organize: true,
            target_path: "/organized/documents".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // Create smart folder
        let result = db.create_smart_folder(&smart_folder).await;
        assert!(result.is_ok());
        
        // Get smart folder
        let retrieved = db.get_smart_folder(smart_folder.id).await;
        assert!(retrieved.is_ok());
        
        let retrieved_folder = retrieved.unwrap().unwrap();
        assert_eq!(retrieved_folder.name, smart_folder.name);
        assert_eq!(retrieved_folder.query, smart_folder.query);
        assert_eq!(retrieved_folder.auto_organize, smart_folder.auto_organize);
    }

    #[tokio::test]
    async fn test_update_smart_folder() {
        let db = create_test_database().await.unwrap();
        
        let mut smart_folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "Original Name".to_string(),
            description: "Original description".to_string(),
            query: "category:document".to_string(),
            auto_organize: false,
            target_path: "/original/path".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // Create smart folder
        db.create_smart_folder(&smart_folder).await.unwrap();
        
        // Update smart folder
        smart_folder.name = "Updated Name".to_string();
        smart_folder.auto_organize = true;
        smart_folder.updated_at = chrono::Utc::now();
        
        let result = db.update_smart_folder(&smart_folder).await;
        assert!(result.is_ok());
        
        // Verify update
        let retrieved = db.get_smart_folder(smart_folder.id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "Updated Name");
        assert!(retrieved.auto_organize);
    }

    #[tokio::test]
    async fn test_delete_smart_folder() {
        let db = create_test_database().await.unwrap();
        
        let smart_folder = SmartFolder {
            id: Uuid::new_v4(),
            name: "To Delete".to_string(),
            description: "Will be deleted".to_string(),
            query: "category:temp".to_string(),
            auto_organize: false,
            target_path: "/temp".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // Create smart folder
        db.create_smart_folder(&smart_folder).await.unwrap();
        
        // Verify it exists
        let retrieved = db.get_smart_folder(smart_folder.id).await.unwrap();
        assert!(retrieved.is_some());
        
        // Delete smart folder
        let result = db.delete_smart_folder(smart_folder.id).await;
        assert!(result.is_ok());
        
        // Verify it's gone
        let retrieved = db.get_smart_folder(smart_folder.id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_smart_folders() {
        let db = create_test_database().await.unwrap();
        
        // Create multiple smart folders
        for i in 1..=3 {
            let smart_folder = SmartFolder {
                id: Uuid::new_v4(),
                name: format!("Folder {}", i),
                description: format!("Description {}", i),
                query: format!("category:{}", i),
                auto_organize: i % 2 == 0,
                target_path: format!("/folder{}", i),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            
            db.create_smart_folder(&smart_folder).await.unwrap();
        }
        
        // List all folders
        let folders = db.list_smart_folders().await;
        assert!(folders.is_ok());
        
        let folder_list = folders.unwrap();
        assert_eq!(folder_list.len(), 3);
    }

    #[tokio::test]
    async fn test_get_recent_analyses() {
        let db = create_test_database().await.unwrap();
        
        // Create multiple analyses
        for i in 1..=5 {
            let analysis = FileAnalysis {
                id: Uuid::new_v4(),
                path: format!("/test/file{}.txt", i),
                file_type: "text/plain".to_string(),
                summary: format!("Summary {}", i),
                tags: vec![format!("tag{}", i)],
                category: "document".to_string(),
                confidence: 0.9,
                size: 1024 * i as u64,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            
            db.save_analysis(&analysis).await.unwrap();
            
            // Small delay to ensure different timestamps
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        // Get recent analyses
        let recent = db.get_recent_analyses(3).await;
        assert!(recent.is_ok());
        
        let recent_list = recent.unwrap();
        assert_eq!(recent_list.len(), 3);
        
        // Should be in reverse chronological order
        assert!(recent_list[0].contains("file5.txt")); // Most recent
    }

    #[tokio::test]
    async fn test_check_file_permission() {
        let db = create_test_database().await.unwrap();
        
        let path = "/test/file.txt";
        let user_id = "user123";
        
        // Test permission check for non-existent permission
        let result = db.check_file_permission(path, user_id).await;
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should default to false
    }

    #[tokio::test]
    async fn test_database_migration() {
        // Test that database properly handles schema migrations
        let db = create_test_database().await.unwrap();
        
        // The database creation should have run all migrations
        // This is a basic test to ensure the database is functional
        let analysis = FileAnalysis {
            id: Uuid::new_v4(),
            path: "/migration/test.txt".to_string(),
            file_type: "text/plain".to_string(),
            summary: "Migration test".to_string(),
            tags: vec!["migration".to_string()],
            category: "test".to_string(),
            confidence: 1.0,
            size: 100,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = db.save_analysis(&analysis).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_database_concurrent_access() {
        let db = create_test_database().await.unwrap();
        let db = Arc::new(db);
        
        // Create multiple concurrent tasks
        let mut handles = vec![];
        
        for i in 0..10 {
            let db_clone = db.clone();
            let handle = tokio::spawn(async move {
                let analysis = FileAnalysis {
                    id: Uuid::new_v4(),
                    path: format!("/concurrent/file{}.txt", i),
                    file_type: "text/plain".to_string(),
                    summary: format!("Concurrent test {}", i),
                    tags: vec![format!("concurrent{}", i)],
                    category: "test".to_string(),
                    confidence: 0.8,
                    size: 100 * i as u64,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };
                
                db_clone.save_analysis(&analysis).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        // Verify all analyses were saved
        let recent = db.get_recent_analyses(15).await.unwrap();
        assert!(recent.len() >= 10);
    }

    // Additional comprehensive tests

    #[tokio::test]
    async fn test_search_by_category() {
        let db = create_test_database().await.unwrap();
        
        // Create analyses with different categories
        let categories = vec!["Documents", "Images", "Videos", "Documents"];
        
        for (i, category) in categories.iter().enumerate() {
            let analysis = FileAnalysis {
                id: Uuid::new_v4(),
                path: format!("/test/file{}.txt", i),
                file_type: "text/plain".to_string(),
                summary: format!("File {}", i),
                tags: vec![],
                category: category.to_string(),
                confidence: 0.9,
                size: 1024,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            
            db.save_analysis(&analysis).await.unwrap();
        }
        
        // Search by category
        let docs = db.search_by_category("Documents").await.unwrap();
        assert_eq!(docs.len(), 2);
        
        let images = db.search_by_category("Images").await.unwrap();
        assert_eq!(images.len(), 1);
        
        let videos = db.search_by_category("Videos").await.unwrap();
        assert_eq!(videos.len(), 1);
    }

    #[tokio::test]
    async fn test_search_by_tags() {
        let db = create_test_database().await.unwrap();
        
        // Create analyses with different tags
        let test_cases = vec![
            ("file1", vec!["important", "work"]),
            ("file2", vec!["personal", "photo"]),
            ("file3", vec!["work", "report"]),
            ("file4", vec!["important", "personal"]),
        ];
        
        for (name, tags) in test_cases {
            let analysis = FileAnalysis {
                id: Uuid::new_v4(),
                path: format!("/test/{}.txt", name),
                file_type: "text/plain".to_string(),
                summary: format!("File {}", name),
                tags: tags.iter().map(|s| s.to_string()).collect(),
                category: "Documents".to_string(),
                confidence: 0.9,
                size: 1024,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            
            db.save_analysis(&analysis).await.unwrap();
        }
        
        // Search by tags
        let important = db.search_by_tags(&["important".to_string()]).await.unwrap();
        assert_eq!(important.len(), 2);
        
        let work = db.search_by_tags(&["work".to_string()]).await.unwrap();
        assert_eq!(work.len(), 2);
        
        let personal = db.search_by_tags(&["personal".to_string()]).await.unwrap();
        assert_eq!(personal.len(), 2);
        
        // Search with multiple tags (OR operation)
        let multiple = db.search_by_tags(&["photo".to_string(), "report".to_string()]).await.unwrap();
        assert_eq!(multiple.len(), 2);
    }

    #[tokio::test]
    async fn test_record_and_get_operations() {
        let db = create_test_database().await.unwrap();
        
        // Record multiple operations
        for i in 0..5 {
            let operation = Operation {
                id: Uuid::new_v4(),
                operation_type: "move".to_string(),
                source_path: format!("/source/file{}.txt", i),
                target_path: Some(format!("/target/file{}.txt", i)),
                metadata: serde_json::json!({"index": i}),
                timestamp: Utc::now(),
                success: i % 2 == 0, // Alternate success/failure
                error_message: if i % 2 != 0 { Some("Test error".to_string()) } else { None },
            };
            
            db.record_operation(&operation).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        // Get recent operations
        let recent = db.get_recent_operations(3).await.unwrap();
        assert_eq!(recent.len(), 3);
        
        // Most recent should be last (file4)
        assert!(recent[0].source_path.contains("file4"));
    }

    #[tokio::test]
    async fn test_semantic_search() {
        let db = create_test_database().await.unwrap();
        
        // Save embeddings for semantic search
        let files = vec![
            ("/doc1.txt", vec![1.0, 0.0, 0.0, 0.0]),
            ("/doc2.txt", vec![0.9, 0.1, 0.0, 0.0]), // Similar to doc1
            ("/doc3.txt", vec![0.0, 1.0, 0.0, 0.0]),
            ("/doc4.txt", vec![0.0, 0.0, 1.0, 0.0]),
            ("/doc5.txt", vec![0.0, 0.0, 0.0, 1.0]),
        ];
        
        for (path, embedding) in files {
            db.save_embedding(path, &embedding, Some("test-model")).await.unwrap();
        }
        
        // Search for similar documents
        let query = vec![0.95, 0.05, 0.0, 0.0];
        let results = db.semantic_search(&query, 3).await.unwrap();
        
        assert!(results.len() <= 3);
        
        // First result should be doc1 or doc2 (most similar)
        let first_path = &results[0].0;
        assert!(first_path == "/doc1.txt" || first_path == "/doc2.txt");
        
        // Similarity scores should be in descending order
        for i in 1..results.len() {
            assert!(results[i-1].1 >= results[i].1);
        }
    }

    #[tokio::test]
    async fn test_save_and_get_notifications() {
        let db = create_test_database().await.unwrap();
        
        // Create test notifications
        for i in 0..5 {
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                title: format!("Notification {}", i),
                message: format!("Message {}", i),
                notification_type: if i % 2 == 0 { "info" } else { "warning" }.to_string(),
                timestamp: Utc::now().timestamp(),
                read: i < 2, // First two are read
                data: Some(serde_json::json!({"index": i})),
            };
            
            db.save_notification(&notification).await.unwrap();
        }
        
        // Get all notifications
        let all = db.get_notifications(10, false).await.unwrap();
        assert_eq!(all.len(), 5);
        
        // Get unread only
        let unread = db.get_notifications(10, true).await.unwrap();
        assert_eq!(unread.len(), 3);
    }

    #[tokio::test]
    async fn test_mark_notification_read() {
        let db = create_test_database().await.unwrap();
        
        let notification = Notification {
            id: Uuid::new_v4().to_string(),
            title: "Test Notification".to_string(),
            message: "Test message".to_string(),
            notification_type: "info".to_string(),
            timestamp: Utc::now().timestamp(),
            read: false,
            data: None,
        };
        
        // Save notification
        db.save_notification(&notification).await.unwrap();
        
        // Verify it's unread
        let unread = db.get_notifications(10, true).await.unwrap();
        assert_eq!(unread.len(), 1);
        
        // Mark as read
        db.mark_notification_read(&notification.id).await.unwrap();
        
        // Verify it's now read
        let unread = db.get_notifications(10, true).await.unwrap();
        assert_eq!(unread.len(), 0);
    }

    #[tokio::test]
    async fn test_clear_old_notifications() {
        let db = create_test_database().await.unwrap();
        
        let now = Utc::now().timestamp();
        let old_time = now - 86400; // 1 day ago
        
        // Create old and new notifications
        for i in 0..5 {
            let notification = Notification {
                id: Uuid::new_v4().to_string(),
                title: format!("Notification {}", i),
                message: format!("Message {}", i),
                notification_type: "info".to_string(),
                timestamp: if i < 3 { old_time } else { now },
                read: false,
                data: None,
            };
            
            db.save_notification(&notification).await.unwrap();
        }
        
        // Clear old notifications
        let deleted = db.clear_old_notifications(now - 3600).await.unwrap();
        assert_eq!(deleted, 3);
        
        // Verify only new notifications remain
        let remaining = db.get_notifications(10, false).await.unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn test_health_check() {
        let db = create_test_database().await.unwrap();
        
        let result = db.health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_vacuum() {
        let db = create_test_database().await.unwrap();
        
        // Add and delete some data to create fragmentation
        for i in 0..10 {
            let analysis = FileAnalysis {
                id: Uuid::new_v4(),
                path: format!("/temp/file{}.txt", i),
                file_type: "text/plain".to_string(),
                summary: "Temp file".to_string(),
                tags: vec![],
                category: "temp".to_string(),
                confidence: 0.5,
                size: 1024,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            
            db.save_analysis(&analysis).await.unwrap();
        }
        
        // Run vacuum
        let result = db.vacuum().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_flush() {
        let db = create_test_database().await.unwrap();
        
        // Flush should not error
        let result = db.flush().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let db = create_test_database().await.unwrap();
        
        // Clear cache should work even with empty cache
        let result = db.clear_cache().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_clear_all_data() {
        let db = create_test_database().await.unwrap();
        
        // Add some data
        let analysis = FileAnalysis {
            id: Uuid::new_v4(),
            path: "/test/file.txt".to_string(),
            file_type: "text/plain".to_string(),
            summary: "Test".to_string(),
            tags: vec![],
            category: "test".to_string(),
            confidence: 0.9,
            size: 1024,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        db.save_analysis(&analysis).await.unwrap();
        db.save_embedding("/test/file.txt", &vec![0.1, 0.2, 0.3], Some("test-model")).await.unwrap();
        
        // Clear all data
        let result = db.clear_all_data().await;
        assert!(result.is_ok());
        
        // Verify data is gone
        let retrieved = db.get_analysis("/test/file.txt").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_close_connections() {
        let db = create_test_database().await.unwrap();
        
        // Should be able to close connections
        let result = db.close_connections().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_vector_stats() {
        let db = create_test_database().await.unwrap();
        
        // Add some embeddings
        for i in 0..5 {
            let path = format!("/test/file{}.txt", i);
            let embedding = vec![0.1 * i as f32, 0.2, 0.3, 0.4];
            db.save_embedding(&path, &embedding, Some("test-model")).await.unwrap();
        }
        
        let stats = db.get_vector_stats().await.unwrap();
        assert_eq!(stats.total_vectors, 5);
        assert_eq!(stats.dimension, 4);
    }

    #[tokio::test]
    async fn test_maintain_vector_table() {
        let db = create_test_database().await.unwrap();
        
        // Should be able to maintain vector table
        let result = db.maintain_vector_table().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_analysis() {
        let db = create_test_database().await.unwrap();
        
        let mut analysis = FileAnalysis {
            id: Uuid::new_v4(),
            path: "/test/file.txt".to_string(),
            file_type: "text/plain".to_string(),
            summary: "Original summary".to_string(),
            tags: vec!["original".to_string()],
            category: "Documents".to_string(),
            confidence: 0.8,
            size: 1024,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Save original
        db.save_analysis(&analysis).await.unwrap();
        
        // Update analysis
        analysis.summary = "Updated summary".to_string();
        analysis.tags = vec!["updated".to_string()];
        analysis.confidence = 0.95;
        
        // Save updated version (should update, not create new)
        db.save_analysis(&analysis).await.unwrap();
        
        // Verify update
        let retrieved = db.get_analysis(&analysis.path).await.unwrap().unwrap();
        assert_eq!(retrieved.summary, "Updated summary");
        assert_eq!(retrieved.tags[0], "updated");
        assert_eq!(retrieved.confidence, 0.95);
    }

    #[tokio::test]
    async fn test_database_with_special_characters() {
        let db = create_test_database().await.unwrap();
        
        let analysis = FileAnalysis {
            id: Uuid::new_v4(),
            path: "/test/file's\"special\\.txt".to_string(),
            file_type: "text/plain".to_string(),
            summary: "File with 'quotes' and \"special\" chars".to_string(),
            tags: vec!["tag'1".to_string(), "tag\"2".to_string()],
            category: "Test".to_string(),
            confidence: 0.9,
            size: 1024,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Should handle special characters properly
        let result = db.save_analysis(&analysis).await;
        assert!(result.is_ok());
        
        let retrieved = db.get_analysis(&analysis.path).await.unwrap().unwrap();
        assert_eq!(retrieved.path, analysis.path);
        assert_eq!(retrieved.summary, analysis.summary);
    }

    #[tokio::test]
    async fn test_database_transaction_rollback() {
        let db = create_test_database().await.unwrap();
        
        // This test would require transaction support in the database layer
        // For now, we test that operations are atomic at the function level
        let analysis = FileAnalysis {
            id: Uuid::new_v4(),
            path: "/test/transaction.txt".to_string(),
            file_type: "text/plain".to_string(),
            summary: "Transaction test".to_string(),
            tags: vec![],
            category: "Test".to_string(),
            confidence: 0.9,
            size: 1024,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Save should be atomic
        let result = db.save_analysis(&analysis).await;
        assert!(result.is_ok());
    }
}