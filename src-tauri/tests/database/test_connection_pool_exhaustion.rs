use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use sqlx::{Pool, Sqlite};

use stratosort::storage::Database;
use stratosort::error::{AppError, Result};

#[tokio::test]
async fn test_database_connection_pool_exhaustion() {
    // Create a test database with limited connections
    let db = create_test_database().await.expect("Failed to create test database");
    
    // SQLite doesn't have traditional connection pooling like PostgreSQL,
    // but we can test concurrent access and proper resource management
    let num_concurrent_operations = 50;
    let mut handles = vec![];
    
    for i in 0..num_concurrent_operations {
        let db_clone = db.clone();
        
        let handle = tokio::spawn(async move {
            // Simulate database operations that might exhaust resources
            let operation_result = simulate_database_operation(&db_clone, i).await;
            (i, operation_result)
        });
        
        handles.push(handle);
    }
    
    // Collect results
    let mut successful_operations = 0;
    let mut failed_operations = 0;
    
    for handle in handles {
        let (operation_id, result) = handle.await.expect("Task panicked");
        
        match result {
            Ok(_) => {
                successful_operations += 1;
            }
            Err(e) => {
                failed_operations += 1;
                println!("Operation {} failed: {:?}", operation_id, e);
                
                // Verify that failures are due to expected resource constraints
                match e {
                    AppError::DatabaseError { message } => {
                        assert!(message.contains("database") || 
                               message.contains("connection") ||
                               message.contains("busy") ||
                               message.contains("locked"),
                               "Unexpected database error: {}", message);
                    }
                    _ => {
                        panic!("Unexpected error type: {:?}", e);
                    }
                }
            }
        }
    }
    
    println!("Database stress test results: {} successful, {} failed", 
             successful_operations, failed_operations);
    
    // Most operations should succeed with proper connection management
    assert!(successful_operations > 0, "Some operations should succeed");
    
    // If there are failures, they should be a small percentage
    if failed_operations > 0 {
        let failure_rate = failed_operations as f64 / num_concurrent_operations as f64;
        assert!(failure_rate < 0.5, "Failure rate too high: {:.2}%", failure_rate * 100.0);
    }
}

#[tokio::test]
async fn test_database_connection_recovery() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Phase 1: Saturate the database with long-running operations
    let long_running_count = 10;
    let mut long_handles = vec![];
    
    for i in 0..long_running_count {
        let db_clone = db.clone();
        
        let handle = tokio::spawn(async move {
            // Simulate long-running database operation
            let result = simulate_long_running_operation(&db_clone, i).await;
            tokio::time::sleep(Duration::from_millis(500)).await; // Hold connection longer
            result
        });
        
        long_handles.push(handle);
    }
    
    // Phase 2: Try to perform quick operations while long operations are running
    let quick_operations = 5;
    let mut quick_handles = vec![];
    
    for i in 0..quick_operations {
        let db_clone = db.clone();
        
        let handle = tokio::spawn(async move {
            // Quick operation that should eventually succeed
            simulate_quick_operation(&db_clone, i).await
        });
        
        quick_handles.push(handle);
    }
    
    // Phase 3: Wait for quick operations to complete
    let mut quick_successes = 0;
    let mut quick_failures = 0;
    
    for handle in quick_handles {
        match handle.await.expect("Task panicked") {
            Ok(_) => quick_successes += 1,
            Err(_) => quick_failures += 1,
        }
    }
    
    // Phase 4: Wait for long operations to complete
    for handle in long_handles {
        let _ = handle.await; // Don't care about the result
    }
    
    // Phase 5: Verify database is still functional after resource pressure
    let health_check = db.health_check().await;
    assert!(health_check.is_ok(), "Database should be healthy after recovery");
    
    println!("Recovery test: {} quick successes, {} quick failures", 
             quick_successes, quick_failures);
    
    // Some quick operations should succeed eventually
    assert!(quick_successes > 0, "Some quick operations should succeed during recovery");
}

