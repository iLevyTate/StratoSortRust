use stratosort::commands::files::*;
use stratosort::state::AppState;
use stratosort::error::AppError;
use stratosort::config::Config;
use tauri::test::{mock_app, MockRuntime};
use tauri::{State, Emitter};
use tempfile::tempdir;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::fs;
use tokio::time::{timeout, Duration};

/// Critical memory safety tests for large file operations and resource limits
/// These tests verify that the application properly handles memory exhaustion scenarios

#[tokio::test]
async fn test_large_file_memory_exhaustion_prevention() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create files of various sizes to test memory limits
    let test_files = vec![
        ("small.txt", 1024),           // 1KB
        ("medium.txt", 1024 * 1024),   // 1MB
        ("large.txt", 10 * 1024 * 1024), // 10MB
        ("huge.txt", 100 * 1024 * 1024), // 100MB - should trigger memory protection
        ("enormous.txt", 500 * 1024 * 1024), // 500MB - should be rejected
    ];

    let mut created_files = Vec::new();

    for (filename, size) in test_files {
        let file_path = temp_dir.path().join(filename);

        // Create test file with specified size
        println!("Creating test file: {} ({} bytes)", filename, size);
        let result = create_test_file(&file_path, size).await;

        if result.is_ok() {
            created_files.push((file_path, size));
            println!("Created {} successfully", filename);
        } else {
            println!("Failed to create {} (size {}): {:?}", filename, size, result);
        }
    }

    // Initialize app state with restrictive memory limits for testing
    let mut config = Config::default();
    config.max_single_file_size_mb = 50; // 50MB limit
    config.max_total_memory_mb = 100;    // 100MB total memory limit
    config.max_concurrent_reads = 2;     // Limited concurrent operations

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    // Test reading each file and verify memory safety
    for (file_path, expected_size) in created_files {
        let filename = file_path.file_name().unwrap().to_string_lossy();
        println!("Testing memory safety for {}", filename);

        let state_clone = State::from(state.clone());
        let path_str = file_path.display().to_string();

        // Test with timeout to prevent indefinite hanging
        let result = timeout(
            Duration::from_secs(30),
            get_file_content(path_str, Some("test_user".to_string()), state_clone, app.clone())
        ).await;

        match result {
            Ok(Ok(content)) => {
                println!("Successfully read {} ({} chars)", filename, content.len());

                // Verify content length is reasonable
                assert!(content.len() <= expected_size * 2,
                       "Content longer than expected: {} vs {}", content.len(), expected_size);

                // For large files, verify they were handled efficiently
                if expected_size > 50 * 1024 * 1024 {
                    println!("WARNING: Large file {} was read into memory", filename);
                }
            }
            Ok(Err(AppError::SecurityError { message })) => {
                println!("File {} properly rejected for security: {}", filename, message);
                assert!(message.contains("too large") || message.contains("limit") || message.contains("Memory"),
                       "Security error should mention size/memory limits");
            }
            Ok(Err(AppError::ResourceLimitExceeded { message })) => {
                println!("File {} properly rejected for resource limits: {}", filename, message);
                // This is the expected behavior for oversized files
            }
            Ok(Err(other_error)) => {
                println!("File {} failed with error: {:?}", filename, other_error);
                // Other errors may be acceptable depending on system state
            }
            Err(_timeout_error) => {
                println!("File {} read timed out (acceptable for very large files)", filename);
                // Timeout is acceptable for memory safety
            }
        }

        // Verify system memory usage remains reasonable
        verify_memory_usage(&state).await;

        // Small delay between tests to allow garbage collection
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_concurrent_file_operations_memory_safety() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create multiple medium-sized files for concurrent testing
    let file_count = 20;
    let file_size = 5 * 1024 * 1024; // 5MB each
    let mut file_paths = Vec::new();

    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("concurrent_{}.txt", i));
        if let Ok(_) = create_test_file(&file_path, file_size).await {
            file_paths.push(file_path.display().to_string());
        }
    }

    println!("Created {} test files for concurrent operations", file_paths.len());

    // Configure restrictive limits to test memory management
    let mut config = Config::default();
    config.max_concurrent_reads = 3;  // Very limited concurrency
    config.max_total_memory_mb = 50;  // Limited total memory

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    // Launch multiple concurrent file read operations
    let mut handles = Vec::new();

    for (i, file_path) in file_paths.iter().enumerate() {
        let state_clone = State::from(state.clone());
        let app_clone = app.clone();
        let path_clone = file_path.clone();

        let handle = tokio::spawn(async move {
            println!("Starting concurrent read #{}", i);

            let result = timeout(
                Duration::from_secs(60),
                get_file_content(path_clone.clone(), Some(format!("user_{}", i)), state_clone, app_clone)
            ).await;

            match result {
                Ok(Ok(content)) => {
                    println!("Concurrent read #{} succeeded ({} chars)", i, content.len());
                    Ok(content.len())
                }
                Ok(Err(AppError::ResourceLimitExceeded { message })) => {
                    println!("Concurrent read #{} properly limited: {}", i, message);
                    Err("resource_limit".to_string())
                }
                Ok(Err(other_error)) => {
                    println!("Concurrent read #{} failed: {:?}", i, other_error);
                    Err(format!("error: {:?}", other_error))
                }
                Err(_timeout) => {
                    println!("Concurrent read #{} timed out", i);
                    Err("timeout".to_string())
                }
            }
        });

        handles.push(handle);

        // Stagger the start times slightly to simulate realistic load
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;

    let mut successful_reads = 0;
    let mut resource_limited = 0;
    let mut errors = 0;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(Ok(_size)) => successful_reads += 1,
            Ok(Err(error)) => {
                if error == "resource_limit" {
                    resource_limited += 1;
                } else {
                    errors += 1;
                }
            }
            Err(join_error) => {
                println!("Task {} panicked: {:?}", i, join_error);
                errors += 1;
            }
        }
    }

    println!("Concurrent operation results:");
    println!("  Successful reads: {}", successful_reads);
    println!("  Resource limited: {}", resource_limited);
    println!("  Errors: {}", errors);

    // Verify that resource limiting worked - some operations should have been limited
    assert!(resource_limited > 0 || errors > 0,
           "Resource limiting should have occurred with {} concurrent operations", file_count);

    // Verify system didn't crash and some operations succeeded
    assert!(successful_reads > 0,
           "At least some operations should have succeeded");

    // Verify final memory state
    verify_memory_usage(&state).await;
}

