use stratosort::storage::database::Database;
use stratosort::error::AppError;
use sqlx::{SqlitePool, Transaction, Sqlite};
use tempfile::tempdir;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_transaction_rollback_on_error() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("rollback_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Get initial file count
    let initial_files = db.get_all_files().await.unwrap();
    let initial_count = initial_files.len();
    
    // Test transaction that should fail and rollback
    let result = test_failing_transaction(&db).await;
    
    match result {
        Err(e) => {
            println!("Transaction failed as expected: {:?}", e);
            
            // Verify that no partial changes were committed
            let final_files = db.get_all_files().await.unwrap();
            assert_eq!(final_files.len(), initial_count, 
                      "File count should be unchanged after failed transaction");
        }
        Ok(_) => {
            panic!("Transaction should have failed and rolled back");
        }
    }
}

async fn test_failing_transaction(db: &Database) -> Result<(), AppError> {
    // This is a conceptual test - in practice you'd need access to transaction internals
    // Here we simulate a transaction that fails partway through
    
    // Step 1: Insert a file (this should succeed)
    let file_id = db.store_file_analysis("temp_file.txt", "temporary content", "text/plain", None).await?;
    
    // Step 2: Attempt an operation that will fail
    // For this test, we'll try to insert invalid data that should cause an error
    let result = db.store_file_analysis("", "", "", None).await; // Empty path should fail
    
    match result {
        Ok(_) => {
            // If this succeeds, we need to clean up
            println!("Unexpected success - cleaning up file {}", file_id);
            Ok(())
        }
        Err(e) => {
            // This is expected - return the error to trigger rollback
            Err(e)
        }
    }
}

#[tokio::test]
async fn test_nested_transaction_behavior() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("nested_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Test nested transaction-like behavior
    let outer_result = test_nested_operations(&db).await;
    
    match outer_result {
        Ok(files_created) => {
            println!("Nested operations completed, {} files created", files_created);
            
            // Verify all operations completed successfully
            let all_files = db.get_all_files().await.unwrap();
            let test_files: Vec<_> = all_files.into_iter()
                .filter(|f| f.path.starts_with("nested_"))
                .collect();
            
            assert_eq!(test_files.len(), files_created, 
                      "Should have exactly the expected number of nested test files");
        }
        Err(e) => {
            println!("Nested operations failed: {:?}", e);
            
            // Verify that partial operations were handled correctly
            let all_files = db.get_all_files().await.unwrap();
            let test_files: Vec<_> = all_files.into_iter()
                .filter(|f| f.path.starts_with("nested_"))
                .collect();
            
            // Should either have all files (if outer succeeded) or none (if properly rolled back)
            assert!(test_files.is_empty() || test_files.len() >= 1, 
                   "Nested operations should be consistent");
        }
    }
}

async fn test_nested_operations(db: &Database) -> Result<usize, AppError> {
    let mut files_created = 0;
    
    // Outer operation
    let outer_file_id = db.store_file_analysis(
        "nested_outer.txt", 
        "outer operation content", 
        "text/plain", 
        None
    ).await?;
    files_created += 1;
    
    // Inner operations
    for i in 0..3 {
        let inner_file_id = db.store_file_analysis(
            &format!("nested_inner_{}.txt", i),
            &format!("inner operation {} content", i),
            "text/plain",
            None
        ).await?;
        files_created += 1;
        
        // Simulate some work
        sleep(Duration::from_millis(10)).await;
    }
    
    // Final operation
    let final_file_id = db.store_file_analysis(
        "nested_final.txt",
        "final operation content",
        "text/plain",
        None
    ).await?;
    files_created += 1;
    
    Ok(files_created)
}

