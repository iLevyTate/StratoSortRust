use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;

use stratosort::state::{FileCache, CachedFile};

#[test]
fn test_concurrent_cache_eviction_race_condition() {
    let cache = Arc::new(FileCache::new());
    let num_threads = 10;
    let operations_per_thread = 100;
    
    // Create a small cache to force frequent evictions
    let small_cache = Arc::new(FileCache {
        entries: dashmap::DashMap::new(),
        max_size: 1024, // 1KB to force evictions
    });
    
    let mut handles = vec![];
    
    for thread_id in 0..num_threads {
        let cache_clone = Arc::clone(&small_cache);
        
        let handle = thread::spawn(move || {
            for i in 0..operations_per_thread {
                let file_path = format!("test_file_{}_{}", thread_id, i);
                let content = vec![b'x'; 512]; // 512 bytes per file
                
                let cached_file = CachedFile {
                    path: file_path.clone(),
                    content,
                    mime_type: "text/plain".to_string(),
                    size: 512,
                    accessed: chrono::Utc::now(),
                };
                
                // This should trigger evictions and test the race condition fix
                cache_clone.insert(file_path, cached_file);
                
                // Small delay to increase chance of race conditions
                thread::sleep(Duration::from_micros(1));
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    
    // Verify cache integrity after concurrent operations
    assert!(small_cache.len() <= small_cache.max_size / 512 + 1); // Should not exceed expected size
    assert!(small_cache.current_size() <= small_cache.max_size); // Should respect size limit
    
    // Verify no corrupted entries
    for entry in small_cache.entries.iter() {
        assert!(!entry.key().is_empty());
        assert!(entry.value().size > 0);
        assert!(!entry.value().content.is_empty());
    }
}

#[test]
fn test_cache_size_enforcement_under_pressure() {
    let cache = Arc::new(FileCache::new());
    let num_threads = 5;
    
    let mut handles = vec![];
    
    for thread_id in 0..num_threads {
        let cache_clone = Arc::clone(&cache);
        
        let handle = thread::spawn(move || {
            for i in 0..50 {
                let file_path = format!("large_file_{}_{}", thread_id, i);
                let content = vec![b'y'; 2 * 1024 * 1024]; // 2MB files
                
                let cached_file = CachedFile {
                    path: file_path.clone(),
                    content,
                    mime_type: "application/octet-stream".to_string(),
                    size: 2 * 1024 * 1024,
                    accessed: chrono::Utc::now(),
                };
                
                cache_clone.insert(file_path, cached_file);
            }
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    
    // Cache should not exceed its max size (100MB)
    assert!(cache.current_size() <= cache.max_size);
    
    // Should have evicted excess entries
    let expected_max_entries = cache.max_size / (2 * 1024 * 1024); // 100MB / 2MB
    assert!(cache.len() <= expected_max_entries + 5); // Allow some margin for timing
}

#[test]
fn test_evict_oldest_atomic_operation() {
    let cache = Arc::new(FileCache {
        entries: dashmap::DashMap::new(),
        max_size: 1024, // Small cache
    });
    
    // Pre-populate with timestamped entries
    for i in 0..10 {
        let file_path = format!("timestamped_file_{}", i);
        let mut cached_file = CachedFile {
            path: file_path.clone(),
            content: vec![b'z'; 200],
            mime_type: "text/plain".to_string(),
            size: 200,
            accessed: chrono::Utc::now(),
        };
        
        // Artificially set different access times
        cached_file.accessed = chrono::Utc::now() - chrono::Duration::seconds(10 - i);
        
        cache.entries.insert(file_path, cached_file);
        thread::sleep(Duration::from_millis(1)); // Ensure different timestamps
    }
    
    let initial_count = cache.len();
    
    // Trigger eviction by adding a large file
    let large_file = CachedFile {
        path: "trigger_eviction".to_string(),
        content: vec![b'w'; 800],
        mime_type: "text/plain".to_string(),
        size: 800,
        accessed: chrono::Utc::now(),
    };
    
    cache.insert("trigger_eviction".to_string(), large_file);
    
    // Should have evicted some entries
    assert!(cache.len() < initial_count);
    assert!(cache.current_size() <= cache.max_size);
    
    // Verify oldest entries were evicted (entries with lower indices should be gone)
    assert!(!cache.entries.contains_key("timestamped_file_0"));
    assert!(!cache.entries.contains_key("timestamped_file_1"));
}

#[test]
fn test_concurrent_cache_access_patterns() {
    let cache = Arc::new(FileCache::new());
    let num_readers = 5;
    let num_writers = 3;
    let operations = 50;
    
    let mut handles = vec![];
    
    // Spawn reader threads
    for reader_id in 0..num_readers {
        let cache_clone = Arc::clone(&cache);
        
        let handle = thread::spawn(move || {
            for i in 0..operations {
                let file_path = format!("shared_file_{}", i % 10);
                let _result = cache_clone.get(&file_path);
                thread::sleep(Duration::from_micros(1));
            }
        });
        
        handles.push(handle);
    }
    
    // Spawn writer threads
    for writer_id in 0..num_writers {
        let cache_clone = Arc::clone(&cache);
        
        let handle = thread::spawn(move || {
            for i in 0..operations {
                let file_path = format!("writer_file_{}_{}", writer_id, i);
                let cached_file = CachedFile {
                    path: file_path.clone(),
                    content: vec![b'a'; 1024],
                    mime_type: "text/plain".to_string(),
                    size: 1024,
                    accessed: chrono::Utc::now(),
                };
                
                cache_clone.insert(file_path, cached_file);
                thread::sleep(Duration::from_micros(1));
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    
    // Verify cache integrity
    assert!(cache.current_size() <= cache.max_size);
    
    // Verify all entries are valid
    for entry in cache.entries.iter() {
        assert!(!entry.key().is_empty());
        assert!(entry.value().size > 0);
        assert_eq!(entry.value().content.len(), entry.value().size);
    }
}

#[test]
fn test_cache_cleanup_during_concurrent_operations() {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let cache = Arc::new(FileCache::new());
        
        // Add some old entries
        for i in 0..20 {
            let file_path = format!("old_file_{}", i);
            let mut cached_file = CachedFile {
                path: file_path.clone(),
                content: vec![b'o'; 1024],
                mime_type: "text/plain".to_string(),
                size: 1024,
                accessed: chrono::Utc::now() - chrono::Duration::hours(25), // Old entries
            };
            
            cache.entries.insert(file_path, cached_file);
        }
        
        let initial_count = cache.len();
        
        // Start concurrent cleanup
        let cache_clone = Arc::clone(&cache);
        let cleanup_task = tokio::spawn(async move {
            cache_clone.cleanup_old_entries().await;
        });
        
        // Concurrent insertions during cleanup
        let cache_clone2 = Arc::clone(&cache);
        let insert_task = tokio::spawn(async move {
            for i in 0..10 {
                let file_path = format!("new_file_{}", i);
                let cached_file = CachedFile {
                    path: file_path.clone(),
                    content: vec![b'n'; 512],
                    mime_type: "text/plain".to_string(),
                    size: 512,
                    accessed: chrono::Utc::now(),
                };
                
                cache_clone2.insert(file_path, cached_file);
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        
        // Wait for both tasks
        let _ = tokio::try_join!(cleanup_task, insert_task);
        
        // Old entries should be cleaned up
        assert!(cache.len() < initial_count);
        
        // Verify no old entries remain
        for i in 0..20 {
            let file_path = format!("old_file_{}", i);
            assert!(!cache.entries.contains_key(&file_path));
        }
        
        // New entries should still exist
        for i in 0..10 {
            let file_path = format!("new_file_{}", i);
            assert!(cache.entries.contains_key(&file_path));
        }
    });
}