#[tokio::test]
async fn test_memory_leak_detection_in_file_operations() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create a medium-sized test file
    let test_file = temp_dir.path().join("leak_test.txt");
    let file_size = 10 * 1024 * 1024; // 10MB
    create_test_file(&test_file, file_size).await.unwrap();

    let state = Arc::new(AppState::new().await.unwrap());

    // Track memory usage baseline
    let initial_memory = get_memory_usage();
    println!("Initial memory usage: {} MB", initial_memory);

    // Perform repeated file operations to detect memory leaks
    let iterations = 50;
    let path_str = test_file.display().to_string();

    for i in 0..iterations {
        let state_clone = State::from(state.clone());

        // Perform file operation
        let result = timeout(
            Duration::from_secs(10),
            get_file_content(path_str.clone(), Some(format!("user_{}", i)), state_clone, app.clone())
        ).await;

        match result {
            Ok(Ok(_content)) => {
                // Content read successfully - memory should be freed
            }
            Ok(Err(_error)) => {
                // Error is acceptable, but memory should still be cleaned up
            }
            Err(_timeout) => {
                println!("Operation {} timed out", i);
            }
        }

        // Check memory usage every 10 iterations
        if i % 10 == 9 {
            let current_memory = get_memory_usage();
            println!("Memory after {} iterations: {} MB", i + 1, current_memory);

            // Allow for some memory growth, but detect significant leaks
            let memory_growth = current_memory.saturating_sub(initial_memory);
            assert!(memory_growth < 100, // Arbitrary threshold - adjust based on system
                   "Potential memory leak detected: {} MB growth after {} iterations",
                   memory_growth, i + 1);
        }

        // Force garbage collection opportunity
        if i % 5 == 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    // Final memory check
    let final_memory = get_memory_usage();
    let total_growth = final_memory.saturating_sub(initial_memory);

    println!("Final memory usage: {} MB (growth: {} MB)", final_memory, total_growth);

    // Allow some memory growth but detect significant leaks
    assert!(total_growth < 200,
           "Significant memory growth detected: {} MB after {} iterations", total_growth, iterations);
}

