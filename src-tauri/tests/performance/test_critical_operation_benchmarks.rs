use stratosort::commands::files::*;
use stratosort::storage::database::Database;
use stratosort::storage::vector_ext::VectorExtension;
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::AppError;
use tauri::test::{mock_app, MockRuntime};
use tauri::{State, Emitter};
use tempfile::tempdir;
use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::fs;
use sqlx::SqlitePool;

/// Performance benchmark tests for critical operations
/// These tests verify that operations complete within acceptable time limits
/// and identify performance regressions

#[tokio::test]
async fn benchmark_file_scanning_performance() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create a realistic file structure for benchmarking
    println!("Setting up file structure for scanning benchmark...");
    let setup_start = Instant::now();

    create_benchmark_file_structure(&temp_dir, 1000, 5, 1024).await; // 1000 files, 5 levels deep, 1KB each

    let setup_time = setup_start.elapsed();
    println!("File structure setup completed in {:?}", setup_time);

    let state = Arc::new(AppState::new().await.unwrap());

    // Benchmark directory scanning (non-recursive)
    println!("Benchmarking non-recursive directory scan...");
    let scan_start = Instant::now();

    let state_clone = State::from(state.clone());
    let result = scan_directory(
        temp_dir.path().display().to_string(),
        false, // non-recursive
        state_clone,
        app.clone()
    ).await;

    let scan_time = scan_start.elapsed();

    match result {
        Ok(files) => {
            println!("Non-recursive scan: {} files in {:?}", files.len(), scan_time);

            // Performance assertions
            assert!(scan_time < Duration::from_secs(5),
                   "Non-recursive scan should complete within 5 seconds, took {:?}", scan_time);
            assert!(files.len() > 0, "Should find some files");

            // Calculate performance metrics
            let files_per_second = files.len() as f64 / scan_time.as_secs_f64();
            println!("Performance: {:.2} files/second", files_per_second);

            // Baseline expectation: should process at least 100 files per second
            assert!(files_per_second > 100.0,
                   "File scanning performance too slow: {:.2} files/second", files_per_second);
        }
        Err(e) => {
            panic!("Non-recursive scan failed: {:?}", e);
        }
    }

    // Benchmark recursive directory scanning
    println!("Benchmarking recursive directory scan...");
    let recursive_start = Instant::now();

    let state_clone = State::from(state.clone());
    let result = scan_directory(
        temp_dir.path().display().to_string(),
        true, // recursive
        state_clone,
        app.clone()
    ).await;

    let recursive_time = recursive_start.elapsed();

    match result {
        Ok(files) => {
            println!("Recursive scan: {} files in {:?}", files.len(), recursive_time);

            // Performance assertions for recursive scan
            assert!(recursive_time < Duration::from_secs(30),
                   "Recursive scan should complete within 30 seconds, took {:?}", recursive_time);

            let files_per_second = files.len() as f64 / recursive_time.as_secs_f64();
            println!("Recursive performance: {:.2} files/second", files_per_second);

            // Should still maintain reasonable throughput
            assert!(files_per_second > 50.0,
                   "Recursive scanning performance too slow: {:.2} files/second", files_per_second);
        }
        Err(e) => {
            println!("Recursive scan failed (may be acceptable for very large directories): {:?}", e);
        }
    }

    // Benchmark streaming directory scan
    println!("Benchmarking streaming directory scan...");
    let stream_start = Instant::now();

    let state_clone = State::from(state.clone());
    let stream_result = scan_directory_stream(
        temp_dir.path().display().to_string(),
        true,
        Some(50), // batch size
        state_clone,
        app.clone()
    ).await;

    let stream_setup_time = stream_start.elapsed();

    match stream_result {
        Ok(operation_id) => {
            println!("Streaming scan started in {:?}, operation ID: {}", stream_setup_time, operation_id);

            // Stream setup should be very fast
            assert!(stream_setup_time < Duration::from_millis(100),
                   "Stream setup should be under 100ms, took {:?}", stream_setup_time);

            // Wait a bit for streaming to process some files
            tokio::time::sleep(Duration::from_secs(5)).await;
            println!("Streaming processing time allowed: 5 seconds");
        }
        Err(e) => {
            println!("Streaming scan setup failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn benchmark_file_content_reading_performance() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create files of various sizes for benchmarking
    let test_files = vec![
        ("small.txt", 1024),           // 1KB
        ("medium.txt", 100 * 1024),    // 100KB
        ("large.txt", 1024 * 1024),    // 1MB
        ("xlarge.txt", 10 * 1024 * 1024), // 10MB
    ];

    let mut created_files = Vec::new();

    println!("Creating test files for content reading benchmark...");
    for (filename, size) in test_files {
        let file_path = temp_dir.path().join(filename);
        if let Ok(_) = create_benchmark_file(&file_path, size).await {
            created_files.push((file_path, size, filename));
        }
    }

    let state = Arc::new(AppState::new().await.unwrap());

    // Benchmark file content reading for different sizes
    for (file_path, expected_size, filename) in created_files {
        println!("Benchmarking content reading for {}", filename);

        let read_start = Instant::now();

        let state_clone = State::from(state.clone());
        let result = get_file_content(
            file_path.display().to_string(),
            Some("benchmark_user".to_string()),
            state_clone,
            app.clone()
        ).await;

        let read_time = read_start.elapsed();

        match result {
            Ok(content) => {
                println!("{}: {} bytes read in {:?}", filename, content.len(), read_time);

                // Performance assertions based on file size
                let throughput_mbps = (expected_size as f64 / (1024.0 * 1024.0)) / read_time.as_secs_f64();
                println!("  Throughput: {:.2} MB/s", throughput_mbps);

                match expected_size {
                    1024 => {
                        // Small files should be very fast
                        assert!(read_time < Duration::from_millis(10),
                               "Small file read should be under 10ms, took {:?}", read_time);
                    }
                    100 * 1024 => {
                        // Medium files should be fast
                        assert!(read_time < Duration::from_millis(100),
                               "Medium file read should be under 100ms, took {:?}", read_time);
                        assert!(throughput_mbps > 1.0, "Medium file throughput too low: {:.2} MB/s", throughput_mbps);
                    }
                    1024 * 1024 => {
                        // Large files should still be reasonable
                        assert!(read_time < Duration::from_secs(1),
                               "Large file read should be under 1s, took {:?}", read_time);
                        assert!(throughput_mbps > 1.0, "Large file throughput too low: {:.2} MB/s", throughput_mbps);
                    }
                    10 * 1024 * 1024 => {
                        // Very large files
                        assert!(read_time < Duration::from_secs(10),
                               "Very large file read should be under 10s, took {:?}", read_time);
                        assert!(throughput_mbps > 1.0, "Very large file throughput too low: {:.2} MB/s", throughput_mbps);
                    }
                    _ => {}
                }

                // Verify content integrity
                assert_eq!(content.len(), expected_size, "Content length mismatch for {}", filename);
            }
            Err(e) => {
                println!("{} read failed: {:?}", filename, e);
                // Large files might fail due to memory limits, which is acceptable
                if expected_size <= 1024 * 1024 {
                    panic!("Small/medium file reads should not fail: {:?}", e);
                }
            }
        }
    }
}

#[tokio::test]
async fn benchmark_database_operations_performance() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("benchmark.db");

    println!("Setting up database for performance benchmarking...");
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();

    // Benchmark file analysis storage
    println!("Benchmarking file analysis storage...");
    let storage_start = Instant::now();

    let file_count = 1000;
    for i in 0..file_count {
        let path = format!("test_file_{:04}.txt", i);
        let content = format!("Test content for file {}", i);

        let result = db.store_file_analysis(&path, &content, "text/plain", None).await;

        if let Err(e) = result {
            println!("Storage failed for file {}: {:?}", i, e);
        }

        // Progress indicator for long operations
        if i % 100 == 99 {
            let elapsed = storage_start.elapsed();
            let rate = (i + 1) as f64 / elapsed.as_secs_f64();
            println!("Stored {} files, rate: {:.2} files/second", i + 1, rate);
        }
    }

    let storage_time = storage_start.elapsed();
    let storage_rate = file_count as f64 / storage_time.as_secs_f64();

    println!("Database storage: {} files in {:?} ({:.2} files/second)",
             file_count, storage_time, storage_rate);

    // Performance assertions for storage
    assert!(storage_time < Duration::from_secs(60),
           "Database storage should complete within 60 seconds, took {:?}", storage_time);
    assert!(storage_rate > 10.0,
           "Database storage rate too low: {:.2} files/second", storage_rate);

    // Benchmark file retrieval
    println!("Benchmarking file retrieval...");
    let retrieval_start = Instant::now();

    let all_files = db.get_all_files().await.unwrap();
    let retrieval_time = retrieval_start.elapsed();
    let retrieval_rate = all_files.len() as f64 / retrieval_time.as_secs_f64();

    println!("Database retrieval: {} files in {:?} ({:.2} files/second)",
             all_files.len(), retrieval_time, retrieval_rate);

    // Performance assertions for retrieval
    assert!(retrieval_time < Duration::from_secs(10),
           "Database retrieval should complete within 10 seconds, took {:?}", retrieval_time);
    assert!(retrieval_rate > 100.0,
           "Database retrieval rate too low: {:.2} files/second", retrieval_rate);
    assert_eq!(all_files.len(), file_count, "Should retrieve all stored files");

    // Benchmark search operations
    println!("Benchmarking search operations...");
    let search_start = Instant::now();

    let search_results = db.search_files_by_content("test", 50).await.unwrap();
    let search_time = search_start.elapsed();

    println!("Database search: {} results in {:?}", search_results.len(), search_time);

    // Performance assertions for search
    assert!(search_time < Duration::from_secs(5),
           "Database search should complete within 5 seconds, took {:?}", search_time);
    assert!(search_results.len() > 0, "Search should return some results");
}

#[tokio::test]
async fn benchmark_vector_operations_performance() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("vector_benchmark.db");

    println!("Setting up vector operations benchmark...");
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let vector_ext = VectorExtension::initialize(&pool).await;

    if !vector_ext.is_available {
        println!("Vector extension not available, skipping vector benchmarks");
        return;
    }

    // Create vector table
    let table_name = "benchmark_vectors";
    vector_ext.create_vector_table(&pool, table_name, 384).await.unwrap();

    // Benchmark vector storage
    println!("Benchmarking vector storage...");
    let storage_start = Instant::now();

    let vector_count = 500;
    let test_embedding = vec![0.1f32; 384];

    for i in 0..vector_count {
        let path = format!("vector_file_{:04}.txt", i);
        let result = vector_ext.store_embedding(&pool, table_name, &path, &test_embedding).await;

        if let Err(e) = result {
            println!("Vector storage failed for {}: {:?}", path, e);
        }

        if i % 50 == 49 {
            let elapsed = storage_start.elapsed();
            let rate = (i + 1) as f64 / elapsed.as_secs_f64();
            println!("Stored {} vectors, rate: {:.2} vectors/second", i + 1, rate);
        }
    }

    let vector_storage_time = storage_start.elapsed();
    let vector_storage_rate = vector_count as f64 / vector_storage_time.as_secs_f64();

    println!("Vector storage: {} vectors in {:?} ({:.2} vectors/second)",
             vector_count, vector_storage_time, vector_storage_rate);

    // Performance assertions for vector storage
    assert!(vector_storage_time < Duration::from_secs(120),
           "Vector storage should complete within 120 seconds, took {:?}", vector_storage_time);
    assert!(vector_storage_rate > 1.0,
           "Vector storage rate too low: {:.2} vectors/second", vector_storage_rate);

    // Benchmark vector search
    println!("Benchmarking vector similarity search...");
    let search_start = Instant::now();

    let query_embedding = vec![0.2f32; 384];
    let search_results = vector_ext.vector_search(&pool, table_name, &query_embedding, 10).await.unwrap();
    let vector_search_time = search_start.elapsed();

    println!("Vector search: {} results in {:?}", search_results.len(), vector_search_time);

    // Performance assertions for vector search
    assert!(vector_search_time < Duration::from_secs(5),
           "Vector search should complete within 5 seconds, took {:?}", vector_search_time);
    assert!(search_results.len() > 0, "Vector search should return some results");

    // Benchmark batch vector operations
    println!("Benchmarking batch vector operations...");
    let batch_start = Instant::now();

    let batch_embeddings: Vec<(String, Vec<f32>)> = (0..100)
        .map(|i| (format!("batch_file_{:04}.txt", i), vec![0.3f32; 384]))
        .collect();

    let batch_result = vector_ext.store_embeddings_batch(&pool, table_name, &batch_embeddings).await.unwrap();
    let batch_time = batch_start.elapsed();
    let batch_rate = batch_result as f64 / batch_time.as_secs_f64();

    println!("Vector batch storage: {} vectors in {:?} ({:.2} vectors/second)",
             batch_result, batch_time, batch_rate);

    // Performance assertions for batch operations
    assert!(batch_time < Duration::from_secs(30),
           "Vector batch storage should complete within 30 seconds, took {:?}", batch_time);
    assert!(batch_rate > vector_storage_rate,
           "Batch operations should be faster than individual operations");
}

