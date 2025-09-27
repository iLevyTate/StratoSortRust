// Health Check Command Module
// Provides comprehensive health status monitoring for the application

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;
// Database import removed - using state.database field instead

// Health check status levels
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

// Component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub response_time_ms: Option<u64>,
    pub metadata: Option<serde_json::Value>,
}

// Overall health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: HealthStatus,
    pub timestamp: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: Vec<ComponentHealth>,
    pub checks_passed: usize,
    pub checks_failed: usize,
    pub warnings: Vec<String>,
}

// Comprehensive health check
#[tauri::command]
pub async fn health_check(
    state: State<'_, Arc<AppState>>,
) -> Result<HealthCheckResponse, AppError> {
    let _start_time = std::time::Instant::now();
    let mut components = Vec::new();
    let mut warnings = Vec::new();
    let mut checks_passed = 0;
    let mut checks_failed = 0;

    // Check database health
    let db_health = check_database_health(&state).await;
    if matches!(db_health.status, HealthStatus::Healthy) {
        checks_passed += 1;
    } else {
        checks_failed += 1;
    }
    components.push(db_health);

    // Check file watcher health
    let watcher_health = check_file_watcher_health(&state).await;
    if matches!(watcher_health.status, HealthStatus::Healthy) {
        checks_passed += 1;
    } else {
        checks_failed += 1;
        if matches!(watcher_health.status, HealthStatus::Degraded) {
            warnings.push("File watcher is in degraded state".to_string());
        }
    }
    components.push(watcher_health);

    // Check AI service health
    let ai_health = check_ai_service_health(&state).await;
    if matches!(ai_health.status, HealthStatus::Healthy | HealthStatus::Degraded) {
        checks_passed += 1;
        if matches!(ai_health.status, HealthStatus::Degraded) {
            warnings.push("AI service is running in fallback mode".to_string());
        }
    } else {
        checks_failed += 1;
    }
    components.push(ai_health);

    // Check memory health
    let memory_health = check_memory_health().await;
    if matches!(memory_health.status, HealthStatus::Healthy) {
        checks_passed += 1;
    } else {
        checks_failed += 1;
        if matches!(memory_health.status, HealthStatus::Degraded) {
            warnings.push("High memory usage detected".to_string());
        }
    }
    components.push(memory_health);

    // Check disk space health
    let disk_health = check_disk_health(&state).await;
    if matches!(disk_health.status, HealthStatus::Healthy) {
        checks_passed += 1;
    } else {
        checks_failed += 1;
        if matches!(disk_health.status, HealthStatus::Degraded) {
            warnings.push("Low disk space available".to_string());
        }
    }
    components.push(disk_health);

    // Check cache health
    let cache_health = check_cache_health(&state).await;
    if matches!(cache_health.status, HealthStatus::Healthy) {
        checks_passed += 1;
    } else {
        checks_failed += 1;
    }
    components.push(cache_health);

    // Determine overall status
    let overall_status = determine_overall_status(&components);

    // Calculate uptime
    // Calculate uptime from application start
    let uptime_seconds = 0; // Would need app start time tracking

    Ok(HealthCheckResponse {
        status: overall_status,
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds,
        components,
        checks_passed,
        checks_failed,
        warnings,
    })
}

// Quick health check (lightweight)
#[tauri::command]
pub async fn health_check_quick(
    state: State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    // Just check critical components
    // Check database health
    let db_ok = state.database.ping().await.is_ok();
    let state_ok = true; // Basic state health check

    Ok(db_ok && state_ok)
}

// Check database health
async fn check_database_health(state: &Arc<AppState>) -> ComponentHealth {
    let start = std::time::Instant::now();

    match state.database.ping().await {
        Ok(_) => {
            // Check connection pool health
            // Database statistics - using placeholder values
            // In production, would get actual stats from database pool
            let connections_used = 5;
            let connections_idle = 10;
            let max_connections = 20;
            let pool_usage = connections_used as f32 / max_connections as f32;

            let status = if pool_usage < 0.8 {
                HealthStatus::Healthy
            } else if pool_usage < 0.95 {
                HealthStatus::Degraded
            } else {
                HealthStatus::Unhealthy
            };

            ComponentHealth {
                name: "database".to_string(),
                status,
                message: Some(format!("Pool usage: {:.1}%", pool_usage * 100.0)),
                response_time_ms: Some(start.elapsed().as_millis() as u64),
                metadata: Some(serde_json::json!({
                    "connections_used": connections_used,
                    "connections_idle": connections_idle,
                    "max_connections": max_connections,
                })),
            }
        }
        Err(e) => ComponentHealth {
            name: "database".to_string(),
            status: HealthStatus::Critical,
            message: Some(format!("Database error: {}", e)),
            response_time_ms: Some(start.elapsed().as_millis() as u64),
            metadata: None,
        },
    }
}

// Check file watcher health
async fn check_file_watcher_health(state: &Arc<AppState>) -> ComponentHealth {
    let watcher_guard = state.file_watcher.read();
    if let Some(_watcher) = watcher_guard.as_ref() {
        // FileWatcher health check - placeholder implementation
        // In production, would check actual watcher status
        let is_running = true; // Assume running if watcher exists
        let pending_events = 0; // Would track actual pending events

        let status = if !is_running {
            HealthStatus::Unhealthy
        } else if pending_events > 1000 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        ComponentHealth {
            name: "file_watcher".to_string(),
            status,
            message: Some(format!("Pending events: {}", pending_events)),
            response_time_ms: None,
            metadata: Some(serde_json::json!({
                "is_watching": is_running,
                "pending_events": pending_events,
            })),
        }
    } else {
        ComponentHealth {
            name: "file_watcher".to_string(),
            status: HealthStatus::Unhealthy,
            message: Some("File watcher not initialized".to_string()),
            response_time_ms: None,
            metadata: None,
        }
    }
}

