use crate::state::AppState;
use std::sync::Arc;
use tauri::Runtime;
use tracing::{error, info};

/// Set up graceful shutdown handler
pub fn setup_shutdown_handler<R: Runtime>(state: Arc<AppState<R>>) {
    // Use the runtime handle instead of spawning directly
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("Shutdown signal received (Ctrl+C)");

                    // Perform graceful shutdown
                    if let Err(e) = shutdown_services(&state).await {
                        error!("Error during graceful shutdown: {}", e);
                    }

                    info!("Graceful shutdown complete");
                    std::process::exit(0);
                }
                Err(err) => {
                    error!("Unable to listen for shutdown signal: {}", err);
                }
            }
        });
    } else {
        // If no runtime available, skip shutdown handler setup
        info!("No Tokio runtime available, skipping shutdown handler setup");
    }
}

/// Gracefully shutdown all services
pub async fn shutdown_services<R: Runtime>(state: &AppState<R>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Shutting down services...");

    // Stop file watcher
    if let Some(watcher) = state.file_watcher.write().take() {
        info!("Stopping file watcher...");
        // FileWatcher Drop implementation will handle cleanup
        drop(watcher);
    }

    // Save learned patterns
    {
        let pattern_learner = state.pattern_learner.read().await;
        let patterns = pattern_learner.save_patterns();
        if !patterns.is_empty() {
            info!("Saving {} learned patterns...", patterns.len());
            // Save to database
            if let Ok(patterns_json) = serde_json::to_string(&patterns) {
                let _ = sqlx::query(
                    "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('learned_patterns', ?)"
                )
                .bind(&patterns_json)
                .execute(state.database.pool())
                .await;
            }
        }
    }

    // Cancel all active operations
    for entry in state.active_operations.iter() {
        let (_id, status) = entry.pair();
        if !status.cancellation_token.is_cancelled() {
            status.cancellation_token.cancel();
        }
    }

    // Wait for background tasks to complete
    {
        let tasks_to_await: Vec<_> = {
            let mut tasks = state.background_tasks.write();
            info!("Waiting for {} background tasks...", tasks.len());
            tasks.drain(..).collect()
        };

        for task in tasks_to_await {
            // Give tasks a chance to complete gracefully
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                task
            ).await;
        }
    }

    // Database pool will be dropped automatically when state is dropped
    info!("Database connections will be closed on drop...");

    info!("All services shut down successfully");
    Ok(())
}