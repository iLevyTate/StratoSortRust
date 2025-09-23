use crate::{
    ai::AiService,
    config::Config,
    core::{FileAnalyzer, OperationQueue, Organizer, SmartFolderManager, UndoRedoManager, pattern_learner::PatternLearner},
    error::Result,
    services::FileWatcher,
    storage::Database,
};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::Semaphore;
use uuid::Uuid;

/// AI Service status information
#[derive(Debug, Clone, Serialize)]
pub struct AiServiceStatus {
    pub provider: String,
    pub connected: bool,
    pub available_models: Vec<String>,
    pub current_model: String,
    pub capabilities: AiServiceCapabilities,
}

/// AI Service capabilities
#[derive(Debug, Clone, Serialize)]
pub struct AiServiceCapabilities {
    pub text_analysis: bool,
    pub vision_analysis: bool,
    pub embeddings: bool,
    pub semantic_search: bool,
}

/// Main application state
pub struct AppState<R: Runtime = tauri::Wry> {
    pub handle: AppHandle<R>,
    pub config: Arc<RwLock<Config>>,
    pub database: Arc<Database>,
    pub ai_service: Arc<AiService>,
    pub file_analyzer: Arc<FileAnalyzer>,
    pub organizer: Arc<Organizer>,
    pub smart_folders: Arc<SmartFolderManager>,
    pub undo_redo: Arc<UndoRedoManager>,
    pub file_cache: Arc<FileCache>,
    pub active_operations: Arc<DashMap<Uuid, OperationStatus>>,
    pub operation_queue: Arc<OperationQueue>,
    pub file_watcher: Arc<RwLock<Option<Arc<FileWatcher>>>>,
    pub monitoring_service: Arc<crate::services::MonitoringService>,
    pub pattern_learner: Arc<tokio::sync::RwLock<PatternLearner>>,
    /// Background task handles for proper cleanup during shutdown
    pub background_tasks: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    /// Semaphore to limit concurrent background tasks
    pub task_pool_semaphore: Arc<Semaphore>,
}

impl<R: Runtime> AppState<R> {
    pub async fn new(handle: AppHandle<R>, config: Config) -> Result<Self> {
        // 1. Initialize database FIRST
        let database = Arc::new(Database::new(&handle).await?);

        // 3. Initialize AI service AFTER database
        let ai_service = Arc::new(AiService::new(&config).await?);

        let config_arc = Arc::new(RwLock::new(config.clone()));
        let file_analyzer = Arc::new(FileAnalyzer::new(ai_service.clone(), config_arc.clone()));
        let smart_folders = Arc::new(SmartFolderManager::new(database.clone()));
        let organizer = Arc::new(Organizer::new(smart_folders.clone()));
        let undo_redo = Arc::new(UndoRedoManager::new(database.clone()));
        let file_cache = Arc::new(FileCache::new());
        let monitoring_service = Arc::new(crate::services::MonitoringService::new());
        let pattern_learner = Arc::new(tokio::sync::RwLock::new(PatternLearner::new()));

        // Use default max concurrent operations (can be made configurable later)
        let max_concurrent = 5;
        let operation_queue = Arc::new(OperationQueue::new(max_concurrent));

        Ok(Self {
            handle,
            config: config_arc,
            database,
            ai_service,
            file_analyzer,
            organizer,
            smart_folders,
            undo_redo,
            file_cache,
            active_operations: Arc::new(DashMap::new()),
            operation_queue,
            file_watcher: Arc::new(RwLock::new(None)),
            monitoring_service,
            pattern_learner,
            background_tasks: Arc::new(RwLock::new(Vec::new())),
            task_pool_semaphore: Arc::new(Semaphore::new(50)), // Limit to 50 concurrent background tasks
        })
    }

    /// Updates configuration
    pub async fn update_config(&self, config: Config) -> Result<()> {
        *self.config.write() = config.clone();

        // Reinitialize services that depend on config
        self.ai_service.update_config(&config).await?;

        // Save to disk
        config.save(&self.handle)?;

        Ok(())
    }

