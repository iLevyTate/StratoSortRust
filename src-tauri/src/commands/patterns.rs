use crate::core::pattern_learner::FolderPattern;
use crate::error::{AppError, Result};
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tracing::info;

/// Save learned patterns to database
#[tauri::command]
pub async fn save_patterns_to_storage(
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let pattern_learner = state.pattern_learner.read().await;
    let patterns = pattern_learner.save_patterns();

    // Serialize patterns to JSON for storage
    let patterns_json = serde_json::to_string(&patterns)
        .map_err(AppError::SerdeJson)?;

    // Store in database
    let database = &state.database;
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('learned_patterns', ?)"
    )
    .bind(&patterns_json)
    .execute(database.pool())
    .await
    .map_err(|e| AppError::DatabaseError {
        message: format!("Failed to save patterns: {}", e),
    })?;

    info!("Successfully saved {} patterns to storage", patterns.len());
    Ok(())
}

/// Load learned patterns from database
#[tauri::command]
pub async fn load_patterns_from_storage(
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let database = &state.database;

    // Load from database
    let result: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_settings WHERE key = 'learned_patterns'"
    )
    .fetch_optional(database.pool())
    .await
    .map_err(|e| AppError::DatabaseError {
        message: format!("Failed to load patterns: {}", e),
    })?;

    if let Some(patterns_json) = result {
        // Deserialize patterns
        let patterns: HashMap<String, FolderPattern> = serde_json::from_str(&patterns_json)
            .map_err(AppError::SerdeJson)?;

        // Load into pattern learner
        let mut pattern_learner = state.pattern_learner.write().await;
        pattern_learner.load_patterns(patterns.clone());

        info!("Successfully loaded {} patterns from storage", patterns.len());
    } else {
        info!("No saved patterns found in storage");
    }

    Ok(())
}

/// Get current learned patterns
#[tauri::command]
pub async fn get_learned_patterns(
    state: State<'_, Arc<AppState>>,
) -> Result<HashMap<String, FolderPattern>> {
    let pattern_learner = state.pattern_learner.read().await;
    Ok(pattern_learner.save_patterns())
}

/// Clear learned patterns
#[tauri::command]
pub async fn clear_learned_patterns(
    state: State<'_, Arc<AppState>>,
) -> Result<()> {
    let mut pattern_learner = state.pattern_learner.write().await;
    pattern_learner.load_patterns(HashMap::new());

    // Also clear from database
    let database = &state.database;
    sqlx::query(
        "DELETE FROM app_settings WHERE key = 'learned_patterns'"
    )
    .execute(database.pool())
    .await
    .map_err(|e| AppError::DatabaseError {
        message: format!("Failed to clear patterns: {}", e),
    })?;

    info!("Cleared all learned patterns");
    Ok(())
}

/// Cleanup old patterns
#[tauri::command]
pub async fn cleanup_old_patterns(
    state: State<'_, Arc<AppState>>,
    max_age_days: i64,
) -> Result<()> {
    let mut pattern_learner = state.pattern_learner.write().await;
    pattern_learner.cleanup_old_patterns(max_age_days);

    // Save cleaned patterns to database
    let patterns = pattern_learner.save_patterns();
    let patterns_json = serde_json::to_string(&patterns)
        .map_err(AppError::SerdeJson)?;

    let database = &state.database;
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('learned_patterns', ?)"
    )
    .bind(&patterns_json)
    .execute(database.pool())
    .await
    .map_err(|e| AppError::DatabaseError {
        message: format!("Failed to save cleaned patterns: {}", e),
    })?;

    info!("Cleaned up old patterns (max age: {} days)", max_age_days);
    Ok(())
}

/// Record a user's choice for pattern learning
#[tauri::command]
pub async fn record_pattern_choice(
    state: State<'_, Arc<AppState>>,
    file_path: String,
    destination_path: String,
    analysis_keywords: Vec<String>,
    rejected_folders: Vec<String>,
) -> Result<()> {
    use crate::services::file_watcher::{UserAction, UserActionType};

    let mut pattern_learner = state.pattern_learner.write().await;

    let action = UserAction {
        file_path,
        action_type: UserActionType::MoveFile,
        destination_path: Some(destination_path),
        timestamp: chrono::Utc::now().timestamp(),
        confidence: 0.8,
        folder_created: None,
        rename_pattern: None,
    };

    pattern_learner.record_user_choice(&action, &analysis_keywords, &rejected_folders);

    // Auto-save after recording
    let patterns = pattern_learner.save_patterns();
    let patterns_json = serde_json::to_string(&patterns)
        .map_err(AppError::SerdeJson)?;

    let database = &state.database;
    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('learned_patterns', ?)"
    )
    .bind(&patterns_json)
    .execute(database.pool())
    .await
    .map_err(|e| AppError::DatabaseError {
        message: format!("Failed to save pattern choice: {}", e),
    })?;

    info!("Recorded and saved pattern choice");
    Ok(())
}

/// Get pattern-based suggestions for a file
#[tauri::command]
pub async fn get_pattern_suggestions(
    state: State<'_, Arc<AppState>>,
    filename: String,
    analysis_keywords: Vec<String>,
) -> Result<Vec<(String, f32)>> {
    let pattern_learner = state.pattern_learner.read().await;
    Ok(pattern_learner.suggest_folder_from_patterns(&filename, &analysis_keywords))
}