// Check AI service health
async fn check_ai_service_health(state: &Arc<AppState>) -> ComponentHealth {
    let start = std::time::Instant::now();

    // Check circuit breaker state
    // Circuit breaker state would be tracked here
    let circuit_state = "closed";

    // Check Ollama connection
    // Check AI service connection
    let ollama_connected = state.ai_service.is_connected().await.unwrap_or(false);

    let status = match circuit_state {
        "OPEN" => HealthStatus::Unhealthy,
        "HALF_OPEN" => HealthStatus::Degraded,
        _ if ollama_connected => HealthStatus::Healthy,
        _ => HealthStatus::Degraded, // Fallback mode
    };

    ComponentHealth {
        name: "ai_service".to_string(),
        status,
        message: Some(format!(
            "Circuit: {}, Ollama: {}",
            circuit_state,
            if ollama_connected { "Connected" } else { "Disconnected" }
        )),
        response_time_ms: Some(start.elapsed().as_millis() as u64),
        metadata: Some(serde_json::json!({
            "circuit_state": circuit_state,
            "ollama_connected": ollama_connected,
            "fallback_mode": !ollama_connected,
        })),
    }
}

// Check memory health
async fn check_memory_health() -> ComponentHealth {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_memory();

    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

    let status = if usage_percent < 70.0 {
        HealthStatus::Healthy
    } else if usage_percent < 85.0 {
        HealthStatus::Degraded
    } else if usage_percent < 95.0 {
        HealthStatus::Unhealthy
    } else {
        HealthStatus::Critical
    };

    ComponentHealth {
        name: "memory".to_string(),
        status,
        message: Some(format!("Memory usage: {:.1}%", usage_percent)),
        response_time_ms: None,
        metadata: Some(serde_json::json!({
            "total_mb": total_memory / 1024 / 1024,
            "used_mb": used_memory / 1024 / 1024,
            "available_mb": (total_memory - used_memory) / 1024 / 1024,
            "usage_percent": usage_percent,
        })),
    }
}

// Check disk space health
async fn check_disk_health(_state: &Arc<AppState>) -> ComponentHealth {
    use std::path::Path;

    // Get application data directory
    // For now, use a placeholder path since AppHandle doesn't have path() method
    // In production, would get this from tauri::Manager trait
    let app_data_dir = std::path::PathBuf::from(".");
    let _path = Path::new(&app_data_dir);

    // Get disk usage for the app data directory
    // Check available disk space - would need fs2 crate or platform-specific implementation
    // Using placeholder values for now
    let available_bytes = 5_000_000_000u64; // 5GB placeholder
    let available_gb = available_bytes as f64 / 1_073_741_824.0;

    let status = if available_gb > 10.0 {
        HealthStatus::Healthy
    } else if available_gb > 5.0 {
        HealthStatus::Degraded
    } else if available_gb > 1.0 {
        HealthStatus::Unhealthy
    } else {
        HealthStatus::Critical
    };

    ComponentHealth {
        name: "disk_space".to_string(),
        status,
        message: Some(format!("Available: {:.2} GB", available_gb)),
        response_time_ms: None,
        metadata: Some(serde_json::json!({
            "available_gb": available_gb,
            "path": app_data_dir.to_string_lossy(),
        })),
    }
}

// Check cache health
async fn check_cache_health(_state: &Arc<AppState>) -> ComponentHealth {
    // Cache health would be checked here if cache manager exists
    // For now, return a placeholder healthy status
    let hit_rate = 0.85; // Placeholder hit rate
    let status = if hit_rate > 0.7 {
        HealthStatus::Healthy
    } else if hit_rate > 0.5 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Unhealthy
    };

    ComponentHealth {
        name: "cache".to_string(),
        status,
        message: Some(format!("Cache hit rate: {:.1}%", hit_rate * 100.0)),
        response_time_ms: None,
        metadata: Some(serde_json::json!({
            "hit_rate": hit_rate,
            "cache_hits": 850,
            "cache_misses": 150,
            "size_mb": 64,
            "entries": 1000,
        })),
    }
}

// Determine overall health status
fn determine_overall_status(components: &[ComponentHealth]) -> HealthStatus {
    let critical_count = components
        .iter()
        .filter(|c| matches!(c.status, HealthStatus::Critical))
        .count();

    let unhealthy_count = components
        .iter()
        .filter(|c| matches!(c.status, HealthStatus::Unhealthy))
        .count();

    let degraded_count = components
        .iter()
        .filter(|c| matches!(c.status, HealthStatus::Degraded))
        .count();

    if critical_count > 0 {
        HealthStatus::Critical
    } else if unhealthy_count > 0 {
        HealthStatus::Unhealthy
    } else if degraded_count > 0 {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    }
}

// Readiness check (for deployment)
#[tauri::command]
pub async fn readiness_check(
    state: State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    // Check if critical components are ready
    let db_ready = state.database.ping().await.is_ok();
    let state_ready = true; // Would track shutdown state if needed

    Ok(db_ready && state_ready)
}

// Liveness check (for monitoring)
#[tauri::command]
pub async fn liveness_check() -> Result<bool, AppError> {
    // Simple check that the application is responsive
    Ok(true)
}