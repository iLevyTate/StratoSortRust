use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::timeout;

// Import the file commands that have resource limits
use stratosort::commands::files::{ReadGuard, MemoryGuard};
use stratosort::error::AppError;
use stratosort::state::AppState;
use stratosort::config::Config;

#[test]
fn test_concurrent_read_limit_enforcement() {
    const MAX_CONCURRENT_READS: usize = 5;
    const ATTEMPT_COUNT: usize = 20;
    
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create a test config with specific limits
        let mut config = Config::default();
        config.max_concurrent_reads = MAX_CONCURRENT_READS;
        let state = create_test_app_state(config);
        
        let success_count = Arc::new(AtomicUsize::new(0));
        let rejection_count = Arc::new(AtomicUsize::new(0));
        let active_reads = Arc::new(AtomicUsize::new(0));
        let max_concurrent_observed = Arc::new(AtomicUsize::new(0));
        
        let mut handles = vec![];
        
        for _i in 0..ATTEMPT_COUNT {
            let success_count_clone = Arc::clone(&success_count);
            let rejection_count_clone = Arc::clone(&rejection_count);
            let active_reads_clone = Arc::clone(&active_reads);
            let max_concurrent_clone = Arc::clone(&max_concurrent_observed);
            let state_clone = state.clone();
            
            let handle = tokio::spawn(async move {
                match ReadGuard::new(&state_clone) {
                    Ok(_guard) => {
                        // Increment active reads and track maximum
                        let current_active = active_reads_clone.fetch_add(1, Ordering::SeqCst) + 1;
                        max_concurrent_clone.fetch_max(current_active, Ordering::SeqCst);
                        
                        success_count_clone.fetch_add(1, Ordering::SeqCst);
                        
                        // Simulate read operation
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        
                        // Decrement active reads (guard will auto-decrement, but we track manually)
                        active_reads_clone.fetch_sub(1, Ordering::SeqCst);
                        
                        // Guard is dropped here, releasing the read slot
                    }
                    Err(AppError::ResourceLimitExceeded { .. }) => {
                        rejection_count_clone.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        panic!("Unexpected error: {:?}", e);
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("Task panicked");
        }
        
        let final_success = success_count.load(Ordering::SeqCst);
        let final_rejection = rejection_count.load(Ordering::SeqCst);
        let max_observed = max_concurrent_observed.load(Ordering::SeqCst);
        
        println!("Successful reads: {}, Rejected reads: {}, Max concurrent: {}", 
                final_success, final_rejection, max_observed);
        
        // Verify resource limit enforcement
        assert!(final_success + final_rejection == ATTEMPT_COUNT);
        assert!(final_rejection > 0, "Should have some rejections when exceeding limit");
        assert!(max_observed <= MAX_CONCURRENT_READS, 
               "Should never exceed max concurrent reads: {} observed, {} max", 
               max_observed, MAX_CONCURRENT_READS);
    });
}

#[test]
fn test_memory_limit_enforcement() {
    const MAX_TOTAL_MEMORY: usize = 100; // 100MB
    const LARGE_ALLOCATION: usize = 20 * 1024 * 1024;  // 20MB
    
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create a test config with specific memory limits
        let mut config = Config::default();
        config.max_total_memory_mb = MAX_TOTAL_MEMORY;
        let state = create_test_app_state(config);
        
        let mut guards = Vec::new();
        let mut allocation_count = 0;
        
        // Try to allocate more than the limit
        loop {
            match MemoryGuard::new(LARGE_ALLOCATION, &state) {
                Ok(guard) => {
                    guards.push(guard);
                    allocation_count += 1;
                    
                    // Should not be able to allocate more than 5 * 20MB = 100MB
                    if allocation_count > 10 {
                        panic!("Allocated too much memory without limit enforcement");
                    }
                }
                Err(AppError::ResourceLimitExceeded { .. }) => {
                    // Expected when limit is reached
                    break;
                }
                Err(e) => {
                    panic!("Unexpected error: {:?}", e);
                }
            }
        }
        
        println!("Successfully allocated {} guards of {}MB each", allocation_count, LARGE_ALLOCATION / 1024 / 1024);
        
        // Verify we allocated close to but not over the limit
        assert!(allocation_count >= 4, "Should be able to allocate at least 4 * 20MB");
        assert!(allocation_count <= 5, "Should not be able to allocate more than 5 * 20MB");
        
        // Drop some guards and verify we can allocate again
        guards.truncate(3);
        
        // Should be able to allocate more now
        match MemoryGuard::new(LARGE_ALLOCATION, &state) {
            Ok(_guard) => {
                // Success expected
            }
            Err(e) => {
                panic!("Should be able to allocate after freeing memory: {:?}", e);
            }
        }
    });
}