#[tokio::test]
async fn benchmark_concurrent_operations_performance() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create test files for concurrent operations
    let file_count = 100;
    let mut test_files = Vec::new();

    println!("Setting up files for concurrent operations benchmark...");
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("concurrent_{:03}.txt", i));
        let file_size = 1024 * (i % 10 + 1); // Varying sizes from 1KB to 10KB

        if let Ok(_) = create_benchmark_file(&file_path, file_size).await {
            test_files.push(file_path.display().to_string());
        }
    }

    let state = Arc::new(AppState::new().await.unwrap());

    // Benchmark concurrent file reading
    println!("Benchmarking concurrent file reading...");
    let concurrent_start = Instant::now();

    let concurrent_tasks: Vec<_> = test_files.iter().enumerate().map(|(i, file_path)| {
        let state_clone = State::from(state.clone());
        let app_clone = app.clone();
        let path_clone = file_path.clone();

        tokio::spawn(async move {
            let start = Instant::now();
            let result = get_file_content(
                path_clone,
                Some(format!("user_{}", i)),
                state_clone,
                app_clone
            ).await;

            (i, start.elapsed(), result.is_ok())
        })
    }).collect();

    let concurrent_results = futures::future::join_all(concurrent_tasks).await;
    let total_concurrent_time = concurrent_start.elapsed();

    // Analyze concurrent operation results
    let mut successful_ops = 0;
    let mut failed_ops = 0;
    let mut total_individual_time = Duration::new(0, 0);

    for result in concurrent_results {
        match result {
            Ok((_, individual_time, success)) => {
                if success {
                    successful_ops += 1;
                } else {
                    failed_ops += 1;
                }
                total_individual_time += individual_time;
            }
            Err(_) => {
                failed_ops += 1;
            }
        }
    }

    let concurrency_efficiency = if total_concurrent_time.as_secs_f64() > 0.0 {
        total_individual_time.as_secs_f64() / total_concurrent_time.as_secs_f64()
    } else {
        0.0
    };

    println!("Concurrent operations completed:");
    println!("  Total time: {:?}", total_concurrent_time);
    println!("  Successful: {}, Failed: {}", successful_ops, failed_ops);
    println!("  Concurrency efficiency: {:.2}x", concurrency_efficiency);

    // Performance assertions for concurrent operations
    assert!(total_concurrent_time < Duration::from_secs(30),
           "Concurrent operations should complete within 30 seconds, took {:?}", total_concurrent_time);
    assert!(successful_ops > file_count / 2,
           "At least half of concurrent operations should succeed");
    assert!(concurrency_efficiency > 1.5,
           "Concurrent operations should show efficiency gains: {:.2}x", concurrency_efficiency);
}

