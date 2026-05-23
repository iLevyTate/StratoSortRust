use crate::{error::Result, state::AppState};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEvent {
    pub event_type: String,
    pub path: String,
    pub timestamp: i64,
    pub file_name: Option<String>,
    pub extension: Option<String>,
    // Enhanced fields for learning
    pub source_path: Option<String>,      // For move operations
    pub destination_path: Option<String>, // For move operations
    pub is_user_action: bool,             // vs system action
    pub operation_id: Option<String>,     // To group related operations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAction {
    pub action_type: UserActionType,
    pub timestamp: i64,
    pub file_path: String,
    pub destination_path: Option<String>,
    pub folder_created: Option<String>,
    pub rename_pattern: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserActionType {
    CreateFolder,
    MoveFile,
    RenameFile,
    DeleteFile,
    CopyFile,
    OrganizeFiles, // Bulk organization action
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchModeConfig {
    pub enabled: bool,
    pub watch_directories: Vec<String>,
    pub auto_organize_delay_ms: u64, // Delay before auto-organizing new files
    pub learning_enabled: bool,
    pub confidence_threshold: f32, // Minimum confidence for auto-organization
    pub max_auto_organize_count: usize, // Max files to auto-organize at once
    pub excluded_extensions: Vec<String>,
    pub excluded_directories: Vec<String>,
}

impl Default for WatchModeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            watch_directories: vec![],
            auto_organize_delay_ms: 2000, // 2 seconds delay
            learning_enabled: true,
            confidence_threshold: 0.7,
            max_auto_organize_count: 10,
            excluded_extensions: vec![".tmp".to_string(), ".lock".to_string()],
            excluded_directories: vec![".git".to_string(), "node_modules".to_string()],
        }
    }
}

#[derive(Debug)]
pub struct PendingFile {
    pub path: PathBuf,
    pub detected_at: Instant,
    pub file_size: u64,
    pub last_modified: SystemTime,
}

pub struct FileWatcher {
    state: Arc<AppState>,
    app_handle: AppHandle,
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    // Enhanced watch mode fields
    watch_config: Arc<RwLock<WatchModeConfig>>,
    pending_files: Arc<RwLock<HashMap<PathBuf, PendingFile>>>,
    user_actions: Arc<RwLock<Vec<UserAction>>>,
    recent_operations: Arc<RwLock<HashMap<String, Vec<FileEvent>>>>, // operation_id -> events
    // Event deduplication to prevent race conditions
    recent_events: Arc<RwLock<HashMap<String, Instant>>>,
    event_debounce_duration: Duration,
}

