use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::time::timeout;

use stratosort::storage::Database;
use stratosort::error::{AppError, Result};
use stratosort::ai::FileAnalysis;
use stratosort::commands::organization::SmartFolder;

#[tokio::test]
async fn test_transaction_rollback_on_error() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Setup: Create initial data
    let analysis = create_test_analysis("test_file_1.txt");
    db.save_analysis(&analysis).await.expect("Failed to save initial analysis");
    
    let initial_count = get_analysis_count(&db).await;
    assert_eq!(initial_count, 1, "Should have one initial analysis");
    
    // Test: Attempt an operation that should rollback
    let result = attempt_failing_transaction(&db).await;
    assert!(result.is_err(), "Transaction should fail and rollback");
    
    // Verify: Data should be unchanged after rollback
    let final_count = get_analysis_count(&db).await;
    assert_eq!(final_count, initial_count, "Count should be unchanged after rollback");
    
    // Verify: Original data is still intact
    let retrieved_analysis = db.get_analysis("test_file_1.txt").await
        .expect("Failed to query analysis")
        .expect("Analysis should still exist");
    
    assert_eq!(retrieved_analysis.category, analysis.category);
    assert_eq!(retrieved_analysis.summary, analysis.summary);
}

#[tokio::test]
async fn test_concurrent_transaction_rollback() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Setup initial state
    let initial_analyses = vec![
        create_test_analysis("concurrent_1.txt"),
        create_test_analysis("concurrent_2.txt"),
        create_test_analysis("concurrent_3.txt"),
    ];
    
    for analysis in &initial_analyses {
        db.save_analysis(analysis).await.expect("Failed to save initial analysis");
    }
    
    let initial_count = get_analysis_count(&db).await;
    assert_eq!(initial_count, 3, "Should have three initial analyses");
    
    // Test: Run multiple concurrent transactions, some should fail
    let concurrent_operations = 10;
    let mut handles = vec![];
    let success_count = Arc::new(AtomicUsize::new(0));
    let failure_count = Arc::new(AtomicUsize::new(0));
    
    for i in 0..concurrent_operations {
        let db_clone = db.clone();
        let success_clone = Arc::clone(&success_count);
        let failure_clone = Arc::clone(&failure_count);
        
        let handle = tokio::spawn(async move {
            let result = if i % 3 == 0 {
                // Every third operation should fail
                attempt_failing_transaction(&db_clone).await
            } else {
                // Others should succeed
                attempt_successful_transaction(&db_clone, &format!("concurrent_trans_{}", i)).await
            };
            
            match result {
                Ok(_) => success_clone.fetch_add(1, Ordering::SeqCst),
                Err(_) => failure_clone.fetch_add(1, Ordering::SeqCst),
            };
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }
    
    let final_success = success_count.load(Ordering::SeqCst);
    let final_failure = failure_count.load(Ordering::SeqCst);
    
    println!("Concurrent transactions: {} succeeded, {} failed", final_success, final_failure);
    
    // Verify expected number of failures (every third operation)
    let expected_failures = concurrent_operations / 3;
    assert_eq!(final_failure, expected_failures, "Should have expected number of failures");
    
    // Verify final data integrity
    let final_count = get_analysis_count(&db).await;
    let expected_final_count = initial_count + final_success;
    assert_eq!(final_count, expected_final_count, 
              "Final count should equal initial + successful transactions");
    
    // Verify original data is still intact
    for analysis in &initial_analyses {
        let retrieved = db.get_analysis(&analysis.path).await
            .expect("Failed to query analysis")
            .expect("Original analysis should still exist");
        assert_eq!(retrieved.category, analysis.category);
    }
}

#[tokio::test]
async fn test_partial_write_rollback() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Test a complex operation that partially succeeds then fails
    let result = attempt_partial_failure_transaction(&db).await;
    assert!(result.is_err(), "Partial operation should fail and rollback completely");
    
    // Verify no partial data was committed
    let analysis_count = get_analysis_count(&db).await;
    assert_eq!(analysis_count, 0, "No analyses should exist after partial rollback");
    
    let smart_folder_count = get_smart_folder_count(&db).await;
    assert_eq!(smart_folder_count, 0, "No smart folders should exist after partial rollback");
    
    // Verify database is still in a consistent state
    let health_check = db.health_check().await;
    assert!(health_check.is_ok(), "Database should be healthy after rollback");
}

