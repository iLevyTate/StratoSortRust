use crate::{
    error::Result,
    utils::security::{is_path_allowed, validate_path},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use sysinfo::{Disks, System};
use tauri::{AppHandle, Emitter, Manager};
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfo {
    pub platform: String,
    pub arch: String,
    pub version: String,
    pub total_memory: u64,
    pub free_memory: u64,
    pub cpu_count: usize,
    pub gpu_available: bool,
    pub home_dir: String,
    pub temp_dir: String,
    pub app_version: String,
    pub rust_version: String,
    pub node_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageInfo {
    pub total_space: u64,
    pub free_space: u64,
    pub used_space: u64,
    pub app_data_size: u64,
    pub cache_size: u64,
    pub database_size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DefaultFolders {
    pub home: String,
    pub documents: String,
    pub downloads: String,
    pub pictures: String,
    pub videos: String,
    pub music: String,
    pub desktop: String,
}

#[tauri::command]
pub async fn frontend_ready(app: AppHandle) -> Result<()> {
    info!("Frontend reported ready - showing main window");

    if let Some(window) = app.get_webview_window("main") {
        // Show the window now that frontend is ready
        if let Err(e) = window.show() {
            tracing::error!("Failed to show window: {}", e);
        }

        // Focus the window
        if let Err(e) = window.set_focus() {
            tracing::error!("Failed to focus window: {}", e);
        }

        info!("Main window shown and focused");
    } else {
        tracing::error!("Main window not found");
    }

    Ok(())
}

#[tauri::command]
pub async fn get_basic_system_info(app: AppHandle) -> Result<SystemInfo> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let platform = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let version = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());

    let total_memory = sys.total_memory();
    let free_memory = sys.available_memory();
    let cpu_count = sys.cpus().len();

    // Simple GPU detection (would need more sophisticated detection in production)
    let gpu_available = check_gpu_availability();

    let home_dir = dirs::home_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let temp_dir = std::env::temp_dir().display().to_string();

    let app_version = app.package_info().version.to_string();
    let rust_version = rustc_version();
    let node_version = node_version().await;

    Ok(SystemInfo {
        platform,
        arch,
        version,
        total_memory,
        free_memory,
        cpu_count,
        gpu_available,
        home_dir,
        temp_dir,
        app_version,
        rust_version,
        node_version,
    })
}

fn check_gpu_availability() -> bool {
    // Simple check - would need more sophisticated detection in production
    #[cfg(windows)]
    {
        // Check for DirectX or common GPU environment variables
        std::env::var("CUDA_PATH").is_ok() || std::env::var("VULKAN_SDK").is_ok()
    }

    #[cfg(not(windows))]
    {
        // Check for common GPU indicators on Unix-like systems
        std::path::Path::new("/dev/dri").exists() || std::env::var("CUDA_PATH").is_ok()
    }
}

fn rustc_version() -> String {
    std::env::var("CARGO_PKG_RUST_VERSION")
        .unwrap_or_else(|_| env!("CARGO_PKG_RUST_VERSION").to_string())
}

async fn node_version() -> String {
    match tokio::process::Command::new("node")
        .arg("--version")
        .output()
        .await
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => "Unknown".to_string(),
    }
}

#[tauri::command]
pub async fn get_storage_info(app: AppHandle) -> Result<StorageInfo> {
    let disks = Disks::new_with_refreshed_list();

    let mut total_space = 0u64;
    let mut free_space = 0u64;

    for disk in disks.list() {
        total_space += disk.total_space();
        free_space += disk.available_space();
    }

    let used_space = total_space.saturating_sub(free_space);

    // Get app data directory size
    let app_data_size = if let Ok(app_data_dir) = app.path().app_data_dir() {
        calculate_directory_size(&app_data_dir).await
    } else {
        0
    };

    let cache_size = if let Ok(cache_dir) = app.path().app_cache_dir() {
        calculate_directory_size(&cache_dir).await
    } else {
        0
    };

    // Estimate database size (simplified)
    let database_size = app_data_size / 10; // Rough estimate

    Ok(StorageInfo {
        total_space,
        free_space,
        used_space,
        app_data_size,
        cache_size,
        database_size,
    })
}

async fn calculate_directory_size(path: &Path) -> u64 {
    let mut total_size = 0u64;

    if let Ok(mut entries) = tokio::fs::read_dir(path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(metadata) = entry.metadata().await {
                if metadata.is_file() {
                    total_size += metadata.len();
                } else if metadata.is_dir() {
                    total_size += Box::pin(calculate_directory_size(&entry.path())).await;
                }
            }
        }
    }

    total_size
}