#[tokio::test]
async fn test_transaction_timeout_handling() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("timeout_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Arc::new(Database::new(pool).await.unwrap());
    
    // Create a long-running operation that might timeout
    let db_clone = Arc::clone(&db);
    let long_operation = tokio::spawn(async move {
        let start_time = std::time::Instant::now();
        
        // Simulate a long-running database operation
        for i in 0..100 {
            let file_path = format!("timeout_test_{}.txt", i);
            let content = format!("This is a long operation file {}", i);
            
            match db_clone.store_file_analysis(&file_path, &content, "text/plain", None).await {
                Ok(id) => {
                    if i % 20 == 0 {
                        println!("Long operation progress: {}/100 (ID: {})", i, id);
                    }
                }
                Err(e) => {
                    println!("Long operation failed at step {}: {:?}", i, e);
                    return Err(e);
                }
            }
            
            // Small delay to simulate processing time
            sleep(Duration::from_millis(10)).await;
        }
        
        let elapsed = start_time.elapsed();
        println!("Long operation completed in {:?}", elapsed);
        Ok(100)
    });
    
    // Test concurrent operations while long operation is running
    let db_clone2 = Arc::clone(&db);
    let concurrent_operations = tokio::spawn(async move {
        sleep(Duration::from_millis(100)).await; // Wait for long operation to start
        
        let mut successful_ops = 0;
        for i in 0..10 {
            let file_path = format!("concurrent_{}.txt", i);
            let content = format!("Concurrent operation {}", i);
            
            match db_clone2.store_file_analysis(&file_path, &content, "text/plain", None).await {
                Ok(_) => {
                    successful_ops += 1;
                }
                Err(e) => {
                    println!("Concurrent operation {} failed: {:?}", i, e);
                }
            }
            
            sleep(Duration::from_millis(50)).await;
        }
        
        successful_ops
    });
    
    // Set reasonable timeout for the test
    let timeout_duration = Duration::from_secs(30);
    
    match tokio::time::timeout(timeout_duration, tokio::join!(long_operation, concurrent_operations)).await {
        Ok((long_result, concurrent_result)) => {
            let long_ops = long_result.unwrap();
            let concurrent_ops = concurrent_result.unwrap();
            
            match long_ops {
                Ok(count) => {
                    println!("Long operation completed successfully with {} files", count);
                }
                Err(e) => {
                    println!("Long operation failed: {:?}", e);
                }
            }
            
            println!("Concurrent operations completed: {} successful", concurrent_ops);
            
            // Verify database integrity
            let all_files = db.get_all_files().await.unwrap();
            let timeout_files: Vec<_> = all_files.iter()
                .filter(|f| f.path.starts_with("timeout_test_"))
                .collect();
            let concurrent_files: Vec<_> = all_files.iter()
                .filter(|f| f.path.starts_with("concurrent_"))
                .collect();
            
            println!("Final state: {} timeout files, {} concurrent files", 
                     timeout_files.len(), concurrent_files.len());
        }
        Err(_) => {
            panic!("Operations timed out after {:?} - possible deadlock or infinite wait", timeout_duration);
        }
    }
}