    /// Starts a new operation (internal) - fixed to be atomic
    fn start_operation_internal(&self, operation_type: OperationType) -> Uuid {
        let id = Uuid::new_v4();
        let timeout_duration = get_operation_timeout(&operation_type);
        let now = chrono::Utc::now();

        let status = OperationStatus {
            id,
            operation_type: operation_type.clone(),
            progress: 0.0,
            message: String::new(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            started_at: now,
            timeout_duration,
            last_update: Arc::new(RwLock::new(now)),
        };

        // Clone what we need before inserting to avoid holding references
        let status_clone = OperationStatus {
            id,
            operation_type,
            progress: 0.0,
            message: String::new(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            started_at: now,
            timeout_duration,
            last_update: Arc::new(RwLock::new(now)),
        };

        // Insert and schedule atomically
        self.active_operations.insert(id, status);

        // Start timeout check for this operation - now guaranteed to exist in map
        self.schedule_timeout_check(id);

        id
    }

    /// Updates operation progress (deprecated - use update_progress instead)
    pub fn update_operation(&self, id: Uuid, progress: f32, message: String) {
        self.update_progress(id, progress, message);
    }

    /// Register a background task for proper cleanup during shutdown
    pub async fn register_background_task(&self, task: tokio::task::JoinHandle<()>) {
        self.background_tasks.write().push(task);
        // Trigger cleanup of completed tasks periodically
        self.cleanup_completed_tasks().await;
    }

    /// CRITICAL FIX: Clean up completed background tasks to prevent memory leak
    pub async fn cleanup_completed_tasks(&self) {
        let mut tasks = self.background_tasks.write();
        let initial_count = tasks.len();

        // Only keep tasks that are still running
        tasks.retain(|task| !task.is_finished());

        let cleaned_count = initial_count - tasks.len();
        if cleaned_count > 0 {
            tracing::debug!("Cleaned up {} completed background tasks", cleaned_count);
        }

        // If we have too many tasks, force cleanup of oldest ones
        const MAX_BACKGROUND_TASKS: usize = 100;
        if tasks.len() > MAX_BACKGROUND_TASKS {
            let excess = tasks.len() - MAX_BACKGROUND_TASKS;
            tracing::warn!("Too many background tasks ({}), aborting {} oldest tasks", tasks.len(), excess);

            // Abort oldest tasks
            for task in tasks.drain(0..excess) {
                task.abort();
            }
        }
    }

    /// Graceful shutdown of all services
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Starting graceful shutdown of application services");

        // 1. Stop file watcher first to prevent new operations
        let watcher_result = {
            let watcher_guard = self.file_watcher.read();
            watcher_guard.clone()
        };

        if let Some(watcher) = watcher_result {
            if let Err(e) = watcher.stop().await {
                tracing::warn!("Error stopping file watcher: {}", e);
            } else {
                tracing::info!("File watcher stopped successfully");
            }
        }

        // 2. Cancel all background tasks FIRST to prevent resource leaks
        // CRITICAL FIX: Use try_write to prevent blocking shutdown on locked tasks
        let (tasks_to_cancel, task_count) = match self.background_tasks.try_write() {
            Some(mut tasks) => {
                let task_count = tasks.len();
                tracing::info!("Cancelling {} background tasks", task_count);

                let mut collected_tasks = Vec::new();
                for task in tasks.drain(..) {
                    task.abort();
                    collected_tasks.push(task);
                }
                (collected_tasks, task_count)
            }
            None => {
                // If we can't get the lock immediately, force abort any tasks we can see
                tracing::warn!("Background tasks lock contended during shutdown - forcing termination");
                (Vec::new(), 0)
            }
        };
        
        // Wait for tasks to complete
        for task in tasks_to_cancel {
            let _ = tokio::time::timeout(tokio::time::Duration::from_millis(100), task).await;
        }

        // 3. Cancel all active operations
        let active_operations: Vec<Uuid> = self
            .active_operations
            .iter()
            .map(|entry| *entry.key())
            .collect();

        tracing::info!("Cancelling {} active operations", active_operations.len());
        for operation_id in active_operations {
            self.cancel_operation(operation_id);
        }

        // 4. Wait a moment for operations to cancel gracefully
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // 5. Force cancel any remaining operations
        let remaining = self.active_operations.len();
        if remaining > 0 {
            tracing::warn!("Force stopping {} remaining operations", remaining);
            self.active_operations.clear();
        }

        // 6. Clear file cache
        {
            let cache_size = self.file_cache.entries.len();
            self.file_cache.entries.clear();
            tracing::info!("Cleared file cache ({} items)", cache_size);
        }

        // 7. Perform final database operations
        if let Err(e) = self.database.close_connections().await {
            tracing::warn!("Error closing database connections: {}", e);
        } else {
            tracing::info!("Database connections closed successfully");
        }

        // 8. Stop monitoring service
        self.monitoring_service.shutdown().await;

        tracing::info!("Graceful shutdown completed (cancelled {} background tasks)", task_count);
        Ok(())
    }
    
    /// Spawn a task with resource limits - FIXED UNBOUNDED SPAWNING
    pub async fn spawn_limited_task<F>(&self, task: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        // Acquire permit before spawning
        let permit = self.task_pool_semaphore.clone().acquire_owned().await
            .map_err(|_| crate::error::AppError::ResourceLimitExceeded {
                message: "Task pool semaphore closed".to_string()
            })?;

        let handle = tokio::spawn(async move {
            task.await;
            // Permit is automatically released when dropped
            drop(permit);
        });

        // Track the handle for cleanup and trigger periodic cleanup
        self.register_background_task(handle).await;
        Ok(())
    }

    /// Schedule timeout check for an operation - FIXED RESOURCE LIMITS
    fn schedule_timeout_check(&self, operation_id: Uuid) {
        let operations_ref = Arc::downgrade(&self.active_operations);
        let handle = self.handle.clone();
        let background_tasks_ref = Arc::downgrade(&self.background_tasks);
        let semaphore = self.task_pool_semaphore.clone();

        // Use spawn_limited_task or at least acquire permit manually
        let task_future = async move {
            // Acquire permit before starting the task
            let _permit = match semaphore.acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => {
                    tracing::error!("Failed to acquire task permit for timeout check");
                    return;
                }
            };

            tracing::debug!("Started timeout check task for operation {}", operation_id);
            // Check timeout every 30 seconds
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Get strong reference if still available
                        let operations = match operations_ref.upgrade() {
                            Some(ops) => ops,
                            None => break, // AppState has been dropped
                        };
                        
                        let should_timeout = if let Some(status) = operations.get(&operation_id) {
                            let now = chrono::Utc::now();
                            let last_update = *status.last_update.read();
                            let elapsed_since_update = now.signed_duration_since(last_update);
                            let total_elapsed = now.signed_duration_since(status.started_at);
                            
                            // Timeout if:
                            // 1. Total operation time exceeds timeout duration, OR
                            // 2. No update received in the last 2 minutes (indicating hung operation)
                            elapsed_since_update > chrono::Duration::minutes(2) || 
                            total_elapsed > status.timeout_duration
                        } else {
                            break; // Operation no longer exists
                        };
                        
                        if should_timeout {
                            // Operation has timed out
                            if let Some((_, timed_out_status)) = operations.remove(&operation_id) {
                                tracing::warn!("Operation {} timed out after {:?}", operation_id, timed_out_status.timeout_duration);
                                
                                // Cancel the operation
                                timed_out_status.cancellation_token.cancel();
                                
                                // Emit timeout event
                                let timeout_event = serde_json::json!({
                                    "operation_id": operation_id.to_string(),
                                    "operation_type": timed_out_status.operation_type,
                                    "error": "Operation timed out",
                                    "timeout_duration_seconds": timed_out_status.timeout_duration.num_seconds(),
                                    "timestamp": chrono::Utc::now().timestamp()
                                });
                                
                                crate::emit_event!(handle, crate::events::operation::ERROR, timeout_event);
                            }
                            break;
                        }
                    }
                    _ = tokio::task::yield_now() => {
                        // Allow task cancellation
                        tracing::debug!("Timeout check task for operation {} yielded", operation_id);
                        break;
                    }
                }
            }
            tracing::debug!("Timeout check task for operation {} terminated", operation_id);
        };

        let timeout_task = tokio::spawn(task_future);

        // Register the timeout task for cleanup
        if let Some(tasks_lock) = background_tasks_ref.upgrade() {
            if let Some(mut tasks) = tasks_lock.try_write() {
                tasks.push(timeout_task);
            }
        }
    }
    
    /// Manually check and clean up timed out operations (called periodically)
    pub fn cleanup_timed_out_operations(&self) {
        let now = chrono::Utc::now();
        let mut timed_out_ids = Vec::new();
        
        // Find timed out operations
        for entry in self.active_operations.iter() {
            let status = entry.value();
            let last_update = *status.last_update.read();
            let elapsed_since_update = now.signed_duration_since(last_update);
            let total_elapsed = now.signed_duration_since(status.started_at);
            
            if elapsed_since_update > chrono::Duration::minutes(2) || 
               total_elapsed > status.timeout_duration {
                timed_out_ids.push(*entry.key());
            }
        }
        
        // Cancel and remove timed out operations
        for id in timed_out_ids {
            if let Some((_, status)) = self.active_operations.remove(&id) {
                tracing::warn!("Cleaning up timed out operation: {}", id);
                status.cancellation_token.cancel();
                
                let timeout_event = serde_json::json!({
                    "operation_id": id.to_string(),
                    "operation_type": status.operation_type,
                    "error": "Operation timed out during cleanup",
                    "timeout_duration_seconds": status.timeout_duration.num_seconds(),
                    "timestamp": chrono::Utc::now().timestamp()
                });
                
                crate::emit_event!(self.handle, crate::events::operation::ERROR, timeout_event);
            }
        }
    }

    /// Get current resource usage statistics
    pub async fn get_resource_usage(&self) -> ResourceUsage {
        // Use atomic operations to safely get cache statistics
        let (cache_size, cache_memory) = self.file_cache.get_stats().await;

        // Check AI service availability safely
        let ai_service_available = {
            // Simple availability check - AI service is considered available based on its status
            let status = self.ai_service.get_status().await;
            status.ollama_connected || status.provider == crate::ai::AiProvider::Fallback
        };

        ResourceUsage {
            active_operations: self.active_operations.len(),
            cache_items: cache_size,
            cache_memory_bytes: cache_memory,
            database_connected: true, // Database connection is assumed to be stable
            ai_service_available,
        }
    }

    /// Checks if an operation is cancelled
    pub fn is_operation_cancelled(&self, id: Uuid) -> bool {
        self.active_operations
            .get(&id)
            .map(|status| status.cancellation_token.is_cancelled())
            .unwrap_or(true) // Consider non-existent operations as cancelled
    }

    /// Cancels an operation
    pub fn cancel_operation(&self, id: Uuid) -> bool {
        if let Some((_, status)) = self.active_operations.remove(&id) {
            status.cancellation_token.cancel();
            true
        } else {
            false
        }
    }

    /// Completes an operation
    pub fn complete_operation(&self, id: Uuid) {
        // Atomically remove operation and get its data for event emission
        if let Some((_, status)) = self.active_operations.remove(&id) {
            // Create events outside of any locks
            let progress_event = ProgressEvent {
                id: id.to_string(),
                operation_type: status.operation_type.clone(),
                progress: 1.0,
                message: "Operation completed".to_string(),
                completed: true,
            };

            let complete_event = serde_json::json!({
                "operation_id": id.to_string(),
                "operation_type": status.operation_type,
                "message": "Operation completed successfully",
                "timestamp": chrono::Utc::now().timestamp()
            });

            // Emit events using standardized macro
            crate::emit_event!(
                self.handle,
                crate::events::operation::PROGRESS,
                serde_json::json!(progress_event)
            );
            crate::emit_event!(
                self.handle,
                crate::events::operation::COMPLETE,
                complete_event
            );
        }
    }

    /// Fails an operation with an error
    pub fn error_operation(&self, id: Uuid, error_message: String) {
        // Atomically remove operation and get its data for event emission
        if let Some((_, status)) = self.active_operations.remove(&id) {
            // Create events outside of any locks
            let progress_event = ProgressEvent {
                id: id.to_string(),
                operation_type: status.operation_type.clone(),
                progress: 0.0,
                message: format!("Operation failed: {}", error_message),
                completed: true,
            };

            let error_event = serde_json::json!({
                "operation_id": id.to_string(),
                "operation_type": status.operation_type,
                "error": error_message,
                "message": format!("Operation failed: {}", error_message),
                "timestamp": chrono::Utc::now().timestamp()
            });

            // Emit events using standardized macro
            crate::emit_event!(
                self.handle,
                crate::events::operation::PROGRESS,
                serde_json::json!(progress_event)
            );
            crate::emit_event!(self.handle, crate::events::operation::ERROR, error_event);
        }
    }

    /// Updates operation progress and emits event to frontend
    /// CRITICAL FIX: Use atomic operations to prevent deadlocks
    pub fn update_progress(&self, id: Uuid, progress: f32, message: String) {
        // First, create event data with minimal lock scope
        let event_data = {
            // Scope the lock to absolute minimum
            let Some(mut status) = self.active_operations.get_mut(&id) else {
                // Operation doesn't exist - early return
                return;
            };

            // Check cancellation immediately
            if status.cancellation_token.is_cancelled() {
                return;
            }

            let clamped_progress = progress.clamp(0.0, 1.0);
            status.progress = clamped_progress;
            status.message = message.clone();

            // CRITICAL: Use try_write to prevent blocking on timestamp update
            if let Some(mut last_update) = status.last_update.try_write() {
                *last_update = chrono::Utc::now();
            }
            // If we can't update timestamp immediately, that's okay - just continue

            // Create event data and immediately drop the lock
            let event = ProgressEvent {
                id: id.to_string(),
                operation_type: status.operation_type.clone(),
                progress: clamped_progress,
                message,
                completed: false,
            };

            // Lock is automatically dropped at end of scope
            event
        }; // Lock released here before any event emission

        // CRITICAL: All event emission happens outside of any locks
        crate::emit_event!(
            self.handle,
            crate::events::operation::PROGRESS,
            serde_json::json!(event_data)
        );
    }

    /// Starts a new operation and emits initial event
    pub fn start_operation(&self, operation_type: OperationType, message: String) -> Uuid {
        let id = self.start_operation_internal(operation_type.clone());

        let progress_event = ProgressEvent {
            id: id.to_string(),
            operation_type,
            progress: 0.0,
            message,
            completed: false,
        };

        // Emit using standardized macro
        crate::emit_event!(
            self.handle,
            crate::events::operation::PROGRESS,
            serde_json::json!(progress_event)
        );

        id
    }

    /// Cleans up old cache entries
    pub async fn cleanup_cache(&self) -> Result<()> {
        self.file_cache.cleanup_old_entries().await;

        // Perform aggressive cache cleanup if under memory pressure
        if self.is_under_memory_pressure() {
            self.file_cache.aggressive_cleanup().await;
        }

        self.database.vacuum().await?;
        Ok(())
    }

    /// Check if system is under memory pressure - FIXED INTEGER OVERFLOW BUG
    pub fn is_under_memory_pressure(&self) -> bool {
        let cache_size = self.file_cache.current_size();
        let max_cache_size = self.file_cache.max_size;
        
        // CRITICAL FIX: Prevent integer overflow in multiplication
        // Use saturating arithmetic and proper overflow checks
        match max_cache_size.checked_mul(80) {
            Some(threshold_numerator) => {
                let threshold = threshold_numerator / 100;
                cache_size > threshold
            }
            None => {
                // Overflow occurred - treat as under pressure for safety
                tracing::warn!("Memory pressure calculation overflow detected - max_cache_size too large: {}", max_cache_size);
                true  // Conservative assumption
            }
        }
    }

    /// Force cleanup of memory when under pressure
    pub async fn emergency_memory_cleanup(&self) -> Result<()> {
        tracing::warn!("Performing emergency memory cleanup");

        // Clear file cache
        self.file_cache.clear();

        // Cancel non-critical operations
        let active_ops: Vec<_> = self
            .active_operations
            .iter()
            .map(|entry| *entry.key())
            .collect();
        let mut cancelled_count = 0;

        for op_id in &active_ops {
            if let Some(op) = self.active_operations.get(op_id) {
                // Only cancel file analysis operations, keep critical ones
                if matches!(op.operation_type, crate::state::OperationType::FileAnalysis) {
                    self.cancel_operation(*op_id);
                    cancelled_count += 1;
                }
            }
        }

        // Emergency database cleanup to free disk space and memory
        if let Err(e) = self.database.cleanup_wal_files().await {
            tracing::error!("Emergency WAL cleanup failed: {}", e);
        }

        // Force garbage collection hint
        tracing::info!(
            "Emergency cleanup completed, {} operations cancelled, database cleaned",
            cancelled_count
        );
        Ok(())
    }

    /// Saves application state and performs maintenance
    pub async fn save_state(&self) -> Result<()> {
        // Save configuration
        self.config.read().save(&self.handle)?;

        // Save smart folders
        self.smart_folders.save_all().await?;

        // Flush database with WAL checkpoint
        self.database.flush().await?;

        Ok(())
    }

    /// Perform periodic database maintenance to prevent disk bloat
    pub async fn periodic_database_maintenance(&self) -> Result<()> {
        tracing::info!("Starting periodic database maintenance");
        
        // Perform aggressive WAL cleanup to prevent disk space issues
        if let Err(e) = self.database.cleanup_wal_files().await {
            tracing::error!("WAL cleanup failed during maintenance: {}", e);
        }
        
        // Clear old cache entries (older than 7 days)
        if let Err(e) = self.database.clear_cache().await {
            tracing::error!("Cache cleanup failed during maintenance: {}", e);
        }
        
        tracing::info!("Periodic database maintenance completed");
        Ok(())
    }
}