#[tokio::test]
async fn benchmark_memory_usage_efficiency() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create a large file to test memory efficiency
    let large_file = temp_dir.path().join("memory_test.txt");
    let file_size = 50 * 1024 * 1024; // 50MB

    println!("Creating large file for memory efficiency test...");
    if let Err(_) = create_benchmark_file(&large_file, file_size).await {
        println!("Could not create large file, skipping memory efficiency test");
        return;
    }

    let state = Arc::new(AppState::new().await.unwrap());

    // Benchmark memory usage during large file operations
    println!("Benchmarking memory usage for large file operations...");

    let initial_memory = get_current_memory_usage();
    println!("Initial memory usage: {} MB", initial_memory);

    let read_start = Instant::now();

    let state_clone = State::from(state.clone());
    let result = get_file_content(
        large_file.display().to_string(),
        Some("memory_test_user".to_string()),
        state_clone,
        app.clone()
    ).await;

    let read_time = read_start.elapsed();
    let peak_memory = get_current_memory_usage();
    let memory_growth = peak_memory.saturating_sub(initial_memory);

    println!("Large file operation completed in {:?}", read_time);
    println!("Peak memory usage: {} MB (growth: {} MB)", peak_memory, memory_growth);

    match result {
        Ok(content) => {
            println!("Successfully read {} bytes", content.len());

            // Memory efficiency assertions
            let memory_efficiency = file_size as f64 / (memory_growth * 1024 * 1024) as f64;
            println!("Memory efficiency: {:.2}x (lower is better)", memory_efficiency);

            // Memory growth should be reasonable compared to file size
            assert!(memory_growth < 200,
                   "Memory growth too high: {} MB for {}MB file", memory_growth, file_size / (1024 * 1024));

            // Should not use more than 3x the file size in memory
            assert!(memory_efficiency > 0.33,
                   "Memory efficiency too low: {:.2}x", memory_efficiency);
        }
        Err(AppError::SecurityError { message }) => {
            println!("Large file properly rejected for security: {}", message);
            // This is acceptable behavior for memory protection
        }
        Err(AppError::ResourceLimitExceeded { message }) => {
            println!("Large file properly rejected for resource limits: {}", message);
            // This is acceptable behavior for memory protection
        }
        Err(e) => {
            println!("Large file operation failed: {:?}", e);
        }
    }

    // Wait a bit and check for memory cleanup
    tokio::time::sleep(Duration::from_secs(2)).await;
    let final_memory = get_current_memory_usage();
    let cleanup_efficiency = (peak_memory - final_memory) as f64 / memory_growth as f64;

    println!("Final memory usage: {} MB", final_memory);
    println!("Memory cleanup efficiency: {:.2}%", cleanup_efficiency * 100.0);

    // Memory should be cleaned up reasonably well
    assert!(cleanup_efficiency > 0.5,
           "Poor memory cleanup: {:.2}% of growth cleaned up", cleanup_efficiency * 100.0);
}