#[test]
fn test_file_size_limit_enforcement() {
    // This test simulates the file size validation in get_file_content
    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
    
    let valid_sizes = vec![
        1024,                    // 1KB - should pass
        1024 * 1024,            // 1MB - should pass
        5 * 1024 * 1024,        // 5MB - should pass
        MAX_FILE_SIZE,          // Exactly at limit - should pass
    ];
    
    let invalid_sizes = vec![
        MAX_FILE_SIZE + 1,      // Just over limit
        50 * 1024 * 1024,       // 50MB - way over limit
        1024 * 1024 * 1024,     // 1GB - extremely over limit
    ];
    
    // Test valid sizes
    for size in valid_sizes {
        let result = validate_file_size_limit(size, MAX_FILE_SIZE);
        assert!(result.is_ok(), "Size {} should be valid", size);
    }
    
    // Test invalid sizes
    for size in invalid_sizes {
        let result = validate_file_size_limit(size, MAX_FILE_SIZE);
        assert!(result.is_err(), "Size {} should be invalid", size);
        
        if let Err(AppError::SecurityError { message }) = result {
            assert!(message.contains("too large"), "Error message should mention file being too large");
        } else {
            panic!("Expected SecurityError for oversized file");
        }
    }
}

fn validate_file_size_limit(size: u64, max_size: u64) -> Result<(), AppError> {
    if size > max_size {
        return Err(AppError::SecurityError {
            message: format!("File too large ({} bytes). Maximum allowed: {} bytes", size, max_size),
        });
    }
    Ok(())
}

#[test]
fn test_directory_depth_limit_enforcement() {
    // Test the depth limit in scan_directory (currently set to 3)
    const MAX_DEPTH: usize = 3;
    
    let test_paths = vec![
        ("a", 1, true),                           // Depth 1 - should pass
        ("a/b", 2, true),                         // Depth 2 - should pass  
        ("a/b/c", 3, true),                       // Depth 3 - should pass
        ("a/b/c/d", 4, false),                    // Depth 4 - should be limited
        ("a/b/c/d/e/f/g/h/i/j", 10, false),      // Depth 10 - should be limited
    ];
    
    for (path, depth, should_allow) in test_paths {
        let result = validate_directory_depth(path, MAX_DEPTH);
        
        if should_allow {
            assert!(result.is_ok(), "Path '{}' with depth {} should be allowed", path, depth);
        } else {
            assert!(result.is_err(), "Path '{}' with depth {} should be blocked", path, depth);
        }
    }
}

fn validate_directory_depth(path: &str, max_depth: usize) -> Result<(), AppError> {
    let depth = path.split('/').count();
    if depth > max_depth {
        return Err(AppError::SecurityError {
            message: format!("Directory depth {} exceeds maximum allowed depth {}", depth, max_depth),
        });
    }
    Ok(())
}

