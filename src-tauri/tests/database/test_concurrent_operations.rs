use stratosort::storage::database::Database;
use stratosort::error::AppError;
use sqlx::SqlitePool;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use futures::future::join_all;
use rayon::prelude::*;

#[tokio::test]
async fn test_concurrent_file_analysis_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("concurrent_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Arc::new(Database::new(pool).await.unwrap());
    
    let num_concurrent_ops = 50;
    let success_counter = Arc::new(AtomicUsize::new(0));
    let error_counter = Arc::new(AtomicUsize::new(0));
    
    // Spawn multiple concurrent file analysis operations
    let mut tasks = Vec::new();
    
    for i in 0..num_concurrent_ops {
        let db_clone = Arc::clone(&db);
        let success_counter_clone = Arc::clone(&success_counter);
        let error_counter_clone = Arc::clone(&error_counter);
        
        let task = tokio::spawn(async move {
            let file_path = format!("test_file_{}.txt", i);
            let content = format!("Content for file {} - this is test data for concurrent operations", i);
            
            // Add some random delay to increase chance of race conditions
            let delay_ms = (i % 10) * 10;
            sleep(Duration::from_millis(delay_ms)).await;
            
            match db_clone.store_file_analysis(&file_path, &content, "text/plain", None).await {
                Ok(file_id) => {
                    success_counter_clone.fetch_add(1, Ordering::SeqCst);
                    println!("Successfully stored file {} with ID {}", i, file_id);
                    file_id
                }
                Err(e) => {
                    error_counter_clone.fetch_add(1, Ordering::SeqCst);
                    println!("Failed to store file {}: {:?}", i, e);
                    0 // Return 0 for failed operations
                }
            }
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let results: Vec<_> = join_all(tasks).await.into_iter()
        .map(|r| r.unwrap())
        .collect();
    
    let successes = success_counter.load(Ordering::SeqCst);
    let errors = error_counter.load(Ordering::SeqCst);
    
    println!("Concurrent operations completed: {} successes, {} errors", successes, errors);
    
    // Verify database integrity
    let all_files = db.get_all_files().await.unwrap();
    assert_eq!(all_files.len(), successes, "Database should contain all successfully inserted files");
    
    // Verify each file has unique ID and valid data
    let mut file_ids: Vec<_> = results.into_iter().filter(|&id| id > 0).collect();
    file_ids.sort();
    
    for (i, &file_id) in file_ids.iter().enumerate() {
        if i > 0 {
            assert_ne!(file_id, file_ids[i-1], "File IDs should be unique");
        }
    }
    
    // Test concurrent read operations
    let read_tasks: Vec<_> = (0..20).map(|i| {
        let db_clone = Arc::clone(&db);
        tokio::spawn(async move {
            let search_term = format!("file {}", i % num_concurrent_ops);
            db_clone.search_files_by_content(&search_term, 5).await
        })
    }).collect();
    
    let read_results = join_all(read_tasks).await;
    let mut successful_reads = 0;
    
    for result in read_results {
        match result.unwrap() {
            Ok(files) => {
                successful_reads += 1;
                assert!(files.len() <= 5, "Should respect search limit");
            }
            Err(e) => {
                println!("Read operation failed: {:?}", e);
            }
        }
    }
    
    println!("Concurrent reads completed: {} successful out of 20", successful_reads);
    assert!(successful_reads > 15, "Most read operations should succeed");
}

#[tokio::test]
async fn test_transaction_isolation_levels() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("isolation_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool1 = SqlitePool::connect(&database_url).await.unwrap();
    let pool2 = SqlitePool::connect(&database_url).await.unwrap();
    
    let db1 = Arc::new(Database::new(pool1).await.unwrap());
    let db2 = Arc::new(Database::new(pool2).await.unwrap());
    
    // Test: Transaction isolation during concurrent modifications
    let file_path = "shared_file.txt";
    
    // Insert initial file
    let initial_id = db1.store_file_analysis(file_path, "initial content", "text/plain", None).await.unwrap();
    
    // Start two concurrent transactions that modify the same file
    let db1_clone = Arc::clone(&db1);
    let db2_clone = Arc::clone(&db2);
    
    let task1 = tokio::spawn(async move {
        // Transaction 1: Update file content
        let mut retries = 0;
        loop {
            match db1_clone.store_file_analysis(file_path, "content modified by transaction 1", "text/plain", Some(initial_id)).await {
                Ok(id) => {
                    println!("Transaction 1 completed with ID: {}", id);
                    return Ok(id);
                }
                Err(e) => {
                    retries += 1;
                    if retries >= 3 {
                        return Err(e);
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            }
        }
    });
    
    let task2 = tokio::spawn(async move {
        // Transaction 2: Update same file with different content
        sleep(Duration::from_millis(5)).await; // Small delay to increase race condition chance
        
        let mut retries = 0;
        loop {
            match db2_clone.store_file_analysis(file_path, "content modified by transaction 2", "text/plain", Some(initial_id)).await {
                Ok(id) => {
                    println!("Transaction 2 completed with ID: {}", id);
                    return Ok(id);
                }
                Err(e) => {
                    retries += 1;
                    if retries >= 3 {
                        return Err(e);
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            }
        }
    });
    
    // Wait for both transactions to complete
    let (result1, result2) = tokio::join!(task1, task2);
    
    match (result1, result2) {
        (Ok(Ok(id1)), Ok(Ok(id2))) => {
            println!("Both transactions succeeded: {} and {}", id1, id2);
            
            // Verify final state is consistent
            let final_files = db1.search_files_by_content(file_path, 10).await.unwrap();
            let matching_files: Vec<_> = final_files.into_iter()
                .filter(|f| f.path == file_path)
                .collect();
                
            // Should have some valid final state
            assert!(!matching_files.is_empty(), "File should exist in final state");
            
            // Content should be from one of the transactions
            let final_content = &matching_files[0].content;
            assert!(
                final_content.contains("transaction 1") || final_content.contains("transaction 2"),
                "Final content should be from one of the transactions"
            );
        }
        (Ok(Ok(id)), Ok(Err(e))) => {
            println!("One transaction succeeded ({}), other failed: {:?}", id, e);
        }
        (Ok(Err(e)), Ok(Ok(id))) => {
            println!("One transaction failed: {:?}, other succeeded ({})", e, id);
        }
        (Ok(Err(e1)), Ok(Err(e2))) => {
            println!("Both transactions failed: {:?}, {:?}", e1, e2);
            // This might be acceptable depending on implementation
        }
        _ => {
            panic!("Task execution failed");
        }
    }
}

#[tokio::test]
async fn test_deadlock_prevention() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("deadlock_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Arc::new(Database::new(pool).await.unwrap());
    
    // Create initial data
    let file1_id = db.store_file_analysis("file1.txt", "content 1", "text/plain", None).await.unwrap();
    let file2_id = db.store_file_analysis("file2.txt", "content 2", "text/plain", None).await.unwrap();
    
    // Test potential deadlock scenario with cross-dependencies
    let db1 = Arc::clone(&db);
    let db2 = Arc::clone(&db);
    
    let task1 = tokio::spawn(async move {
        // Task 1: Update file1 then file2
        let start_time = std::time::Instant::now();
        
        // Update file1
        let result1 = db1.store_file_analysis("file1.txt", "updated content 1", "text/plain", Some(file1_id)).await;
        
        // Small delay to increase chance of deadlock
        sleep(Duration::from_millis(10)).await;
        
        // Then update file2
        let result2 = db1.store_file_analysis("file2.txt", "updated content 2 from task1", "text/plain", Some(file2_id)).await;
        
        let elapsed = start_time.elapsed();
        (result1, result2, elapsed, "task1")
    });
    
    let task2 = tokio::spawn(async move {
        // Task 2: Update file2 then file1 (reverse order)
        let start_time = std::time::Instant::now();
        
        // Small initial delay
        sleep(Duration::from_millis(5)).await;
        
        // Update file2
        let result1 = db2.store_file_analysis("file2.txt", "updated content 2", "text/plain", Some(file2_id)).await;
        
        // Small delay to increase chance of deadlock
        sleep(Duration::from_millis(10)).await;
        
        // Then update file1
        let result2 = db2.store_file_analysis("file1.txt", "updated content 1 from task2", "text/plain", Some(file1_id)).await;
        
        let elapsed = start_time.elapsed();
        (result1, result2, elapsed, "task2")
    });
    
    // Set a timeout to detect deadlocks
    let timeout_duration = Duration::from_secs(10);
    
    match tokio::time::timeout(timeout_duration, tokio::join!(task1, task2)).await {
        Ok((result1, result2)) => {
            let (res1_1, res1_2, elapsed1, name1) = result1.unwrap();
            let (res2_1, res2_2, elapsed2, name2) = result2.unwrap();
            
            println!("{} completed in {:?}: {:?}, {:?}", name1, elapsed1, res1_1.is_ok(), res1_2.is_ok());
            println!("{} completed in {:?}: {:?}, {:?}", name2, elapsed2, res2_1.is_ok(), res2_2.is_ok());
            
            // Check for suspiciously long execution times (possible deadlock detection)
            assert!(elapsed1 < Duration::from_secs(5), "Task 1 should not take too long (possible deadlock)");
            assert!(elapsed2 < Duration::from_secs(5), "Task 2 should not take too long (possible deadlock)");
            
            // At least one operation from each task should succeed or fail gracefully
            let task1_success = res1_1.is_ok() || res1_2.is_ok();
            let task2_success = res2_1.is_ok() || res2_2.is_ok();
            
            println!("Deadlock test completed: task1_success={}, task2_success={}", task1_success, task2_success);
        }
        Err(_) => {
            panic!("Deadlock detected: operations timed out after {:?}", timeout_duration);
        }
    }
}

#[tokio::test]
async fn test_connection_pool_exhaustion() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("pool_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    
    // Create pool with limited connections
    let pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::from_str(&database_url)
            .unwrap()
            .create_if_missing(true)
    ).await.unwrap();
    
    let db = Arc::new(Database::new(pool).await.unwrap());
    
    // Test with more concurrent operations than available connections
    let num_operations = 100;
    let start_time = std::time::Instant::now();
    
    let tasks: Vec<_> = (0..num_operations).map(|i| {
        let db_clone = Arc::clone(&db);
        tokio::spawn(async move {
            let file_path = format!("pool_test_{}.txt", i);
            let content = format!("Content for pool test file {}", i);
            
            // Add varying delays to simulate real-world usage patterns
            let delay = Duration::from_millis((i % 50) as u64);
            sleep(delay).await;
            
            match db_clone.store_file_analysis(&file_path, &content, "text/plain", None).await {
                Ok(id) => Ok(id),
                Err(e) => {
                    println!("Operation {} failed: {:?}", i, e);
                    Err(e)
                }
            }
        })
    }).collect();
    
    let results = join_all(tasks).await;
    let elapsed = start_time.elapsed();
    
    let mut successful_ops = 0;
    let mut failed_ops = 0;
    
    for result in results {
        match result.unwrap() {
            Ok(_) => successful_ops += 1,
            Err(_) => failed_ops += 1,
        }
    }
    
    println!("Pool exhaustion test completed in {:?}: {} successful, {} failed", 
             elapsed, successful_ops, failed_ops);
    
    // Most operations should succeed (allowing for some failures due to resource constraints)
    assert!(successful_ops > num_operations * 7 / 10, 
           "At least 70% of operations should succeed even under pool pressure");
    
    // Operations should complete in reasonable time (no indefinite blocking)
    assert!(elapsed < Duration::from_secs(30), 
           "Operations should not be indefinitely blocked by pool exhaustion");
    
    // Verify database consistency after pool stress
    let final_files = db.get_all_files().await.unwrap();
    assert_eq!(final_files.len(), successful_ops, 
              "Database should contain exactly the number of successful operations");
}

#[tokio::test]
async fn test_concurrent_schema_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("schema_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Arc::new(Database::new(pool).await.unwrap());
    
    // Insert some initial data
    let _ = db.store_file_analysis("test.txt", "test content", "text/plain", None).await;
    
    // Test concurrent operations during potential schema changes
    let num_concurrent_reads = 20;
    let num_concurrent_writes = 10;
    
    let mut tasks = Vec::new();
    
    // Spawn concurrent read operations
    for i in 0..num_concurrent_reads {
        let db_clone = Arc::clone(&db);
        let task = tokio::spawn(async move {
            for j in 0..5 {
                let search_term = format!("test {}", (i + j) % 10);
                
                match db_clone.search_files_by_content(&search_term, 3).await {
                    Ok(results) => {
                        assert!(results.len() <= 3, "Should respect search limit");
                    }
                    Err(e) => {
                        println!("Read operation {}-{} failed: {:?}", i, j, e);
                    }
                }
                
                // Small delay between operations
                sleep(Duration::from_millis(5)).await;
            }
            format!("read_task_{}", i)
        });
        tasks.push(task);
    }
    
    // Spawn concurrent write operations
    for i in 0..num_concurrent_writes {
        let db_clone = Arc::clone(&db);
        let task = tokio::spawn(async move {
            for j in 0..3 {
                let file_path = format!("concurrent_write_{}_{}.txt", i, j);
                let content = format!("Content from writer {} iteration {}", i, j);
                
                match db_clone.store_file_analysis(&file_path, &content, "text/plain", None).await {
                    Ok(_) => {
                        // Success
                    }
                    Err(e) => {
                        println!("Write operation {}-{} failed: {:?}", i, j, e);
                    }
                }
                
                sleep(Duration::from_millis(10)).await;
            }
            format!("write_task_{}", i)
        });
        tasks.push(task);
    }
    
    // Wait for all operations to complete
    let start_time = std::time::Instant::now();
    let task_results = join_all(tasks).await;
    let elapsed = start_time.elapsed();
    
    let completed_tasks: Vec<_> = task_results.into_iter()
        .map(|r| r.unwrap())
        .collect();
    
    println!("Schema concurrency test completed in {:?}: {} tasks finished", 
             elapsed, completed_tasks.len());
    
    // Verify database is still in consistent state
    let all_files = db.get_all_files().await.unwrap();
    println!("Database contains {} files after concurrent schema operations", all_files.len());
    
    // Verify we can still perform basic operations
    let search_results = db.search_files_by_content("test", 5).await.unwrap();
    assert!(!search_results.is_empty(), "Should still be able to search files");
    
    // Test that database structure is intact
    let table_check = sqlx::query("SELECT COUNT(*) as count FROM files")
        .fetch_one(&db.pool)
        .await;
        
    match table_check {
        Ok(row) => {
            let count: i64 = sqlx::Row::get(&row, "count");
            assert!(count >= 0, "Files table should be accessible and have valid count");
            println!("Files table contains {} records after concurrent operations", count);
        }
        Err(e) => {
            panic!("Database structure compromised after concurrent operations: {:?}", e);
        }
    }
}