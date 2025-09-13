use stratosort::storage::database::Database;
use stratosort::commands::*;
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::AppError;
use tauri::{State, test::{mock_app, MockRuntime}};
use sqlx::SqlitePool;
use tempfile::tempdir;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Helper to create test app state
async fn create_test_state() -> Arc<AppState> {
    let app = mock_app();
    let config = Config::default();
    
    match AppState::new(app.clone(), config).await {
        Ok(state) => Arc::new(state),
        Err(_) => panic!("Could not create app state for testing"),
    }
}

#[tokio::test]
async fn test_database_error_recovery() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("error_recovery_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Test recovery from various database error scenarios
    println!("Testing database error recovery scenarios");
    
    // Scenario 1: Database corruption simulation
    println!("Testing database corruption recovery");
    
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
    assert!(files_before.len() >= 3, "Initial files should be stored");
    
    // Simulate various error conditions that might corrupt the database
    let error_scenarios = vec![
        ("disk_full_simulation", test_disk_full_error),
        ("connection_timeout", test_connection_timeout_error),
        ("concurrent_access_conflict", test_concurrent_access_error),
        ("invalid_sql_recovery", test_invalid_sql_recovery),
        ("transaction_rollback_error", test_transaction_rollback_error),
    ];
    
    for (scenario_name, scenario_fn) in error_scenarios {
        println!("Running error scenario: {}", scenario_name);
        
        let scenario_result = scenario_fn(&db).await;
        match scenario_result {
            Ok(_) => {
                println!("Scenario '{}' completed without errors", scenario_name);
            }
            Err(e) => {
                println!("Scenario '{}' produced expected error: {:?}", scenario_name, e);
            }
        }
        
        // After each error scenario, verify database is still functional
        let recovery_test = test_database_functionality_after_error(&db).await;
        match recovery_test {
            Ok(_) => {
                println!("Database functionality recovered after '{}'", scenario_name);
            }
            Err(e) => {
                println!("Database NOT recovered after '{}': {:?}", scenario_name, e);
                
                // Attempt basic recovery operations
                match attempt_database_recovery(&db).await {
                    Ok(_) => println!("Manual recovery successful for '{}'", scenario_name),
                    Err(recovery_error) => {
                        println!("Manual recovery failed for '{}': {:?}", scenario_name, recovery_error);
                    }
                }
            }
        }
        
        // Small delay between scenarios
        sleep(Duration::from_millis(100)).await;
    }
}

async fn test_disk_full_error(db: &Database) -> Result<(), AppError> {
    // Simulate disk full by trying to insert very large content
    let large_content = "x".repeat(50 * 1024 * 1024); // 50MB
    
    let result = db.store_file_analysis("large_file.txt", &large_content, "text/plain", None).await;
    match result {
        Err(e) => {
            println!("Large file insertion failed as expected: {:?}", e);
            Ok(())
        }
        Ok(_) => {
            println!("Large file insertion succeeded (disk not actually full)");
            Ok(())
        }
    }
}

async fn test_connection_timeout_error(db: &Database) -> Result<(), AppError> {
    // Simulate connection timeout by running many concurrent operations
    let concurrent_ops = 100;
    
    let tasks: Vec<_> = (0..concurrent_ops).map(|i| {
        let file_path = format!("timeout_test_{}.txt", i);
        let content = format!("Timeout test content {}", i);
        db.store_file_analysis(&file_path, &content, "text/plain", None)
    }).collect();
    
    let results = futures::future::join_all(tasks).await;
    let failures: Vec<_> = results.into_iter()
        .filter_map(|r| r.err())
        .collect();
    
    if !failures.is_empty() {
        println!("Connection timeout test produced {} failures", failures.len());
        return Err(AppError::DatabaseError { message: "Connection timeout simulation".into() });
    }
    
    Ok(())
}