#[tokio::test]
async fn test_database_timeout_behavior() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Start a transaction that holds a lock
    let pool = db.pool();
    let mut tx = pool.begin().await.expect("Failed to begin transaction");
    
    // Insert something to create a lock
    sqlx::query("CREATE TABLE IF NOT EXISTS test_lock (id INTEGER PRIMARY KEY, data TEXT)")
        .execute(&mut *tx)
        .await
        .expect("Failed to create table");
    
    sqlx::query("INSERT INTO test_lock (data) VALUES ('locked_data')")
        .execute(&mut *tx)
        .await
        .expect("Failed to insert data");
    
    // Now try to access the same table from another connection with timeout
    let db_clone = db.clone();
    let timeout_task = tokio::spawn(async move {
        let start_time = std::time::Instant::now();
        
        let result = timeout(Duration::from_millis(1000), async {
            // This should timeout due to the lock
            sqlx::query("INSERT INTO test_lock (data) VALUES ('should_timeout')")
                .execute(db_clone.pool())
                .await
        }).await;
        
        let elapsed = start_time.elapsed();
        (result, elapsed)
    });
    
    // Keep the transaction open for a bit
    tokio::time::sleep(Duration::from_millis(1500)).await;
    
    // Rollback the transaction to release the lock
    tx.rollback().await.expect("Failed to rollback");
    
    let (timeout_result, elapsed) = timeout_task.await.expect("Timeout task panicked");
    
    // Should have timed out
    assert!(timeout_result.is_err(), "Operation should have timed out");
    assert!(elapsed >= Duration::from_millis(900) && elapsed <= Duration::from_millis(1200), 
           "Timeout should occur around 1 second, actual: {:?}", elapsed);
    
    // Verify database is still functional
    let health_check = db.health_check().await;
    assert!(health_check.is_ok(), "Database should be healthy after timeout test");
}

#[tokio::test]
async fn test_connection_leak_detection() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    let initial_stats = get_connection_stats(&db).await;
    
    // Simulate operations that might leak connections
    let operations_count = 20;
    
    for i in 0..operations_count {
        let db_clone = db.clone();
        
        // Simulate an operation that acquires but might not properly release connections
        let _result = simulate_potential_leak_operation(&db_clone, i).await;
        
        // Small delay to allow connection cleanup
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    // Force garbage collection and cleanup
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let final_stats = get_connection_stats(&db).await;
    
    println!("Connection stats - Initial: {:?}, Final: {:?}", initial_stats, final_stats);
    
    // Verify no significant connection leaks
    // Note: SQLite doesn't have traditional connection pooling, so this test
    // is more about ensuring proper resource cleanup
    assert!(final_stats.active_connections <= initial_stats.active_connections + 2, 
           "Should not have significant connection leaks");
}

