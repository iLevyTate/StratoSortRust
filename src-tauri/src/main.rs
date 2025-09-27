#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use stratosort::run;

fn setup_panic_handler() {
    use std::panic;
    use std::fs::{self, File};
    use std::io::Write;

    panic::set_hook(Box::new(|panic_info| {
        let message = format!(
            "PANIC: {}\nLocation: {:?}\n",
            panic_info,
            panic_info.location()
        );

        eprintln!("{}", message);

        // CRITICAL FIX: Use proper app data directory for crash logs
        let crash_log_path = get_crash_log_path();

        // Ensure the parent directory exists
        if let Some(parent) = crash_log_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Write crash dump with proper error handling
        match File::create(&crash_log_path) {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", message);
                let _ = writeln!(file, "Timestamp: {}", chrono::Utc::now());
                let _ = writeln!(file, "Platform: {}", std::env::consts::OS);
                let _ = writeln!(file, "Arch: {}", std::env::consts::ARCH);
                eprintln!("Crash log written to: {:?}", crash_log_path);
            }
            Err(e) => {
                eprintln!("Failed to write crash log to {:?}: {}", crash_log_path, e);

                // Fallback: try to write to current directory as last resort
                if let Ok(mut fallback) = File::create("crash_emergency.log") {
                    let _ = writeln!(fallback, "{}", message);
                    let _ = writeln!(fallback, "Timestamp: {}", chrono::Utc::now());
                    eprintln!("Emergency crash log written to current directory");
                }
            }
        }
    }));
}

fn get_crash_log_path() -> PathBuf {
    use chrono::Utc;

    // Try to get standard app data directories
    #[cfg(target_os = "windows")]
    let base_dir = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
        });

    #[cfg(target_os = "macos")]
    let base_dir = std::env::var("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"))
        .unwrap_or_else(|_| PathBuf::from("."));

    #[cfg(target_os = "linux")]
    let base_dir = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|_| PathBuf::from("."));

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let base_dir = PathBuf::from(".");

    // Create timestamped crash log filename
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("stratosort_crash_{}.log", timestamp);

    base_dir
        .join("stratosort")
        .join("logs")
        .join(filename)
}

fn main() {
    // Set up panic handler before anything else
    setup_panic_handler();

    if let Err(e) = run() {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }
}
