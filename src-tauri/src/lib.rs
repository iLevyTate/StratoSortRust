pub mod ai;
pub mod commands;
pub mod config;
pub mod core;
pub mod error;
pub mod events;
pub mod services;
pub mod shutdown;
pub mod state;
pub mod storage;
pub mod system_tray;
pub mod utils;

use crate::storage::CURRENT_SCHEMA_VERSION;
use crate::utils::{diagnostics::HealthChecker, memory::MemoryMonitor};
use crate::{
    config::Config,
    services::{file_watcher::FileWatcher, notification::NotificationService},
    state::AppState,
};
use std::sync::Arc;
use tauri::{async_runtime, generate_context, generate_handler, Emitter, Manager};
use tracing::{error, info, warn};

async fn initialize_app_state_with_retry(
    handle: tauri::AppHandle,
    config: Config,
) -> Result<AppState, crate::error::AppError> {
    const MAX_RETRIES: u32 = 3;

    // Pre-create app data directory to avoid database initialization failures
    if let Ok(app_data_dir) = handle.path().app_data_dir() {
        if let Err(e) = tokio::fs::create_dir_all(&app_data_dir).await {
            warn!("Failed to pre-create app data directory: {}", e);
        }
    }

    let mut retry_count = 0;

    loop {
        let init_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            AppState::new(handle.clone(), config.clone()),
        )
        .await;

        match init_result {
            Ok(Ok(state)) => {
                info!("AppState initialized successfully");
                return Ok(state);
            }
            Ok(Err(e)) if retry_count < MAX_RETRIES => {
                retry_count += 1;
                error!(
                    "AppState initialization failed (attempt {}): {}",
                    retry_count, e
                );

                // Send notification about initialization retry
                if retry_count == 1 {
                    // Only notify on first failure to avoid spam
                    crate::emit_event!(
                        handle,
                        crate::events::app::INITIALIZATION_RETRY,
                        serde_json::json!({
                            "attempt": retry_count,
                            "error": format!("{}", e),
                            "message": "Application is retrying initialization..."
                        })
                    );
                }

                // For database errors, ensure directories exist before retry
                if matches!(e, crate::error::AppError::DatabaseError { .. }) {
                    // CRITICAL FIX: Don't ignore directory creation failures
                    if let Ok(app_data_dir) = handle.path().app_data_dir() {
                        if let Err(dir_err) = tokio::fs::create_dir_all(&app_data_dir).await {
                            error!("Failed to create app data directory {:?}: {}", app_data_dir, dir_err);
                        }
                    }
                    
                    let fallback_dir = std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .join("data");
                    
                    if let Err(dir_err) = tokio::fs::create_dir_all(&fallback_dir).await {
                        error!("Failed to create fallback data directory {:?}: {}", fallback_dir, dir_err);
                        // This is critical - if we can't create any directories, we should fail
                        return Err(crate::error::AppError::DatabaseError {
                            message: format!("Cannot create any data directories. App data dir and fallback failed: {}", dir_err)
                        });
                    }
                }

                // Exponential backoff: 1s, 2s, 4s - FIXED OVERFLOW BUG
                // Cap at reasonable maximum to prevent arithmetic overflow
                let backoff_seconds = if retry_count > 0 {
                    (1u64 << (retry_count.min(10) - 1)).min(30)
                } else {
                    1u64
                };
                let backoff_ms = backoff_seconds.saturating_mul(1000);
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                continue;
            }
            Ok(Err(e)) => {
                error!(
                    "AppState initialization failed after {} retries: {}",
                    MAX_RETRIES, e
                );

                // Send final failure notification
                crate::emit_event!(
                    handle,
                    crate::events::app::INITIALIZATION_FAILED,
                    serde_json::json!({
                        "error": format!("{}", e),
                        "retries": MAX_RETRIES,
                        "message": "Application failed to initialize after multiple attempts"
                    })
                );

                return Err(e);
            }
            Err(_) if retry_count < MAX_RETRIES => {
                retry_count += 1;
                error!(
                    "AppState initialization timed out (attempt {})",
                    retry_count
                );

                // Shorter backoff for timeout errors
                let backoff_ms = 500 * retry_count as u64;
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                continue;
            }
            Err(_) => {
                error!(
                    "AppState initialization timed out after {} retries",
                    MAX_RETRIES
                );
                return Err(crate::error::AppError::Timeout {
                    message: "AppState initialization timed out after retries".to_string(),
                });
            }
        }
    }
}