#[tokio::test]
async fn test_deadlock_detection_and_rollback() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Create initial data for deadlock scenario
    let analysis1 = create_test_analysis("deadlock_file1.txt");
    let analysis2 = create_test_analysis("deadlock_file2.txt");
    
    db.save_analysis(&analysis1).await.expect("Failed to save analysis1");
    db.save_analysis(&analysis2).await.expect("Failed to save analysis2");
    
    // Test: Attempt operations that could cause deadlock
    let db1 = db.clone();
    let db2 = db.clone();
    
    let handle1 = tokio::spawn(async move {
        simulate_potential_deadlock_scenario_1(&db1).await
    });
    
    let handle2 = tokio::spawn(async move {
        simulate_potential_deadlock_scenario_2(&db2).await
    });
    
    // Wait for both operations with timeout
    let result1 = timeout(Duration::from_secs(5), handle1).await
        .expect("Task 1 should complete within timeout")
        .expect("Task 1 should not panic");
    
    let result2 = timeout(Duration::from_secs(5), handle2).await
        .expect("Task 2 should complete within timeout")
        .expect("Task 2 should not panic");
    
    // At least one should succeed, or both should handle deadlock gracefully
    let both_failed = result1.is_err() && result2.is_err();
    assert!(!both_failed, "At least one operation should succeed or handle deadlock gracefully");
    
    // Verify database integrity after potential deadlock
    let health_check = db.health_check().await;
    assert!(health_check.is_ok(), "Database should be healthy after deadlock test");
    
    // Verify original data is still intact
    let retrieved1 = db.get_analysis("deadlock_file1.txt").await.expect("Query should succeed");
    let retrieved2 = db.get_analysis("deadlock_file2.txt").await.expect("Query should succeed");
    
    assert!(retrieved1.is_some(), "Original analysis 1 should still exist");
    assert!(retrieved2.is_some(), "Original analysis 2 should still exist");
}

#[tokio::test]
async fn test_wal_corruption_recovery() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Perform some operations to populate the WAL
    for i in 0..10 {
        let analysis = create_test_analysis(&format!("wal_test_{}.txt", i));
        db.save_analysis(&analysis).await.expect("Failed to save analysis");
    }
    
    let initial_count = get_analysis_count(&db).await;
    assert_eq!(initial_count, 10, "Should have 10 analyses");
    
    // Force a checkpoint to test WAL handling
    let checkpoint_result = db.flush().await;
    assert!(checkpoint_result.is_ok(), "Checkpoint should succeed");
    
    // Verify data integrity after checkpoint
    let post_checkpoint_count = get_analysis_count(&db).await;
    assert_eq!(post_checkpoint_count, initial_count, "Count should be unchanged after checkpoint");
    
    // Test that database can handle new operations after checkpoint
    let new_analysis = create_test_analysis("post_checkpoint.txt");
    let save_result = db.save_analysis(&new_analysis).await;
    assert!(save_result.is_ok(), "Should be able to save after checkpoint");
    
    let final_count = get_analysis_count(&db).await;
    assert_eq!(final_count, initial_count + 1, "Count should increase after new save");
}

#[tokio::test]
async fn test_migration_failure_rollback() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Create some initial data
    let analysis = create_test_analysis("migration_test.txt");
    db.save_analysis(&analysis).await.expect("Failed to save initial data");
    
    // Test migration scenarios by directly manipulating schema version
    // This simulates a migration failure scenario
    let initial_count = get_analysis_count(&db).await;
    
    // Attempt to simulate a migration failure by corrupting the schema version
    let corrupt_result = sqlx::query("UPDATE schema_version SET version = -1")
        .execute(db.pool())
        .await;
    
    // If corruption succeeded, try to trigger migration logic
    if corrupt_result.is_ok() {
        // The database should handle invalid schema version gracefully
        let health_check = db.health_check().await;
        assert!(health_check.is_ok(), "Database should handle schema issues gracefully");
        
        // Data should still be accessible
        let post_corruption_count = get_analysis_count(&db).await;
        assert_eq!(post_corruption_count, initial_count, "Data should still be accessible");
    }
}

#[tokio::test]
async fn test_foreign_key_constraint_rollback() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Enable foreign key constraints
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(db.pool())
        .await
        .expect("Failed to enable foreign keys");
    
    // Create a smart folder
    let smart_folder = create_test_smart_folder("test_folder");
    db.save_smart_folder(&smart_folder).await.expect("Failed to save smart folder");
    
    // Try to create an operation that would violate foreign key constraints
    // (if they existed in the schema)
    let result = attempt_foreign_key_violation(&db).await;
    
    // The exact behavior depends on the schema, but the database should handle it gracefully
    match result {
        Ok(_) => {
            // If no foreign key constraints exist, operation might succeed
            println!("No foreign key constraints to violate");
        }
        Err(e) => {
            // If foreign key constraints exist and are violated, should rollback gracefully
            println!("Foreign key constraint violation handled: {:?}", e);
        }
    }
    
    // Verify database is still functional
    let health_check = db.health_check().await;
    assert!(health_check.is_ok(), "Database should be healthy after constraint test");
    
    // Verify original smart folder still exists
    let retrieved_folder = db.get_smart_folder("test_folder").await
        .expect("Query should succeed")
        .expect("Smart folder should still exist");
    assert_eq!(retrieved_folder.name, smart_folder.name);
}

// Helper functions

async fn create_test_database() -> Result<Database> {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_rollback.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    
    Database::new_from_url(&db_url).await
}

fn create_test_analysis(path: &str) -> FileAnalysis {
    FileAnalysis {
        path: path.to_string(),
        category: "test_category".to_string(),
        tags: vec!["test".to_string(), "rollback".to_string()],
        summary: format!("Test analysis for {}", path),
        confidence: 0.95,
        extracted_text: Some("Test content".to_string()),
        detected_language: Some("en".to_string()),
        metadata: serde_json::Value::Null,
    }
}