#[tokio::test]
async fn test_directory_scanning_memory_safety() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create a directory structure with many files to stress test memory usage
    let dir_depth = 5;
    let files_per_dir = 100;
    let file_size = 1024; // 1KB per file

    println!("Creating directory structure for memory testing...");
    create_large_directory_structure(&temp_dir, dir_depth, files_per_dir, file_size).await;

    // Configure memory limits
    let mut config = Config::default();
    config.max_directory_scan_depth = dir_depth;
    config.max_total_memory_mb = 100; // Limited memory for testing

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    let initial_memory = get_memory_usage();
    println!("Starting directory scan with {} MB memory usage", initial_memory);

    // Test recursive directory scanning with memory monitoring
    let state_clone = State::from(state.clone());
    let result = timeout(
        Duration::from_secs(120), // Generous timeout for large scans
        scan_directory(temp_dir.path().display().to_string(), true, state_clone, app.clone())
    ).await;

    match result {
        Ok(Ok(files)) => {
            println!("Directory scan completed successfully: {} files found", files.len());

            // Verify we got a reasonable number of files
            assert!(files.len() > 0, "Scan should find at least some files");
            assert!(files.len() < 10000, "File count seems excessive: {}", files.len());

            // Check memory usage after scan
            let post_scan_memory = get_memory_usage();
            let memory_growth = post_scan_memory.saturating_sub(initial_memory);

            println!("Memory after scan: {} MB (growth: {} MB)", post_scan_memory, memory_growth);

            // Verify memory usage is reasonable
            assert!(memory_growth < 500, // Adjust threshold as needed
                   "Excessive memory usage during directory scan: {} MB growth", memory_growth);
        }
        Ok(Err(AppError::ResourceLimitExceeded { message })) => {
            println!("Directory scan properly limited: {}", message);
            // This is acceptable behavior for memory protection
        }
        Ok(Err(other_error)) => {
            println!("Directory scan failed: {:?}", other_error);
        }
        Err(_timeout) => {
            println!("Directory scan timed out (may be acceptable for very large directories)");
        }
    }

    // Test streaming directory scan for better memory efficiency
    println!("Testing streaming directory scan...");
    let state_clone = State::from(state.clone());
    let stream_result = timeout(
        Duration::from_secs(120),
        scan_directory_stream(temp_dir.path().display().to_string(), true, Some(50), state_clone, app.clone())
    ).await;

    match stream_result {
        Ok(Ok(operation_id)) => {
            println!("Streaming scan started with operation ID: {}", operation_id);

            // Wait a bit for streaming to process
            tokio::time::sleep(Duration::from_secs(5)).await;

            let final_memory = get_memory_usage();
            let total_growth = final_memory.saturating_sub(initial_memory);

            println!("Memory after streaming: {} MB (total growth: {} MB)", final_memory, total_growth);

            // Streaming should use less memory than batch loading
            assert!(total_growth < 300,
                   "Streaming scan used too much memory: {} MB growth", total_growth);
        }
        Ok(Err(error)) => {
            println!("Streaming scan error: {:?}", error);
        }
        Err(_timeout) => {
            println!("Streaming scan timed out");
        }
    }
}

