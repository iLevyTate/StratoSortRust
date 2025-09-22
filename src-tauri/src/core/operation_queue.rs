use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Semaphore, OwnedSemaphorePermit};
use uuid::Uuid;

/// Types of operations that can be queued
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueuedOperationType {
    FileMove { from: String, to: String },
    FileCopy { from: String, to: String },
    FileRename { path: String, new_name: String },
    BatchOperation { operations: Vec<QueuedOperationType> },
    SmartFolderOrganization { folder_id: String },
    UndoOperation { operation_id: Uuid },
    RedoOperation { operation_id: Uuid },
}

/// Status of a queued operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

/// A queued operation with metadata
#[derive(Debug, Clone)]
pub struct QueuedOperation {
    pub id: Uuid,
    pub operation_type: QueuedOperationType,
    pub status: OperationStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub priority: i32,
}

/// Thread-safe operation queue with concurrency control
pub struct OperationQueue {
    /// Queue of pending operations
    pending: Arc<Mutex<VecDeque<QueuedOperation>>>,
    /// Currently executing operations
    executing: Arc<Mutex<Vec<QueuedOperation>>>,
    /// Completed operations (for history)
    completed: Arc<Mutex<Vec<QueuedOperation>>>,
    /// Semaphore to limit concurrent operations
    concurrency_limit: Arc<Semaphore>,
    /// Track permits for each operation to ensure proper cleanup
    active_permits: Arc<Mutex<HashMap<Uuid, OwnedSemaphorePermit>>>,
    /// Maximum number of operations to keep in history
    max_history: usize,
}

impl OperationQueue {
    /// Create a new operation queue with specified concurrency limit
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            pending: Arc::new(Mutex::new(VecDeque::new())),
            executing: Arc::new(Mutex::new(Vec::new())),
            completed: Arc::new(Mutex::new(Vec::new())),
            concurrency_limit: Arc::new(Semaphore::new(max_concurrent)),
            active_permits: Arc::new(Mutex::new(HashMap::new())),
            max_history: 1000,
        }
    }

    /// Add an operation to the queue
    pub fn enqueue(&self, operation_type: QueuedOperationType, priority: i32) -> Uuid {
        let operation = QueuedOperation {
            id: Uuid::new_v4(),
            operation_type,
            status: OperationStatus::Pending,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            priority,
        };

        let id = operation.id;
        let mut queue = self.pending.lock();

        // Insert based on priority (higher priority first)
        let position = queue.iter().position(|op| op.priority < priority).unwrap_or(queue.len());
        queue.insert(position, operation);

        tracing::debug!("Enqueued operation {} with priority {}", id, priority);
        id
    }

    /// Get the next operation to execute - FIXED DEADLOCK BUG AND PERMIT MANAGEMENT
    pub async fn dequeue(&self) -> Option<QueuedOperation> {
        // CRITICAL FIX: Acquire all locks BEFORE getting semaphore to prevent deadlock
        let mut operation = {
            let mut pending = self.pending.lock();
            pending.pop_front()?
        };

        // Now safely acquire semaphore permit (locks are released)
        let permit = self.concurrency_limit.clone().acquire_owned().await.ok()?;

        // Update operation status
        operation.status = OperationStatus::InProgress;
        operation.started_at = Some(chrono::Utc::now());

        // Store the permit for proper cleanup
        {
            let mut active_permits = self.active_permits.lock();
            active_permits.insert(operation.id, permit);
        }

        // Add to executing list (quick operation)
        {
            let mut executing = self.executing.lock();
            executing.push(operation.clone());
        }

        Some(operation)
    }

    /// Mark an operation as completed - FIXED PERMIT LEAK
    pub fn complete(&self, id: Uuid, success: bool, error: Option<String>) {
        // CRITICAL FIX: Release semaphore permit to prevent resource leak
        {
            let mut active_permits = self.active_permits.lock();
            if let Some(permit) = active_permits.remove(&id) {
                // Permit is automatically released when dropped
                drop(permit);
                tracing::debug!("Released semaphore permit for operation {}", id);
            } else {
                tracing::warn!("No permit found for operation {} during completion", id);
            }
        }

        let mut executing = self.executing.lock();
        if let Some(position) = executing.iter().position(|op| op.id == id) {
            let mut operation = executing.remove(position);
            operation.completed_at = Some(chrono::Utc::now());
            operation.status = if success {
                OperationStatus::Completed
            } else {
                OperationStatus::Failed(error.unwrap_or_else(|| "Unknown error".to_string()))
            };

            // Add to completed history
            let mut completed = self.completed.lock();
            completed.push(operation);

            // Trim history if needed
            if completed.len() > self.max_history {
                let drain_count = completed.len() - self.max_history;
                completed.drain(0..drain_count);
            }
        }
    }

    /// Cancel a pending operation
    pub fn cancel(&self, id: Uuid) -> bool {
        let mut pending = self.pending.lock();

        if let Some(position) = pending.iter().position(|op| op.id == id) {
            let mut operation = pending.remove(position).unwrap();
            operation.status = OperationStatus::Cancelled;
            operation.completed_at = Some(chrono::Utc::now());

            let mut completed = self.completed.lock();
            completed.push(operation);

            tracing::debug!("Cancelled operation {}", id);
            return true;
        }

        // Check if it's currently executing
        let executing = self.executing.lock();
        if executing.iter().any(|op| op.id == id) {
            tracing::warn!("Cannot cancel operation {} - already executing", id);
            return false;
        }

        false
    }

    /// Force cleanup of an operation (used during shutdown or error recovery)
    pub fn force_cleanup(&self, id: Uuid) {
        // Release permit if it exists
        {
            let mut active_permits = self.active_permits.lock();
            if let Some(permit) = active_permits.remove(&id) {
                drop(permit);
                tracing::debug!("Force cleaned permit for operation {}", id);
            }
        }

        // Remove from executing if present
        {
            let mut executing = self.executing.lock();
            if let Some(position) = executing.iter().position(|op| op.id == id) {
                let mut operation = executing.remove(position);
                operation.status = OperationStatus::Cancelled;
                operation.completed_at = Some(chrono::Utc::now());

                let mut completed = self.completed.lock();
                completed.push(operation);
            }
        }
    }

    /// Get number of active permits (for monitoring)
    pub fn active_permit_count(&self) -> usize {
        self.active_permits.lock().len()
    }

    /// Get current queue status
    pub fn status(&self) -> QueueStatus {
        let pending = self.pending.lock();
        let executing = self.executing.lock();
        let completed = self.completed.lock();

        QueueStatus {
            pending_count: pending.len(),
            executing_count: executing.len(),
            completed_count: completed.len(),
            next_operation: pending.front().map(|op| op.id),
        }
    }

    /// Clear all pending operations
    pub fn clear_pending(&self) {
        let mut pending = self.pending.lock();
        let now = chrono::Utc::now();

        let mut completed = self.completed.lock();
        for mut operation in pending.drain(..) {
            operation.status = OperationStatus::Cancelled;
            operation.completed_at = Some(now);
            completed.push(operation);
        }
    }

    /// Check if an operation type would conflict with currently executing operations
    pub fn would_conflict(&self, operation_type: &QueuedOperationType) -> bool {
        let executing = self.executing.lock();

        for executing_op in executing.iter() {
            if Self::operations_conflict(&executing_op.operation_type, operation_type) {
                return true;
            }
        }

        false
    }

    /// Determine if two operations would conflict
    fn operations_conflict(op1: &QueuedOperationType, op2: &QueuedOperationType) -> bool {
        match (op1, op2) {
            // File operations on the same path conflict
            (QueuedOperationType::FileMove { from: f1, .. }, QueuedOperationType::FileMove { from: f2, .. }) => f1 == f2,
            (QueuedOperationType::FileMove { from: f1, .. }, QueuedOperationType::FileCopy { from: f2, .. }) => f1 == f2,
            (QueuedOperationType::FileMove { from: f1, .. }, QueuedOperationType::FileRename { path: f2, .. }) => f1 == f2,
            (QueuedOperationType::FileCopy { from: f1, .. }, QueuedOperationType::FileRename { path: f2, .. }) => f1 == f2,
            (QueuedOperationType::FileRename { path: p1, .. }, QueuedOperationType::FileRename { path: p2, .. }) => p1 == p2,

            // Smart folder operations on the same folder conflict
            (QueuedOperationType::SmartFolderOrganization { folder_id: id1 },
             QueuedOperationType::SmartFolderOrganization { folder_id: id2 }) => id1 == id2,

            // Undo/Redo operations always conflict with each other
            (QueuedOperationType::UndoOperation { .. }, QueuedOperationType::UndoOperation { .. }) => true,
            (QueuedOperationType::RedoOperation { .. }, QueuedOperationType::RedoOperation { .. }) => true,
            (QueuedOperationType::UndoOperation { .. }, QueuedOperationType::RedoOperation { .. }) => true,

            // Batch operations need recursive checking
            (QueuedOperationType::BatchOperation { operations: ops1 }, other) => {
                ops1.iter().any(|op| Self::operations_conflict(op, other))
            }
            (other, QueuedOperationType::BatchOperation { operations: ops2 }) => {
                ops2.iter().any(|op| Self::operations_conflict(other, op))
            }

            _ => false,
        }
    }
}

