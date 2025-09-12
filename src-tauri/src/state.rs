use crate::{
    ai::AiService,
    config::Config,
    core::{FileAnalyzer, Organizer, SmartFolderManager, UndoRedoManager},
    error::Result,
    services::FileWatcher,
    storage::Database,
};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

/// Main application state
pub struct AppState {
    pub handle: AppHandle,
    pub config: Arc<RwLock<Config>>,
    pub database: Arc<Database>,
    pub ai_service: Arc<AiService>,
    pub file_analyzer: Arc<FileAnalyzer>,
    pub organizer: Arc<Organizer>,
    pub smart_folders: Arc<SmartFolderManager>,
    pub undo_redo: Arc<UndoRedoManager>,
    pub file_cache: Arc<FileCache>,
    pub active_operations: Arc<DashMap<Uuid, OperationStatus>>,
    pub file_watcher: Arc<RwLock<Option<Arc<FileWatcher>>>>,
    pub monitoring_service: Arc<crate::services::MonitoringService>,
}

impl AppState {
    pub async fn new(handle: AppHandle, config: Config) -> Result<Self> {
        let database = Arc::new(Database::new(&handle).await?);
        let ai_service = Arc::new(AiService::new(&config).await?);
        let config_arc = Arc::new(RwLock::new(config));
        let file_analyzer = Arc::new(FileAnalyzer::new(ai_service.clone(), config_arc.clone()));
        let smart_folders = Arc::new(SmartFolderManager::new(database.clone()));
        let organizer = Arc::new(Organizer::new(smart_folders.clone()));
        let undo_redo = Arc::new(UndoRedoManager::new(database.clone()));
        let file_cache = Arc::new(FileCache::new());
        let monitoring_service = Arc::new(crate::services::MonitoringService::new());
        
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
            file_watcher: Arc::new(RwLock::new(None)),
            monitoring_service,
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
    
    /// Starts a new operation (internal)
    fn start_operation_internal(&self, operation_type: OperationType) -> Uuid {
        let id = Uuid::new_v4();
        let status = OperationStatus {
            id,
            operation_type,
            progress: 0.0,
            message: String::new(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            started_at: chrono::Utc::now(),
        };
        
        self.active_operations.insert(id, status);
        id
    }
    
    /// Updates operation progress (deprecated - use update_progress instead)
    pub fn update_operation(&self, id: Uuid, progress: f32, message: String) {
        self.update_progress(id, progress, message);
    }
    
    /// Graceful shutdown of all services
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Starting graceful shutdown of application services");
        
        // 1. Stop file watcher first to prevent new operations
        {
            let watcher = self.file_watcher.read().clone();
            if let Some(watcher) = watcher {
                if let Err(e) = watcher.stop().await {
                    tracing::warn!("Error stopping file watcher: {}", e);
                } else {
                    tracing::info!("File watcher stopped successfully");
                }
            }
        }
        
        // 2. Cancel all active operations
        let active_operations: Vec<Uuid> = self.active_operations.iter()
            .map(|entry| *entry.key())
            .collect();
        
        tracing::info!("Cancelling {} active operations", active_operations.len());
        for operation_id in active_operations {
            self.cancel_operation(operation_id);
        }
        
        // 3. Wait a moment for operations to cancel gracefully
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // 4. Force cancel any remaining operations
        let remaining = self.active_operations.len();
        if remaining > 0 {
            tracing::warn!("Force stopping {} remaining operations", remaining);
            self.active_operations.clear();
        }
        
        // 5. Clear file cache
        {
            let cache_size = self.file_cache.entries.len();
            self.file_cache.entries.clear();
            tracing::info!("Cleared file cache ({} items)", cache_size);
        }
        
        // 6. Perform final database operations
        if let Err(e) = self.database.close_connections().await {
            tracing::warn!("Error closing database connections: {}", e);
        } else {
            tracing::info!("Database connections closed successfully");
        }
        
        // 7. Stop monitoring service
        self.monitoring_service.shutdown().await;
        
        tracing::info!("Graceful shutdown completed");
        Ok(())
    }
    
    /// Get current resource usage statistics
    pub async fn get_resource_usage(&self) -> ResourceUsage {
        let cache_size = self.file_cache.entries.len();
        let cache_memory = self.file_cache.entries.iter()
            .map(|entry| entry.value().content.len())
            .sum::<usize>();
        
        ResourceUsage {
            active_operations: self.active_operations.len(),
            cache_items: cache_size,
            cache_memory_bytes: cache_memory,
            database_connected: true, // Could check actual connection status
            ai_service_available: false, // Would need async check
        }
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
        // Emit completion event before removing
        let progress_event = ProgressEvent {
            id: id.to_string(),
            operation_type: OperationType::BulkOperation, // Default, should be passed from caller
            progress: 1.0,
            message: "Operation completed".to_string(),
            completed: true,
        };
        
        if let Err(e) = self.handle.emit("operation_progress", &progress_event) {
            tracing::error!("Failed to emit completion event: {}", e);
        }
        
        self.active_operations.remove(&id);
    }
    
    /// Updates operation progress and emits event to frontend
    pub fn update_progress(&self, id: Uuid, progress: f32, message: String) {
        if let Some(mut status) = self.active_operations.get_mut(&id) {
            status.progress = progress.clamp(0.0, 1.0);
            status.message = message.clone();
            
            let progress_event = ProgressEvent {
                id: id.to_string(),
                operation_type: status.operation_type.clone(),
                progress: status.progress,
                message,
                completed: false,
            };
            
            if let Err(e) = self.handle.emit("operation_progress", &progress_event) {
                tracing::error!("Failed to emit progress event: {}", e);
            }
        }
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
        
        if let Err(e) = self.handle.emit("operation_progress", &progress_event) {
            tracing::error!("Failed to emit start event: {}", e);
        }
        
        id
    }
    
    /// Cleans up old cache entries
    pub async fn cleanup_cache(&self) -> Result<()> {
        self.file_cache.cleanup_old_entries().await;
        self.database.vacuum().await?;
        Ok(())
    }
    
    /// Saves application state
    pub async fn save_state(&self) -> Result<()> {
        // Save configuration
        self.config.read().save(&self.handle)?;
        
        // Save smart folders
        self.smart_folders.save_all().await?;
        
        // Flush database
        self.database.flush().await?;
        
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
    
    pub fn get(&self, path: &str) -> Option<CachedFile> {
        self.entries.get(path).map(|e| e.clone())
    }
    
    pub fn insert(&self, path: String, file: CachedFile) {
        // Don't insert if file itself is larger than max cache size
        if file.size > self.max_size {
            tracing::warn!("File {} ({} bytes) is larger than max cache size ({} bytes), skipping cache", 
                          path, file.size, self.max_size);
            return;
        }
        
        // Calculate total entry size including metadata overhead
        let entry_overhead = path.len() + std::mem::size_of::<CachedFile>() + std::mem::size_of::<String>();
        let total_entry_size = file.size + entry_overhead;
        
        // Enforce cache size limits before insertion
        while self.current_size() + total_entry_size > self.max_size && !self.entries.is_empty() {
            self.evict_oldest();
        }
        
        self.entries.insert(path, file);
    }
    
    pub async fn cleanup_old_entries(&self) {
        let now = chrono::Utc::now();
        let mut to_remove = Vec::new();
        
        for entry in self.entries.iter() {
            if now.signed_duration_since(entry.accessed)
                > chrono::Duration::hours(24)
            {
                to_remove.push(entry.key().clone());
            }
        }
        
        for key in to_remove {
            self.entries.remove(&key);
        }
    }
    
    pub fn current_size(&self) -> usize {
        self.entries.iter().map(|entry| {
            let key_size = entry.key().len();
            let file_size = entry.value().size;
            let metadata_size = std::mem::size_of::<CachedFile>() + std::mem::size_of::<String>();
            key_size + file_size + metadata_size
        }).sum()
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
    
    fn evict_oldest(&self) {
        // Find oldest entry key first
        let oldest_key = self.entries.iter()
            .min_by_key(|entry| entry.accessed)
            .map(|entry| entry.key().clone());
        
        // Remove the oldest entry if found
        if let Some(key) = oldest_key {
            self.entries.remove(&key);
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