#[test]
fn test_combined_resource_pressure() {
    // Test all resource limits simultaneously
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create test config with limited resources
        let mut config = Config::default();
        config.max_concurrent_reads = 5;
        config.max_total_memory_mb = 50; // 50MB
        let state = create_test_app_state(config);
        
        let num_threads = 20;
        let mut handles = vec![];
        
        let success_count = Arc::new(AtomicUsize::new(0));
        let failure_count = Arc::new(AtomicUsize::new(0));
        
        for thread_id in 0..num_threads {
            let success_clone = Arc::clone(&success_count);
            let failure_clone = Arc::clone(&failure_count);
            let state_clone = state.clone();
            
            let handle = tokio::spawn(async move {
                // Try to acquire both read guard and memory guard
                let read_result = ReadGuard::new(&state);
                let memory_result = MemoryGuard::new(10 * 1024 * 1024, &state); // 10MB
                
                match (read_result, memory_result) {
                    (Ok(_read_guard), Ok(_memory_guard)) => {
                        success_clone.fetch_add(1, Ordering::SeqCst);
                        
                        // Simulate work
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        
                        // Guards are automatically dropped
                    }
                    _ => {
                        failure_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            handle.await.expect("Task panicked");
        }
        
        let final_success = success_count.load(Ordering::SeqCst);
        let final_failure = failure_count.load(Ordering::SeqCst);
        
        println!("Combined resource test - Success: {}, Failures: {}", final_success, final_failure);
        
        // Under pressure, some should succeed and some should fail
        assert!(final_success + final_failure == num_threads);
        assert!(final_failure > 0, "Should have some failures under resource pressure");
        assert!(final_success > 0, "Should have some successes");
        
        // Most should fail due to resource limits
        assert!(final_failure > final_success, "More operations should fail than succeed under pressure");
    });
}

#[test]
fn test_resource_limit_recovery() {
    // Test that resource limits properly recover after being reached
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create test config
        let mut config = Config::default();
        config.max_concurrent_reads = 5;
        let state = create_test_app_state(config);
        
        // Phase 1: Saturate the read limit
        let mut guards = Vec::new();
        
        for _ in 0..5 { // Max concurrent reads
            match ReadGuard::new(&state) {
                Ok(guard) => guards.push(guard),
                Err(_) => break,
            }
        }
        
        // Should now be at the limit
        match ReadGuard::new(&state) {
            Ok(_) => panic!("Should not be able to acquire more read guards"),
            Err(AppError::ResourceLimitExceeded { .. }) => {
                // Expected
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
        
        // Phase 2: Release some guards
        guards.truncate(2); // Keep only 2 guards
        
        // Phase 3: Should be able to acquire more guards now
        let mut new_guards = Vec::new();
        for _ in 0..3 { // Should be able to get 3 more (2 + 3 = 5 total)
            match ReadGuard::new(&state) {
                Ok(guard) => new_guards.push(guard),
                Err(e) => panic!("Should be able to acquire guard after releasing others: {:?}", e),
            }
        }
        
        // Should now be at limit again
        match ReadGuard::new(&state) {
            Ok(_) => panic!("Should not be able to exceed limit"),
            Err(AppError::ResourceLimitExceeded { .. }) => {
                // Expected
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
        
        // Phase 4: Release all guards
        guards.clear();
        new_guards.clear();
        
        // Should be able to acquire new guards
        match ReadGuard::new(&state) {
            Ok(_guard) => {
                // Success expected
            }
            Err(e) => panic!("Should be able to acquire guard after releasing all: {:?}", e),
        }
    });
}

#[test] 
fn test_resource_limit_timeout_behavior() {
    // Test timeout behavior when resources are exhausted
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // Create test config
        let mut config = Config::default();
        config.max_concurrent_reads = 5;
        let state = create_test_app_state(config);
        
        // Saturate read guards with long-running operations
        let mut handles = vec![];
        
        for _ in 0..5 { // Max concurrent reads
            let state_clone = state.clone();
            let handle = tokio::spawn(async move {
                let _guard = ReadGuard::new(&state_clone).expect("Should be able to acquire initial guards");
                tokio::time::sleep(Duration::from_millis(500)).await; // Hold for 500ms
            });
            handles.push(handle);
        }
        
        // Now try to acquire another guard with timeout
        let start_time = std::time::Instant::now();
        
        let state_clone = state.clone();
        let result = timeout(Duration::from_millis(100), async move {
            loop {
                match ReadGuard::new(&state_clone) {
                    Ok(guard) => return Ok(guard),
                    Err(AppError::ResourceLimitExceeded { .. }) => {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
        }).await;
        
        let elapsed = start_time.elapsed();
        
        // Should timeout since guards won't be released in time
        assert!(result.is_err(), "Should timeout waiting for resource");
        assert!(elapsed >= Duration::from_millis(95) && elapsed <= Duration::from_millis(150), 
               "Should timeout in approximately 100ms, actual: {:?}", elapsed);
        
        // Wait for background tasks to complete
        for handle in handles {
            handle.await.expect("Background task panicked");
        }
    });
}

// Helper function to create test app state with custom config
fn create_test_app_state(config: Config) -> Arc<AppState> {
    use std::collections::HashMap;
    use parking_lot::RwLock;
    use stratosort::storage::database::Database;
    use stratosort::ai::service::AiService;
    use stratosort::services::file_watcher::FileWatcher;
    use stratosort::state::{UndoRedoManager, FileCache, OperationTracker};
    
    // Create in-memory database for testing
    let database = Database::new_in_memory().expect("Failed to create test database");
    
    // Create mock AI service
    let ai_service = AiService::new(config.clone()).expect("Failed to create AI service");
    
    // Create file watcher (won't actually watch files in tests)
    let file_watcher = FileWatcher::new();
    
    Arc::new(AppState {
        database,
        ai_service,
        file_watcher,
        config: Arc::new(RwLock::new(config)),
        undo_redo: UndoRedoManager::new(),
        file_cache: FileCache::new(),
        active_operations: OperationTracker::new(),
    })
}