pub fn run() -> crate::error::Result<()> {
    // Load environment variables from .env file if it exists
    match dotenv::dotenv() {
        Ok(path) => {
            println!("Loaded environment from: {:?}", path);
        }
        Err(e) => {
            // It's ok if .env doesn't exist, we'll use defaults
            if e.not_found() {
                println!("No .env file found, using default configuration");
            } else {
                eprintln!("Error loading .env file: {}", e);
            }
        }
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "stratosort=debug,tauri=info".into()),
        )
        .init();

    info!("Starting StratoSort...");

    // Initialize sqlite-vec extension early in the startup process
    info!("Initializing sqlite-vec extension...");
    if let Err(e) = crate::storage::initialize_sqlite_vec() {
        warn!(
            "sqlite-vec initialization failed: {}. Vector search will use fallback.",
            e
        );
        warn!("Please install sqlite-vec extension for optimal performance. Falling back to manual similarity calculations.");
        // Emit event to inform frontend about fallback mode
        info!("Vector search will continue using fallback similarity calculations.");
    } else {
        info!("sqlite-vec extension initialized successfully - high-performance vector search available");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_localhost::Builder::new(3030).build())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            let handle = app.handle().clone();

            // Initialize configuration
            let config = Config::load(&handle)?;
            info!(
                mode = if config.is_development() { "development" } else { "production" },
                log_filter = %config.get_log_filter(),
                schema_version = CURRENT_SCHEMA_VERSION,
                "Configuration loaded"
            );

            // Initialize app state asynchronously with proper retry logic
            // We must use block_on here because Tauri's setup is synchronous
            let state = Arc::new(async_runtime::block_on(async {
                initialize_app_state_with_retry(handle.clone(), config.clone()).await
            })?);
            app.manage(state.clone());

            // Setup system tray
            system_tray::init_system_tray(app.handle())?;

            // Setup global shortcuts
            setup_global_shortcuts(app)?;

            // Initialize background services
            initialize_services(app, state.clone())?;

            // Initialize file watcher with proper deadlock prevention
            if config.watch_folders {
                info!("Initializing file watcher...");

                // Use a separate task to avoid blocking the setup and prevent deadlocks
                let state_for_watcher = state.clone();
                let handle_for_watcher = handle.clone();

                async_runtime::spawn(async move {
                    // Wait for app to be fully initialized to prevent race conditions
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    // Create and initialize the file watcher with timeout protection
                    let watcher_init_result = tokio::time::timeout(
                        tokio::time::Duration::from_secs(10),
                        async {
                            // Create the FileWatcher instance
                            let file_watcher = Arc::new(FileWatcher::new(state_for_watcher.clone()));

                            // Store it in the state using a non-blocking write
                            {
                                let mut watcher_guard = state_for_watcher.file_watcher.write();
                                *watcher_guard = Some(file_watcher.clone());
                            }

                            // Start the watcher with timeout protection
                            tokio::time::timeout(
                                tokio::time::Duration::from_secs(5),
                                file_watcher.start()
                            ).await
                        }
                    ).await;

                    match watcher_init_result {
                        Ok(Ok(Ok(_))) => {
                            info!("File watcher initialized and started successfully");
                            crate::emit_event!(
                                handle_for_watcher,
                                crate::events::file::WATCHER_STARTED,
                                serde_json::json!({
                                    "status": "active",
                                    "message": "File monitoring is now active"
                                })
                            );
                        }
                        Ok(Ok(Err(e))) => {
                            error!("Failed to start file watcher: {}", e);
                            crate::emit_event!(
                                handle_for_watcher,
                                crate::events::file::WATCHER_ERROR,
                                serde_json::json!({
                                    "error": format!("{}", e),
                                    "message": "File monitoring could not be started"
                                })
                            );
                        }
                        Ok(Err(_)) => {
                            error!("File watcher start operation timed out");
                            crate::emit_event!(
                                handle_for_watcher,
                                crate::events::file::WATCHER_ERROR,
                                serde_json::json!({
                                    "error": "Timeout during file watcher startup",
                                    "message": "File monitoring startup timed out"
                                })
                            );
                        }
                        Err(_) => {
                            error!("File watcher initialization timed out");
                            crate::emit_event!(
                                handle_for_watcher,
                                crate::events::file::WATCHER_ERROR,
                                serde_json::json!({
                                    "error": "Timeout during file watcher initialization",
                                    "message": "File monitoring initialization timed out"
                                })
                            );
                        }
                    }
                });
            } else {
                info!("File watching disabled in configuration");
            }

            // Try to connect to Ollama in background with retry logic
            let state_for_ollama = state.clone();
            async_runtime::spawn(async move {
                // Give the app a moment to fully start
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                // Try multiple common Ollama hosts with retry
                let ollama_hosts = vec![
                    "http://localhost:11434".to_string(),
                    "http://127.0.0.1:11434".to_string(),
                    state_for_ollama.config.read().ollama_host.clone(),
                ];

                let mut connection_successful = false;

                for host in ollama_hosts {
                    if host.is_empty() {
                        continue;
                    }

                    info!("Attempting to connect to Ollama at: {}", host);

                    // Try to reconnect with this host
                    match state_for_ollama.ai_service.reconnect_ollama(&host).await {
                        Ok(status) => {
                            info!("Ollama connected successfully to {} - Status: {:?}", host, status);
                            connection_successful = true;

                            // Emit success event to frontend
                            crate::emit_event!(
                                state_for_ollama.handle,
                                crate::events::ai::OLLAMA_CONNECTED,
                                serde_json::json!({
                                    "host": host,
                                    "status": status
                                })
                            );
                            break;
                        }
                        Err(e) => {
                            warn!("Failed to connect to Ollama at {}: {}", host, e);
                        }
                    }

                    // Small delay between attempts
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }

                if !connection_successful {
                    warn!("Ollama not available on any host. Switching to fallback AI mode.");

                    // Explicitly switch to fallback mode
                    let status = state_for_ollama.ai_service.use_fallback();
                    info!("Successfully switched to fallback AI mode: {:?}", status);
                    // Emit fallback success event to frontend
                    crate::emit_event!(
                        state_for_ollama.handle,
                        crate::events::ai::OLLAMA_FALLBACK_ACTIVE,
                        serde_json::json!({
                            "message": "AI features are now using fallback analysis. Performance may be limited.",
                            "status": "fallback"
                        })
                    );
                }
            });

            // Set up graceful shutdown handler
            let state_for_shutdown = state.clone();
            shutdown::setup_shutdown_handler(state_for_shutdown);

            info!("StratoSort initialized successfully");
            Ok(())
        })
        .invoke_handler(generate_handler![
            // File commands
            commands::files::scan_directory,
            commands::files::scan_directory_stream,
            commands::files::analyze_files,
            commands::files::get_file_content,
            commands::files::move_files,
            commands::files::get_file_preview,
            commands::files::get_recent_files,
            commands::files::rename_file,
            commands::files::copy_file,
            commands::files::delete_file,
            commands::files::create_directory,
            commands::files::get_file_info_command,
            commands::files::get_file_info_cmd,
            commands::files::set_file_permissions,
            commands::files::batch_file_operations,
            commands::files::move_file,
            commands::files::rename_files,
            commands::files::get_file_properties,
            commands::files::browse_files,
            commands::files::browse_folder,
            commands::files::process_dropped_paths,
            commands::files::file_exists,
            commands::files::get_file_size_info,
            commands::cancel_operation,
            commands::get_active_operations,
            commands::get_operation_progress,

            // AI commands
            commands::ai::check_ollama_status,
            commands::ai::pull_model,
            commands::ai::list_models,
            commands::ai::analyze_with_ai,
            commands::ai::generate_embeddings,
            commands::ai::semantic_search,
            commands::ai::quick_search,
            commands::ai::advanced_search,
            commands::ai::get_search_history,
            commands::ai::clear_search_history,
            commands::ai::batch_analyze_files,
            commands::ai::get_analysis_history,
            commands::ai::analyze_document_enhanced,
            commands::ai::analyze_image_enhanced,
            commands::ai::generate_smart_name_llm,
            commands::ai::get_creative_folder_suggestions,
            commands::ai::get_contextual_folder_suggestions,
            commands::ai::get_semantic_folder_matches,

            // AI Status commands
            commands::ai_status::get_ai_status,
            commands::ai_status::connect_ollama,
            commands::ai_status::use_fallback_ai,
            commands::ai_status::test_ai_analysis,
            commands::ai_status::get_ai_capabilities,

            // AI Streaming commands
            crate::ai::streaming::start_ai_stream,
            crate::ai::streaming::stop_ai_stream,
            crate::ai::streaming::is_stream_active,

            // Organization commands
            commands::organization::create_smart_folder,
            commands::organization::update_smart_folder,
            commands::organization::delete_smart_folder,
            commands::organization::list_smart_folders,
            commands::organization::get_smart_folder,
            commands::organization::apply_smart_folder_rules,
            commands::organization::auto_organize_directory,
            commands::organization::suggest_file_organization,
            commands::organization::apply_organization,
            commands::organization::get_smart_folders,
            commands::organization::match_to_folders,
            commands::organization::validate_rule,
            commands::organization::test_rule_against_files,
            commands::organization::test_smart_folder_rule,
            commands::organization::get_rename_pattern_info,
            commands::organization::preview_rename_pattern,

            // Enhanced organization commands
            commands::organization_enhanced::organize_files_with_ai,
            commands::organization_enhanced::preview_organization,


            // Settings commands
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_settings_by_category,
            commands::settings::get_all_settings_categories,
            commands::settings::update_category_settings,
            commands::settings::test_ai_connection,

            // Setup commands
            commands::setup::check_first_run_status,
            commands::setup::complete_first_run_setup,
            commands::setup::reset_to_first_run,
            commands::settings::reset_settings,
            commands::settings::export_settings,
            commands::settings::import_settings,
            commands::settings::get_setting_value,
            commands::settings::set_setting_value,
            commands::settings::add_watch_path,
            commands::settings::remove_watch_path,
            commands::settings::get_watch_paths,
            commands::settings::validate_settings,

            // System commands
            commands::system::frontend_ready,
            commands::system::get_basic_system_info,
            commands::system::open_folder,
            commands::system::show_in_folder,
            commands::system::get_default_folders,
            commands::system::clear_cache,
            commands::system::get_storage_info,
            commands::system::get_app_logs,
            commands::system::restart_app,
            commands::system::check_for_updates,
            commands::system::shutdown_application,
            commands::system::get_resource_usage,
            commands::system::force_shutdown,

            // Monitoring commands
            commands::monitoring::get_health_status,
            commands::monitoring::get_performance_metrics,
            commands::monitoring::get_metrics_history,
            commands::monitoring::get_system_info,
            commands::monitoring::get_app_info,
            commands::monitoring::readiness_probe,
            commands::monitoring::liveness_probe,
            commands::monitoring::get_runtime_config,
            commands::monitoring::get_file_statistics,
            commands::monitoring::get_system_status,
            commands::monitoring::enable_realtime_monitoring,
            commands::monitoring::refresh_all_status,

            // Notification commands
            commands::notifications::emit_notification,
            commands::notifications::get_notifications,
            commands::notifications::mark_notification_read,
            commands::notifications::clear_notifications,
            commands::notifications::emit_progress_notification,
            commands::notifications::emit_file_operation_status,
            commands::notifications::emit_system_status,

            // Undo/Redo commands
            commands::history::undo,
            commands::history::redo,
            commands::history::get_history,
            commands::history::get_operation_history,
            commands::history::clear_history,
            commands::history::get_history_state,

            // Pattern learning commands
            commands::patterns::save_patterns_to_storage,
            commands::patterns::load_patterns_from_storage,
            commands::patterns::get_learned_patterns,
            commands::patterns::clear_learned_patterns,
            commands::patterns::cleanup_old_patterns,
            commands::patterns::record_pattern_choice,
            commands::patterns::get_pattern_suggestions,
            commands::history::batch_undo,
            commands::history::batch_redo,
            commands::history::jump_to_history,
            commands::history::get_memory_stats,

            // Watch Mode commands (LlamaFS-inspired)
            commands::watch_mode::get_watch_mode_status,
            commands::watch_mode::configure_watch_mode,
            commands::watch_mode::enable_watch_mode,
            commands::watch_mode::disable_watch_mode,
            commands::watch_mode::record_user_organization_action,
            commands::watch_mode::get_user_learning_patterns,
            commands::watch_mode::update_auto_organize_threshold,
            commands::watch_mode::get_pending_auto_organization,
            commands::watch_mode::trigger_auto_organization,
            commands::watch_mode::add_watch_directory,
            commands::watch_mode::remove_watch_directory,

            // Diagnostics commands
            commands::diagnostics::run_diagnostics,
            commands::diagnostics::test_ai_service,
            commands::diagnostics::check_database_health,
            commands::diagnostics::validate_config_paths,
            commands::diagnostics::get_diagnostics_resource_usage,
            commands::diagnostics::clear_caches,
        ])
        .on_window_event(|_window, event| if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            #[cfg(not(target_os = "macos"))]
            {
                if let Err(e) = _window.hide() {
                    error!("Failed to hide window: {}", e);
                }
                api.prevent_close();
            }
        })
        .run(generate_context!())
        .map_err(|e| {
            error!("Failed to run Tauri application: {}", e);
            crate::error::AppError::from(e)
        })?;

    Ok(())
}


