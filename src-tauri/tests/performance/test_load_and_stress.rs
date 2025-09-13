use stratosort::storage::database::Database;
use stratosort::commands::*;
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::AppError;
use tauri::{State, test::{mock_app, MockRuntime}};
use sqlx::SqlitePool;
use tempfile::tempdir;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use tokio::time::{sleep, Duration, Instant};
use futures::future::join_all;
use sysinfo::System;

// Performance test configuration
const LIGHT_LOAD_OPERATIONS: usize = 100;
const MEDIUM_LOAD_OPERATIONS: usize = 500;
const HEAVY_LOAD_OPERATIONS: usize = 2000;
const STRESS_TEST_OPERATIONS: usize = 5000;

// Performance thresholds (adjust based on hardware)
const MAX_RESPONSE_TIME_MS: u64 = 1000;
const MAX_MEMORY_INCREASE_MB: u64 = 500;
const MIN_SUCCESS_RATE: f64 = 0.95;

// Helper to create test app state
async fn create_performance_test_state() -> Arc<AppState> {
    let app = mock_app();
    let config = Config::default();
    
    match AppState::new(app.clone(), config).await {
        Ok(state) => Arc::new(state),
        Err(_) => panic!("Could not create app state for performance testing"),
    }
}

#[tokio::test]
async fn test_database_performance_under_load() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("perf_test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Arc::new(Database::new(pool).await.unwrap());
    
    println!("Testing database performance under various loads");
    
    let load_scenarios = vec![
        ("light_load", LIGHT_LOAD_OPERATIONS),
        ("medium_load", MEDIUM_LOAD_OPERATIONS),
        ("heavy_load", HEAVY_LOAD_OPERATIONS),
    ];
    
    for (scenario_name, num_operations) in load_scenarios {
        println!("\n=== Testing {} with {} operations ===", scenario_name, num_operations);
        
        let start_time = Instant::now();
        let success_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        
        // Create concurrent database operations
        let tasks: Vec<_> = (0..num_operations).map(|i| {
            let db_clone = Arc::clone(&db);
            let success_counter = Arc::clone(&success_count);
            let error_counter = Arc::clone(&error_count);
            
            tokio::spawn(async move {
                let file_path = format!("perf_test_{}_{}.txt", scenario_name, i);
                let content = format!("Performance test content for operation {} in scenario {}", i, scenario_name);
                
                let op_start = Instant::now();
                let result = db_clone.store_file_analysis(&file_path, &content, "text/plain", None).await;
                let op_duration = op_start.elapsed();
                
                match result {
                    Ok(_) => {
                        success_counter.fetch_add(1, Ordering::SeqCst);
                        op_duration
                    }
                    Err(e) => {
                        error_counter.fetch_add(1, Ordering::SeqCst);
                        println!("Operation {} failed: {:?}", i, e);
                        op_duration
                    }
                }
            })
        }).collect();
        
        // Wait for all operations to complete
        let operation_durations: Vec<_> = join_all(tasks).await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();
        
        let total_duration = start_time.elapsed();
        let successes = success_count.load(Ordering::SeqCst);
        let errors = error_count.load(Ordering::SeqCst);
        let success_rate = successes as f64 / num_operations as f64;
        
        // Calculate performance metrics
        let avg_duration = operation_durations.iter().sum::<Duration>() / operation_durations.len() as u32;
        let mut sorted_durations = operation_durations;
        sorted_durations.sort();
        let p95_duration = sorted_durations.get(sorted_durations.len() * 95 / 100).unwrap_or(&Duration::ZERO);
        let p99_duration = sorted_durations.get(sorted_durations.len() * 99 / 100).unwrap_or(&Duration::ZERO);
        
        let ops_per_second = num_operations as f64 / total_duration.as_secs_f64();
        
        println!("Results for {}:", scenario_name);
        println!("  Total duration: {:?}", total_duration);
        println!("  Operations/second: {:.2}", ops_per_second);
        println!("  Success rate: {:.2}% ({}/{})", success_rate * 100.0, successes, num_operations);
        println!("  Errors: {}", errors);
        println!("  Average operation time: {:?}", avg_duration);
        println!("  95th percentile: {:?}", p95_duration);
        println!("  99th percentile: {:?}", p99_duration);
        
        // Performance assertions
        assert!(success_rate >= MIN_SUCCESS_RATE, 
               "Success rate {:.2}% below threshold {:.2}%", 
               success_rate * 100.0, MIN_SUCCESS_RATE * 100.0);
        
        assert!(p95_duration.as_millis() <= MAX_RESPONSE_TIME_MS as u128,
               "95th percentile response time {}ms exceeds threshold {}ms",
               p95_duration.as_millis(), MAX_RESPONSE_TIME_MS);
        
        // Test read performance after writes
        let read_start = Instant::now();
        let read_tasks: Vec<_> = (0..100).map(|i| {
            let db_clone = Arc::clone(&db);
            tokio::spawn(async move {
                let search_term = format!("content {}", i % 10);
                db_clone.search_files_by_content(&search_term, 5).await
            })
        }).collect();
        
        let read_results = join_all(read_tasks).await;
        let read_duration = read_start.elapsed();
        let successful_reads = read_results.into_iter()
            .filter_map(|r| r.ok())
            .filter(|r| r.is_ok())
            .count();
        
        println!("  Read performance: {} successful reads in {:?}", successful_reads, read_duration);
        
        // Clean up for next scenario
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_memory_usage_under_load() {
    let state = create_performance_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    println!("Testing memory usage under load");
    
    let mut system = System::new_all();
    system.refresh_memory();
    let initial_memory = system.used_memory();
    
    println!("Initial memory usage: {} MB", initial_memory / 1024 / 1024);
    
    let memory_test_scenarios = vec![
        ("small_files", 1000, "Small content".repeat(10)),
        ("medium_files", 500, "Medium content ".repeat(100)),
        ("large_files", 100, "Large content ".repeat(1000)),
        ("mixed_sizes", 300, "Mixed content"), // Will vary sizes
    ];
    
    for (scenario_name, num_operations, base_content) in memory_test_scenarios {
        println!("\n=== Testing memory usage: {} ===", scenario_name);
        
        system.refresh_memory();
        let scenario_start_memory = system.used_memory();
        
        let tasks: Vec<_> = (0..num_operations).map(|i| {
            let state_clone = state_ref.clone();
            let content = if scenario_name == "mixed_sizes" {
                match i % 4 {
                    0 => "Small".repeat(10),
                    1 => "Medium ".repeat(100),
                    2 => "Large ".repeat(500),
                    _ => "Variable ".repeat(200),
                }
            } else {
                base_content.clone()
            };
            
            tokio::spawn(async move {
                // Simulate AI analysis which should consume memory
                let analyze_result = ai::analyze_with_ai(
                    content.clone(),
                    "text/plain".to_string(),
                    state_clone.clone()
                ).await;
                
                // Also test embedding generation
                let embedding_result = ai::generate_embeddings(content, state_clone).await;
                
                (analyze_result.is_ok(), embedding_result.is_ok())
            })
        }).collect();
        
        let start_time = Instant::now();
        let results = join_all(tasks).await;
        let duration = start_time.elapsed();
        
        // Measure memory after operations
        system.refresh_memory();
        let scenario_end_memory = system.used_memory();
        let memory_increase = scenario_end_memory.saturating_sub(scenario_start_memory);
        
        let successful_operations = results.into_iter()
            .filter_map(|r| r.ok())
            .filter(|(analyze_ok, embedding_ok)| *analyze_ok || *embedding_ok)
            .count();
        
        println!("Scenario '{}' results:", scenario_name);
        println!("  Operations: {}", num_operations);
        println!("  Successful: {}", successful_operations);
        println!("  Duration: {:?}", duration);
        println!("  Memory before: {} MB", scenario_start_memory / 1024 / 1024);
        println!("  Memory after: {} MB", scenario_end_memory / 1024 / 1024);
        println!("  Memory increase: {} MB", memory_increase / 1024 / 1024);
        println!("  Memory per operation: {} KB", memory_increase / 1024 / num_operations as u64);
        
        // Memory usage assertions
        assert!(memory_increase / 1024 / 1024 <= MAX_MEMORY_INCREASE_MB,
               "Memory increase {} MB exceeds threshold {} MB",
               memory_increase / 1024 / 1024, MAX_MEMORY_INCREASE_MB);
        
        // Allow memory to be reclaimed
        sleep(Duration::from_millis(500)).await;
        system.refresh_memory();
        let cleanup_memory = system.used_memory();
        let memory_reclaimed = scenario_end_memory.saturating_sub(cleanup_memory);
        
        if memory_reclaimed > 0 {
            println!("  Memory reclaimed after cleanup: {} MB", memory_reclaimed / 1024 / 1024);
        }
    }
    
    // Final memory check
    system.refresh_memory();
    let final_memory = system.used_memory();
    let total_memory_increase = final_memory.saturating_sub(initial_memory);
    
    println!("\nOverall memory analysis:");
    println!("  Initial memory: {} MB", initial_memory / 1024 / 1024);
    println!("  Final memory: {} MB", final_memory / 1024 / 1024);
    println!("  Net increase: {} MB", total_memory_increase / 1024 / 1024);
    
    // Overall memory leak check
    assert!(total_memory_increase / 1024 / 1024 <= MAX_MEMORY_INCREASE_MB * 2,
           "Total memory increase {} MB suggests potential memory leak",
           total_memory_increase / 1024 / 1024);
}

#[tokio::test]
async fn test_concurrent_user_simulation() {
    let state = create_performance_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    println!("Testing concurrent user simulation");
    
    let num_users = 50;
    let operations_per_user = 20;
    
    println!("Simulating {} concurrent users with {} operations each", num_users, operations_per_user);
    
    let start_time = Instant::now();
    let overall_success_count = Arc::new(AtomicUsize::new(0));
    let overall_error_count = Arc::new(AtomicUsize::new(0));
    
    // Create tasks for concurrent users
    let user_tasks: Vec<_> = (0..num_users).map(|user_id| {
        let state_clone = state_ref.clone();
        let app_clone = app.clone();
        let success_counter = Arc::clone(&overall_success_count);
        let error_counter = Arc::clone(&overall_error_count);
        
        tokio::spawn(async move {
            let user_start = Instant::now();
            let mut user_successes = 0;
            let mut user_errors = 0;
            
            for operation_id in 0..operations_per_user {
                // Simulate realistic user behavior with different operation types
                let operation_type = operation_id % 6;
                
                let result = match operation_type {
                    0 => {
                        // File analysis
                        let content = format!("User {} content {}", user_id, operation_id);
                        ai::analyze_with_ai(content, "text/plain".to_string(), state_clone.clone())
                            .await.map(|_| ())
                    }
                    1 => {
                        // Search operation
                        let query = format!("user {} search", user_id);
                        ai::semantic_search(query, 10, state_clone.clone())
                            .await.map(|_| ())
                    }
                    2 => {
                        // System info request
                        monitoring::get_system_info(state_clone.clone())
                            .await.map(|_| ())
                    }
                    3 => {
                        // Database stats
                        monitoring::get_database_stats(state_clone.clone())
                            .await.map(|_| ())
                    }
                    4 => {
                        // File scan (limited scope)
                        let temp_dir = tempfile::tempdir().unwrap();
                        let scan_path = temp_dir.path().to_string_lossy().to_string();
                        files::scan_directory(scan_path, state_clone.clone(), app_clone.clone())
                            .await.map(|_| ())
                    }
                    _ => {
                        // Settings read
                        settings::get_setting("theme".to_string(), state_clone.clone())
                            .await.map(|_| ())
                    }
                };
                
                match result {
                    Ok(_) => {
                        user_successes += 1;
                        success_counter.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        user_errors += 1;
                        error_counter.fetch_add(1, Ordering::SeqCst);
                        
                        // Log errors for analysis (but don't spam)
                        if user_errors <= 3 {
                            println!("User {} operation {} failed: {:?}", user_id, operation_id, e);
                        }
                    }
                }
                
                // Simulate realistic user pacing
                if operation_id % 5 == 0 {
                    sleep(Duration::from_millis(50)).await;
                }
            }
            
            let user_duration = user_start.elapsed();
            (user_id, user_successes, user_errors, user_duration)
        })
    }).collect();
    
    // Wait for all users to complete
    let user_results = join_all(user_tasks).await;
    let total_duration = start_time.elapsed();
    
    // Analyze results
    let successful_users = user_results.iter()
        .filter_map(|r| r.as_ref().ok())
        .filter(|(_, successes, _, _)| *successes > operations_per_user / 2)
        .count();
    
    let total_successes = overall_success_count.load(Ordering::SeqCst);
    let total_errors = overall_error_count.load(Ordering::SeqCst);
    let total_operations = num_users * operations_per_user;
    let overall_success_rate = total_successes as f64 / total_operations as f64;
    
    // Calculate user performance statistics
    let user_durations: Vec<_> = user_results.iter()
        .filter_map(|r| r.as_ref().ok().map(|(_, _, _, duration)| *duration))
        .collect();
    
    let avg_user_duration = user_durations.iter().sum::<Duration>() / user_durations.len() as u32;
    let mut sorted_user_durations = user_durations;
    sorted_user_durations.sort();
    let median_user_duration = sorted_user_durations[sorted_user_durations.len() / 2];
    let slowest_user_duration = sorted_user_durations.last().unwrap();
    
    println!("\nConcurrent user simulation results:");
    println!("  Users: {}", num_users);
    println!("  Operations per user: {}", operations_per_user);
    println!("  Total operations: {}", total_operations);
    println!("  Total duration: {:?}", total_duration);
    println!("  Successful operations: {} ({:.1}%)", total_successes, overall_success_rate * 100.0);
    println!("  Failed operations: {}", total_errors);
    println!("  Users with >50% success: {} ({:.1}%)", successful_users, successful_users as f64 / num_users as f64 * 100.0);
    println!("  Average user completion time: {:?}", avg_user_duration);
    println!("  Median user completion time: {:?}", median_user_duration);
    println!("  Slowest user completion time: {:?}", slowest_user_duration);
    println!("  System throughput: {:.2} operations/second", total_operations as f64 / total_duration.as_secs_f64());
    
    // Performance assertions
    assert!(overall_success_rate >= MIN_SUCCESS_RATE,
           "Overall success rate {:.1}% below threshold {:.1}%",
           overall_success_rate * 100.0, MIN_SUCCESS_RATE * 100.0);
    
    assert!(successful_users as f64 / num_users as f64 >= 0.90,
           "Too many users had poor experience: only {} out of {} users succeeded",
           successful_users, num_users);
    
    assert!(slowest_user_duration.as_secs() <= 30,
           "Slowest user took too long: {:?}", slowest_user_duration);
}

#[tokio::test]
async fn test_stress_test_breaking_point() {
    let state = create_performance_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    println!("Running stress test to find system breaking point");
    
    let stress_phases = vec![
        ("warmup", 100, "Warmup phase"),
        ("moderate", 500, "Moderate stress"),
        ("high", 1000, "High stress"),
        ("extreme", 2000, "Extreme stress"),
        ("breaking_point", 5000, "Breaking point test"),
    ];
    
    let mut system_still_stable = true;
    let mut last_successful_load = 0;
    
    for (phase_name, num_operations, description) in stress_phases {
        if !system_still_stable {
            println!("System no longer stable, stopping stress test at {} operations", last_successful_load);
            break;
        }
        
        println!("\n=== {} ({} operations) ===", description, num_operations);
        
        let phase_start = Instant::now();
        let success_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let timeout_count = Arc::new(AtomicUsize::new(0));
        
        // Create stress operations
        let tasks: Vec<_> = (0..num_operations).map(|i| {
            let state_clone = state_ref.clone();
            let success_counter = Arc::clone(&success_count);
            let error_counter = Arc::clone(&error_count);
            let timeout_counter = Arc::clone(&timeout_count);
            
            tokio::spawn(async move {
                let op_start = Instant::now();
                
                // Vary operation types to stress different parts of the system
                let operation_result = match i % 4 {
                    0 => {
                        let content = format!("Stress test content {}", i);
                        ai::analyze_with_ai(content, "text/plain".to_string(), state_clone).await
                            .map(|_| "ai_analysis")
                    }
                    1 => {
                        let query = format!("stress query {}", i % 100);
                        ai::semantic_search(query, 5, state_clone).await
                            .map(|_| "search")
                    }
                    2 => {
                        monitoring::get_system_info(state_clone).await
                            .map(|_| "system_info")
                    }
                    _ => {
                        monitoring::get_database_stats(state_clone).await
                            .map(|_| "db_stats")
                    }
                };
                
                let op_duration = op_start.elapsed();
                
                match operation_result {
                    Ok(op_type) => {
                        if op_duration.as_secs() > 10 {
                            timeout_counter.fetch_add(1, Ordering::SeqCst);
                            println!("Operation {} ({}) timed out after {:?}", i, op_type, op_duration);
                        } else {
                            success_counter.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(e) => {
                        error_counter.fetch_add(1, Ordering::SeqCst);
                        if i < 10 || i % 100 == 0 {
                            println!("Stress operation {} failed: {:?}", i, e);
                        }
                    }
                }
                
                op_duration
            })
        }).collect();
        
        // Set a timeout for the entire phase
        let phase_timeout = Duration::from_secs(60);
        let phase_result = tokio::time::timeout(phase_timeout, join_all(tasks)).await;
        
        let phase_duration = phase_start.elapsed();
        let successes = success_count.load(Ordering::SeqCst);
        let errors = error_count.load(Ordering::SeqCst);
        let timeouts = timeout_count.load(Ordering::SeqCst);
        
        match phase_result {
            Ok(operation_results) => {
                let completed_operations = operation_results.into_iter()
                    .filter_map(|r| r.ok())
                    .collect::<Vec<_>>();
                
                let success_rate = successes as f64 / num_operations as f64;
                let completion_rate = completed_operations.len() as f64 / num_operations as f64;
                
                println!("Phase '{}' completed:", phase_name);
                println!("  Duration: {:?}", phase_duration);
                println!("  Completion rate: {:.1}% ({}/{})", completion_rate * 100.0, completed_operations.len(), num_operations);
                println!("  Success rate: {:.1}% ({}/{})", success_rate * 100.0, successes, num_operations);
                println!("  Errors: {} ({:.1}%)", errors, errors as f64 / num_operations as f64 * 100.0);
                println!("  Timeouts: {} ({:.1}%)", timeouts, timeouts as f64 / num_operations as f64 * 100.0);
                
                // Determine if system is still stable
                if success_rate >= 0.80 && completion_rate >= 0.90 {
                    system_still_stable = true;
                    last_successful_load = num_operations;
                    println!("  System remains stable at {} operations", num_operations);
                } else {
                    system_still_stable = false;
                    println!("  System showing instability: success_rate={:.1}%, completion_rate={:.1}%", 
                            success_rate * 100.0, completion_rate * 100.0);
                }
            }
            Err(_) => {
                println!("Phase '{}' timed out after {:?}", phase_name, phase_timeout);
                println!("  Partial results: {} successes, {} errors, {} timeouts", successes, errors, timeouts);
                system_still_stable = false;
            }
        }
        
        // Test system recovery between phases
        println!("Testing system recovery...");
        let recovery_start = Instant::now();
        
        match monitoring::get_system_info(state_ref.clone()).await {
            Ok(_) => {
                let recovery_time = recovery_start.elapsed();
                println!("  System responsive after stress phase: {:?}", recovery_time);
                
                if recovery_time.as_secs() > 5 {
                    println!("  WARNING: System recovery is slow");
                }
            }
            Err(e) => {
                println!("  System not responsive after stress phase: {:?}", e);
                system_still_stable = false;
            }
        }
        
        // Allow system to cool down between phases
        sleep(Duration::from_millis(1000)).await;
    }
    
    println!("\nStress test summary:");
    println!("  Maximum stable load: {} operations", last_successful_load);
    
    if system_still_stable {
        println!("  System handled all stress phases successfully");
    } else {
        println!("  System reached breaking point at {} operations", last_successful_load);
    }
    
    // Ensure system can still handle basic operations after stress test
    let final_health_check = monitoring::get_system_info(state_ref.clone()).await;
    match final_health_check {
        Ok(_) => println!("  Final health check: PASSED"),
        Err(e) => {
            println!("  Final health check: FAILED - {:?}", e);
            panic!("System did not recover after stress test");
        }
    }
}

#[tokio::test]
async fn test_cache_performance_and_eviction() {
    let state = create_performance_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    println!("Testing cache performance and eviction behavior");
    
    // Test cache warming
    println!("Warming up cache with initial data...");
    let warmup_operations = 100;
    
    let warmup_tasks: Vec<_> = (0..warmup_operations).map(|i| {
        let state_clone = state_ref.clone();
        tokio::spawn(async move {
            let content = format!("Cache warmup content {}", i);
            ai::analyze_with_ai(content, "text/plain".to_string(), state_clone).await
        })
    }).collect();
    
    let warmup_results = join_all(warmup_tasks).await;
    let warmup_successes = warmup_results.into_iter()
        .filter_map(|r| r.ok())
        .filter(|r| r.is_ok())
        .count();
    
    println!("Cache warmed up with {} successful operations", warmup_successes);
    
    // Test cache hit performance
    println!("Testing cache hit performance...");
    let cache_test_operations = 200;
    
    let cache_hit_start = Instant::now();
    let cache_tasks: Vec<_> = (0..cache_test_operations).map(|i| {
        let state_clone = state_ref.clone();
        tokio::spawn(async move {
            // Repeat content from warmup to trigger cache hits
            let content = format!("Cache warmup content {}", i % warmup_operations);
            let op_start = Instant::now();
            let result = ai::analyze_with_ai(content, "text/plain".to_string(), state_clone).await;
            let op_duration = op_start.elapsed();
            
            (result.is_ok(), op_duration)
        })
    }).collect();
    
    let cache_results = join_all(cache_tasks).await;
    let cache_hit_duration = cache_hit_start.elapsed();
    
    let cache_successes = cache_results.iter()
        .filter_map(|r| r.as_ref().ok())
        .filter(|(success, _)| *success)
        .count();
    
    let cache_response_times: Vec<_> = cache_results.into_iter()
        .filter_map(|r| r.ok())
        .map(|(_, duration)| duration)
        .collect();
    
    if !cache_response_times.is_empty() {
        let avg_cache_response = cache_response_times.iter().sum::<Duration>() / cache_response_times.len() as u32;
        let mut sorted_times = cache_response_times;
        sorted_times.sort();
        let median_cache_response = sorted_times[sorted_times.len() / 2];
        
        println!("Cache performance results:");
        println!("  Total cache operations: {}", cache_test_operations);
        println!("  Successful operations: {}", cache_successes);
        println!("  Total duration: {:?}", cache_hit_duration);
        println!("  Average response time: {:?}", avg_cache_response);
        println!("  Median response time: {:?}", median_cache_response);
        println!("  Operations/second: {:.2}", cache_test_operations as f64 / cache_hit_duration.as_secs_f64());
        
        // Cache performance should be fast
        assert!(avg_cache_response.as_millis() <= 100,
               "Average cache response time {}ms is too slow", avg_cache_response.as_millis());
    }
    
    // Test cache eviction under pressure
    println!("Testing cache eviction under pressure...");
    let pressure_operations = 1000;
    
    let pressure_tasks: Vec<_> = (0..pressure_operations).map(|i| {
        let state_clone = state_ref.clone();
        tokio::spawn(async move {
            // Create unique content to force cache eviction
            let content = format!("Unique cache pressure content {} - {}", i, "x".repeat(100));
            ai::analyze_with_ai(content, "text/plain".to_string(), state_clone).await
        })
    }).collect();
    
    let pressure_start = Instant::now();
    let pressure_results = join_all(pressure_tasks).await;
    let pressure_duration = pressure_start.elapsed();
    
    let pressure_successes = pressure_results.into_iter()
        .filter_map(|r| r.ok())
        .filter(|r| r.is_ok())
        .count();
    
    println!("Cache pressure test results:");
    println!("  Pressure operations: {}", pressure_operations);
    println!("  Successful operations: {}", pressure_successes);
    println!("  Duration: {:?}", pressure_duration);
    println!("  Operations/second: {:.2}", pressure_operations as f64 / pressure_duration.as_secs_f64());
    
    // Verify system is still responsive after cache pressure
    let post_pressure_test = monitoring::get_system_info(state_ref.clone()).await;
    match post_pressure_test {
        Ok(_) => println!("  System remains responsive after cache pressure"),
        Err(e) => {
            println!("  WARNING: System not responsive after cache pressure: {:?}", e);
        }
    }
}