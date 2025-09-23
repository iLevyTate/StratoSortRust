use crate::error::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

/// Represents the state of a long-running operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationProgress {
    pub id: String,
    pub operation_type: String,
    pub description: String,
    pub current: usize,
    pub total: usize,
    pub percentage: f32,
    pub status: OperationStatus,
    pub message: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
    pub estimated_completion: Option<i64>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Internal tracking structure for operations
#[derive(Debug, Clone)]
struct TrackedOperation {
    progress: OperationProgress,
    start_time: Instant,
    last_update: Instant,
    update_count: u32,
    rate_per_second: f32,
}

/// Progress tracker for long-running operations
pub struct ProgressTracker<R: Runtime> {
    operations: Arc<DashMap<String, TrackedOperation>>,
    app_handle: AppHandle<R>,
}

impl<R: Runtime> Clone for ProgressTracker<R> {
    fn clone(&self) -> Self {
        Self {
            operations: self.operations.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}

impl<R: Runtime> ProgressTracker<R> {
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self {
            operations: Arc::new(DashMap::new()),
            app_handle,
        }
    }

    /// Start tracking a new operation
    pub fn start_operation(
        &self,
        operation_type: impl Into<String>,
        description: impl Into<String>,
        total: usize,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let progress = OperationProgress {
            id: id.clone(),
            operation_type: operation_type.into(),
            description: description.into(),
            current: 0,
            total,
            percentage: 0.0,
            status: OperationStatus::InProgress,
            message: None,
            started_at: now,
            updated_at: now,
            estimated_completion: None,
            metadata: serde_json::Value::Null,
        };

        let tracked = TrackedOperation {
            progress: progress.clone(),
            start_time: Instant::now(),
            last_update: Instant::now(),
            update_count: 0,
            rate_per_second: 0.0,
        };

        self.operations.insert(id.clone(), tracked);

        // Emit initial progress event
        self.emit_progress_update(&progress);

        Ok(id)
    }

    /// Update the progress of an operation
    pub fn update_progress(
        &self,
        id: &str,
        current: usize,
        message: Option<String>,
    ) -> Result<()> {
        let mut entry = self.operations.get_mut(id).ok_or_else(|| {
            crate::error::AppError::NotFound {
                message: format!("Operation {} not found", id),
            }
        })?;

        let now = Instant::now();
        let elapsed = now.duration_since(entry.last_update);

        // Calculate rate of progress
        if elapsed.as_secs_f32() > 0.0 {
            let items_processed = current.saturating_sub(entry.progress.current) as f32;
            entry.rate_per_second = items_processed / elapsed.as_secs_f32();
        }

        // Update progress
        entry.progress.current = current;
        entry.progress.percentage = if entry.progress.total > 0 {
            (current as f32 / entry.progress.total as f32 * 100.0).min(100.0)
        } else {
            0.0
        };
        entry.progress.message = message;
        entry.progress.updated_at = chrono::Utc::now().timestamp();

        // Calculate estimated completion time
        if entry.rate_per_second > 0.0 && current < entry.progress.total {
            let remaining = (entry.progress.total - current) as f32;
            let seconds_remaining = remaining / entry.rate_per_second;
            entry.progress.estimated_completion = Some(
                chrono::Utc::now().timestamp() + seconds_remaining as i64
            );
        }

        entry.last_update = now;
        entry.update_count += 1;

        let progress = entry.progress.clone();
        drop(entry); // Release lock before emitting

        // Throttle updates - only emit every 100ms or on significant changes
        if elapsed.as_millis() > 100 || current == progress.total {
            self.emit_progress_update(&progress);
        }

        Ok(())
    }

    /// Mark an operation as completed
    pub fn complete_operation(&self, id: &str) -> Result<()> {
        self.update_status(id, OperationStatus::Completed, Some("Operation completed successfully".to_string()))
    }

    /// Mark an operation as failed
    pub fn fail_operation(&self, id: &str, error: String) -> Result<()> {
        self.update_status(id, OperationStatus::Failed, Some(error))
    }

    /// Cancel an operation
    pub fn cancel_operation(&self, id: &str) -> Result<()> {
        self.update_status(id, OperationStatus::Cancelled, Some("Operation cancelled by user".to_string()))
    }

    /// Update the status of an operation
    fn update_status(
        &self,
        id: &str,
        status: OperationStatus,
        message: Option<String>,
    ) -> Result<()> {
        let mut entry = self.operations.get_mut(id).ok_or_else(|| {
            crate::error::AppError::NotFound {
                message: format!("Operation {} not found", id),
            }
        })?;

        entry.progress.status = status.clone();
        entry.progress.message = message;
        entry.progress.updated_at = chrono::Utc::now().timestamp();

        // Set to 100% if completed
        if status == OperationStatus::Completed {
            entry.progress.percentage = 100.0;
            entry.progress.current = entry.progress.total;
        }

        let progress = entry.progress.clone();
        drop(entry); // Release lock before emitting

        self.emit_progress_update(&progress);

        // Clean up completed/failed/cancelled operations after 30 seconds
        if matches!(status, OperationStatus::Completed | OperationStatus::Failed | OperationStatus::Cancelled) {
            let operations = self.operations.clone();
            let id = id.to_string();
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                operations.remove(&id);
            });
        }

