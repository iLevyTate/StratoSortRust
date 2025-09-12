use crate::{
    ai::{AiServiceStatus},
    error::Result,
    state::AppState,
};
use tauri::{AppHandle, Emitter, State};

/// Get current AI service status
#[tauri::command]
pub async fn get_ai_status(state: State<'_, AppState>) -> Result<AiServiceStatus> {
    let status = state.ai_service.get_status().await;
    
    // Emit status change event to frontend
    let _ = state.handle.emit("ai-status-changed", &status);
    
    Ok(status)
}

/// Try to connect to Ollama with a specific host
#[tauri::command]
pub async fn connect_ollama(
    host: String,
    state: State<'_, AppState>,
) -> Result<AiServiceStatus> {
    tracing::info!("Attempting to connect to Ollama at: {}", host);
    
    // Get current status first
    let current_status = state.ai_service.get_status().await;
    
    // If already connected to the same host, return current status
    if current_status.ollama_host == host && current_status.ollama_connected {
        tracing::info!("Already connected to Ollama at {}", host);
        return Ok(current_status);
    }
    
    // Attempt to connect to new host
    match state.ai_service.reconnect_ollama(&host).await {
        Ok(new_status) => {
            // Update config with new host if connection successful
            if new_status.ollama_connected {
                let mut config = state.config.write();
                config.ollama_host = host.clone();
                
                // Save updated config
                if let Err(e) = config.save(&state.handle) {
                    tracing::warn!("Failed to save updated config: {}", e);
                }
                
                // Emit status change event to frontend
                let _ = state.handle.emit("ai-status-changed", &new_status);
                
                tracing::info!("Successfully connected to Ollama at {}", host);
            } else {
                tracing::warn!("Failed to connect to Ollama at {}: {:?}", host, new_status.last_error);
            }
            
            Ok(new_status)
        }
        Err(e) => {
            tracing::error!("Error connecting to Ollama at {}: {}", host, e);
            let mut status = current_status;
            status.last_error = Some(format!("Failed to connect to {}: {}", host, e));
            Ok(status)
        }
    }
}

/// Switch to fallback AI provider
#[tauri::command]
pub async fn use_fallback_ai(state: State<'_, AppState>) -> Result<AiServiceStatus> {
    let status = state.ai_service.use_fallback();
    
    // Emit status change event to frontend
    let _ = state.handle.emit("ai-status-changed", &status);
    
    Ok(status)
}

/// Test AI functionality with a sample analysis
#[tauri::command]
pub async fn test_ai_analysis(state: State<'_, AppState>) -> Result<String> {
    let test_content = "This is a test document to verify AI analysis functionality.";
    
    match state.ai_service.analyze_file(test_content, "text/plain").await {
        Ok(analysis) => {
            Ok(format!(
                "AI analysis successful! Category: {}, Tags: {:?}, Confidence: {:.1}%",
                analysis.category,
                analysis.tags,
                analysis.confidence * 100.0
            ))
        }
        Err(e) => {
            Ok(format!("AI analysis failed: {}", e))
        }
    }
}

/// Get detailed AI capabilities and model information
#[tauri::command]
pub async fn get_ai_capabilities(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let status = state.ai_service.get_status().await;
    
    let capabilities = serde_json::json!({
        "provider": status.provider,
        "is_available": status.is_available,
        "capabilities": status.capabilities,
        "models_available": status.models_available,
        "ollama_connected": status.ollama_connected,
        "ollama_host": status.ollama_host,
        "last_error": status.last_error,
        "features": {
            "file_analysis": true,
            "embeddings": status.capabilities.contains(&"embeddings".to_string()),
            "organization": true,
            "semantic_search": status.capabilities.contains(&"embeddings".to_string()),
            "auto_organization": true,
            "learning": true
        }
    });
    
    Ok(capabilities)
}

/// Monitor AI service health and emit status updates
pub async fn start_ai_status_monitoring(app_handle: AppHandle, state: AppState) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            
            let status = state.ai_service.get_status().await;
            
            // Only emit if status has changed significantly
            let _ = app_handle.emit("ai-status-update", &status);
            
            // Log status changes
            if !status.is_available {
                tracing::warn!("AI service not available: {:?}", status.last_error);
            }
        }
    });
}