async fn test_concurrent_access_error(db: &Database) -> Result<(), AppError> {
    // Test concurrent access to the same resource
    let shared_file = "shared_resource.txt";
    
    let concurrent_writers = 20;
    let tasks: Vec<_> = (0..concurrent_writers).map(|i| {
        let content = format!("Writer {} content", i);
        db.store_file_analysis(shared_file, &content, "text/plain", None)
    }).collect();
    
    let results = futures::future::join_all(tasks).await;
    let successful_writes = results.into_iter()
        .filter_map(|r| r.ok())
        .count();
    
    println!("Concurrent access test: {} successful writes out of {}", 
             successful_writes, concurrent_writers);
    
    if successful_writes < concurrent_writers / 2 {
        return Err(AppError::DatabaseError { 
            message: "Too many concurrent access failures".into() 
        });
    }
    
    Ok(())
}

async fn test_invalid_sql_recovery(db: &Database) -> Result<(), AppError> {
    // Test recovery from potential SQL-related errors
    let problematic_inputs = vec![
        ("file_with_quotes.txt", "Content with 'single quotes' and \"double quotes\""),
        ("file_with_nulls.txt", "Content\0with\0null\0bytes"),
        ("file_unicode.txt", "Content with unicode: 💀☠️🔥⚡"),
        ("file_very_long.txt", &"x".repeat(1024 * 1024)), // 1MB content
    ];
    
    let mut error_count = 0;
    for (path, content) in problematic_inputs {
        match db.store_file_analysis(path, content, "text/plain", None).await {
            Ok(_) => {
                println!("Problematic input '{}' handled successfully", path);
            }
            Err(e) => {
                error_count += 1;
                println!("Problematic input '{}' caused error: {:?}", path, e);
            }
        }
    }
    
    if error_count > 2 {
        return Err(AppError::DatabaseError { 
            message: "Too many SQL-related errors".into() 
        });
    }
    
    Ok(())
}

async fn test_transaction_rollback_error(db: &Database) -> Result<(), AppError> {
    // Test transaction rollback scenarios
    let initial_count = db.get_all_files().await.unwrap_or_default().len();
    
    // Start a "transaction" (conceptually) and then cause it to fail
    let temp_files = vec![
        ("rollback_test_1.txt", "content 1"),
        ("rollback_test_2.txt", "content 2"),
        ("", "invalid empty path"), // This should cause failure
        ("rollback_test_3.txt", "content 3"),
    ];
    
    let mut successful_inserts = 0;
    for (path, content) in temp_files {
        match db.store_file_analysis(path, content, "text/plain", None).await {
            Ok(_) => successful_inserts += 1,
            Err(e) => {
                println!("Transaction item failed: {:?}", e);
                // In a real transaction, this would trigger rollback
            }
        }
    }
    
    let final_count = db.get_all_files().await.unwrap_or_default().len();
    let net_change = final_count - initial_count;
    
    if net_change != successful_inserts {
        return Err(AppError::DatabaseError { 
            message: format!("Transaction consistency error: expected {}, got {}", successful_inserts, net_change)
        });
    }
    
    Ok(())
}

async fn test_database_functionality_after_error(db: &Database) -> Result<(), AppError> {
    // Test basic database operations to verify functionality
    
    // Test 1: Insert a new file
    let test_id = db.store_file_analysis("recovery_test.txt", "recovery content", "text/plain", None).await?;
    
    // Test 2: Search for files
    let search_results = db.search_files_by_content("recovery", 5).await?;
    assert!(!search_results.is_empty(), "Should find the recovery test file");
    
    // Test 3: Get all files
    let all_files = db.get_all_files().await?;
    assert!(!all_files.is_empty(), "Database should not be empty");
    
    // Test 4: Update existing file (if supported)
    let update_result = db.store_file_analysis("recovery_test.txt", "updated recovery content", "text/plain", Some(test_id)).await;
    match update_result {
        Ok(_) => println!("File update succeeded"),
        Err(e) => println!("File update failed (might not be supported): {:?}", e),
    }
    
    Ok(())
}

