use crate::{
    error::Result,
    services::file_watcher::{UserAction, UserActionType, WatchModeConfig},
    state::AppState,
    utils::security::validate_directory_path,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tracing::{info, warn};

/// Validate a batch of directory paths, returning the canonicalized list.
/// First validation error stops the batch — partial enable would leave the
/// config in a half-applied state.
fn validate_directories(dirs: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(dirs.len());
    for d in dirs {
        let canonical = validate_directory_path(d)?;
        out.push(canonical.to_string_lossy().to_string());
    }
    Ok(out)
}

/// Mirror watch-mode runtime state into the persisted Config so the user's
/// choice survives a restart. Mutating the in-memory FileWatcher config without
/// this leaves Config out of sync — on the next boot the watcher would start
/// disabled and empty even though the user had it on. Failures are logged but
/// not propagated; the runtime change still took effect for this session.
fn persist_watch_state(
    state: &State<'_, Arc<AppState>>,
    enabled: Option<bool>,
    directories: Option<Vec<String>>,
) {
    let snapshot = {
        let mut cfg = state.config.write();
        if let Some(enabled) = enabled {
            cfg.watch_folders = enabled;
        }
        if let Some(dirs) = directories {
            cfg.watch_paths = dirs;
        }
        cfg.clone()
    };

    if let Err(e) = snapshot.save(&state.handle) {
        warn!("Failed to persist watch-mode config: {}", e);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchModeStatus {
    pub enabled: bool,
    pub watching_directories: Vec<String>,
    pub pending_files_count: usize,
    pub auto_organize_threshold: f32,
    pub learning_enabled: bool,
    pub recent_actions_count: usize,
}

/// Get current watch mode status and configuration
#[tauri::command]
pub async fn get_watch_mode_status(state: State<'_, Arc<AppState>>) -> Result<WatchModeStatus> {
    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(std::sync::Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let config = watcher_arc.get_watch_config().await;
        let pending_count = watcher_arc.get_pending_files_count().await;
        let actions_count = watcher_arc.get_recent_user_actions(100).await.len();

        Ok(WatchModeStatus {
            enabled: config.enabled,
            watching_directories: config.watch_directories,
            pending_files_count: pending_count,
            auto_organize_threshold: config.confidence_threshold,
            learning_enabled: config.learning_enabled,
            recent_actions_count: actions_count,
        })
    } else {
        // Return default status if file watcher is not initialized
        Ok(WatchModeStatus {
            enabled: false,
            watching_directories: vec![],
            pending_files_count: 0,
            auto_organize_threshold: 0.8,
            learning_enabled: false,
            recent_actions_count: 0,
        })
    }
}

/// Configure watch mode settings
#[tauri::command]
pub async fn configure_watch_mode(
    mut config: WatchModeConfig,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    info!(
        "Configuring watch mode: enabled={}, directories={:?}",
        config.enabled, config.watch_directories
    );

    // Validate every directory before mutating runtime or persisted state. If
    // any path is malformed/missing/system, the whole call is rejected so we
    // never partially update the watcher.
    config.watch_directories = validate_directories(&config.watch_directories)?;

    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let enabled = config.enabled;
        let directories = config.watch_directories.clone();
        watcher_arc.configure_watch_mode(config).await?;
        persist_watch_state(&state, Some(enabled), Some(directories));
    }
    Ok(())
}

/// Enable watch mode
#[tauri::command]
pub async fn enable_watch_mode(
    directories: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let directories = validate_directories(&directories)?;

    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let mut config = watcher_arc.get_watch_config().await;
        config.enabled = true;
        config.watch_directories = directories.clone();

        watcher_arc.configure_watch_mode(config).await?;
        persist_watch_state(&state, Some(true), Some(directories));
        info!("Watch mode enabled");
    }
    Ok(())
}

/// Disable watch mode
#[tauri::command]
pub async fn disable_watch_mode(state: State<'_, Arc<AppState>>) -> Result<()> {
    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let mut config = watcher_arc.get_watch_config().await;
        config.enabled = false;

        watcher_arc.configure_watch_mode(config).await?;
        persist_watch_state(&state, Some(false), None);
        info!("Watch mode disabled");
    }
    Ok(())
}