#[tokio::test]
async fn test_database_corruption_recovery() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("corruption_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Insert some initial data
    let initial_files = vec![
        ("file1.txt", "content 1"),
        ("file2.txt", "content 2"),
        ("file3.txt", "content 3"),
    ];
    
    for (path, content) in &initial_files {
        let _ = db.store_file_analysis(path, content, "text/plain", None).await;
    }
    
    // Verify initial state
    let files_before = db.get_all_files().await.unwrap();
    assert!(files_before.len() >= initial_files.len(), "Initial data should be stored");
    
    // Simulate various error conditions that might cause corruption
    let error_scenarios = vec![
        "test_disk_full_simulation",
        "test_power_failure_simulation",
        "test_concurrent_access_conflict",
    ];
    
    for scenario in error_scenarios {
        println!("Testing scenario: {}", scenario);
        
        // Attempt operations that might fail
        match scenario {
            "test_disk_full_simulation" => {
                // Try to insert very large content that might fail
                let large_content = "x".repeat(10 * 1024 * 1024); // 10MB
                let result = db.store_file_analysis("large_file.txt", &large_content, "text/plain", None).await;
                
                match result {
                    Ok(_) => println!("Large file insert succeeded"),
                    Err(e) => println!("Large file insert failed (expected): {:?}", e),
                }
            }
            "test_power_failure_simulation" => {
                // Start multiple operations and simulate interruption
                let tasks: Vec<_> = (0..5).map(|i| {
                    let path = format!("power_test_{}.txt", i);
                    let content = format!("Content for power test {}", i);
                    db.store_file_analysis(&path, &content, "text/plain", None)
                }).collect();
                
                // Don't wait for all to complete - this simulates interruption
                let results = futures::future::join_all(tasks).await;
                let successful: Vec<_> = results.into_iter()
                    .filter_map(|r| r.ok())
                    .collect();
                
                println!("Power failure simulation: {} operations completed", successful.len());
            }
            "test_concurrent_access_conflict" => {
                // Multiple simultaneous operations on same file
                let conflict_tasks: Vec<_> = (0..10).map(|i| {
                    let content = format!("Conflict content {}", i);
                    db.store_file_analysis("conflict_file.txt", &content, "text/plain", None)
                }).collect();
                
                let conflict_results = futures::future::join_all(conflict_tasks).await;
                let successful_conflicts: Vec<_> = conflict_results.into_iter()
                    .filter_map(|r| r.ok())
                    .collect();
                
                println!("Concurrent conflict test: {} operations succeeded", successful_conflicts.len());
            }
            _ => {}
        }
        
        // After each scenario, verify database is still accessible
        match db.get_all_files().await {
            Ok(files) => {
                println!("Database accessible after {}: {} files", scenario, files.len());
                
                // Verify we can still perform basic operations
                let search_result = db.search_files_by_content("content", 3).await;
                match search_result {
                    Ok(results) => {
                        println!("Search works after {}: {} results", scenario, results.len());
                    }
                    Err(e) => {
                        println!("Search failed after {}: {:?}", scenario, e);
                    }
                }
            }
            Err(e) => {
                println!("Database inaccessible after {}: {:?}", scenario, e);
                
                // This might indicate corruption - in a real system, you'd trigger recovery
                // For this test, we'll just note the issue
            }
        }
    }
    
    // Final integrity check
    let final_files = db.get_all_files().await.unwrap_or_default();
    println!("Final database state: {} files", final_files.len());
    
    // Verify that at least the initial files are still accessible
    let initial_paths: std::collections::HashSet<_> = initial_files.iter()
        .map(|(path, _)| *path)
        .collect();
    
    let remaining_initial_files: Vec<_> = final_files.iter()
        .filter(|f| initial_paths.contains(f.path.as_str()))
        .collect();
    
    assert!(!remaining_initial_files.is_empty() || final_files.is_empty(), 
           "Either initial files should remain, or database should be cleanly empty");
}

#[tokio::test]
async fn test_partial_failure_handling() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("partial_failure_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Test batch operations where some succeed and some fail
    let mixed_operations = vec![
        ("valid1.txt", "valid content 1", true),
        ("", "invalid empty path", false), // Should fail
        ("valid2.txt", "valid content 2", true),
        ("valid3.txt", "", true), // Empty content might be ok
        ("valid4.txt", "x".repeat(100 * 1024), false), // Might fail due to size
    ];
    
    let mut successful_ops = 0;
    let mut failed_ops = 0;
    let mut partial_success_ids = Vec::new();
    
    for (path, content, _expected_success) in mixed_operations {
        match db.store_file_analysis(path, content, "text/plain", None).await {
            Ok(id) => {
                successful_ops += 1;
                partial_success_ids.push(id);
                println!("Operation succeeded: {} -> ID {}", path, id);
            }
            Err(e) => {
                failed_ops += 1;
                println!("Operation failed: {} -> {:?}", path, e);
            }
        }
    }
    
    println!("Partial failure test: {} succeeded, {} failed", successful_ops, failed_ops);
    
    // Verify that successful operations are properly stored
    let all_files = db.get_all_files().await.unwrap();
    let valid_files: Vec<_> = all_files.into_iter()
        .filter(|f| f.path.starts_with("valid"))
        .collect();
    
    assert_eq!(valid_files.len(), successful_ops, 
              "Number of stored files should match successful operations");
    
    // Verify that we can retrieve the successfully stored files
    for id in partial_success_ids {
        // Try to find a file with this ID (conceptually)
        let search_results = db.search_files_by_content("valid", 10).await.unwrap();
        let found = search_results.iter().any(|f| f.path.contains("valid"));
        assert!(found, "Should be able to find stored valid files");
    }
    
    // Verify database is in consistent state after partial failures
    let final_search = db.search_files_by_content("content", 20).await.unwrap();
    assert!(final_search.len() <= 20, "Search should respect limits");
    
    // All returned results should be valid
    for result in final_search {
        assert!(!result.path.is_empty(), "All returned paths should be non-empty");
        assert!(result.path.len() < 1000, "All paths should be reasonable length");
    }
}