/// Current status of the operation queue
#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    pub pending_count: usize,
    pub executing_count: usize,
    pub completed_count: usize,
    pub next_operation: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_operation_queue_priority() {
        let queue = OperationQueue::new(2);

        // Add operations with different priorities
        let _low = queue.enqueue(
            QueuedOperationType::FileMove { from: "a.txt".to_string(), to: "b.txt".to_string() },
            1
        );
        let high = queue.enqueue(
            QueuedOperationType::FileMove { from: "c.txt".to_string(), to: "d.txt".to_string() },
            10
        );
        let medium = queue.enqueue(
            QueuedOperationType::FileMove { from: "e.txt".to_string(), to: "f.txt".to_string() },
            5
        );

        // Should dequeue in priority order
        let op1 = queue.dequeue().await.unwrap();
        assert_eq!(op1.id, high);

        let op2 = queue.dequeue().await.unwrap();
        assert_eq!(op2.id, medium);
    }

    #[test]
    fn test_conflict_detection() {
        let _queue = OperationQueue::new(2);

        // Test conflicting file operations
        let op1 = QueuedOperationType::FileMove {
            from: "/path/to/file.txt".to_string(),
            to: "/new/path.txt".to_string()
        };
        let op2 = QueuedOperationType::FileRename {
            path: "/path/to/file.txt".to_string(),
            new_name: "renamed.txt".to_string()
        };

        assert!(OperationQueue::operations_conflict(&op1, &op2));

        // Test non-conflicting operations
        let op3 = QueuedOperationType::FileMove {
            from: "/different/file.txt".to_string(),
            to: "/another/path.txt".to_string()
        };

        assert!(!OperationQueue::operations_conflict(&op1, &op3));
    }
}