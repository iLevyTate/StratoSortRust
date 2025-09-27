pub mod ai;
pub mod ai_status;
pub mod archive;
pub mod diagnostics;
pub mod files;
pub mod health; // Add health check module for comprehensive monitoring
pub mod history;
pub mod monitoring;
pub mod notifications;
pub mod organization;
pub mod organization_enhanced;
pub mod patterns;
pub mod settings;
pub mod setup;
pub mod system;
pub mod watch_mode;

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[tauri::command]
pub async fn cancel_operation(
    id: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<bool, crate::error::AppError> {
    use uuid::Uuid;
    let uuid = Uuid::parse_str(&id).map_err(|_| crate::error::AppError::InvalidInput {
        message: "Invalid UUID".into(),
    })?;
    Ok(state.cancel_operation(uuid))
}

/// Get all active operations with their current status
#[tauri::command]
pub async fn get_active_operations(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<ActiveOperationInfo>, crate::error::AppError> {
    let operations = state
        .active_operations
        .iter()
        .map(|entry| {
            let (id, status) = entry.pair();
            ActiveOperationInfo {
                id: id.to_string(),
                operation_type: status.operation_type.clone(),
                progress: status.progress,
                message: status.message.clone(),
                can_cancel: !status.cancellation_token.is_cancelled(),
                started_at: status.started_at,
            }
        })
        .collect();

    Ok(operations)
}

/// Internal helper function for get_active_operations that works with direct AppState reference
pub async fn get_active_operations_internal(
    state: &AppState,
) -> Result<Vec<ActiveOperationInfo>, crate::error::AppError> {
    let operations = state
        .active_operations
        .iter()
        .map(|entry| {
            let (id, status) = entry.pair();
            ActiveOperationInfo {
                id: id.to_string(),
                operation_type: status.operation_type.clone(),
                progress: status.progress,
                message: status.message.clone(),
                can_cancel: !status.cancellation_token.is_cancelled(),
                started_at: status.started_at,
            }
        })
        .collect();

    Ok(operations)
}

/// Get detailed progress information for a specific operation
#[tauri::command]
pub async fn get_operation_progress(
    id: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Option<ActiveOperationInfo>, crate::error::AppError> {
    use uuid::Uuid;
    let uuid = Uuid::parse_str(&id).map_err(|_| crate::error::AppError::InvalidInput {
        message: "Invalid UUID".into(),
    })?;

    if let Some(status) = state.active_operations.get(&uuid) {
        Ok(Some(ActiveOperationInfo {
            id,
            operation_type: status.operation_type.clone(),
            progress: status.progress,
            message: status.message.clone(),
            can_cancel: !status.cancellation_token.is_cancelled(),
            started_at: chrono::Utc::now(), // Would be better to track actual start time
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveOperationInfo {
    pub id: String,
    pub operation_type: crate::state::OperationType,
    pub progress: f32,
    pub message: String,
    pub can_cancel: bool,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

// Command modules - accessed via full paths in lib.rs