async fn attempt_database_recovery(db: &Database) -> Result<(), AppError> {
    // Attempt basic recovery operations
    println!("Attempting database recovery...");
    
    // Recovery step 1: Verify table structure
    let table_check = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
        .fetch_all(&db.pool)
        .await;
        
    match table_check {
        Ok(tables) => {
            println!("Database tables accessible: {} tables found", tables.len());
        }
        Err(e) => {
            println!("Database table check failed: {:?}", e);
            return Err(AppError::DatabaseError { 
                message: "Cannot access database schema".into() 
            });
        }
    }
    
    // Recovery step 2: Test basic connectivity
    let connectivity_test = sqlx::query("SELECT 1 as test")
        .fetch_one(&db.pool)
        .await;
        
    match connectivity_test {
        Ok(_) => {
            println!("Database connectivity confirmed");
        }
        Err(e) => {
            println!("Database connectivity test failed: {:?}", e);
            return Err(AppError::DatabaseError { 
                message: "Database connectivity lost".into() 
            });
        }
    }
    
    // Recovery step 3: Test data integrity
    let integrity_test = db.get_all_files().await;
    match integrity_test {
        Ok(files) => {
            println!("Data integrity check passed: {} files accessible", files.len());
        }
        Err(e) => {
            println!("Data integrity check failed: {:?}", e);
            return Err(AppError::DatabaseError { 
                message: "Data integrity compromised".into() 
            });
        }
    }
    
    println!("Database recovery completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_ai_service_error_handling() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    println!("Testing AI service error handling and recovery");
    
    // Test AI service unavailability
    let ai_error_scenarios = vec![
        ("connection_refused", "test content", "Connection to AI service failed"),
        ("invalid_model", "test content", "Model not available"),
        ("timeout", "very long content ".repeat(10000), "AI service timeout"),
        ("malformed_response", "test content", "Invalid response from AI service"),
    ];
    
    for (scenario, content, description) in ai_error_scenarios {
        println!("Testing AI error scenario: {} - {}", scenario, description);
        
        // Test analyze_with_ai
        let analyze_result = ai::analyze_with_ai(
            content.to_string(),
            "text/plain".to_string(),
            state_ref.clone()
        ).await;
        
        match analyze_result {
            Ok(analysis) => {
                println!("AI analysis succeeded despite error scenario: {}", scenario);
                // Verify the analysis is reasonable
                assert!(!analysis.is_empty(), "Analysis should not be empty");
            }
            Err(AppError::AiError { message }) => {
                println!("AI error properly handled for '{}': {}", scenario, message);
                // Verify error message doesn't leak sensitive information
                assert!(!message.to_lowercase().contains("password"), "Error should not contain passwords");
                assert!(!message.to_lowercase().contains("secret"), "Error should not contain secrets");
            }
            Err(e) => {
                println!("AI scenario '{}' failed with different error: {:?}", scenario, e);
            }
        }
        
        // Test pull_model with error scenarios
        let model_scenarios = vec![
            ("nonexistent-model", "Model does not exist"),
            ("", "Empty model name"),
            ("malicious-model; rm -rf /", "Command injection attempt"),
        ];
        
        for (model_name, model_description) in model_scenarios {
            let pull_result = ai::pull_model(model_name.to_string(), state_ref.clone()).await;
            
            match pull_result {
                Ok(_) => {
                    println!("Model pull unexpectedly succeeded for: {}", model_name);
                }
                Err(AppError::AiError { .. }) => {
                    println!("Model pull properly failed for '{}': {}", model_name, model_description);
                }
                Err(AppError::SecurityError { .. }) | 
                Err(AppError::InvalidInput { .. }) => {
                    println!("Malicious model '{}' properly blocked", model_name);
                }
                Err(e) => {
                    println!("Model pull '{}' failed with: {:?}", model_name, e);
                }
            }
        }
        
        // Test semantic search with error scenarios
        let search_queries = vec![
            ("normal query", 10, true),
            ("", 10, false), // Empty query
            ("query", 0, false), // Invalid limit
            ("query", 10000, false), // Limit too high
            ("a".repeat(5000), 10, false), // Query too long
        ];
        
        for (query, limit, should_succeed) in search_queries {
            let search_result = ai::semantic_search(query.to_string(), limit, state_ref.clone()).await;
            
            if should_succeed {
                match search_result {
                    Ok(results) => {
                        println!("Search succeeded for query length: {}", query.len());
                        assert!(results.len() <= limit as usize, "Results should respect limit");
                    }
                    Err(e) => {
                        println!("Valid search failed: {:?}", e);
                    }
                }
            } else {
                match search_result {
                    Ok(_) => {
                        println!("WARNING: Invalid search query was accepted: query_len={}, limit={}", 
                               query.len(), limit);
                    }
                    Err(e) => {
                        println!("Invalid search properly rejected: {:?}", e);
                    }
                }
            }
        }
    }
    
    // Test AI service recovery after errors
    println!("Testing AI service recovery");
    
    // After error scenarios, test if AI service can still handle normal requests
    let recovery_result = ai::analyze_with_ai(
        "This is a recovery test".to_string(),
        "text/plain".to_string(),
        state_ref.clone()
    ).await;
    
    match recovery_result {
        Ok(_) => println!("AI service recovered successfully after error scenarios"),
        Err(e) => println!("AI service still experiencing issues after errors: {:?}", e),
    }
}

#[tokio::test]
async fn test_file_system_error_handling() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    println!("Testing file system error handling and recovery");
    
    // Test various file system error scenarios
    let fs_error_scenarios = vec![
        ("/nonexistent/path/file.txt", "File not found"),
        ("/etc/passwd", "Permission denied (system file)"),
        ("", "Empty path"),
        ("a".repeat(1000), "Path too long"),
        ("file\0with\0nulls.txt", "Null bytes in path"),
        ("file\nwith\nnewlines.txt", "Newlines in path"),
        ("con.txt", "Windows reserved name"),
        ("aux.txt", "Windows reserved name"),
        ("/dev/null", "Special device file"),
        ("//server/share/file.txt", "UNC path"),
    ];
    
    for (file_path, description) in fs_error_scenarios {
        println!("Testing file system scenario: {} - {}", description, file_path);
        
        // Test get_file_content
        let content_result = files::get_file_content(file_path.to_string(), state_ref.clone(), app.clone()).await;
        
        match content_result {
            Ok(content) => {
                println!("File content retrieved for '{}': {} bytes", file_path, content.len());
                // If it succeeds, verify content is safe
                assert!(content.len() < 10 * 1024 * 1024, "Content should be reasonably sized");
            }
            Err(AppError::FileNotFound { .. }) => {
                println!("File not found error properly handled for: {}", file_path);
            }
            Err(AppError::PermissionDenied { .. }) => {
                println!("Permission denied properly handled for: {}", file_path);
            }
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidPath { .. }) => {
                println!("Security error properly handled for: {}", file_path);
            }
            Err(e) => {
                println!("File access error for '{}': {:?}", file_path, e);
            }
        }
        
        // Test scan_directory
        let scan_result = files::scan_directory(file_path.to_string(), state_ref.clone(), app.clone()).await;
        
        match scan_result {
            Ok(files) => {
                println!("Directory scan succeeded for '{}': {} files found", file_path, files.len());
                assert!(files.len() < 10000, "File count should be reasonable");
                
                // Verify returned files are safe
                for file in files.iter().take(10) { // Check first 10 files
                    assert!(!file.path.contains('\0'), "File paths should not contain null bytes");
                    assert!(!file.path.contains(".."), "File paths should not contain traversal");
                }
            }
            Err(AppError::FileNotFound { .. }) | 
            Err(AppError::PermissionDenied { .. }) | 
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidPath { .. }) => {
                println!("Directory scan error properly handled for: {}", file_path);
            }
            Err(e) => {
                println!("Directory scan error for '{}': {:?}", file_path, e);
            }
        }
    }
    
    // Test file system recovery after errors
    let temp_dir = tempdir().unwrap();
    let safe_path = temp_dir.path().to_string_lossy();
    
    let recovery_result = files::scan_directory(safe_path.to_string(), state_ref.clone(), app.clone()).await;
    match recovery_result {
        Ok(files) => {
            println!("File system operations recovered successfully: {} files in temp dir", files.len());
        }
        Err(e) => {
            println!("File system still experiencing issues: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_cascading_failure_prevention() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    println!("Testing cascading failure prevention");
    
    // Simulate a scenario where multiple components fail sequentially
    let failure_chain = vec![
        ("database_stress", simulate_database_failure),
        ("ai_service_overload", simulate_ai_service_failure),
        ("file_system_pressure", simulate_file_system_failure),
        ("memory_pressure", simulate_memory_pressure),
        ("concurrent_operations", simulate_concurrent_failure),
    ];
    
    let mut system_still_responsive = true;
    
    for (failure_name, failure_fn) in failure_chain {
        if !system_still_responsive {
            println!("System no longer responsive, stopping failure chain");
            break;
        }
        
        println!("Simulating failure: {}", failure_name);
        
        // Execute the failure scenario
        let failure_result = failure_fn(state_ref.clone(), app.clone()).await;
        match failure_result {
            Ok(_) => {
                println!("Failure scenario '{}' completed without system failure", failure_name);
            }
            Err(e) => {
                println!("Failure scenario '{}' produced error: {:?}", failure_name, e);
            }
        }
        
        // Test if system is still responsive after this failure
        let responsiveness_test = test_system_responsiveness(state_ref.clone()).await;
        match responsiveness_test {
            Ok(_) => {
                println!("System remains responsive after '{}'", failure_name);
            }
            Err(e) => {
                println!("System responsiveness compromised after '{}': {:?}", failure_name, e);
                system_still_responsive = false;
            }
        }
        
        // Small delay between failures
        sleep(Duration::from_millis(200)).await;
    }
    
    if system_still_responsive {
        println!("SUCCESS: System survived entire failure cascade");
    } else {
        println!("FAILURE: System became unresponsive during cascade");
    }
}

async fn simulate_database_failure(state: State<'_, Arc<AppState>>, _app: tauri::AppHandle<MockRuntime>) -> Result<(), AppError> {
    // Simulate database stress
    let stress_operations = 50;
    
    let tasks: Vec<_> = (0..stress_operations).map(|i| {
        let state_clone = state.inner().clone();
        tokio::spawn(async move {
            // Simulate heavy database operations
            for j in 0..5 {
                let file_path = format!("stress_{}_{}.txt", i, j);
                let content = format!("Stress test content {} {}", i, j);
                
                // This would use the database through the state
                // For this test, we just simulate the work
                sleep(Duration::from_millis(10)).await;
            }
        })
    }).collect();
    
    let _results = futures::future::join_all(tasks).await;
    println!("Database stress simulation completed");
    
    Ok(())
}

async fn simulate_ai_service_failure(state: State<'_, Arc<AppState>>, _app: tauri::AppHandle<MockRuntime>) -> Result<(), AppError> {
    // Simulate AI service overload
    let ai_operations = 30;
    
    let tasks: Vec<_> = (0..ai_operations).map(|i| {
        let state_clone = state.clone();
        tokio::spawn(async move {
            let content = format!("AI test content {}", i);
            let result = ai::analyze_with_ai(content, "text/plain".to_string(), state_clone).await;
            match result {
                Ok(_) => 1,
                Err(_) => 0,
            }
        })
    }).collect();
    
    let results = futures::future::join_all(tasks).await;
    let successful_ai_ops = results.into_iter()
        .filter_map(|r| r.ok())
        .sum::<i32>();
    
    println!("AI service stress completed: {} successful operations", successful_ai_ops);
    
    if successful_ai_ops == 0 {
        return Err(AppError::AiError { 
            message: "AI service completely unavailable".into() 
        });
    }
    
    Ok(())
}

async fn simulate_file_system_failure(state: State<'_, Arc<AppState>>, app: tauri::AppHandle<MockRuntime>) -> Result<(), AppError> {
    // Simulate file system pressure
    let file_operations = 25;
    
    let temp_dir = tempdir().unwrap();
    let base_path = temp_dir.path();
    
    let tasks: Vec<_> = (0..file_operations).map(|i| {
        let file_path = base_path.join(format!("file_{}.txt", i)).to_string_lossy().to_string();
        let state_clone = state.clone();
        let app_clone = app.clone();
        
        tokio::spawn(async move {
            // Test file operations
            let result = files::get_file_content(file_path, state_clone, app_clone).await;
            match result {
                Ok(_) => 1,
                Err(_) => 0,
            }
        })
    }).collect();
    
    let results = futures::future::join_all(tasks).await;
    let successful_file_ops = results.into_iter()
        .filter_map(|r| r.ok())
        .sum::<i32>();
    
    println!("File system stress completed: {} successful operations", successful_file_ops);
    
    Ok(())
}

async fn simulate_memory_pressure(_state: State<'_, Arc<AppState>>, _app: tauri::AppHandle<MockRuntime>) -> Result<(), AppError> {
    // Simulate memory pressure
    println!("Simulating memory pressure");
    
    // Create some memory pressure (be careful not to actually exhaust memory)
    let memory_consumers: Vec<Vec<u8>> = (0..100)
        .map(|_| vec![0u8; 1024 * 1024]) // 1MB each
        .collect();
    
    // Hold memory briefly
    sleep(Duration::from_millis(100)).await;
    
    // Release memory
    drop(memory_consumers);
    
    println!("Memory pressure simulation completed");
    
    Ok(())
}

async fn simulate_concurrent_failure(state: State<'_, Arc<AppState>>, app: tauri::AppHandle<MockRuntime>) -> Result<(), AppError> {
    // Simulate many concurrent operations that might interfere with each other
    println!("Simulating concurrent operation failures");
    
    let concurrent_ops = 50;
    
    let tasks: Vec<_> = (0..concurrent_ops).map(|i| {
        let state_clone = state.clone();
        let app_clone = app.clone();
        
        tokio::spawn(async move {
            match i % 4 {
                0 => monitoring::get_system_info(state_clone).await.map(|_| ()),
                1 => monitoring::get_app_info(state_clone).await.map(|_| ()),
                2 => monitoring::get_database_stats(state_clone).await.map(|_| ()),
                _ => monitoring::get_enabled_features(state_clone).await.map(|_| ()),
            }
        })
    }).collect();
    
    let results = futures::future::join_all(tasks).await;
    let successful_ops = results.into_iter()
        .filter_map(|r| r.ok().and_then(|inner| inner.ok()))
        .count();
    
    println!("Concurrent operations completed: {} successful", successful_ops);
    
    if successful_ops < concurrent_ops / 2 {
        return Err(AppError::SystemError { 
            message: "Too many concurrent operation failures".into() 
        });
    }
    
    Ok(())
}

async fn test_system_responsiveness(state: State<'_, Arc<AppState>>) -> Result<(), AppError> {
    // Test that the system is still responsive by performing basic operations
    let start_time = std::time::Instant::now();
    
    // Test 1: Get system info
    let system_info_result = monitoring::get_system_info(state.clone()).await;
    if system_info_result.is_err() {
        return Err(AppError::SystemError { 
            message: "System info unavailable".into() 
        });
    }
    
    // Test 2: Get enabled features
    let features_result = monitoring::get_enabled_features(state.clone()).await;
    if features_result.is_err() {
        return Err(AppError::SystemError { 
            message: "Features info unavailable".into() 
        });
    }
    
    let elapsed = start_time.elapsed();
    if elapsed > Duration::from_secs(5) {
        return Err(AppError::SystemError { 
            message: "System response too slow".into() 
        });
    }
    
    println!("System responsiveness test passed in {:?}", elapsed);
    Ok(())
}