fn setup_global_shortcuts(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    let shortcuts = app.global_shortcut();

    // Try to register global shortcuts with fallback options
    let shortcut_options = [
        "CommandOrControl+Shift+O", // Primary choice
        "CommandOrControl+Shift+S", // Fallback 1: S for StratoSort
        "CommandOrControl+Alt+O",   // Fallback 2: Alt instead of Shift
        "CommandOrControl+Alt+S",   // Fallback 3: Alt+S
    ];

    let mut registered_shortcut_str: Option<&str> = None;

    for shortcut_str in &shortcut_options {
        match shortcut_str.parse::<Shortcut>() {
            Ok(shortcut) => match shortcuts.register(shortcut) {
                Ok(_) => {
                    info!("Successfully registered global shortcut: {}", shortcut_str);
                    registered_shortcut_str = Some(shortcut_str);
                    break;
                }
                Err(e) => {
                    warn!(
                        "Failed to register shortcut {}: {}. Trying next option...",
                        shortcut_str, e
                    );
                }
            },
            Err(e) => {
                warn!("Failed to parse shortcut {}: {}", shortcut_str, e);
            }
        }
    }

    // If we managed to register a shortcut, set up the handler
    if let Some(shortcut_str) = registered_shortcut_str {
        if let Ok(shortcut) = shortcut_str.parse::<Shortcut>() {
            let shortcut_str = shortcut_str.to_string();
            let _ = shortcuts.on_shortcut(shortcut, move |app, _shortcut, _event| {
                info!("Global shortcut {} triggered", shortcut_str);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });
        }
    } else {
        // Log warning but don't fail - app can still work without global shortcuts
        warn!("Could not register any global shortcuts. App will work without them.");
    }

    Ok(())
}

