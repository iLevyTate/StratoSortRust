use crate::{
    error::Result,
    storage::{Database, Operation},
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use uuid::Uuid;

pub struct UndoRedoManager {
    database: Arc<Database>,
    undo_stack: Arc<RwLock<VecDeque<Operation>>>,
    redo_stack: Arc<RwLock<VecDeque<Operation>>>,
    max_size: usize,
    max_memory_mb: usize,
}

impl UndoRedoManager {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            undo_stack: Arc::new(RwLock::new(VecDeque::new())),
            redo_stack: Arc::new(RwLock::new(VecDeque::new())),
            max_size: 50,
            max_memory_mb: 100, // 100MB limit for metadata/backup content
        }
    }

    pub fn with_limits(
        database: Arc<Database>,
        max_operations: usize,
        max_memory_mb: usize,
    ) -> Self {
        Self {
            database,
            undo_stack: Arc::new(RwLock::new(VecDeque::new())),
            redo_stack: Arc::new(RwLock::new(VecDeque::new())),
            max_size: max_operations,
            max_memory_mb,
        }
    }

    pub async fn record_move(&self, source: &str, destination: &str) -> Result<()> {
        let operation = Operation {
            id: Uuid::new_v4().to_string(),
            operation_type: "move".to_string(),
            source: source.to_string(),
            destination: Some(destination.to_string()),
            timestamp: chrono::Utc::now().timestamp(),
            metadata: None,
        };

        self.record_operation(operation).await
    }

    pub async fn record_create(&self, path: &str) -> Result<()> {
        let operation = Operation {
            id: Uuid::new_v4().to_string(),
            operation_type: "create".to_string(),
            source: path.to_string(),
            destination: None,
            timestamp: chrono::Utc::now().timestamp(),
            metadata: None,
        };

        self.record_operation(operation).await
    }

    pub async fn record_delete(&self, path: &str, backup_content: Option<Vec<u8>>) -> Result<()> {
        let metadata = backup_content.map(|content| {
            serde_json::json!({
                "backup_content": BASE64_STANDARD.encode(content)
            })
        });

        let operation = Operation {
            id: Uuid::new_v4().to_string(),
            operation_type: "delete".to_string(),
            source: path.to_string(),
            destination: None,
            timestamp: chrono::Utc::now().timestamp(),
            metadata,
        };

        self.record_operation(operation).await
    }

    pub async fn record_operation(&self, operation: Operation) -> Result<()> {
        // Save to database
        self.database.record_operation(&operation).await?;

        // Add to undo stack
        {
            let mut stack = self.undo_stack.write();
            stack.push_back(operation);

            // Limit stack size
            if stack.len() > self.max_size {
                stack.pop_front();
            }
        }

        // Clear redo stack when new operation is recorded
        {
            let mut redo = self.redo_stack.write();
            redo.clear();
        }

        // Check memory usage and cleanup if necessary
        self.cleanup_if_memory_exceeded().await?;

        Ok(())
    }

    pub async fn undo(&self) -> Result<Option<Operation>> {
        let operation = {
            let mut stack = self.undo_stack.write();
            stack.pop_back()
        };

        if let Some(op) = operation.clone() {
            let mut redo = self.redo_stack.write();
            redo.push_back(op);
        }

        Ok(operation)
    }

    pub async fn redo(&self) -> Result<Option<Operation>> {
        let operation = {
            let mut stack = self.redo_stack.write();
            stack.pop_back()
        };

        if let Some(op) = operation.clone() {
            let mut undo = self.undo_stack.write();
            undo.push_back(op);
        }

        Ok(operation)
    }

    pub async fn can_undo(&self) -> bool {
        !self.undo_stack.read().is_empty()
    }

    pub async fn can_redo(&self) -> bool {
        !self.redo_stack.read().is_empty()
    }

    pub async fn undo_count(&self) -> usize {
        self.undo_stack.read().len()
    }

    pub async fn redo_count(&self) -> usize {
        self.redo_stack.read().len()
    }

    pub async fn clear(&self) -> Result<()> {
        self.undo_stack.write().clear();
        self.redo_stack.write().clear();
        Ok(())
    }

    /// Calculate approximate memory usage of operations in stacks
    async fn calculate_memory_usage(&self) -> usize {
        let undo_stack = self.undo_stack.read();
        let redo_stack = self.redo_stack.read();

        let mut total_bytes = 0;

        // Calculate memory usage for undo stack
        for op in undo_stack.iter() {
            total_bytes += self.estimate_operation_size(op);
        }

        // Calculate memory usage for redo stack
        for op in redo_stack.iter() {
            total_bytes += self.estimate_operation_size(op);
        }

        total_bytes
    }

    /// Estimate the memory footprint of an operation (primarily metadata)
    fn estimate_operation_size(&self, operation: &Operation) -> usize {
        let mut size = operation.id.len() + operation.operation_type.len() + operation.source.len();

        if let Some(dest) = &operation.destination {
            size += dest.len();
        }

        if let Some(metadata) = &operation.metadata {
            // Estimate JSON size - this is an approximation
            size += serde_json::to_string(metadata).unwrap_or_default().len();
        }

        size
    }

    /// Clean up old operations if memory usage exceeds limit
    async fn cleanup_if_memory_exceeded(&self) -> Result<()> {
        let memory_usage_bytes = self.calculate_memory_usage().await;
        let memory_usage_mb = memory_usage_bytes / (1024 * 1024);

        if memory_usage_mb > self.max_memory_mb {
            tracing::info!(
                "Memory usage ({} MB) exceeds limit ({} MB), cleaning up old operations",
                memory_usage_mb,
                self.max_memory_mb
            );

            // Remove the oldest 25% of operations from both stacks
            {
                let mut undo_stack = self.undo_stack.write();
                let remove_count = undo_stack.len() / 4;
                for _ in 0..remove_count {
                    undo_stack.pop_front();
                }
            }

            {
                let mut redo_stack = self.redo_stack.write();
                let remove_count = redo_stack.len() / 4;
                for _ in 0..remove_count {
                    redo_stack.pop_front();
                }
            }

            tracing::info!("Cleaned up old operations to reduce memory usage");
        }

        Ok(())
    }

    /// Get memory usage statistics
    pub async fn get_memory_stats(&self) -> MemoryStats {
        let memory_usage_bytes = self.calculate_memory_usage().await;
        let memory_usage_mb = memory_usage_bytes / (1024 * 1024);

        MemoryStats {
            memory_usage_mb,
            memory_limit_mb: self.max_memory_mb,
            undo_operations: self.undo_stack.read().len(),
            redo_operations: self.redo_stack.read().len(),
            operation_limit: self.max_size,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryStats {
    pub memory_usage_mb: usize,
    pub memory_limit_mb: usize,
    pub undo_operations: usize,
    pub redo_operations: usize,
    pub operation_limit: usize,
}
