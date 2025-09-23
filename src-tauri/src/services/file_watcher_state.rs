use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
// use serde::{Deserialize, Serialize}; // Commented out - not currently used
use tokio::sync::RwLock;
use std::sync::Arc;

use super::file_watcher::{FileEvent, UserAction, PendingFile, WatchModeConfig};

/// Consolidated state for file watcher to prevent lock ordering issues
/// All fields that were previously separate RwLocks are now in a single struct
pub struct FileWatcherState {
    pub watch_config: WatchModeConfig,
    pub pending_files: HashMap<PathBuf, PendingFile>,
    pub user_actions: Vec<UserAction>,
    pub recent_operations: HashMap<String, Vec<FileEvent>>,
    pub recent_events: HashMap<String, Instant>,
}

impl Default for FileWatcherState {
    fn default() -> Self {
        Self {
            watch_config: WatchModeConfig::default(),
            pending_files: HashMap::new(),
            user_actions: Vec::new(),
            recent_operations: HashMap::new(),
            recent_events: HashMap::new(),
        }
    }
}

impl FileWatcherState {
    /// Clean up old user actions to prevent unbounded growth
    pub fn cleanup_old_user_actions(&mut self, max_age_seconds: i64) {
        let cutoff = chrono::Utc::now().timestamp() - max_age_seconds;
        self.user_actions.retain(|action| action.timestamp > cutoff);

        // Also limit total number of actions
        const MAX_USER_ACTIONS: usize = 1000;
        if self.user_actions.len() > MAX_USER_ACTIONS {
            let excess = self.user_actions.len() - MAX_USER_ACTIONS;
            self.user_actions.drain(0..excess);
        }
    }

    /// Clean up old recent events
    pub fn cleanup_old_events(&mut self, max_age: std::time::Duration) {
        let now = Instant::now();
        self.recent_events.retain(|_, instant| {
            now.duration_since(*instant) < max_age
        });
    }

    /// Clean up old operations
    pub fn cleanup_old_operations(&mut self, max_operations: usize) {
        if self.recent_operations.len() > max_operations {
            // Remove oldest operations
            let mut keys: Vec<_> = self.recent_operations.keys().cloned().collect();
            keys.sort();

            let to_remove = self.recent_operations.len() - max_operations;
            for key in keys.iter().take(to_remove) {
                self.recent_operations.remove(key);
            }
        }
    }
}

/// Thread-safe wrapper for file watcher state
pub struct SharedFileWatcherState {
    inner: Arc<RwLock<FileWatcherState>>,
}

impl SharedFileWatcherState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(FileWatcherState::default())),
        }
    }

    /// Get a read lock on the state
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, FileWatcherState> {
        self.inner.read().await
    }

    /// Get a write lock on the state
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, FileWatcherState> {
        self.inner.write().await
    }

    /// Perform periodic cleanup
    pub async fn cleanup(&self) {
        let mut state = self.write().await;
        state.cleanup_old_user_actions(3600); // Keep 1 hour of actions
        state.cleanup_old_events(std::time::Duration::from_secs(300)); // Keep 5 minutes
        state.cleanup_old_operations(100); // Keep last 100 operations
    }
}

impl Clone for SharedFileWatcherState {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}