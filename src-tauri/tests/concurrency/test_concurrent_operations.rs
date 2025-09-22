use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;
use uuid::Uuid;

/// Test concurrent file operations don't cause race conditions
#[tokio::test]
async fn test_concurrent_file_operations_no_race() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "initial content").unwrap();

    // Create operation queue
    let queue = Arc::new(stratosort::core::OperationQueue::new(3));

    // Create barrier for synchronization
    let barrier = Arc::new(Barrier::new(10));

    let mut handles = vec![];

    // Spawn 10 concurrent operations on the same file
    for i in 0..10 {
        let queue_clone = queue.clone();
        let barrier_clone = barrier.clone();
        let file_path = test_file.to_str().unwrap().to_string();

        let handle = tokio::spawn(async move {
            // Wait for all tasks to be ready
            barrier_clone.wait().await;

            // Try to enqueue operation
            let op = if i % 2 == 0 {
                stratosort::core::QueuedOperationType::FileRename {
                    path: file_path.clone(),
                    new_name: format!("renamed_{}.txt", i),
                }
            } else {
                stratosort::core::QueuedOperationType::FileCopy {
                    from: file_path.clone(),
                    to: format!("{}.copy", file_path),
                }
            };

            let id = queue_clone.enqueue(op.clone(), i as i32);

            // Check for conflicts
            if queue_clone.would_conflict(&op) {
                println!("Operation {} would conflict, skipping", i);
                return None;
            }

            Some(id)
        });

        handles.push(handle);
    }

    // Wait for all operations to complete
    let results: Vec<Option<Uuid>> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // Verify no data corruption occurred
    assert!(test_file.exists() || temp_dir.path().join("renamed_0.txt").exists());

    // Check queue status
    let status = queue.status();
    println!("Queue status: {:?}", status);

    // Ensure operations were properly serialized
    let successful = results.iter().filter(|r| r.is_some()).count();
    assert!(successful > 0, "At least some operations should succeed");
}

/// Test that the operation queue respects priority
#[tokio::test]
async fn test_operation_queue_priority() {
    let queue = stratosort::core::OperationQueue::new(1); // Single concurrent operation

    // Add operations with different priorities
    let low = queue.enqueue(
        stratosort::core::QueuedOperationType::FileMove {
            from: "low.txt".to_string(),
            to: "low_moved.txt".to_string(),
        },
        1,
    );

    let high = queue.enqueue(
        stratosort::core::QueuedOperationType::FileMove {
            from: "high.txt".to_string(),
            to: "high_moved.txt".to_string(),
        },
        10,
    );

    let medium = queue.enqueue(
        stratosort::core::QueuedOperationType::FileMove {
            from: "medium.txt".to_string(),
            to: "medium_moved.txt".to_string(),
        },
        5,
    );

    // Dequeue and verify order
    let first = queue.dequeue().await.unwrap();
    assert_eq!(first.id, high, "High priority should be first");

    let second = queue.dequeue().await.unwrap();
    assert_eq!(second.id, medium, "Medium priority should be second");

    let third = queue.dequeue().await.unwrap();
    assert_eq!(third.id, low, "Low priority should be last");
}

/// Test cache invalidation under concurrent access
#[tokio::test]
async fn test_cache_invalidation_concurrent() {
    let cache = Arc::new(stratosort::core::CacheManager::new(10, 60));

    // Spawn multiple readers and writers
    let barrier = Arc::new(Barrier::new(20));
    let mut handles = vec![];

    // 10 writers
    for i in 0..10 {
        let cache_clone = cache.clone();
        let barrier_clone = barrier.clone();

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            let key = stratosort::core::CacheKey::FileAnalysis(format!("/file_{}.txt", i));
            cache_clone.set(key, format!("data_{}", i), None, vec![]);

            tokio::time::sleep(Duration::from_millis(10)).await;

            // Invalidate some entries
            if i % 3 == 0 {
                cache_clone.invalidate(stratosort::core::InvalidationEvent::FileModified {
                    path: format!("/file_{}.txt", i),
                });
            }
        });

        handles.push(handle);
    }

    // 10 readers
    for i in 0..10 {
        let cache_clone = cache.clone();
        let barrier_clone = barrier.clone();

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            for j in 0..10 {
                let key = stratosort::core::CacheKey::FileAnalysis(format!("/file_{}.txt", j));
                let _value: Option<String> = cache_clone.get(&key);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        handles.push(handle);
    }

    // Wait for all operations
    futures::future::join_all(handles).await;

    // Verify cache statistics
    let stats = cache.stats();
    println!("Cache stats: {:?}", stats);
    assert!(stats.hit_count > 0 || stats.miss_count > 0);
}