#[tauri::command]
pub async fn get_default_folders() -> Result<DefaultFolders> {
    Ok(DefaultFolders {
        home: dirs::home_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        documents: dirs::document_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        downloads: dirs::download_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        pictures: dirs::picture_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        videos: dirs::video_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        music: dirs::audio_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        desktop: dirs::desktop_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn open_folder(path: String, app: AppHandle) -> Result<()> {
    // Validate and sanitize path to prevent command injection
    let validated_path = validate_path(&path, &app)?;
    let sanitized_path = validated_path.canonical();

    // Ensure path exists and is a directory
    if !sanitized_path.exists() || !sanitized_path.is_dir() {
        return Err(crate::error::AppError::FileNotFound {
            path: sanitized_path.display().to_string(),
        });
    }

    // Ensure path is within allowed directories
    if !is_path_allowed(sanitized_path, &app)? {
        return Err(crate::error::AppError::SecurityError {
            message: "Access to this directory is not allowed".to_string(),
        });
    }

    #[cfg(target_os = "windows")]
    {
        // Use explorer with /select to be more specific
        // Pass path as separate argument to prevent injection
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg("/select,").arg(sanitized_path);

        // Set environment variables to prevent DLL injection
        cmd.env_clear();
        cmd.env(
            "SYSTEMROOT",
            std::env::var("SYSTEMROOT").unwrap_or_else(|_| "C:\\Windows".to_string()),
        );

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to open folder: {}", e),
            })?;
    }

    #[cfg(target_os = "macos")]
    {
        // Use open with -R for reveal in Finder
        // Pass path as separate argument to prevent injection
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-R").arg(sanitized_path);

        // Clear environment for security
        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin");

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to open folder: {}", e),
            })?;
    }

    #[cfg(target_os = "linux")]
    {
        // Use xdg-open with the validated path
        // Pass path as separate argument to prevent injection
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(sanitized_path);

        // Clear environment for security
        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin");

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to open folder: {}", e),
            })?;
    }

    Ok(())
}

#[tauri::command]
pub async fn open_with_default(path: String, app: AppHandle) -> Result<()> {
    // Validate and sanitize path to prevent command injection
    let validated_path = validate_path(&path, &app)?;
    let sanitized_path = validated_path.canonical();

    // Ensure path exists
    if !sanitized_path.exists() {
        return Err(crate::error::AppError::FileNotFound {
            path: sanitized_path.display().to_string(),
        });
    }

    // Ensure path is within allowed directories
    if !is_path_allowed(sanitized_path, &app)? {
        return Err(crate::error::AppError::SecurityError {
            message: "Access to this file is not allowed".to_string(),
        });
    }

    // Additional security check - prevent opening of potentially dangerous files
    let path_str = sanitized_path.to_string_lossy().to_lowercase();
    let dangerous_extensions = [
        ".exe", ".bat", ".cmd", ".com", ".scr", ".pif", ".vbs", ".js", ".jar",
    ];

    if dangerous_extensions
        .iter()
        .any(|ext| path_str.ends_with(ext))
    {
        return Err(crate::error::AppError::SecurityError {
            message: "Cannot open potentially dangerous file types".to_string(),
        });
    }

    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/c", "start", ""]).arg(sanitized_path);
        cmd.env_clear();
        cmd.env(
            "SYSTEMROOT",
            std::env::var("SYSTEMROOT").unwrap_or_else(|_| "C:\\Windows".to_string()),
        );

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to open file: {}", e),
            })?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        cmd.arg(sanitized_path);
        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin");

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to open file: {}", e),
            })?;
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg("--")
            .arg(sanitized_path);
        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin");

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to open file: {}", e),
            })?;
    }

    Ok(())
}

