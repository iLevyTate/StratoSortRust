use stratosort::core::{UndoRedoManager, Operation, OperationType};
use stratosort::storage::Database;
use stratosort::error::{AppError, Result};
use tauri::test::{mock_app, mock_context};
use tempfile::tempdir;
use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;

#[cfg(test)]
mod undo_redo_tests {
    use super::*;

    async fn create_test_manager() -> Arc<UndoRedoManager> {
        let app = mock_app(mock_context());
        let db = Arc::new(Database::new(&app.handle()).await.unwrap());
        Arc::new(UndoRedoManager::new(db))
    }

    async fn create_move_operation(from: &str, to: &str) -> Operation {
        Operation {
            id: Uuid::new_v4(),
            operation_type: OperationType::Move,
            source_path: from.to_string(),
            destination_path: Some(to.to_string()),
            metadata: None,
            timestamp: chrono::Utc::now(),
            user_id: Some("test_user".to_string()),
        }
    }

    async fn create_delete_operation(path: &str) -> Operation {
        Operation {
            id: Uuid::new_v4(),
            operation_type: OperationType::Delete,
            source_path: path.to_string(),
            destination_path: None,
            metadata: None,
            timestamp: chrono::Utc::now(),
            user_id: Some("test_user".to_string()),
        }
    }

    async fn create_rename_operation(from: &str, to: &str) -> Operation {
        Operation {
            id: Uuid::new_v4(),
            operation_type: OperationType::Rename,
            source_path: from.to_string(),
            destination_path: Some(to.to_string()),
            metadata: None,
            timestamp: chrono::Utc::now(),
            user_id: Some("test_user".to_string()),
        }
    }

    // Basic Operation Recording
    #[tokio::test]
    async fn test_record_move_operation() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        std::fs::write(&source, "content").unwrap();
        
        let op = create_move_operation(
            &source.to_string_lossy(),
            &dest.to_string_lossy()
        ).await;
        
        let result = manager.record_operation(op.clone()).await;
        assert!(result.is_ok());
        