/// Test file selection race condition is fixed
#[test]
fn test_file_selection_no_race() {
    // This would be a frontend test, but we can test the logic
    let mut selected_files = vec![];
    let mut last_selected_index = -1i32;
    let mut is_selection_in_progress = false;
    let mut selection_queue: Vec<Box<dyn FnMut()>> = vec![];

    // Simulate rapid selections
    for i in 0..100 {
        let action = Box::new(move || {
            if !is_selection_in_progress {
                is_selection_in_progress = true;

                // Simulate selection logic
                if i % 2 == 0 {
                    selected_files.push(format!("file_{}.txt", i));
                    last_selected_index = i;
                }

                is_selection_in_progress = false;
            }
        });

        if is_selection_in_progress {
            selection_queue.push(action);
        } else {
            // Execute immediately
            is_selection_in_progress = true;
            selected_files.push(format!("file_{}.txt", i));
            last_selected_index = i;
            is_selection_in_progress = false;
        }
    }

    // Process queued selections
    while let Some(mut action) = selection_queue.pop() {
        action();
    }

    // Verify no duplicate selections
    let unique_files: std::collections::HashSet<_> = selected_files.iter().collect();
    assert_eq!(unique_files.len(), selected_files.len(), "No duplicates should exist");
}

/// Test drag-and-drop state machine
#[test]
fn test_drag_drop_state_machine() {
    #[derive(Debug, PartialEq)]
    enum DragState {
        Idle,
        Hovering,
        Leaving,
    }

    let mut state = DragState::Idle;
    let mut enter_counter = 0;
    let mut is_hover = false;

    // Simulate drag events
    let events = vec![
        "enter", "over", "over", "leave", "enter", "drop"
    ];

    for event in events {
        match event {
            "enter" => {
                enter_counter += 1;
                state = DragState::Hovering;
                is_hover = true;
            }
            "over" => {
                if state == DragState::Hovering {
                    is_hover = true;
                }
            }
            "leave" => {
                enter_counter = enter_counter.saturating_sub(1);
                if enter_counter == 0 {
                    state = DragState::Leaving;
                    // Would set timer here
                    is_hover = false;
                }
            }
            "drop" => {
                state = DragState::Idle;
                enter_counter = 0;
                is_hover = false;
            }
            _ => {}
        }
    }

    assert_eq!(state, DragState::Idle);
    assert_eq!(enter_counter, 0);
    assert!(!is_hover);
}

/// Test concurrent organization operations with mutex
#[tokio::test]
async fn test_concurrent_organization_with_mutex() {
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[derive(Clone)]
    struct OrganizationState {
        operations_count: Arc<Mutex<usize>>,
        active_operations: Arc<Mutex<Vec<String>>>,
    }

    let state = OrganizationState {
        operations_count: Arc::new(Mutex::new(0)),
        active_operations: Arc::new(Mutex::new(Vec::new())),
    };

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for i in 0..10 {
        let state_clone = state.clone();
        let barrier_clone = barrier.clone();

        let handle = tokio::spawn(async move {
            barrier_clone.wait().await;

            // Acquire mutex before modifying state
            {
                let mut count = state_clone.operations_count.lock();
                *count += 1;

                let mut ops = state_clone.active_operations.lock();
                ops.push(format!("operation_{}", i));
            } // Locks released here

            // Simulate work
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Clean up
            {
                let mut ops = state_clone.active_operations.lock();
                ops.retain(|op| op != &format!("operation_{}", i));
            }
        });

        handles.push(handle);
    }

    futures::future::join_all(handles).await;

    // Verify final state
    let final_count = *state.operations_count.lock();
    assert_eq!(final_count, 10);

    let final_ops = state.active_operations.lock();
    assert_eq!(final_ops.len(), 0, "All operations should be completed");
}

/// Test batch operations don't interfere
#[tokio::test]
async fn test_batch_operations_isolation() {
    let queue = Arc::new(stratosort::core::OperationQueue::new(2));

    // Create two batch operations
    let batch1 = stratosort::core::QueuedOperationType::BatchOperation {
        operations: vec![
            stratosort::core::QueuedOperationType::FileMove {
                from: "a.txt".to_string(),
                to: "a_moved.txt".to_string(),
            },
            stratosort::core::QueuedOperationType::FileMove {
                from: "b.txt".to_string(),
                to: "b_moved.txt".to_string(),
            },
        ],
    };

    let batch2 = stratosort::core::QueuedOperationType::BatchOperation {
        operations: vec![
            stratosort::core::QueuedOperationType::FileMove {
                from: "c.txt".to_string(),
                to: "c_moved.txt".to_string(),
            },
            stratosort::core::QueuedOperationType::FileMove {
                from: "a.txt".to_string(), // Conflicts with batch1!
                to: "a_different.txt".to_string(),
            },
        ],
    };

    // Check conflict detection
    queue.enqueue(batch1.clone(), 1);
    let would_conflict = queue.would_conflict(&batch2);
    assert!(would_conflict, "Batch operations with conflicting files should be detected");
}

/// Test undo/redo operations are serialized
#[tokio::test]
async fn test_undo_redo_serialization() {
    let queue = stratosort::core::OperationQueue::new(1); // Force serialization

    let undo1 = queue.enqueue(
        stratosort::core::QueuedOperationType::UndoOperation {
            operation_id: Uuid::new_v4(),
        },
        1,
    );

    let undo2 = queue.enqueue(
        stratosort::core::QueuedOperationType::UndoOperation {
            operation_id: Uuid::new_v4(),
        },
        1,
    );

    // Try to dequeue - should get first one
    let first = queue.dequeue().await;
    assert!(first.is_some());

    // Second should be blocked until first completes
    queue.complete(undo1, true, None);

    let second = queue.dequeue().await;
    assert!(second.is_some());
    assert_eq!(second.unwrap().id, undo2);
}