#[tauri::command]
pub async fn show_in_folder(path: String, app: AppHandle) -> Result<()> {
    // Validate and sanitize path to prevent command injection
    let validated_path = validate_path(&path, &app)?;
    let sanitized_path = validated_path.canonical();

    // Ensure path exists
    if !sanitized_path.exists() {
        return Err(crate::error::AppError::FileNotFound {
            path: sanitized_path.display().to_string(),
        });
    }

    // Ensure path is within allowed directories
    if !is_path_allowed(sanitized_path, &app)? {
        return Err(crate::error::AppError::SecurityError {
            message: "Access to this path is not allowed".to_string(),
        });
    }

    // Get parent directory for showing in folder
    let _parent = sanitized_path
        .parent()
        .ok_or_else(|| crate::error::AppError::InvalidInput {
            message: "Cannot show root directory".to_string(),
        })?;

    let _path_str = sanitized_path.display().to_string();

    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("explorer");
        cmd.arg("/select,").arg(sanitized_path);
        cmd.env_clear();
        cmd.env(
            "SYSTEMROOT",
            std::env::var("SYSTEMROOT").unwrap_or_else(|_| "C:\\Windows".to_string()),
        );

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to show in folder: {}", e),
            })?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-R").arg(sanitized_path);
        cmd.env_clear();
        cmd.env("PATH", "/usr/bin:/bin");

        cmd.spawn()
            .map_err(|e| crate::error::AppError::SystemError {
                message: format!("Failed to show in folder: {}", e),
            })?;
    }

    #[cfg(target_os = "linux")]
    {
        // Different file managers have different commands
        // Try nautilus first, then fallback to xdg-open on parent
        let file_managers = vec![
            ("nautilus", vec!["--select"]),
            ("dolphin", vec!["--select"]),
            ("nemo", vec![]),
            ("thunar", vec![]),
        ];

        let mut success = false;
        for (manager, args) in file_managers {
            let mut cmd = std::process::Command::new(manager);
            for arg in args {
                cmd.arg(arg);
            }
            cmd.arg(sanitized_path);
            cmd.env_clear();
            cmd.env("PATH", "/usr/bin:/bin");

            if cmd.spawn().is_ok() {
                success = true;
                break;
            }
        }

        if !success {
            // Fallback to opening parent directory
            if let Some(parent) = sanitized_path.parent() {
                let mut cmd = std::process::Command::new("xdg-open");
                cmd.arg(parent);
                cmd.env_clear();
                cmd.env("PATH", "/usr/bin:/bin");

                cmd.spawn()
                    .map_err(|e| crate::error::AppError::SystemError {
                        message: format!("Failed to show in folder: {}", e),
                    })?;
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn clear_cache(app: AppHandle) -> Result<()> {
    // Clear application cache directory
    if let Ok(cache_dir) = app.path().app_cache_dir() {
        if cache_dir.exists() {
            tokio::fs::remove_dir_all(&cache_dir).await?;
            tokio::fs::create_dir_all(&cache_dir).await?;
            info!("Application cache cleared successfully");
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn get_app_logs(app: AppHandle) -> Result<String> {
    // Get application log file or recent logs
    if let Ok(log_dir) = app.path().app_log_dir() {
        let log_file = log_dir.join("stratosort.log");
        if log_file.exists() {
            let content = tokio::fs::read_to_string(&log_file).await?;
            // Return last 10000 characters to avoid huge responses
            let truncated = if content.len() > 10000 {
                let start = content.len() - 10000;
                format!("...(truncated)...\n{}", &content[start..])
            } else {
                content
            };
            return Ok(truncated);
        }
    }
    
    Ok("No log file found".to_string())
}

#[tauri::command]
pub async fn restart_app(app: AppHandle) -> Result<()> {
    info!("Application restart requested");
    
    // Emit restart event to frontend
    let _ = app.emit(
        "app-restart-requested",
        serde_json::json!({
            "message": "Application will restart shortly"
        }),
    );
    
    // Give a moment for the event to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Restart the application
    app.restart()
}

#[tauri::command]
pub async fn check_for_updates() -> Result<serde_json::Value> {
    // For now, just return current version info
    // In a full implementation, this would check against a remote server
    Ok(serde_json::json!({
        "current_version": env!("CARGO_PKG_VERSION"),
        "update_available": false,
        "latest_version": env!("CARGO_PKG_VERSION"),
        "message": "Update checking not implemented yet"
    }))
}

#[tauri::command]
pub async fn shutdown_application(app: AppHandle) -> Result<()> {
    info!("Application shutdown requested");
    
    // Emit shutdown event to frontend
    let _ = app.emit(
        "app-shutdown-requested",
        serde_json::json!({
            "message": "Application will shutdown shortly"
        }),
    );
    
    // Give a moment for cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Exit the application
    app.exit(0);
    
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_usage: u64,
    pub cpu_usage: f32,
    pub disk_usage: u64,
    pub network_usage: u64,
}

#[tauri::command]
pub async fn get_resource_usage() -> Result<ResourceUsage> {
    let mut sys = System::new_all();
    sys.refresh_all();
    
    // Get current process info
    let pid = sysinfo::get_current_pid().map_err(|e| crate::error::AppError::SystemError {
        message: format!("Failed to get current PID: {}", e),
    })?;
    
    if let Some(process) = sys.process(pid) {
        Ok(ResourceUsage {
            memory_usage: process.memory(),
            cpu_usage: process.cpu_usage(),
            disk_usage: 0, // Would need more complex calculation
            network_usage: 0, // Would need network monitoring
        })
    } else {
        Ok(ResourceUsage {
            memory_usage: 0,
            cpu_usage: 0.0,
            disk_usage: 0,
            network_usage: 0,
        })
    }
}

#[tauri::command]
pub async fn force_shutdown(app: AppHandle) -> Result<()> {
    info!("Forced application shutdown requested");
    
    // Immediate shutdown without waiting for cleanup
    app.exit(0);
    
    Ok(())
}