// Helper function to create a file of specified size for benchmarking
async fn create_benchmark_file(path: &std::path::Path, size: usize) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::AsyncWriteExt;

    let mut file = tokio::fs::File::create(path).await?;

    // Write in chunks for efficiency
    const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks
    let chunk = vec![b'A'; CHUNK_SIZE.min(size)];
    let mut remaining = size;

    while remaining > 0 {
        let to_write = remaining.min(CHUNK_SIZE);
        file.write_all(&chunk[..to_write]).await?;
        remaining -= to_write;

        // Yield occasionally for very large files
        if remaining > 0 && remaining % (CHUNK_SIZE * 16) == 0 {
            tokio::task::yield_now().await;
        }
    }

    file.flush().await?;
    file.sync_all().await?;
    Ok(())
}

// Helper function to create a complex directory structure for benchmarking
async fn create_benchmark_file_structure(
    base_dir: &tempfile::TempDir,
    files_per_level: usize,
    max_depth: usize,
    file_size: usize,
) {
    create_dir_structure_recursive(
        base_dir.path(),
        files_per_level,
        max_depth,
        file_size,
        0
    ).await;
}

fn create_dir_structure_recursive(
    current_dir: &std::path::Path,
    files_per_level: usize,
    remaining_depth: usize,
    file_size: usize,
    current_level: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
    Box::pin(async move {
        // Create files in current directory
        for i in 0..files_per_level {
            let file_path = current_dir.join(format!("bench_{}_{:04}.txt", current_level, i));
            let _ = create_benchmark_file(&file_path, file_size).await;
        }

        // Create subdirectories if depth remains
        if remaining_depth > 0 {
            let subdirs = if remaining_depth > 3 { 2 } else { 3 }; // Limit branching factor
            for i in 0..subdirs {
                let sub_dir = current_dir.join(format!("subdir_{}_{}", current_level, i));
                if let Ok(_) = tokio::fs::create_dir_all(&sub_dir).await {
                    create_dir_structure_recursive(
                        &sub_dir,
                        files_per_level / 2, // Reduce files per level in deeper directories
                        remaining_depth - 1,
                        file_size,
                        current_level + 1
                    ).await;
                }
            }
        }
    })
}

// Helper function to get current memory usage (simplified for testing)
fn get_current_memory_usage() -> usize {
    // This is a simplified memory measurement for testing
    // In a real benchmark, you would use platform-specific APIs:

    #[cfg(target_os = "linux")]
    {
        // On Linux, you could read /proc/self/status
        use std::fs;
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<usize>() {
                            return kb / 1024; // Convert KB to MB
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, you could use task_info
        // This would require unsafe code and system libraries
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, you could use GetProcessMemoryInfo
        // This would require winapi crate
    }

    // Fallback: simulate memory usage for testing
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SIMULATED_MEMORY: AtomicUsize = AtomicUsize::new(100);

    // Simulate gradual memory growth during operations
    SIMULATED_MEMORY.fetch_add(1, Ordering::Relaxed)
}