impl FileWatcher {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state: state.clone(),
            app_handle: state.handle.clone(),
            watcher: Arc::new(Mutex::new(None)),
            shutdown_tx: Arc::new(Mutex::new(None)),
            watch_config: Arc::new(RwLock::new(WatchModeConfig::default())),
            pending_files: Arc::new(RwLock::new(HashMap::new())),
            user_actions: Arc::new(RwLock::new(Vec::new())),
            recent_operations: Arc::new(RwLock::new(HashMap::new())),
            recent_events: Arc::new(RwLock::new(HashMap::new())),
            event_debounce_duration: Duration::from_millis(100), // 100ms debounce
        }
    }

    /// Start the enhanced file watcher with learning capabilities
    pub async fn start(&self) -> Result<()> {
        {
            let watcher_guard = self.watcher.lock().await;
            if watcher_guard.is_some() {
                info!("Enhanced file watcher already running");
                return Ok(());
            }
        }

        let (tx, mut rx) = mpsc::channel(1000); // Increased buffer for high-activity directories
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

        let app_handle = self.app_handle.clone();
        let state = self.state.clone();
        let watch_config = self.watch_config.clone();
        let pending_files = self.pending_files.clone();
        let user_actions = self.user_actions.clone();
        let recent_operations = self.recent_operations.clone();

        // Create enhanced watcher with learning capabilities
        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(event) => {
                let tx_clone = tx.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = tx_clone.blocking_send(event);
                });
            }
            Err(e) => {
                error!("Enhanced file watcher error: {}", e);
            }
        })?;

        // Add watch paths from configuration
        let config = watch_config.read().await;
        for path_str in &config.watch_directories {
            let path = Path::new(path_str);
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)?;
                info!("Enhanced watching path: {}", path.display());
            }
        }

        *self.watcher.lock().await = Some(watcher);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        // Spawn enhanced event handler with learning and deduplication
        let event_handler_config = watch_config.clone();
        let event_handler_pending = pending_files.clone();
        let event_handler_actions = user_actions.clone();
        let event_handler_operations = recent_operations.clone();
        let event_handler_recent_events = self.recent_events.clone();
        let event_handler_debounce_duration = self.event_debounce_duration;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        // Check for event deduplication
                        if let Some(event_path) = event.paths.first() {
                            let event_key = format!("{}:{:?}", event_path.display(), event.kind);
                            let now = Instant::now();

                            // Check if this event was recently processed
                            {
                                let mut recent_events = event_handler_recent_events.write().await;

                                // Clean up old events (older than debounce duration)
                                recent_events.retain(|_, &mut timestamp| {
                                    now.duration_since(timestamp) < event_handler_debounce_duration * 5
                                });

                                // Check if this event is a duplicate
                                if let Some(&last_time) = recent_events.get(&event_key) {
                                    if now.duration_since(last_time) < event_handler_debounce_duration {
                                        // Skip duplicate event
                                        debug!("Skipping duplicate event: {}", event_key);
                                        continue;
                                    }
                                }

                                // Record this event
                                recent_events.insert(event_key, now);
                            }
                        }

                        handle_enhanced_file_event(
                            event,
                            &app_handle,
                            &state,
                            &event_handler_config,
                            &event_handler_pending,
                            &event_handler_actions,
                            &event_handler_operations
                        ).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Enhanced file watcher shutting down");
                        break;
                    }
                }
            }
        });

        // Spawn auto-organization processor
        let auto_org_config = watch_config.clone();
        let auto_org_pending = pending_files.clone();
        let auto_org_state = self.state.clone(); // Clone from self instead of moved variable
        let auto_org_app = self.app_handle.clone(); // Clone from self instead of moved variable

        tokio::spawn(async move {
            Self::auto_organization_processor(
                auto_org_config,
                auto_org_pending,
                auto_org_state,
                auto_org_app,
            )
            .await;
        });

        info!("Enhanced file watcher with learning started");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }

        *self.watcher.lock().await = None;
        info!("File watcher stopped");
        Ok(())
    }

    /// Configure watch mode settings.
    ///
    /// Beyond storing the new config, this also reconciles the notify watcher's
    /// registered directories with the new list (new dirs get registered, removed
    /// dirs get unregistered) and — if the config transitions from disabled to
    /// enabled, or watches a new directory — enqueues any *existing* files in
    /// those directories so the AI pipeline processes them on next tick. Without
    /// this, a user who turns on watch mode after dropping files into the folder
    /// would never see anything happen, because notify only fires on future
    /// Create events.
    pub async fn configure_watch_mode(&self, config: WatchModeConfig) -> Result<()> {
        let previous = {
            let guard = self.watch_config.read().await;
            guard.clone()
        };

        // Compute directory diff before we swap the config.
        let previous_dirs: std::collections::HashSet<String> =
            previous.watch_directories.iter().cloned().collect();
        let new_dirs: std::collections::HashSet<String> =
            config.watch_directories.iter().cloned().collect();
        let added_dirs: Vec<String> = new_dirs.difference(&previous_dirs).cloned().collect();
        let removed_dirs: Vec<String> = previous_dirs.difference(&new_dirs).cloned().collect();

        let just_enabled = !previous.enabled && config.enabled;

        // Capture excluded extensions/dirs for the scan filter while we still
        // own the new config (the swap below moves it).
        let scan_config = config.clone();

        {
            let mut current_config = self.watch_config.write().await;
            *current_config = config;
        }

        // Register newly-added directories with the notify watcher.
        for dir in &added_dirs {
            if let Err(e) = self.add_watch_path(dir).await {
                warn!("Failed to register watch path {}: {}", dir, e);
            }
        }
        for dir in &removed_dirs {
            if let Err(e) = self.remove_watch_path(dir).await {
                warn!("Failed to unregister watch path {}: {}", dir, e);
            }
        }

        // Enqueue existing files for any directory the user just started
        // watching, or every watched directory if watch mode was just toggled on.
        if scan_config.enabled {
            let to_scan: Vec<String> = if just_enabled {
                scan_config.watch_directories.clone()
            } else {
                added_dirs.clone()
            };
            if !to_scan.is_empty() {
                info!(
                    "Initial scan: enqueueing existing files from {} director{}",
                    to_scan.len(),
                    if to_scan.len() == 1 { "y" } else { "ies" }
                );
                let enqueued = self.enqueue_existing_files(&to_scan, &scan_config).await;
                info!("Initial scan complete: {} files enqueued", enqueued);
            }
        }

        info!("Watch mode configuration updated");
        Ok(())
    }

    /// Walk the given directories and add any files we find to the pending
    /// auto-organization queue. Delegates to the free function
    /// `walk_existing_files_into` so the same logic can be unit-tested without
    /// constructing a full `FileWatcher` (which needs an `AppHandle`).
    async fn enqueue_existing_files(
        &self,
        directories: &[String],
        config: &WatchModeConfig,
    ) -> usize {
        let mut target = self.pending_files.write().await;
        walk_existing_files_into(
            directories,
            config,
            &mut target,
            MAX_INITIAL_SCAN_DEPTH,
            MAX_INITIAL_SCAN_FILES,
        )
        .await
    }

    /// Get current watch mode configuration
    pub async fn get_watch_config(&self) -> WatchModeConfig {
        self.watch_config.read().await.clone()
    }

    /// Record a user action for learning
    pub async fn record_user_action(&self, action: UserAction) {
        let mut actions = self.user_actions.write().await;
        actions.push(action.clone());

        // Keep only recent actions (last 1000)
        if actions.len() > 1000 {
            actions.drain(0..500); // Remove older half
        }

        debug!("Recorded user action: {:?}", action);
    }

    /// Get recent user actions for pattern learning
    pub async fn get_recent_user_actions(&self, limit: usize) -> Vec<UserAction> {
        let actions = self.user_actions.read().await;
        actions.iter().rev().take(limit).cloned().collect()
    }

    /// Get pending files count
    pub async fn get_pending_files_count(&self) -> usize {
        self.pending_files.read().await.len()
    }

    /// Get pending file paths
    pub async fn get_pending_file_paths(&self) -> Vec<String> {
        let pending = self.pending_files.read().await;
        pending
            .keys()
            .map(|path| path.to_string_lossy().to_string())
            .collect()
    }

    /// Clear pending files (used for manual trigger)
    pub async fn clear_pending_files(&self) -> usize {
        let mut pending = self.pending_files.write().await;
        let count = pending.len();
        pending.clear();
        count
    }

    pub async fn add_watch_path(&self, path: &str) -> Result<()> {
        let mut watcher_guard = self.watcher.lock().await;
        if let Some(watcher) = &mut *watcher_guard {
            let path = Path::new(path);
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)?;
                info!("Added watch path: {}", path.display());
            }
        }
        Ok(())
    }

    pub async fn remove_watch_path(&self, path: &str) -> Result<()> {
        let mut watcher_guard = self.watcher.lock().await;
        if let Some(watcher) = &mut *watcher_guard {
            let path = Path::new(path);
            watcher.unwatch(path)?;
            info!("Removed watch path: {}", path.display());
        }
        Ok(())
    }

    /// Auto-organization processor that runs continuously
    async fn auto_organization_processor(
        config: Arc<RwLock<WatchModeConfig>>,
        pending_files: Arc<RwLock<HashMap<PathBuf, PendingFile>>>,
        state: Arc<AppState>,
        app_handle: AppHandle,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(1000)); // Check every second

        loop {
            interval.tick().await;

            let config_read = config.read().await;
            if !config_read.enabled {
                continue;
            }

            let auto_organize_delay = Duration::from_millis(config_read.auto_organize_delay_ms);
            let confidence_threshold = config_read.confidence_threshold;
            let max_count = config_read.max_auto_organize_count;
            drop(config_read);

            let mut pending = pending_files.write().await;
            let now = Instant::now();

            // Find files ready for auto-organization
            let ready_files: Vec<PathBuf> = pending
                .iter()
                .filter(|(_, pending_file)| {
                    now.duration_since(pending_file.detected_at) >= auto_organize_delay
                })
                .take(max_count)
                .map(|(path, _)| path.clone())
                .collect();

            // Remove processed files from pending
            for path in &ready_files {
                pending.remove(path);
            }
            drop(pending);

            if !ready_files.is_empty() {
                info!("Auto-organizing {} files", ready_files.len());

                // Process files for auto-organization
                for file_path in ready_files {
                    if let Err(e) = Self::auto_organize_single_file(
                        &file_path,
                        &state,
                        &app_handle,
                        confidence_threshold,
                    )
                    .await
                    {
                        warn!(
                            "Auto-organization failed for {}: {}",
                            file_path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    /// Auto-organize a single file based on AI analysis and learned patterns
    async fn auto_organize_single_file(
        file_path: &PathBuf,
        state: &Arc<AppState>,
        app_handle: &AppHandle,
        confidence_threshold: f32,
    ) -> Result<()> {
        let path_str = file_path.to_string_lossy().to_string();

        // Honor the user's `auto_analyze_on_add` setting. When false, we still
        // surface the file to the UI (it stays in the pending queue / emits the
        // file-created event) but skip the AI analysis + auto-move entirely —
        // the user has opted out of having models touch newly-added files.
        if !state.config.read().auto_analyze_on_add {
            debug!(
                "auto_analyze_on_add is disabled; skipping analysis for {}",
                path_str
            );
            return Ok(());
        }

        // 1. Analyze file with AI (if not already cached). Dispatches by file
        // type so images go to vision, documents get text-extracted, and every
        // analyzed file gets an embedding stored for semantic search.
        let ai_analysis = match state.database.get_analysis(&path_str).await? {
            Some(existing_analysis) => Some(existing_analysis),
            None => match state.ai_service.analyze_path_with_ai(&path_str).await {
                Ok(analysis) => {
                    let _ = state.database.save_analysis(&analysis).await;

                    let embed_text =
                        format!("{} {}", analysis.summary, analysis.tags.join(" "));
                    if !embed_text.trim().is_empty() {
                        match state.ai_service.generate_embeddings(&embed_text).await {
                            Ok(embedding) => {
                                let model_name =
                                    state.config.read().ollama_embedding_model.clone();
                                if let Err(e) = state
                                    .database
                                    .save_embedding(&path_str, &embedding, Some(&model_name))
                                    .await
                                {
                                    warn!("Failed to save embedding for {}: {}", path_str, e);
                                }
                            }
                            Err(e) => {
                                warn!("Embedding generation failed for {}: {}", path_str, e);
                            }
                        }
                    }

                    Some(analysis)
                }
                Err(e) => {
                    warn!("AI analysis failed for {}: {}", path_str, e);
                    None
                }
            },
        };

        // 2. Find best matching smart folder (using existing logic)
        let smart_folders = state.database.list_smart_folders().await?;
        let mut best_match = None;
        let mut highest_confidence = 0.0;

        for folder in smart_folders {
            if !folder.enabled {
                continue;
            }

            // Use existing confidence calculation from organization.rs
            let confidence = crate::commands::organization::calculate_folder_match_confidence(
                &path_str, &folder,
            )
            .await;

            if confidence > highest_confidence {
                highest_confidence = confidence;
                best_match = Some(folder);
            }
        }

        // 3. Auto-organize if confidence is high enough
        if highest_confidence >= confidence_threshold {
            if let Some(target_folder) = best_match {
                let target_dir = Path::new(&target_folder.target_path);

                // Create target directory if needed
                if let Some(parent) = target_dir.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }

                // Generate smart filename
                let smart_filename = if let Some(ref analysis) = ai_analysis {
                    generate_smart_filename_from_analysis(&path_str, analysis)
                } else {
                    file_path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown_file".to_string())
                };

                let target_path = target_dir.join(&smart_filename);

                // Perform the move
                match tokio::fs::rename(&file_path, &target_path).await {
                    Ok(_) => {
                        info!(
                            "Auto-organized: {} -> {} (confidence: {:.1}%)",
                            file_path.display(),
                            target_path.display(),
                            highest_confidence * 100.0
                        );

                        // Emit success notification to frontend
                        let _ = app_handle.emit(
                            "file-auto-organized",
                            serde_json::json!({
                                "source": path_str,
                                "target": target_path.to_string_lossy(),
                                "confidence": highest_confidence,
                                "folder": target_folder.name
                            }),
                        );
                    }
                    Err(e) => {
                        warn!("Failed to auto-organize {}: {}", path_str, e);
                    }
                }
            }
        } else {
            debug!(
                "Skipping auto-organization for {} (confidence too low: {:.1}%)",
                path_str,
                highest_confidence * 100.0
            );
        }

        Ok(())
    }
}

/// Enhanced file event handler with learning capabilities
async fn handle_enhanced_file_event(
    event: Event,
    app_handle: &AppHandle,
    _state: &Arc<AppState>,
    config: &Arc<RwLock<WatchModeConfig>>,
    pending_files: &Arc<RwLock<HashMap<PathBuf, PendingFile>>>,
    _user_actions: &Arc<RwLock<Vec<UserAction>>>,
    _recent_operations: &Arc<RwLock<HashMap<String, Vec<FileEvent>>>>,
) {
    debug!("Enhanced file event: {:?}", event);

    let config_read = config.read().await;
    if !config_read.enabled {
        return;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_else(|_| {
            warn!("System time is before UNIX epoch, using fallback timestamp");
            0
        });

    match event.kind {
        EventKind::Create(_) => {
            for path in event.paths {
                // Skip if file should be ignored
                if should_ignore_file_enhanced(&path, &config_read) {
                    continue;
                }

                // Add to pending files for auto-organization
                if path.is_file() {
                    if let Ok(metadata) = tokio::fs::metadata(&path).await {
                        let pending_file = PendingFile {
                            path: path.clone(),
                            detected_at: Instant::now(),
                            file_size: metadata.len(),
                            last_modified: metadata.modified().unwrap_or(SystemTime::now()),
                        };

                        pending_files
                            .write()
                            .await
                            .insert(path.clone(), pending_file);

                        debug!("Added file to auto-organization queue: {}", path.display());
                    }
                }

                // Emit enhanced event to frontend
                let file_event = FileEvent {
                    event_type: "created".to_string(),
                    path: path.to_string_lossy().to_string(),
                    timestamp,
                    file_name: path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string()),
                    extension: path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_string()),
                    source_path: None,
                    destination_path: None,
                    is_user_action: false, // System detected file creation
                    operation_id: None,
                };

                let _ = app_handle.emit("enhanced-file-event", &file_event);
            }
        }
        EventKind::Modify(_) => {
            // Handle file modifications - could indicate user is actively working with file
            for path in event.paths {
                if should_ignore_file_enhanced(&path, &config_read) {
                    continue;
                }

                let file_event = FileEvent {
                    event_type: "modified".to_string(),
                    path: path.to_string_lossy().to_string(),
                    timestamp,
                    file_name: path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string()),
                    extension: path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_string()),
                    source_path: None,
                    destination_path: None,
                    is_user_action: true, // File modification likely user action
                    operation_id: None,
                };

                let _ = app_handle.emit("enhanced-file-event", &file_event);
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                // Remove from pending files if it was queued
                pending_files.write().await.remove(&path);

                let file_event = FileEvent {
                    event_type: "removed".to_string(),
                    path: path.to_string_lossy().to_string(),
                    timestamp,
                    file_name: path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string()),
                    extension: path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_string()),
                    source_path: None,
                    destination_path: None,
                    is_user_action: true, // File deletion likely user action
                    operation_id: None,
                };

                let _ = app_handle.emit("enhanced-file-event", &file_event);
            }
        }
        _ => {
            // Handle other event types (access, rename, etc.)
            debug!("Unhandled event type: {:?}", event.kind);
        }
    }
}