        Ok(())
    }

    /// Get the current progress of an operation
    pub fn get_progress(&self, id: &str) -> Option<OperationProgress> {
        self.operations.get(id).map(|entry| entry.progress.clone())
    }

    /// Get all active operations
    pub fn get_active_operations(&self) -> Vec<OperationProgress> {
        self.operations
            .iter()
            .filter(|entry| matches!(entry.progress.status, OperationStatus::InProgress))
            .map(|entry| entry.progress.clone())
            .collect()
    }

    /// Get all operations
    pub fn get_all_operations(&self) -> Vec<OperationProgress> {
        self.operations
            .iter()
            .map(|entry| entry.progress.clone())
            .collect()
    }

    /// Check if an operation is still active
    pub fn is_active(&self, id: &str) -> bool {
        self.operations
            .get(id)
            .map(|entry| matches!(entry.progress.status, OperationStatus::InProgress))
            .unwrap_or(false)
    }

    /// Clean up old completed operations
    pub fn cleanup_old_operations(&self, max_age_seconds: i64) {
        let cutoff = chrono::Utc::now().timestamp() - max_age_seconds;

        let to_remove: Vec<String> = self.operations
            .iter()
            .filter(|entry| {
                !matches!(entry.progress.status, OperationStatus::InProgress) &&
                entry.progress.updated_at < cutoff
            })
            .map(|entry| entry.key().clone())
            .collect();

        for id in to_remove {
            self.operations.remove(&id);
        }
    }

    /// Emit progress update to frontend
    fn emit_progress_update(&self, progress: &OperationProgress) {
        let _ = self.app_handle.emit("operation-progress", progress);
    }

    /// Create a scoped progress tracker for sub-operations
    pub fn create_sub_tracker(&self, parent_id: &str, weight: f32) -> SubProgressTracker<R> {
        SubProgressTracker {
            parent_id: parent_id.to_string(),
            weight: weight.clamp(0.0, 1.0),
            tracker: self.clone(),
        }
    }
}

/// Sub-progress tracker for nested operations
pub struct SubProgressTracker<R: Runtime> {
    parent_id: String,
    weight: f32,
    tracker: ProgressTracker<R>,
}

impl<R: Runtime> SubProgressTracker<R> {
    /// Update parent progress based on sub-operation progress
    pub fn update(&self, sub_progress: f32) -> Result<()> {
        if let Some(mut parent) = self.tracker.operations.get_mut(&self.parent_id) {
            // Calculate weighted contribution to parent progress
            let contribution = sub_progress * self.weight;
            let new_progress = parent.progress.percentage + contribution;
            parent.progress.percentage = new_progress.min(100.0);
            parent.progress.updated_at = chrono::Utc::now().timestamp();

            let progress = parent.progress.clone();
            drop(parent);

            self.tracker.emit_progress_update(&progress);
        }
        Ok(())
    }
}

/// Macro for easy progress tracking
#[macro_export]
macro_rules! track_progress {
    ($tracker:expr, $op_type:expr, $desc:expr, $total:expr, $body:expr) => {{
        let op_id = $tracker.start_operation($op_type, $desc, $total)?;
        let result = $body(&op_id, &$tracker);

        match &result {
            Ok(_) => $tracker.complete_operation(&op_id)?,
            Err(e) => $tracker.fail_operation(&op_id, format!("{}", e))?,
        }

        result
    }};
}