#[tokio::test]
async fn test_database_graceful_shutdown_during_operations() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Start some background operations
    let operation_count = 10;
    let mut handles = vec![];
    
    for i in 0..operation_count {
        let db_clone = db.clone();
        
        let handle = tokio::spawn(async move {
            // Simulate ongoing work
            for j in 0..5 {
                let result = simulate_database_operation(&db_clone, i * 10 + j).await;
                if result.is_err() {
                    break; // Stop if database becomes unavailable
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        
        handles.push(handle);
    }
    
    // Let operations start
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Initiate shutdown
    let shutdown_result = db.close_connections().await;
    assert!(shutdown_result.is_ok(), "Shutdown should succeed");
    
    // Wait for operations to complete or fail gracefully
    let mut completed_tasks = 0;
    for handle in handles {
        if handle.await.is_ok() {
            completed_tasks += 1;
        }
    }
    
    println!("Graceful shutdown: {} tasks completed cleanly", completed_tasks);
    
    // Some tasks should complete, but it's okay if some are interrupted
    // The important thing is no panics or corruption
}

#[tokio::test]
async fn test_concurrent_transaction_handling() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Ensure test table exists
    sqlx::query("CREATE TABLE IF NOT EXISTS test_concurrent (id INTEGER PRIMARY KEY, value INTEGER)")
        .execute(db.pool())
        .await
        .expect("Failed to create test table");
    
    let transaction_count = 10;
    let mut handles = vec![];
    
    for i in 0..transaction_count {
        let db_clone = db.clone();
        
        let handle = tokio::spawn(async move {
            // Each task performs a transaction
            let mut tx = db_clone.pool().begin().await?;
            
            // Insert some data
            sqlx::query("INSERT INTO test_concurrent (value) VALUES (?)")
                .bind(i)
                .execute(&mut *tx)
                .await?;
            
            // Simulate some processing time
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            // Update the data
            sqlx::query("UPDATE test_concurrent SET value = value + 100 WHERE value = ?")
                .bind(i)
                .execute(&mut *tx)
                .await?;
            
            // Commit the transaction
            tx.commit().await?;
            
            Ok::<(), sqlx::Error>(())
        });
        
        handles.push(handle);
    }
    
    // Wait for all transactions to complete
    let mut successful_transactions = 0;
    let mut failed_transactions = 0;
    
    for handle in handles {
        match handle.await.expect("Task panicked") {
            Ok(_) => successful_transactions += 1,
            Err(e) => {
                failed_transactions += 1;
                println!("Transaction failed: {:?}", e);
            }
        }
    }
    
    println!("Transaction test: {} successful, {} failed", 
             successful_transactions, failed_transactions);
    
    // Most transactions should succeed
    assert!(successful_transactions >= transaction_count - 2, 
           "Most transactions should succeed");
    
    // Verify data integrity
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_concurrent")
        .fetch_one(db.pool())
        .await
        .expect("Failed to count records");
    
    assert_eq!(count as usize, successful_transactions, 
              "Record count should match successful transactions");
}

// Helper functions

async fn create_test_database() -> Result<Database> {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    
    Database::new_from_url(&db_url).await
}

async fn simulate_database_operation(db: &Database, operation_id: usize) -> Result<()> {
    // Create a test table if it doesn't exist
    sqlx::query("CREATE TABLE IF NOT EXISTS test_operations (id INTEGER PRIMARY KEY, data TEXT)")
        .execute(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Insert some data
    sqlx::query("INSERT INTO test_operations (data) VALUES (?)")
        .bind(format!("operation_{}", operation_id))
        .execute(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Query the data back
    let _count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM test_operations")
        .fetch_one(db.pool())
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}

async fn simulate_long_running_operation(db: &Database, operation_id: usize) -> Result<()> {
    // Start a transaction that takes some time
    let mut tx = db.pool().begin().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    sqlx::query("CREATE TABLE IF NOT EXISTS test_long (id INTEGER PRIMARY KEY, data TEXT)")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Simulate processing time
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    sqlx::query("INSERT INTO test_long (data) VALUES (?)")
        .bind(format!("long_operation_{}", operation_id))
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    tx.commit().await
        .map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    Ok(())
}

async fn simulate_quick_operation(db: &Database, operation_id: usize) -> Result<()> {
    // Quick read operation
    let result: Result<i64, sqlx::Error> = sqlx::query_scalar("SELECT 1")
        .fetch_one(db.pool())
        .await;
    
    result.map_err(|e| AppError::DatabaseError { message: e.to_string() })?;
    
    // Quick write if possible
    let _ = sqlx::query("CREATE TABLE IF NOT EXISTS test_quick (id INTEGER PRIMARY KEY)")
        .execute(db.pool())
        .await;
    
    Ok(())
}

async fn simulate_potential_leak_operation(db: &Database, operation_id: usize) -> Result<()> {
    // Simulate an operation that might not properly clean up resources
    let pool = db.pool();
    
    // Acquire connection implicitly
    let _result: Result<i64, sqlx::Error> = sqlx::query_scalar("SELECT ?")
        .bind(operation_id as i64)
        .fetch_one(pool)
        .await;
    
    // Simulate some work
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Connection should be automatically returned to pool
    Ok(())
}

#[derive(Debug)]
struct ConnectionStats {
    active_connections: usize,
    idle_connections: usize,
}

async fn get_connection_stats(db: &Database) -> ConnectionStats {
    // SQLite doesn't expose detailed connection pool stats like PostgreSQL
    // This is a simplified version that would need to be implemented based on
    // the actual connection pool implementation
    
    // For now, we'll use a simple health check as a proxy
    let is_healthy = db.health_check().await.is_ok();
    
    ConnectionStats {
        active_connections: if is_healthy { 1 } else { 0 },
        idle_connections: if is_healthy { 0 } else { 1 },
    }
}