        let recorded = result.unwrap();
        assert_eq!(recorded.source_path, op.source_path);
        assert_eq!(recorded.destination_path, op.destination_path);
    }

    #[tokio::test]
    async fn test_record_delete_operation() {
        let manager = create_test_manager().await;
        
        let op = create_delete_operation("/test/file.txt").await;
        
        let result = manager.record_operation(op.clone()).await;
        assert!(result.is_ok());
        
        let recorded = result.unwrap();
        assert_eq!(recorded.operation_type, OperationType::Delete);
        assert!(recorded.destination_path.is_none());
    }

    #[tokio::test]
    async fn test_record_rename_operation() {
        let manager = create_test_manager().await;
        
        let op = create_rename_operation(
            "/test/old_name.txt",
            "/test/new_name.txt"
        ).await;
        
        let result = manager.record_operation(op).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_record_with_metadata() {
        let manager = create_test_manager().await;
        
        let mut op = create_move_operation("/from.txt", "/to.txt").await;
        op.metadata = Some(serde_json::json!({
            "file_size": 1024,
            "mime_type": "text/plain",
            "reason": "user_organized"
        }));
        
        let result = manager.record_operation(op.clone()).await;
        assert!(result.is_ok());
        
        let recorded = result.unwrap();
        assert!(recorded.metadata.is_some());
        
        let metadata = recorded.metadata.unwrap();
        assert_eq!(metadata["file_size"], 1024);
        assert_eq!(metadata["mime_type"], "text/plain");
    }

    // Undo Operations
    #[tokio::test]
    async fn test_undo_move_operation() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        
        // Create source file and move it
        std::fs::write(&source, "content").unwrap();
        std::fs::rename(&source, &dest).unwrap();
        
        // Record the move operation
        let op = create_move_operation(
            &source.to_string_lossy(),
            &dest.to_string_lossy()
        ).await;
        let recorded = manager.record_operation(op).await.unwrap();
        
        // Undo the operation
        let result = manager.undo_operation(recorded.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // File should be back at source
        assert!(source.exists());
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn test_undo_rename_operation() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let old_path = temp_dir.path().join("old_name.txt");
        let new_path = temp_dir.path().join("new_name.txt");
        
        // Create file with new name (simulating completed rename)
        std::fs::write(&new_path, "content").unwrap();
        
        // Record the rename operation
        let op = create_rename_operation(
            &old_path.to_string_lossy(),
            &new_path.to_string_lossy()
        ).await;
        let recorded = manager.record_operation(op).await.unwrap();
        
        // Undo the rename
        let result = manager.undo_operation(recorded.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // File should have old name
        assert!(old_path.exists());
        assert!(!new_path.exists());
    }

    #[tokio::test]
    async fn test_undo_nonexistent_operation() {
        let manager = create_test_manager().await;
        
        let result = manager.undo_operation(Uuid::new_v4()).await;
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false
    }

    #[tokio::test]
    async fn test_undo_already_undone_operation() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        
        std::fs::write(&source, "content").unwrap();
        std::fs::rename(&source, &dest).unwrap();
        
        let op = create_move_operation(
            &source.to_string_lossy(),
            &dest.to_string_lossy()
        ).await;
        let recorded = manager.record_operation(op).await.unwrap();
        
        // First undo should succeed
        assert!(manager.undo_operation(recorded.id).await.unwrap());
        
        // Second undo should fail (already undone)
        assert!(!manager.undo_operation(recorded.id).await.unwrap());
    }

    // Redo Operations
    #[tokio::test]
    async fn test_redo_move_operation() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        
        std::fs::write(&source, "content").unwrap();
        std::fs::rename(&source, &dest).unwrap();
        
        let op = create_move_operation(
            &source.to_string_lossy(),
            &dest.to_string_lossy()
        ).await;
        let recorded = manager.record_operation(op).await.unwrap();
        
        // Undo first
        manager.undo_operation(recorded.id).await.unwrap();
        assert!(source.exists());
        assert!(!dest.exists());
        
        // Now redo
        let result = manager.redo_operation(recorded.id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // File should be at destination again
        assert!(!source.exists());
        assert!(dest.exists());
    }

    #[tokio::test]
    async fn test_redo_without_undo() {
        let manager = create_test_manager().await;
        
        let op = create_move_operation("/from.txt", "/to.txt").await;
        let recorded = manager.record_operation(op).await.unwrap();
        
        // Try to redo without undoing first
        let result = manager.redo_operation(recorded.id).await;
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false
    }

    // History Management
    #[tokio::test]
    async fn test_get_operation_history() {
        let manager = create_test_manager().await;
        
        // Record multiple operations
        for i in 0..5 {
            let op = create_move_operation(
                &format!("/from{}.txt", i),
                &format!("/to{}.txt", i)
            ).await;
            manager.record_operation(op).await.unwrap();
        }
        
        let history = manager.get_history(10, 0).await.unwrap();
        assert_eq!(history.len(), 5);
        
        // Should be ordered by timestamp (newest first)
        for i in 0..4 {
            assert!(history[i].timestamp >= history[i+1].timestamp);
        }
    }

    #[tokio::test]
    async fn test_get_history_with_pagination() {
        let manager = create_test_manager().await;
        
        // Record 10 operations
        for i in 0..10 {
            let op = create_move_operation(
                &format!("/from{}.txt", i),
                &format!("/to{}.txt", i)
            ).await;
            manager.record_operation(op).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        // Get first page
        let page1 = manager.get_history(5, 0).await.unwrap();
        assert_eq!(page1.len(), 5);
        
        // Get second page
        let page2 = manager.get_history(5, 5).await.unwrap();
        assert_eq!(page2.len(), 5);
        
        // Ensure no overlap
        for op1 in &page1 {
            for op2 in &page2 {
                assert_ne!(op1.id, op2.id);
            }
        }
    }

    #[tokio::test]
    async fn test_get_user_specific_history() {
        let manager = create_test_manager().await;
        
        // Record operations for different users
        for i in 0..3 {
            let mut op = create_move_operation(
                &format!("/user1_from{}.txt", i),
                &format!("/user1_to{}.txt", i)
            ).await;
            op.user_id = Some("user1".to_string());
            manager.record_operation(op).await.unwrap();
        }
        
        for i in 0..2 {
            let mut op = create_move_operation(
                &format!("/user2_from{}.txt", i),
                &format!("/user2_to{}.txt", i)
            ).await;
            op.user_id = Some("user2".to_string());
            manager.record_operation(op).await.unwrap();
        }
        
        let user1_history = manager.get_user_history("user1", 10, 0).await.unwrap();
        assert_eq!(user1_history.len(), 3);
        
        let user2_history = manager.get_user_history("user2", 10, 0).await.unwrap();
        assert_eq!(user2_history.len(), 2);
        
        // Verify correct user filtering
        for op in user1_history {
            assert_eq!(op.user_id.unwrap(), "user1");
        }
    }

    #[tokio::test]
    async fn test_clear_history() {
        let manager = create_test_manager().await;
        
        // Record some operations
        for i in 0..5 {
            let op = create_move_operation(
                &format!("/from{}.txt", i),
                &format!("/to{}.txt", i)
            ).await;
            manager.record_operation(op).await.unwrap();
        }
        
        // Verify history exists
        let history = manager.get_history(10, 0).await.unwrap();
        assert_eq!(history.len(), 5);
        
        // Clear history
        let result = manager.clear_history().await;
        assert!(result.is_ok());
        
        // Verify history is empty
        let history_after = manager.get_history(10, 0).await.unwrap();
        assert!(history_after.is_empty());
    }

    #[tokio::test]
    async fn test_clear_old_operations() {
        let manager = create_test_manager().await;
        
        // Record old operations
        for i in 0..3 {
            let mut op = create_move_operation(
                &format!("/old{}.txt", i),
                &format!("/old_dest{}.txt", i)
            ).await;
            op.timestamp = chrono::Utc::now() - chrono::Duration::days(40);
            manager.record_operation(op).await.unwrap();
        }
        
        // Record recent operations
        for i in 0..2 {
            let op = create_move_operation(
                &format!("/recent{}.txt", i),
                &format!("/recent_dest{}.txt", i)
            ).await;
            manager.record_operation(op).await.unwrap();
        }
        
        // Clear operations older than 30 days
        let result = manager.clear_old_operations(30).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3); // Should have deleted 3 old operations
        
        // Verify only recent operations remain
        let history = manager.get_history(10, 0).await.unwrap();
        assert_eq!(history.len(), 2);
    }

    // Complex Scenarios
    #[tokio::test]
    async fn test_undo_redo_sequence() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create initial files
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        let file1_new = temp_dir.path().join("file1_moved.txt");
        let file2_new = temp_dir.path().join("file2_moved.txt");
        
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();
        
        // Perform and record moves
        std::fs::rename(&file1, &file1_new).unwrap();
        let op1 = create_move_operation(
            &file1.to_string_lossy(),
            &file1_new.to_string_lossy()
        ).await;
        let rec1 = manager.record_operation(op1).await.unwrap();
        
        std::fs::rename(&file2, &file2_new).unwrap();
        let op2 = create_move_operation(
            &file2.to_string_lossy(),
            &file2_new.to_string_lossy()
        ).await;
        let rec2 = manager.record_operation(op2).await.unwrap();
        
        // Undo second operation
        manager.undo_operation(rec2.id).await.unwrap();
        assert!(file2.exists());
        assert!(!file2_new.exists());
        assert!(file1_new.exists()); // First move still in effect
        
        // Undo first operation
        manager.undo_operation(rec1.id).await.unwrap();
        assert!(file1.exists());
        assert!(!file1_new.exists());
        
        // Redo first operation
        manager.redo_operation(rec1.id).await.unwrap();
        assert!(!file1.exists());
        assert!(file1_new.exists());
        
        // Redo second operation
        manager.redo_operation(rec2.id).await.unwrap();
        assert!(!file2.exists());
        assert!(file2_new.exists());
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let manager = create_test_manager().await;
        
        // Spawn multiple tasks recording operations
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let manager_clone = manager.clone();
                tokio::spawn(async move {
                    let op = create_move_operation(
                        &format!("/from{}.txt", i),
                        &format!("/to{}.txt", i)
                    ).await;
                    manager_clone.record_operation(op).await
                })
            })
            .collect();
        
        // Wait for all to complete
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
        
        // Verify all operations were recorded
        let history = manager.get_history(20, 0).await.unwrap();
        assert_eq!(history.len(), 10);
    }

    #[tokio::test]
    async fn test_undo_with_missing_files() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        
        // Record operation without actual files
        let op = create_move_operation(
            &source.to_string_lossy(),
            &dest.to_string_lossy()
        ).await;
        let recorded = manager.record_operation(op).await.unwrap();
        
        // Try to undo - should handle missing files gracefully
        let result = manager.undo_operation(recorded.id).await;
        assert!(result.is_err() || !result.unwrap());
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let manager = create_test_manager().await;
        let temp_dir = tempdir().unwrap();
        
        // Create batch of related operations
        let batch_id = Uuid::new_v4();
        let mut operations = vec![];
        
        for i in 0..5 {
            let source = temp_dir.path().join(format!("batch_file{}.txt", i));
            let dest = temp_dir.path().join(format!("organized/file{}.txt", i));
            
            std::fs::write(&source, format!("content{}", i)).unwrap();
            
            let mut op = create_move_operation(
                &source.to_string_lossy(),
                &dest.to_string_lossy()
            ).await;
            
            // Add batch metadata
            op.metadata = Some(serde_json::json!({
                "batch_id": batch_id.to_string(),
                "batch_index": i,
                "batch_total": 5
            }));
            
            operations.push(manager.record_operation(op).await.unwrap());
        }
        
        // Get operations by batch
        let history = manager.get_history(10, 0).await.unwrap();
        let batch_ops: Vec<_> = history
            .iter()
            .filter(|op| {
                op.metadata.as_ref()
                    .and_then(|m| m.get("batch_id"))
                    .and_then(|id| id.as_str())
                    .map(|id| id == batch_id.to_string())
                    .unwrap_or(false)
            })
            .collect();
        
        assert_eq!(batch_ops.len(), 5);
    }

    #[tokio::test]
    async fn test_operation_with_large_metadata() {
        let manager = create_test_manager().await;
        
        let mut op = create_move_operation("/from.txt", "/to.txt").await;
        
        // Create large metadata
        let large_data = vec!["x".to_string(); 1000];
        op.metadata = Some(serde_json::json!({
            "large_array": large_data,
            "nested": {
                "deeply": {
                    "nested": {
                        "data": "value"
                    }
                }
            }
        }));
        
        let result = manager.record_operation(op.clone()).await;
        assert!(result.is_ok());
        
        let recorded = result.unwrap();
        assert!(recorded.metadata.is_some());
        
        let metadata = recorded.metadata.unwrap();
        assert!(metadata["large_array"].as_array().unwrap().len() == 1000);
    }

    #[tokio::test]
    async fn test_history_ordering_with_concurrent_undos() {
        let manager = create_test_manager().await;
        
        // Record multiple operations
        let mut ops = vec![];
        for i in 0..5 {
            let op = create_move_operation(
                &format!("/from{}.txt", i),
                &format!("/to{}.txt", i)
            ).await;
            ops.push(manager.record_operation(op).await.unwrap());
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        // Undo some operations
        manager.undo_operation(ops[1].id).await.unwrap();
        manager.undo_operation(ops[3].id).await.unwrap();
        
        // Get history and verify ordering is maintained
        let history = manager.get_history(10, 0).await.unwrap();
        assert_eq!(history.len(), 5);
        
        // Check undo status
        let undone_ops: Vec<_> = history
            .iter()
            .filter(|op| op.id == ops[1].id || op.id == ops[3].id)
            .collect();
        
        assert_eq!(undone_ops.len(), 2);
    }
}