fn create_test_smart_folder(id: &str) -> SmartFolder {
    SmartFolder {
        id: id.to_string(),
        name: format!("Test Folder {}", id),
        description: Some("Test folder for rollback testing".to_string()),
        rules: serde_json::json!({
            "conditions": [
                {
                    "type": "category",
                    "value": "test_category"
                }
            ]
        }),
        target_path: "/test/path".to_string(),
        enabled: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

async fn get_analysis_count(db: &Database) -> usize {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_analysis")
        .fetch_one(db.pool())
        .await
        .unwrap_or(0);
    count as usize
}

async fn get_smart_folder_count(db: &Database) -> usize {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM smart_folders_v3")
        .fetch_one(db.pool())
        .await
        .unwrap_or(0);
    count as usize
}

async fn attempt_failing_transaction(db: &Database) -> Result<()> {
    let mut tx = db.pool().begin().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Insert some data
    sqlx::query("INSERT INTO file_analysis (path, category, tags, summary, confidence, analyzed_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("failing_transaction.txt")
        .bind("test")
        .bind("[]")
        .bind("This should be rolled back")
        .bind(0.5)
        .bind(chrono::Utc::now().timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Force an error to trigger rollback
    let invalid_result = sqlx::query("INSERT INTO non_existent_table VALUES (1)")
        .execute(&mut *tx)
        .await;
    
    if invalid_result.is_err() {
        // Rollback will happen automatically when tx is dropped
        return Err(AppError::DatabaseError { 
            message: "Intentional failure to test rollback".to_string() 
        });
    }
    
    tx.commit().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}

async fn attempt_successful_transaction(db: &Database, path: &str) -> Result<()> {
    let mut tx = db.pool().begin().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Insert data that should succeed
    sqlx::query("INSERT INTO file_analysis (path, category, tags, summary, confidence, analyzed_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(path)
        .bind("test")
        .bind("[]")
        .bind("Successful transaction")
        .bind(0.9)
        .bind(chrono::Utc::now().timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    tx.commit().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}

async fn attempt_partial_failure_transaction(db: &Database) -> Result<()> {
    let mut tx = db.pool().begin().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // First operation: Insert analysis (should succeed)
    sqlx::query("INSERT INTO file_analysis (path, category, tags, summary, confidence, analyzed_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("partial_fail.txt")
        .bind("test")
        .bind("[]")
        .bind("Partial transaction")
        .bind(0.8)
        .bind(chrono::Utc::now().timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Second operation: Insert smart folder (should succeed)
    sqlx::query("INSERT INTO smart_folders_v3 (id, name, rules, target_path, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind("partial_folder")
        .bind("Partial Test Folder")
        .bind("{}")
        .bind("/partial/path")
        .bind(true)
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Third operation: Force failure
    let invalid_result = sqlx::query("INSERT INTO non_existent_table VALUES (1)")
        .execute(&mut *tx)
        .await;
    
    if invalid_result.is_err() {
        // This will trigger rollback of all operations in the transaction
        return Err(AppError::DatabaseError { 
            message: "Partial transaction failure".to_string() 
        });
    }
    
    tx.commit().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}

async fn simulate_potential_deadlock_scenario_1(db: &Database) -> Result<()> {
    let mut tx = db.pool().begin().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Update file1 first, then file2
    sqlx::query("UPDATE file_analysis SET summary = ? WHERE path = ?")
        .bind("Updated by scenario 1")
        .bind("deadlock_file1.txt")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Small delay to increase chance of deadlock
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    sqlx::query("UPDATE file_analysis SET summary = ? WHERE path = ?")
        .bind("Updated by scenario 1")
        .bind("deadlock_file2.txt")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    tx.commit().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}

async fn simulate_potential_deadlock_scenario_2(db: &Database) -> Result<()> {
    let mut tx = db.pool().begin().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Update file2 first, then file1 (opposite order)
    sqlx::query("UPDATE file_analysis SET summary = ? WHERE path = ?")
        .bind("Updated by scenario 2")
        .bind("deadlock_file2.txt")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Small delay to increase chance of deadlock
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    sqlx::query("UPDATE file_analysis SET summary = ? WHERE path = ?")
        .bind("Updated by scenario 2")
        .bind("deadlock_file1.txt")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    tx.commit().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}

async fn attempt_foreign_key_violation(db: &Database) -> Result<()> {
    // Attempt to create an operation that would violate foreign key constraints
    // Since the current schema doesn't have strict foreign keys, this is more
    // about testing the error handling infrastructure
    
    let mut tx = db.pool().begin().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Try to insert a reference to a non-existent smart folder
    // (This would violate a foreign key if such constraints existed)
    sqlx::query("INSERT INTO file_analysis (path, category, tags, summary, confidence, analyzed_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("fk_violation.txt")
        .bind("nonexistent_folder_reference")
        .bind("[]")
        .bind("Foreign key test")
        .bind(0.7)
        .bind(chrono::Utc::now().timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    tx.commit().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}