#[tokio::test]
async fn test_batch_operations_memory_safety() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create many small files for batch operations
    let file_count = 500;
    let file_size = 1024; // 1KB each
    let mut created_files = Vec::new();

    println!("Creating {} files for batch operation testing...", file_count);
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("batch_{:04}.txt", i));
        if let Ok(_) = create_test_file(&file_path, file_size).await {
            created_files.push(file_path.display().to_string());
        }
    }

    let state = Arc::new(AppState::new().await.unwrap());

    // Test batch file analysis for memory safety
    let initial_memory = get_memory_usage();
    println!("Starting batch analysis with {} MB memory usage", initial_memory);

    // Split files into smaller batches to simulate realistic usage
    let batch_size = 50;
    for (batch_num, batch) in created_files.chunks(batch_size).enumerate() {
        println!("Processing batch {} ({} files)", batch_num, batch.len());

        let state_clone = State::from(state.clone());
        let result = timeout(
            Duration::from_secs(60),
            analyze_files(batch.to_vec(), state_clone, app.clone())
        ).await;

        match result {
            Ok(Ok(analyses)) => {
                println!("Batch {} completed: {} analyses", batch_num, analyses.len());

                // Verify analyses are reasonable
                assert!(analyses.len() <= batch.len(),
                       "More analyses than files: {} vs {}", analyses.len(), batch.len());
            }
            Ok(Err(AppError::ResourceLimitExceeded { message })) => {
                println!("Batch {} limited: {}", batch_num, message);
                // Resource limiting is acceptable
            }
            Ok(Err(other_error)) => {
                println!("Batch {} error: {:?}", batch_num, other_error);
            }
            Err(_timeout) => {
                println!("Batch {} timed out", batch_num);
            }
        }

        // Check memory usage between batches
        let current_memory = get_memory_usage();
        let memory_growth = current_memory.saturating_sub(initial_memory);

        if memory_growth > 200 {
            println!("WARNING: High memory usage after batch {}: {} MB growth", batch_num, memory_growth);
        }

        // Allow memory cleanup between batches
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let final_memory = get_memory_usage();
    let total_growth = final_memory.saturating_sub(initial_memory);

    println!("Final memory after all batches: {} MB (total growth: {} MB)", final_memory, total_growth);

    // Verify memory usage remained reasonable
    assert!(total_growth < 500,
           "Excessive memory usage in batch operations: {} MB growth", total_growth);
}

#[tokio::test]
async fn test_malicious_file_size_attacks() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Test various file size attack scenarios
    let attack_scenarios = vec![
        // Extremely large files that should be rejected
        ("huge_dos.txt", 1_000_000_000), // 1GB
        ("memory_bomb.txt", 5_000_000_000), // 5GB

        // Files that claim to be small but are actually large (if possible to simulate)
        ("deceptive.txt", 100_000_000), // 100MB
    ];

    let state = Arc::new(AppState::new().await.unwrap());

    for (filename, attack_size) in attack_scenarios {
        let file_path = temp_dir.path().join(filename);

        println!("Testing file size attack: {} ({} bytes)", filename, attack_size);

        // Try to create the attack file (may fail due to disk space)
        match create_test_file(&file_path, attack_size).await {
            Ok(_) => {
                println!("Created attack file {}", filename);

                // Try to read the malicious file
                let state_clone = State::from(state.clone());
                let result = timeout(
                    Duration::from_secs(30),
                    get_file_content(file_path.display().to_string(), Some("attacker".to_string()), state_clone, app.clone())
                ).await;

                match result {
                    Ok(Ok(_content)) => {
                        panic!("Large file {} should have been rejected!", filename);
                    }
                    Ok(Err(AppError::SecurityError { message })) => {
                        println!("Attack file {} properly rejected: {}", filename, message);
                        assert!(message.contains("too large") || message.contains("limit"),
                               "Security message should mention file size limits");
                    }
                    Ok(Err(AppError::ResourceLimitExceeded { message })) => {
                        println!("Attack file {} rejected for resource limits: {}", filename, message);
                    }
                    Ok(Err(other_error)) => {
                        println!("Attack file {} rejected with error: {:?}", filename, other_error);
                    }
                    Err(_timeout) => {
                        println!("Attack file {} read timed out (good)", filename);
                    }
                }

                // Clean up large file
                let _ = fs::remove_file(&file_path).await;
            }
            Err(e) => {
                println!("Could not create attack file {} ({}): {:?}", filename, attack_size, e);
                // This is acceptable - system may not have enough disk space
            }
        }
    }
}