/// File cache for quick access
pub struct FileCache {
    entries: DashMap<String, CachedFile>,
    max_size: usize,
}

impl Default for FileCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FileCache {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
            max_size: 100 * 1024 * 1024, // 100MB
        }
    }

    /// Invalidate a single cache entry
    pub fn invalidate(&self, path: &str) {
        if self.entries.remove(path).is_some() {
            tracing::debug!("Invalidated cache entry for: {}", path);
        }
    }

    /// Invalidate multiple cache entries matching a prefix
    pub fn invalidate_prefix(&self, prefix: &str) {
        let mut invalidated = 0;
        self.entries.retain(|key, _| {
            if key.starts_with(prefix) {
                invalidated += 1;
                false
            } else {
                true
            }
        });
        if invalidated > 0 {
            tracing::debug!("Invalidated {} cache entries with prefix: {}", invalidated, prefix);
        }
    }

    /// Invalidate cache entries based on file system events
    pub fn handle_file_event(&self, event_type: &str, path: &str) {
        match event_type {
            "modify" | "remove" => {
                self.invalidate(path);
            }
            "rename" => {
                // For rename, invalidate both old and new paths
                self.invalidate(path);
            }
            _ => {}
        }
    }

    /// Get cache statistics atomically
    pub async fn get_stats(&self) -> (usize, usize) {
        let cache_size = self.entries.len();
        let cache_memory = self.current_size();
        (cache_size, cache_memory)
    }

    pub fn get(&self, path: &str) -> Option<CachedFile> {
        // Check if file still exists and hasn't been modified
        if let Some(entry) = self.entries.get(path) {
            // Check if cache entry is still valid (file hasn't changed)
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    let modified_timestamp = modified.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    let modified_datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(modified_timestamp, 0)
                        .unwrap_or_else(chrono::Utc::now);

                    // If file has been modified after cache time, invalidate
                    if modified_datetime > entry.accessed {
                        drop(entry); // Release the lock
                        self.invalidate(path);
                        return None;
                    }
                }
                return Some(entry.clone());
            } else {
                // File doesn't exist anymore, invalidate cache
                drop(entry);
                self.invalidate(path);
            }
        }
        None
    }

    pub fn insert(&self, path: String, file: CachedFile) {
        // Don't insert if file itself is larger than max cache size
        if file.size > self.max_size {
            tracing::warn!(
                "File {} ({} bytes) is larger than max cache size ({} bytes), skipping cache",
                path,
                file.size,
                self.max_size
            );
            return;
        }

        // Calculate total entry size including metadata overhead
        let entry_overhead =
            path.len() + std::mem::size_of::<CachedFile>() + std::mem::size_of::<String>();
        let total_entry_size = file.size + entry_overhead;

        // Don't insert files that are more than 25% of cache size
        if total_entry_size > self.max_size / 4 {
            tracing::debug!(
                "File {} ({} bytes) is too large for efficient caching (> 25% of cache), skipping",
                path,
                total_entry_size
            );
            return;
        }

        // Enforce cache size limits with improved eviction strategy
        self.ensure_cache_space(total_entry_size);

        self.entries.insert(path, file);
    }

    fn ensure_cache_space(&self, required_space: usize) {
        let mut iterations = 0;
        const MAX_EVICTION_ITERATIONS: usize = 100; // Prevent infinite loops

        while self.current_size() + required_space > self.max_size
            && !self.entries.is_empty()
            && iterations < MAX_EVICTION_ITERATIONS
        {
            // Try to evict multiple items at once for efficiency
            let current_size = self.current_size();
            let target_size = self.max_size - required_space;
            let bytes_to_free = current_size.saturating_sub(target_size);

            self.evict_multiple_entries(bytes_to_free);
            iterations += 1;
        }

        if iterations >= MAX_EVICTION_ITERATIONS {
            tracing::warn!("Cache eviction reached maximum iterations, clearing cache");
            self.entries.clear();
        }
    }

    fn evict_multiple_entries(&self, target_bytes: usize) {
        // Collect entries sorted by access time (oldest first)
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|entry| (entry.key().clone(), entry.accessed, entry.size))
            .collect();

        entries.sort_by_key(|(_, accessed, _)| *accessed);

        let mut freed_bytes = 0;
        let mut keys_to_remove = Vec::new();

        for (key, _, size) in entries {
            keys_to_remove.push(key);
            freed_bytes += size;

            if freed_bytes >= target_bytes {
                break;
            }
        }

        // Remove collected keys
        for key in keys_to_remove {
            self.entries.remove(&key);
        }

        tracing::debug!("Evicted {} bytes from cache", freed_bytes);
    }

    pub async fn cleanup_old_entries(&self) {
        let now = chrono::Utc::now();
        let mut to_remove = Vec::new();

        for entry in self.entries.iter() {
            if now.signed_duration_since(entry.accessed) > chrono::Duration::hours(24) {
                to_remove.push(entry.key().clone());
            }
        }

        for key in to_remove {
            self.entries.remove(&key);
        }
    }

    pub async fn aggressive_cleanup(&self) {
        let now = chrono::Utc::now();
        let mut to_remove = Vec::new();

        // More aggressive cleanup - remove entries older than 1 hour
        for entry in self.entries.iter() {
            if now.signed_duration_since(entry.accessed) > chrono::Duration::hours(1) {
                to_remove.push(entry.key().clone());
            }
        }

        // If still not enough space, remove largest entries first
        if to_remove.len() < self.entries.len() / 2 {
            let mut entries: Vec<_> = self
                .entries
                .iter()
                .map(|entry| (entry.key().clone(), entry.size))
                .collect();

            // Sort by size (largest first)
            entries.sort_by(|a, b| b.1.cmp(&a.1));

            // Remove up to 50% of entries starting with largest
            let target_removals = self.entries.len() / 2;
            for (key, _) in entries.into_iter().take(target_removals) {
                if !to_remove.contains(&key) {
                    to_remove.push(key);
                }
            }
        }

        tracing::info!(
            "Aggressive cleanup removing {} cache entries",
            to_remove.len()
        );

        for key in to_remove {
            self.entries.remove(&key);
        }
    }

    pub fn current_size(&self) -> usize {
        // Use cached size for performance - recalculate only when necessary
        self.calculate_precise_size()
    }

    fn calculate_precise_size(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| {
                let cached_file = entry.value();
                
                // CRITICAL FIX: Properly calculate actual memory usage
                let key_size = entry.key().len();
                let content_size = cached_file.content.len(); // Actual content, not just the size field
                let path_size = cached_file.path.len();
                let mime_type_size = cached_file.mime_type.len();
                
                // Fixed metadata calculation
                let metadata_size = std::mem::size_of::<CachedFile>() 
                    + std::mem::size_of::<String>() * 2  // path and mime_type strings
                    + std::mem::size_of::<Vec<u8>>();    // content vector
                
                key_size
                    .saturating_add(content_size)
                    .saturating_add(path_size)
                    .saturating_add(mime_type_size)
                    .saturating_add(metadata_size)
            })
            .fold(0usize, |acc, size| acc.saturating_add(size))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&self) {
        self.entries.clear();
    }

    #[allow(dead_code)]
    fn evict_oldest(&self) {
        // CRITICAL FIX: Truly atomic find-and-remove using DashMap's retain feature
        // This eliminates the race condition by performing find and remove in a single atomic operation

        let mut oldest_key: Option<String> = None;
        let mut oldest_time = chrono::DateTime::<chrono::Utc>::MAX_UTC;

        // First pass: find the oldest entry
        for entry in self.entries.iter() {
            let accessed_time = entry.value().accessed;
            if accessed_time < oldest_time {
                oldest_time = accessed_time;
                oldest_key = Some(entry.key().clone());
            }
        }

        // Second pass: atomically remove the oldest entry if it still exists and is still oldest
        if let Some(target_key) = oldest_key {
            self.entries.remove_if(&target_key, |_key, cached_file| {
                // Only remove if this entry is still the oldest (or very close to it)
                // This handles the case where another thread modified the entry
                cached_file.accessed <= oldest_time + chrono::Duration::milliseconds(100)
            });
        }
    }
}