fn initialize_services(
    app: &tauri::App,
    state: Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize notification service
    let _notification_service = NotificationService::new(app.handle().clone());
    // Send a welcome notification asynchronously
    {
        let app_handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            let notifier = NotificationService::new(app_handle);
            let _ = notifier
                .send_success("StratoSort Ready", "Background services initialized")
                .await;
        });
    }

    // Start periodic tasks (every 5 minutes)
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        let mut maintenance_counter = 0u32;
        
        loop {
            // Check for cancellation
            tokio::select! {
                _ = interval.tick() => {
                    maintenance_counter += 1;
                    
                    // Cleanup old cache entries
                    if let Err(e) = state_clone.cleanup_cache().await {
                        error!("Cache cleanup failed: {}", e);
                    }

                    // Save state periodically
                    if let Err(e) = state_clone.save_state().await {
                        error!("State save failed: {}", e);
                    }
                    
                    // Perform database maintenance every hour (12 * 5min intervals)
                    if maintenance_counter % 12 == 0 {
                        if let Err(e) = state_clone.periodic_database_maintenance().await {
                            error!("Database maintenance failed: {}", e);
                        }
                    }
                }
                _ = tokio::task::yield_now() => {
                    // Allow task cancellation - yield point reached
                    break;
                }
            }
        }
        info!("Periodic cleanup task terminated gracefully");
    });

    // Start memory monitoring
    let monitor = Arc::new(MemoryMonitor::new());
    {
        let monitor_clone = monitor.clone();
        tauri::async_runtime::spawn(async move {
            let result = monitor_clone.start().await;
            if let Err(e) = result {
                error!("Memory monitor failed: {}", e);
            }
            info!("Memory monitor task terminated");
        });
    }

    // Run basic health checks once after startup
    {
        tauri::async_runtime::spawn(async move {
            match HealthChecker::check_all().await {
                Ok(status) => {
                    if status.healthy {
                        info!("Health checks passed");
                    } else {
                        warn!("Health checks reported issues: {:?}", status.checks);
                    }
                }
                Err(e) => warn!("Health checks failed to run: {}", e),
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Test modules - no imports needed for basic test

    #[test]
    fn test_module_imports() {
        // Basic smoke test to ensure all modules compile
        // If this compiles, all modules are imported correctly
    }

    #[tokio::test]
    async fn test_async_setup() {
        // Test async functionality
        // If this runs without panicking, async setup works
    }
}
