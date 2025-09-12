use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Clean up lock files that might cause issues on Windows
    let lock_paths = vec!["target/debug/.cargo-lock", "target/release/.cargo-lock"];

    for lock_path in lock_paths {
        if Path::new(lock_path).exists() {
            let _ = fs::remove_file(lock_path);
        }
    }
    // Set build date
    let output = Command::new("powershell")
        .args(["-Command", "Get-Date -Format 'yyyy-MM-dd HH:mm:ss'"])
        .output()
        .unwrap_or_else(|_| {
            // Fallback for non-Windows systems
            Command::new("date")
                .args(["+%Y-%m-%d %H:%M:%S"])
                .output()
                .unwrap_or_else(|_| std::process::Output {
                    status: std::process::ExitStatus::default(),
                    stdout: "Unknown".as_bytes().to_vec(),
                    stderr: vec![],
                })
        });

    let build_date = String::from_utf8_lossy(&output.stdout).trim().to_string();
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);

    // Set Rust version
    let rust_version = env!("CARGO_PKG_VERSION");
    println!("cargo:rustc-env=RUST_VERSION={}", rust_version);

    // Set target triple
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=TARGET_TRIPLE={}", target);

    // Tell Cargo to rerun this script if anything changes
    println!("cargo:rerun-if-changed=build.rs");

    // Call Tauri's build process
    tauri_build::build()
}