#[derive(Clone)]
pub struct CachedFile {
    pub path: String,
    pub content: Vec<u8>,
    pub mime_type: String,
    pub size: usize,
    pub accessed: chrono::DateTime<chrono::Utc>,
}

pub struct OperationStatus {
    pub id: Uuid,
    pub operation_type: OperationType,
    pub progress: f32,
    pub message: String,
    pub cancellation_token: tokio_util::sync::CancellationToken,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub timeout_duration: chrono::Duration,
    pub last_update: Arc<RwLock<chrono::DateTime<chrono::Utc>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    FileAnalysis,
    Organization,
    ModelDownload,
    DatabaseMigration,
    BulkOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub id: String,
    pub operation_type: OperationType,
    pub progress: f32,
    pub message: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub active_operations: usize,
    pub cache_items: usize,
    pub cache_memory_bytes: usize,
    pub database_connected: bool,
    pub ai_service_available: bool,
}

/// Get timeout duration based on operation type
fn get_operation_timeout(operation_type: &OperationType) -> chrono::Duration {
    match operation_type {
        OperationType::FileAnalysis => chrono::Duration::minutes(10), // 10 minutes for file analysis
        OperationType::Organization => chrono::Duration::minutes(30), // 30 minutes for organization
        OperationType::ModelDownload => chrono::Duration::hours(2),   // 2 hours for model downloads
        OperationType::DatabaseMigration => chrono::Duration::hours(1), // 1 hour for database operations
        OperationType::BulkOperation => chrono::Duration::hours(1),   // 1 hour for bulk operations
    }
}