/// Default depth/count caps for the initial directory scan. Surfaced as
/// constants so tests can reference them and so a user pointing at `~/` does
/// not enqueue half a million files on every watch-mode toggle.
pub(crate) const MAX_INITIAL_SCAN_DEPTH: usize = 8;
pub(crate) const MAX_INITIAL_SCAN_FILES: usize = 5_000;

/// Walk `directories` and insert any matching files into `target`, applying
/// the same filters as the live notify event handler and capping both
/// recursion depth and total inserted entries. Returns the number of files
/// actually inserted (which can be < total caller intends if either cap is
/// hit). Designed to be callable both from the live watcher (`enqueue_existing_files`)
/// and from unit tests that own a plain `HashMap`.
pub(crate) async fn walk_existing_files_into(
    directories: &[String],
    config: &WatchModeConfig,
    target: &mut HashMap<PathBuf, PendingFile>,
    max_depth: usize,
    max_files: usize,
) -> usize {
    let mut enqueued = 0usize;
    for dir in directories {
        let root = Path::new(dir);
        if !root.exists() || !root.is_dir() {
            debug!("Initial scan: skipping non-directory {}", dir);
            continue;
        }

        let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
        while let Some((current, depth)) = stack.pop() {
            if enqueued >= max_files {
                warn!("Initial scan hit cap of {} files; stopping early", max_files);
                return enqueued;
            }

            let mut entries = match tokio::fs::read_dir(&current).await {
                Ok(e) => e,
                Err(e) => {
                    debug!("Initial scan: cannot read {}: {}", current.display(), e);
                    continue;
                }
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let file_type = match entry.file_type().await {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                if file_type.is_dir() {
                    if depth + 1 < max_depth && !should_ignore_file_enhanced(&path, config) {
                        stack.push((path, depth + 1));
                    }
                    continue;
                }

                if !file_type.is_file() || should_ignore_file_enhanced(&path, config) {
                    continue;
                }

                let metadata = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                target.insert(
                    path.clone(),
                    PendingFile {
                        path,
                        detected_at: Instant::now(),
                        file_size: metadata.len(),
                        last_modified: metadata.modified().unwrap_or(SystemTime::now()),
                    },
                );
                enqueued += 1;

                if enqueued >= max_files {
                    warn!("Initial scan hit cap of {} files; stopping early", max_files);
                    return enqueued;
                }
            }
        }
    }
    enqueued
}

/// Enhanced file filtering with watch mode configuration
pub(crate) fn should_ignore_file_enhanced(path: &Path, config: &WatchModeConfig) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();

    // Check excluded extensions
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = format!(".{}", extension.to_lowercase());
        if config.excluded_extensions.contains(&ext_lower) {
            return true;
        }
    }

    // Check excluded directories
    for excluded_dir in &config.excluded_directories {
        if path_str.contains(&excluded_dir.to_lowercase()) {
            return true;
        }
    }

    // Skip hidden files and temporary files
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        if file_name.starts_with('.')
            || file_name.starts_with('~')
            || file_name.ends_with('~')
            || file_name.starts_with("Thumbs.db")
            || file_name.starts_with(".DS_Store")
        {
            return true;
        }
    }

    false
}

