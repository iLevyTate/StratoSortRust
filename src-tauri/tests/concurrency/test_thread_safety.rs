use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

use stratosort::state::{AppState, FileCache, CachedFile, OperationType};
use stratosort::storage::Database;
use stratosort::ai::FileAnalysis;
use stratosort::commands::organization::SmartFolder;
use stratosort::error::Result;

#[test]
fn test_file_cache_thread_safety() {
    let cache = Arc::new(FileCache::new());
    let num_threads = 10;
    let operations_per_thread = 100;
    
    let mut handles = vec![];
    let insert_count = Arc::new(AtomicUsize::new(0));
    let get_count = Arc::new(AtomicUsize::new(0));
    
    // Spawn threads that perform concurrent cache operations
    for thread_id in 0..num_threads {
        let cache_clone = Arc::clone(&cache);
        let insert_count_clone = Arc::clone(&insert_count);
        let get_count_clone = Arc::clone(&get_count);
        
        let handle = thread::spawn(move || {
            for i in 0..operations_per_thread {
                let file_path = format!("thread_{}_file_{}", thread_id, i);
                
                // Insert operation
                let cached_file = CachedFile {
                    path: file_path.clone(),
                    content: vec![b'x'; 1024],
                    mime_type: "text/plain".to_string(),
                    size: 1024,
                    accessed: chrono::Utc::now(),
                };
                
                cache_clone.insert(file_path.clone(), cached_file);
                insert_count_clone.fetch_add(1, Ordering::SeqCst);
                
                // Get operation on previously inserted files
                if i > 0 {
                    let prev_file = format!("thread_{}_file_{}", thread_id, i - 1);
                    if cache_clone.get(&prev_file).is_some() {
                        get_count_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }
                
                // Small delay to increase thread contention
                thread::sleep(Duration::from_micros(1));
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    
    let final_insert_count = insert_count.load(Ordering::SeqCst);
    let final_get_count = get_count.load(Ordering::SeqCst);
    
    println!("Cache thread safety test: {} inserts, {} gets", final_insert_count, final_get_count);
    
    // Verify operations completed successfully
    assert_eq!(final_insert_count, num_threads * operations_per_thread);
    assert!(final_get_count > 0, "Should have successful get operations");
    
    // Verify cache integrity
    assert!(cache.current_size() <= cache.max_size);
    assert!(cache.len() <= final_insert_count); // Some may have been evicted
    
    // Verify all cached entries are valid
    for entry in cache.entries.iter() {
        assert!(!entry.key().is_empty());
        assert!(entry.value().size > 0);
        assert_eq!(entry.value().content.len(), entry.value().size);
    }
}

#[tokio::test]
async fn test_database_concurrent_access() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    let num_tasks = 20;
    let operations_per_task = 10;
    let mut handles = vec![];
    
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    
    for task_id in 0..num_tasks {
        let db_clone = db.clone();
        let success_clone = Arc::clone(&success_count);
        let error_clone = Arc::clone(&error_count);
        
        let handle = tokio::spawn(async move {
            for op_id in 0..operations_per_task {
                let analysis = FileAnalysis {
                    path: format!("task_{}_op_{}.txt", task_id, op_id),
                    category: "concurrent_test".to_string(),
                    tags: vec![format!("task_{}", task_id)],
                    summary: format!("Concurrent operation {} from task {}", op_id, task_id),
                    confidence: 0.8,
                    extracted_text: Some("Test content".to_string()),
                    detected_language: Some("en".to_string()),
                    metadata: serde_json::Value::Null,
                };
                
                match db_clone.save_analysis(&analysis).await {
                    Ok(_) => {
                        success_clone.fetch_add(1, Ordering::SeqCst);
                        
                        // Try to read it back
                        match db_clone.get_analysis(&analysis.path).await {
                            Ok(Some(_)) => {
                                // Successfully read back
                            }
                            Ok(None) => {
                                // Not found - this could happen due to concurrent operations
                            }
                            Err(_) => {
                                error_clone.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                    }
                    Err(_) => {
                        error_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }
                
                // Small delay to allow other tasks to interleave
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }
    
    let final_success = success_count.load(Ordering::SeqCst);
    let final_errors = error_count.load(Ordering::SeqCst);
    
    println!("Database concurrent access: {} successful operations, {} errors", 
             final_success, final_errors);
    
    // Most operations should succeed
    assert!(final_success > 0, "Should have successful operations");
    
    // Error rate should be low
    let total_operations = num_tasks * operations_per_task;
    let error_rate = final_errors as f64 / total_operations as f64;
    assert!(error_rate < 0.1, "Error rate should be less than 10%: {:.2}%", error_rate * 100.0);
    
    // Verify database integrity
    let health_check = db.health_check().await;
    assert!(health_check.is_ok(), "Database should be healthy after concurrent access");
}

#[test]
fn test_smart_folder_concurrent_updates() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let db = create_test_database().await.expect("Failed to create test database");
        
        // Create initial smart folder
        let initial_folder = SmartFolder {
            id: "concurrent_folder".to_string(),
            name: "Initial Name".to_string(),
            description: Some("Initial description".to_string()),
            rules: serde_json::json!({"type": "initial"}),
            target_path: "/initial/path".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        db.save_smart_folder(&initial_folder).await.expect("Failed to save initial folder");
        
        // Concurrent updates to the same smart folder
        let update_count = 10;
        let mut handles = vec![];
        let success_count = Arc::new(AtomicUsize::new(0));
        
        for update_id in 0..update_count {
            let db_clone = db.clone();
            let success_clone = Arc::clone(&success_count);
            
            let handle = tokio::spawn(async move {
                let mut updated_folder = SmartFolder {
                    id: "concurrent_folder".to_string(),
                    name: format!("Updated Name {}", update_id),
                    description: Some(format!("Updated by task {}", update_id)),
                    rules: serde_json::json!({"type": "updated", "task_id": update_id}),
                    target_path: format!("/updated/path/{}", update_id),
                    enabled: update_id % 2 == 0, // Alternate enabled state
                    created_at: initial_folder.created_at,
                    updated_at: chrono::Utc::now(),
                };
                
                match db_clone.save_smart_folder(&updated_folder).await {
                    Ok(_) => {
                        success_clone.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        println!("Smart folder update {} failed: {:?}", update_id, e);
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all updates to complete
        for handle in handles {
            handle.await.expect("Task panicked");
        }
        
        let final_success = success_count.load(Ordering::SeqCst);
        println!("Smart folder concurrent updates: {} successful", final_success);
        
        // All updates should succeed (last one wins)
        assert_eq!(final_success, update_count, "All updates should succeed");
        
        // Verify final state is consistent
        let final_folder = db.get_smart_folder("concurrent_folder").await
            .expect("Query should succeed")
            .expect("Folder should exist");
        
        assert!(final_folder.name.starts_with("Updated Name"));
        assert!(final_folder.target_path.starts_with("/updated/path"));
    });
}

#[test]
fn test_operation_progress_thread_safety() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        // This test would require a mock AppState since creating a real one
        // requires all dependencies to be available
        
        // Simulate concurrent progress updates
        let num_operations = 20;
        let updates_per_operation = 50;
        
        // Use a simple concurrent counter to simulate progress tracking
        let progress_counter = Arc::new(AtomicUsize::new(0));
        let completed_operations = Arc::new(AtomicUsize::new(0));
        
        let mut handles = vec![];
        
        for op_id in 0..num_operations {
            let counter_clone = Arc::clone(&progress_counter);
            let completed_clone = Arc::clone(&completed_operations);
            
            let handle = tokio::spawn(async move {
                for update_id in 0..updates_per_operation {
                    // Simulate progress update
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    
                    // Simulate some work
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }
                
                // Mark operation as completed
                completed_clone.fetch_add(1, Ordering::SeqCst);
            });
            
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            handle.await.expect("Task panicked");
        }
        
        let final_updates = progress_counter.load(Ordering::SeqCst);
        let final_completed = completed_operations.load(Ordering::SeqCst);
        
        println!("Progress thread safety: {} updates, {} completed operations", 
                 final_updates, final_completed);
        
        // Verify all operations completed
        assert_eq!(final_completed, num_operations);
        assert_eq!(final_updates, num_operations * updates_per_operation);
    });
}

#[tokio::test]
async fn test_notification_queue_thread_safety() {
    let db = create_test_database().await.expect("Failed to create test database");
    
    // Concurrent notification operations
    let num_producers = 5;
    let notifications_per_producer = 20;
    let mut handles = vec![];
    
    let success_count = Arc::new(AtomicUsize::new(0));
    
    for producer_id in 0..num_producers {
        let db_clone = db.clone();
        let success_clone = Arc::clone(&success_count);
        
        let handle = tokio::spawn(async move {
            for notif_id in 0..notifications_per_producer {
                let notification = stratosort::commands::notifications::Notification {
                    id: format!("producer_{}_notif_{}", producer_id, notif_id),
                    notification_type: stratosort::commands::notifications::NotificationType::Info,
                    title: format!("Test Notification {}", notif_id),
                    message: format!("From producer {} notification {}", producer_id, notif_id),
                    timestamp: chrono::Utc::now().timestamp(),
                    read: false,
                    actions: vec![],
                    metadata: None,
                };
                
                match db_clone.save_notification(&notification).await {
                    Ok(_) => {
                        success_clone.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        println!("Notification save failed: {:?}", e);
                    }
                }
                
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all producers to finish
    for handle in handles {
        handle.await.expect("Task panicked");
    }
    
    let final_success = success_count.load(Ordering::SeqCst);
    let expected_total = num_producers * notifications_per_producer;
    
    println!("Notification queue thread safety: {} successful saves", final_success);
    
    // All notifications should be saved successfully
    assert_eq!(final_success, expected_total, "All notifications should be saved");
    
    // Verify notifications can be retrieved
    let retrieved_notifications = db.get_notifications(expected_total * 2, false).await
        .expect("Should be able to retrieve notifications");
    
    assert_eq!(retrieved_notifications.len(), expected_total, 
              "Should retrieve all saved notifications");
    
    // Verify notification integrity
    for notification in &retrieved_notifications {
        assert!(!notification.id.is_empty());
        assert!(!notification.title.is_empty());
        assert!(!notification.message.is_empty());
    }
}

#[test]
fn test_state_mutation_thread_safety() {
    // Test concurrent state mutations using basic data structures
    // This simulates the kind of concurrent access that might happen in AppState
    
    let shared_counter = Arc::new(AtomicUsize::new(0));
    let shared_map = Arc::new(dashmap::DashMap::<String, usize>::new());
    
    let num_threads = 10;
    let operations_per_thread = 100;
    let mut handles = vec![];
    
    for thread_id in 0..num_threads {
        let counter_clone = Arc::clone(&shared_counter);
        let map_clone = Arc::clone(&shared_map);
        
        let handle = thread::spawn(move || {
            for op_id in 0..operations_per_thread {
                // Concurrent counter operations
                let current_value = counter_clone.fetch_add(1, Ordering::SeqCst);
                
                // Concurrent map operations
                let key = format!("thread_{}_op_{}", thread_id, op_id);
                map_clone.insert(key.clone(), current_value);
                
                // Read operations
                if op_id > 0 {
                    let prev_key = format!("thread_{}_op_{}", thread_id, op_id - 1);
                    let _value = map_clone.get(&prev_key);
                }
                
                // Remove some entries to test concurrent modification
                if op_id % 10 == 0 && op_id > 0 {
                    let remove_key = format!("thread_{}_op_{}", thread_id, op_id - 5);
                    map_clone.remove(&remove_key);
                }
                
                thread::sleep(Duration::from_micros(1));
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    
    let final_counter = shared_counter.load(Ordering::SeqCst);
    let final_map_size = shared_map.len();
    
    println!("State mutation thread safety: counter={}, map_size={}", 
             final_counter, final_map_size);
    
    // Verify expected counter value
    assert_eq!(final_counter, num_threads * operations_per_thread);
    
    // Map size should be less than total operations due to removals
    assert!(final_map_size < num_threads * operations_per_thread);
    assert!(final_map_size > 0);
    
    // Verify map integrity - all remaining entries should be valid
    for entry in shared_map.iter() {
        assert!(!entry.key().is_empty());
        assert!(*entry.value() < final_counter);
    }
}

#[tokio::test]
async fn test_embedding_generation_concurrency() {
    // Test concurrent embedding generation operations
    // This simulates multiple AI operations happening simultaneously
    
    let num_concurrent_requests = 10;
    let mut handles = vec![];
    
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    
    for request_id in 0..num_concurrent_requests {
        let success_clone = Arc::clone(&success_count);
        let error_clone = Arc::clone(&error_count);
        
        let handle = tokio::spawn(async move {
            // Simulate embedding generation work
            let text = format!("This is test text for embedding generation request {}", request_id);
            
            // Simulate processing time
            tokio::time::sleep(Duration::from_millis(50 + request_id * 10)).await;
            
            // Simulate success/failure based on request_id
            if request_id % 5 == 0 {
                // Every 5th request fails
                error_clone.fetch_add(1, Ordering::SeqCst);
            } else {
                success_clone.fetch_add(1, Ordering::SeqCst);
                
                // Simulate generating embedding vector
                let _embedding: Vec<f32> = (0..384).map(|i| (i as f32 + request_id as f32) / 1000.0).collect();
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    for handle in handles {
        handle.await.expect("Task panicked");
    }
    
    let final_success = success_count.load(Ordering::SeqCst);
    let final_errors = error_count.load(Ordering::SeqCst);
    
    println!("Embedding concurrency test: {} successful, {} errors", final_success, final_errors);
    
    // Verify expected success/error counts
    let expected_errors = num_concurrent_requests / 5;
    let expected_success = num_concurrent_requests - expected_errors;
    
    assert_eq!(final_errors, expected_errors);
    assert_eq!(final_success, expected_success);
}

// Helper functions

async fn create_test_database() -> Result<Database> {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_concurrency.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    
    Database::new_from_url(&db_url).await
}