/// Record a user action for learning (called when user manually organizes files)
#[tauri::command]
pub async fn record_user_organization_action(
    source_path: String,
    destination_path: String,
    action_type: String,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let action_type = match action_type.as_str() {
        "move" => UserActionType::MoveFile,
        "rename" => UserActionType::RenameFile,
        "create_folder" => UserActionType::CreateFolder,
        "organize" => UserActionType::OrganizeFiles,
        _ => UserActionType::MoveFile,
    };

    let user_action = UserAction {
        action_type,
        timestamp: chrono::Utc::now().timestamp(),
        file_path: source_path,
        destination_path: Some(destination_path),
        folder_created: None,
        rename_pattern: None,
        confidence: 1.0, // User action has 100% confidence
    };

    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(std::sync::Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        watcher_arc.record_user_action(user_action).await;
        info!("Recorded user organization action for learning");
    }
    Ok(())
}

/// Get recent user actions for pattern analysis
#[tauri::command]
pub async fn get_user_learning_patterns(
    limit: Option<usize>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<UserAction>> {
    let limit = limit.unwrap_or(50);
    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(std::sync::Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let actions = watcher_arc.get_recent_user_actions(limit).await;
        Ok(actions)
    } else {
        Ok(vec![])
    }
}

/// Update watch mode confidence threshold
#[tauri::command]
pub async fn update_auto_organize_threshold(
    threshold: f32,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err(crate::error::AppError::ValidationError {
            field: "threshold".to_string(),
            message: "Threshold must be between 0.0 and 1.0".to_string(),
        });
    }

    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let mut config = watcher_arc.get_watch_config().await;
        config.confidence_threshold = threshold;

        watcher_arc.configure_watch_mode(config).await?;
    }
    info!("Updated auto-organize threshold to {}", threshold);
    Ok(())
}

/// Get pending files awaiting auto-organization
#[tauri::command]
pub async fn get_pending_auto_organization(state: State<'_, Arc<AppState>>) -> Result<Vec<String>> {
    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(std::sync::Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let file_paths = watcher_arc.get_pending_file_paths().await;
        Ok(file_paths)
    } else {
        Ok(vec![])
    }
}

/// Manually trigger auto-organization of pending files
#[tauri::command]
pub async fn trigger_auto_organization(state: State<'_, Arc<AppState>>) -> Result<usize> {
    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(std::sync::Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let file_count = watcher_arc.clear_pending_files().await;

        if file_count > 0 {
            info!(
                "Manually triggering auto-organization for {} files",
                file_count
            );
        }

        Ok(file_count)
    } else {
        Ok(0)
    }
}

/// Add directory to watch list
#[tauri::command]
pub async fn add_watch_directory(
    directory_path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let directory_path = validate_directory_path(&directory_path)?
        .to_string_lossy()
        .to_string();

    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let mut config = watcher_arc.get_watch_config().await;

        if !config.watch_directories.contains(&directory_path) {
            config.watch_directories.push(directory_path.clone());
            let new_dirs = config.watch_directories.clone();
            watcher_arc.configure_watch_mode(config).await?;
            persist_watch_state(&state, None, Some(new_dirs));
            info!("Added directory to watch list: {}", directory_path);
        }
    }

    Ok(())
}

/// Remove directory from watch list
#[tauri::command]
pub async fn remove_watch_directory(
    directory_path: String,
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let watcher_arc = {
        let watcher_guard = state.file_watcher.read();
        watcher_guard.as_ref().map(Arc::clone)
    };

    if let Some(watcher_arc) = watcher_arc {
        let mut config = watcher_arc.get_watch_config().await;

        config
            .watch_directories
            .retain(|dir| dir != &directory_path);
        let new_dirs = config.watch_directories.clone();
        watcher_arc.configure_watch_mode(config).await?;
        persist_watch_state(&state, None, Some(new_dirs));
        info!("Removed directory from watch list: {}", directory_path);
    }

    Ok(())
}