/// Generate smart filename based on AI analysis
fn generate_smart_filename_from_analysis(
    original_path: &str,
    analysis: &crate::ai::FileAnalysis,
) -> String {
    let path = Path::new(original_path);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Use AI suggested name if available and high confidence
    if analysis.confidence > 0.8 && !analysis.summary.is_empty() {
        // Extract meaningful words from summary
        let meaningful_words: Vec<&str> = analysis
            .summary
            .split_whitespace()
            .filter(|word| {
                word.len() > 3
                    && ![
                        "the", "and", "for", "with", "this", "that", "from", "they", "have", "been",
                    ]
                    .contains(&word.to_lowercase().as_str())
            })
            .take(3)
            .collect();

        if !meaningful_words.is_empty() {
            let smart_name = meaningful_words
                .join("_")
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>();

            return format!("{}.{}", smart_name, extension);
        }
    }

    // Fallback to original filename
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod walk_tests {
    //! Unit tests for the free function `walk_existing_files_into`.
    //! Exercises the initial-scan path that runs when watch mode flips on or
    //! a new directory is added — the load-bearing piece of the "I turned it
    //! on and nothing happened" fix. Tests touch only the filesystem and the
    //! free function, so they need neither an `AppHandle` nor Ollama.

    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    async fn touch(dir: &std::path::Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        let mut f = tokio::fs::File::create(&p).await.unwrap();
        f.write_all(b"x").await.unwrap();
        f.flush().await.ok();
        p
    }

    fn cfg() -> WatchModeConfig {
        WatchModeConfig::default()
    }

    #[tokio::test]
    async fn enqueues_files_in_flat_directory() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "a.txt").await;
        touch(tmp.path(), "b.png").await;
        touch(tmp.path(), "c.pdf").await;

        let mut target = HashMap::new();
        let dirs = vec![tmp.path().to_string_lossy().to_string()];
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 8, 5_000).await;

        assert_eq!(n, 3, "all three plain files should be enqueued");
        assert_eq!(target.len(), 3);
    }

    #[tokio::test]
    async fn skips_hidden_and_temp_files() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), ".hidden").await;
        touch(tmp.path(), "real.txt").await;
        touch(tmp.path(), "scratch.tmp").await;
        touch(tmp.path(), "Thumbs.db").await;
        touch(tmp.path(), ".DS_Store").await;
        touch(tmp.path(), "~temp").await;

        let mut target = HashMap::new();
        let dirs = vec![tmp.path().to_string_lossy().to_string()];
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 8, 5_000).await;

        assert_eq!(n, 1, "only real.txt should survive filtering");
        assert!(target
            .keys()
            .any(|p| p.file_name().and_then(|s| s.to_str()) == Some("real.txt")));
    }

    #[tokio::test]
    async fn recurses_into_subdirectories_within_depth_cap() {
        let tmp = TempDir::new().unwrap();
        // Build: tmp/lvl1/lvl2/lvl3/leaf.txt
        let lvl3 = tmp.path().join("lvl1").join("lvl2").join("lvl3");
        tokio::fs::create_dir_all(&lvl3).await.unwrap();
        touch(&lvl3, "leaf.txt").await;
        touch(tmp.path(), "top.txt").await;

        let mut target = HashMap::new();
        let dirs = vec![tmp.path().to_string_lossy().to_string()];
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 8, 5_000).await;
        assert_eq!(n, 2, "both top.txt and leaf.txt should be enqueued");
    }

    #[tokio::test]
    async fn respects_depth_cap() {
        let tmp = TempDir::new().unwrap();
        // Chain 5 deep: root/d1/d2/d3/d4 each with one file
        let mut cur = tmp.path().to_path_buf();
        touch(&cur, "f0.txt").await;
        for i in 1..=4 {
            cur = cur.join(format!("d{}", i));
            tokio::fs::create_dir(&cur).await.unwrap();
            touch(&cur, &format!("f{}.txt", i)).await;
        }

        let mut target = HashMap::new();
        let dirs = vec![tmp.path().to_string_lossy().to_string()];
        // Cap depth at 2 — only files at depth 0 and 1 should be reached.
        // Depth 0 = root (f0.txt). Depth 1 = root/d1 (f1.txt). f2.txt is at
        // depth 2 which is excluded by `depth + 1 < max_depth` (the directory
        // d2 would be at depth 2; pushing it requires depth+1=2 < max_depth=2,
        // which is false, so d2 is not entered).
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 2, 5_000).await;
        assert_eq!(n, 2, "depth cap of 2 should yield f0.txt and f1.txt only");
        let names: std::collections::HashSet<_> = target
            .keys()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
            .collect();
        assert!(names.contains("f0.txt"));
        assert!(names.contains("f1.txt"));
        assert!(!names.contains("f2.txt"));
    }

    #[tokio::test]
    async fn respects_file_count_cap() {
        let tmp = TempDir::new().unwrap();
        for i in 0..50 {
            touch(tmp.path(), &format!("file_{:03}.txt", i)).await;
        }

        let mut target = HashMap::new();
        let dirs = vec![tmp.path().to_string_lossy().to_string()];
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 8, 10).await;
        assert_eq!(n, 10, "scan must stop at the count cap");
        assert_eq!(target.len(), 10);
    }

    #[tokio::test]
    async fn nonexistent_directory_is_skipped_silently() {
        let mut target = HashMap::new();
        let dirs = vec![
            "/nonexistent/path/abc123".to_string(),
            "/another/missing/one".to_string(),
        ];
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 8, 5_000).await;
        assert_eq!(n, 0);
        assert!(target.is_empty());
    }

    #[tokio::test]
    async fn excluded_extensions_from_config_are_filtered() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "doc.txt").await;
        touch(tmp.path(), "trace.log").await;
        touch(tmp.path(), "data.csv").await;

        let mut c = cfg();
        c.excluded_extensions = vec![".log".to_string(), ".csv".to_string()];

        let mut target = HashMap::new();
        let dirs = vec![tmp.path().to_string_lossy().to_string()];
        let n = walk_existing_files_into(&dirs, &c, &mut target, 8, 5_000).await;
        assert_eq!(n, 1, "only doc.txt should survive the extension filter");
        assert!(target
            .keys()
            .any(|p| p.file_name().and_then(|s| s.to_str()) == Some("doc.txt")));
    }

    #[tokio::test]
    async fn excluded_directories_skip_whole_subtrees() {
        let tmp = TempDir::new().unwrap();
        let git_dir = tmp.path().join(".git");
        tokio::fs::create_dir(&git_dir).await.unwrap();
        touch(&git_dir, "HEAD").await;
        touch(&git_dir, "config").await;
        touch(tmp.path(), "src.rs").await;

        let mut target = HashMap::new();
        let dirs = vec![tmp.path().to_string_lossy().to_string()];
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 8, 5_000).await;
        // The default WatchModeConfig excludes ".git" — the two files inside
        // it should be skipped, leaving only src.rs.
        assert_eq!(n, 1, ".git contents should be skipped");
        assert!(target
            .keys()
            .any(|p| p.file_name().and_then(|s| s.to_str()) == Some("src.rs")));
    }

    #[tokio::test]
    async fn multiple_directories_are_all_walked() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        touch(tmp1.path(), "from_first.txt").await;
        touch(tmp2.path(), "from_second.txt").await;

        let mut target = HashMap::new();
        let dirs = vec![
            tmp1.path().to_string_lossy().to_string(),
            tmp2.path().to_string_lossy().to_string(),
        ];
        let n = walk_existing_files_into(&dirs, &cfg(), &mut target, 8, 5_000).await;
        assert_eq!(n, 2);
        let names: std::collections::HashSet<_> = target
            .keys()
            .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
            .collect();
        assert!(names.contains("from_first.txt"));
        assert!(names.contains("from_second.txt"));
    }
}