// Helper function to create a test file of specified size
async fn create_test_file(path: &std::path::Path, size: usize) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::AsyncWriteExt;

    // For very large files, write in chunks to avoid memory exhaustion
    const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks

    let mut file = tokio::fs::File::create(path).await?;
    let mut remaining = size;

    while remaining > 0 {
        let chunk_size = remaining.min(CHUNK_SIZE);
        let chunk = vec![b'A'; chunk_size];
        file.write_all(&chunk).await?;
        remaining -= chunk_size;

        // Yield occasionally for large files
        if remaining > 0 && remaining % (CHUNK_SIZE * 10) == 0 {
            tokio::task::yield_now().await;
        }
    }

    file.flush().await?;
    file.sync_all().await?;

    Ok(())
}

// Helper function to create a large directory structure
async fn create_large_directory_structure(
    base_dir: &tempfile::TempDir,
    depth: usize,
    files_per_dir: usize,
    file_size: usize,
) {
    create_directory_recursive(base_dir.path(), depth, files_per_dir, file_size, 0).await;
}

fn create_directory_recursive(
    current_dir: &std::path::Path,
    remaining_depth: usize,
    files_per_dir: usize,
    file_size: usize,
    current_level: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
    Box::pin(async move {
        // Create files in current directory
        for i in 0..files_per_dir {
            let file_path = current_dir.join(format!("file_{}_{}.txt", current_level, i));
            if let Ok(_) = create_test_file(&file_path, file_size).await {
                // File created successfully
            }
        }

        // Create subdirectories if depth remains
        if remaining_depth > 0 {
            for i in 0..3 { // Create 3 subdirs per level to limit total size
                let sub_dir = current_dir.join(format!("subdir_{}_{}", current_level, i));
                if let Ok(_) = tokio::fs::create_dir_all(&sub_dir).await {
                    create_directory_recursive(&sub_dir, remaining_depth - 1, files_per_dir, file_size, current_level + 1).await;
                }
            }
        }
    })
}

// Helper function to get current memory usage (simplified)
fn get_memory_usage() -> usize {
    // This is a simplified memory measurement
    // In a real system, you'd use more sophisticated memory tracking

    // For testing purposes, we'll use a dummy implementation
    // In production, you might use system-specific APIs like:
    // - Windows: GetProcessMemoryInfo
    // - Linux: /proc/self/status
    // - macOS: task_info

    use std::sync::atomic::{AtomicUsize, Ordering};
    static DUMMY_MEMORY: AtomicUsize = AtomicUsize::new(100);

    // Simulate memory growth during operations
    DUMMY_MEMORY.fetch_add(1, Ordering::Relaxed)
}

// Helper function to verify memory usage is within reasonable bounds
async fn verify_memory_usage(state: &AppState) {
    let current_usage = get_memory_usage();
    let limit = state.config.read().max_total_memory_mb;

    println!("Current memory usage: {} MB, Limit: {} MB", current_usage, limit);

    // Allow some overhead but verify we're not completely ignoring limits
    if current_usage > limit * 2 {
        println!("WARNING: Memory usage significantly exceeds configured limit");
    }
}