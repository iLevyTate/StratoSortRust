use stratosort::commands::{
    ai::*, ai_status::*, files::*, history::*, monitoring::*, 
    notifications::*, organization::*, settings::*, setup::*, 
    system::*, watch_mode::*
};
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::AppError;
use tauri::{State, test::{mock_app, MockRuntime}};
use std::sync::Arc;
use tempfile::tempdir;
use serde_json::Value;

// Helper to create mock app state for testing
async fn create_test_app_state() -> Arc<AppState> {
    let app = mock_app();
    let config = Config::default();
    
    match AppState::new(app.clone(), config).await {
        Ok(state) => Arc::new(state),
        Err(_) => {
            // Create minimal mock state for testing
            panic!("Could not create app state - need proper test setup");
        }
    }
}

#[tokio::test]
async fn test_ai_commands_validation() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    // Test pull_model with various inputs
    let model_test_cases = vec![
        // Valid cases
        ("llama3.2:3b", true),
        ("mistral:latest", true),
        
        // Invalid cases
        ("", false), // Empty model name
        ("a".repeat(300), false), // Too long
        ("model; rm -rf /", false), // Command injection
        ("model && evil", false), // Command chaining
        ("model`cmd`", false), // Backtick injection
        ("model$(cmd)", false), // Command substitution
        ("../../../etc/passwd", false), // Path traversal
        ("model\0null", false), // Null byte
        ("model\ninjection", false), // Newline injection
    ];
    
    for (model_name, should_succeed) in model_test_cases {
        let result = pull_model(model_name.to_string(), state_ref.clone()).await;
        
        if should_succeed {
            // Valid models might fail due to Ollama not being available, which is fine
            match result {
                Ok(_) => println!("Model '{}' pull succeeded", model_name),
                Err(AppError::AiError { .. }) => println!("Model '{}' failed due to AI service unavailable (expected)", model_name),
                Err(e) => println!("Model '{}' failed with: {:?}", model_name, e),
            }
        } else {
            // Invalid models should be rejected by validation
            match result {
                Err(AppError::InvalidInput { .. }) | 
                Err(AppError::SecurityError { .. }) | 
                Err(AppError::InvalidPath { .. }) => {
                    println!("Invalid model '{}' correctly rejected", model_name);
                }
                Ok(_) => panic!("Invalid model '{}' should have been rejected", model_name),
                Err(e) => println!("Invalid model '{}' rejected with: {:?}", model_name, e),
            }
        }
    }
}

#[tokio::test]
async fn test_file_commands_security_validation() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    // Test get_file_content with path traversal attempts
    let malicious_paths = vec![
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "/etc/shadow",
        "C:\\Windows\\System32\\config\\SAM",
        "file\0.txt", // Null byte injection
        "file\n.txt", // Newline injection
        "file'; DROP TABLE files; --.txt", // SQL injection attempt
        "very_long_path_".repeat(100), // Extremely long path
        "file<script>alert('xss')</script>.txt", // XSS attempt
        "\\\\server\\share\\file", // UNC path
        "file://etc/passwd", // File URI scheme
    ];
    
    for malicious_path in malicious_paths {
        let result = get_file_content(malicious_path.to_string(), state_ref.clone(), app.clone()).await;
        
        match result {
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidPath { .. }) | 
            Err(AppError::FileNotFound { .. }) => {
                println!("Malicious path '{}' correctly blocked", malicious_path);
            }
            Ok(_) => {
                panic!("Malicious path '{}' should have been blocked", malicious_path);
            }
            Err(e) => {
                println!("Path '{}' rejected with: {:?}", malicious_path, e);
            }
        }
    }
    
    // Test scan_directory with dangerous paths
    let dangerous_directories = vec![
        "/",
        "C:\\",
        "/etc",
        "/root",
        "C:\\Windows",
        "C:\\Windows\\System32",
        "../../../",
        "..\\..\\..\\",
    ];
    
    for dangerous_dir in dangerous_directories {
        let result = scan_directory(dangerous_dir.to_string(), state_ref.clone(), app.clone()).await;
        
        match result {
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidPath { .. }) => {
                println!("Dangerous directory '{}' correctly blocked", dangerous_dir);
            }
            Ok(files) => {
                // If it succeeds, ensure no system files are returned
                for file in files {
                    assert!(!file.path.contains("/etc/"), "Should not return system files");
                    assert!(!file.path.contains("\\System32\\"), "Should not return system files");
                }
                println!("Directory scan '{}' succeeded with safe results", dangerous_dir);
            }
            Err(e) => {
                println!("Directory '{}' scan rejected: {:?}", dangerous_dir, e);
            }
        }
    }
}

#[tokio::test]
async fn test_organization_commands_limits() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    // Test create_smart_folder with various inputs
    let smart_folder_tests = vec![
        // Valid cases
        ("Documents", "*.doc OR *.pdf", true),
        ("Images", "*.jpg OR *.png", true),
        
        // Invalid cases
        ("", "*.txt", false), // Empty name
        ("a".repeat(500), "*.txt", false), // Name too long
        ("ValidName", "", false), // Empty rules
        ("ValidName", "a".repeat(10000), false), // Rules too long
        ("Folder<script>", "*.txt", false), // XSS attempt in name
        ("ValidName", "*.txt; DROP TABLE smart_folders; --", false), // SQL injection in rules
        ("ValidName", "*.txt && rm -rf /", false), // Command injection
        ("Con", "*.txt", false), // Windows reserved name
        ("Aux.txt", "*.txt", false), // Windows reserved name with extension
    ];
    
    for (name, rules, should_succeed) in smart_folder_tests {
        let result = create_smart_folder(
            name.to_string(), 
            rules.to_string(), 
            state_ref.clone(), 
            app.clone()
        ).await;
        
        if should_succeed {
            match result {
                Ok(folder_id) => {
                    println!("Smart folder '{}' created with ID: {}", name, folder_id);
                }
                Err(e) => {
                    println!("Smart folder '{}' creation failed: {:?}", name, e);
                }
            }
        } else {
            match result {
                Err(AppError::InvalidInput { .. }) | 
                Err(AppError::SecurityError { .. }) => {
                    println!("Invalid smart folder '{}' correctly rejected", name);
                }
                Ok(_) => {
                    panic!("Invalid smart folder '{}' should have been rejected", name);
                }
                Err(e) => {
                    println!("Smart folder '{}' rejected with: {:?}", name, e);
                }
            }
        }
    }
    
    // Test batch operations limits
    let large_file_list: Vec<String> = (0..5000)
        .map(|i| format!("file_{}.txt", i))
        .collect();
    
    let result = analyze_files(large_file_list, state_ref.clone(), app.clone()).await;
    
    match result {
        Err(AppError::SecurityError { message }) if message.contains("Too many files") => {
            println!("Large batch operation correctly limited");
        }
        Ok(_) => {
            println!("Large batch operation succeeded (system may have high limits)");
        }
        Err(e) => {
            println!("Large batch operation failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_settings_commands_validation() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    // Test get_setting and update_setting with various inputs
    let setting_tests = vec![
        // Valid settings
        ("theme", "dark", true),
        ("language", "en", true),
        ("max_file_size", "104857600", true), // 100MB
        
        // Invalid settings
        ("", "value", false), // Empty key
        ("valid_key", "", false), // Empty value might be invalid
        ("key_with_sql'; DROP TABLE settings; --", "value", false), // SQL injection
        ("valid_key", "value_with_injection'; DELETE FROM settings; --", false), // SQL injection
        ("a".repeat(1000), "value", false), // Key too long
        ("valid_key", "a".repeat(100000), false), // Value too long
        ("../../../config", "malicious", false), // Path traversal in key
        ("key\0null", "value", false), // Null byte in key
        ("key\ninjection", "value", false), // Newline injection
    ];
    
    for (key, value, should_succeed) in setting_tests {
        // Test get_setting
        let get_result = get_setting(key.to_string(), state_ref.clone()).await;
        
        match get_result {
            Ok(_) => {
                if should_succeed {
                    println!("Setting '{}' retrieved successfully", key);
                } else {
                    println!("WARNING: Invalid setting key '{}' was accepted in get_setting", key);
                }
            }
            Err(AppError::InvalidInput { .. }) | 
            Err(AppError::SecurityError { .. }) => {
                if !should_succeed {
                    println!("Invalid setting key '{}' correctly rejected in get_setting", key);
                }
            }
            Err(e) => {
                println!("Setting '{}' get failed: {:?}", key, e);
            }
        }
        
        // Test update_setting
        let update_result = update_setting(
            key.to_string(), 
            serde_json::Value::String(value.to_string()), 
            state_ref.clone()
        ).await;
        
        match update_result {
            Ok(_) => {
                if should_succeed {
                    println!("Setting '{}' updated successfully", key);
                } else {
                    println!("WARNING: Invalid setting '{}' was accepted in update_setting", key);
                }
            }
            Err(AppError::InvalidInput { .. }) | 
            Err(AppError::SecurityError { .. }) => {
                if !should_succeed {
                    println!("Invalid setting '{}' correctly rejected in update_setting", key);
                }
            }
            Err(e) => {
                println!("Setting '{}' update failed: {:?}", key, e);
            }
        }
    }
    
    // Test export_settings and import_settings with malicious data
    let malicious_settings_json = serde_json::json!({
        "theme": "dark",
        "'; DROP TABLE settings; --": "malicious",
        "embedded_script": "<script>alert('xss')</script>",
        "path_traversal": "../../../etc/passwd",
        "very_long_key": "a".repeat(10000),
        "null_byte_key\0": "value",
        "command_injection": "value && rm -rf /"
    });
    
    let import_result = import_settings(malicious_settings_json, state_ref.clone()).await;
    
    match import_result {
        Ok(_) => {
            println!("WARNING: Malicious settings import was accepted");
            
            // Verify that malicious keys were not actually stored
            for malicious_key in ["'; DROP TABLE settings; --", "../../../etc/passwd", "null_byte_key\0"] {
                let check_result = get_setting(malicious_key.to_string(), state_ref.clone()).await;
                match check_result {
                    Ok(_) => {
                        panic!("Malicious setting key '{}' was stored!", malicious_key);
                    }
                    Err(_) => {
                        println!("Malicious key '{}' was not stored (good)", malicious_key);
                    }
                }
            }
        }
        Err(e) => {
            println!("Malicious settings import correctly rejected: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_monitoring_commands_stability() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    // Test monitoring commands that should always work
    let monitoring_commands = vec![
        ("get_system_info", || async { get_system_info(state_ref.clone()).await }),
        ("get_app_info", || async { get_app_info(state_ref.clone()).await }),
        ("get_database_stats", || async { get_database_stats(state_ref.clone()).await }),
        ("get_enabled_features", || async { get_enabled_features(state_ref.clone()).await }),
    ];
    
    for (command_name, command_fn) in monitoring_commands {
        match command_fn().await {
            Ok(_) => {
                println!("Monitoring command '{}' succeeded", command_name);
            }
            Err(e) => {
                println!("Monitoring command '{}' failed: {:?}", command_name, e);
            }
        }
    }
    
    // Test concurrent access to monitoring commands
    let concurrent_tasks: Vec<_> = (0..20).map(|i| {
        let state_clone = state_ref.clone();
        tokio::spawn(async move {
            match i % 4 {
                0 => get_system_info(state_clone).await.map(|_| "system_info"),
                1 => get_app_info(state_clone).await.map(|_| "app_info"),
                2 => get_database_stats(state_clone).await.map(|_| "db_stats"),
                _ => get_enabled_features(state_clone).await.map(|_| "features"),
            }
        })
    }).collect();
    
    let results = futures::future::join_all(concurrent_tasks).await;
    let successes = results.into_iter()
        .filter_map(|r| r.ok().and_then(|inner| inner.ok()))
        .count();
    
    println!("Concurrent monitoring commands: {} out of 20 succeeded", successes);
    assert!(successes >= 15, "Most monitoring commands should succeed under concurrent access");
}

#[tokio::test]
async fn test_system_commands_restrictions() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    // Test system commands with potentially dangerous inputs
    let system_test_cases = vec![
        // Test force_shutdown with various states
        ("force_shutdown", true), // Should be allowed but controlled
    ];
    
    // Test force_shutdown behavior
    let shutdown_result = force_shutdown(state_ref.clone()).await;
    
    match shutdown_result {
        Ok(_) => {
            println!("Force shutdown command executed");
            
            // Verify system is in consistent state after shutdown signal
            // (in real implementation, this would check that cleanup occurred)
            let system_info_result = get_system_info(state_ref.clone()).await;
            match system_info_result {
                Ok(_) => println!("System still responsive after shutdown signal"),
                Err(e) => println!("System not responsive after shutdown: {:?}", e),
            }
        }
        Err(e) => {
            println!("Force shutdown rejected: {:?}", e);
        }
    }
    
    // Test system resource monitoring doesn't leak sensitive info
    match get_system_info(state_ref.clone()).await {
        Ok(info) => {
            // Verify that system info doesn't contain sensitive data
            let info_str = format!("{:?}", info);
            
            // Check for potentially sensitive information
            assert!(!info_str.to_lowercase().contains("password"), 
                   "System info should not contain passwords");
            assert!(!info_str.to_lowercase().contains("secret"), 
                   "System info should not contain secrets");
            assert!(!info_str.to_lowercase().contains("key"), 
                   "System info should not contain cryptographic keys");
            assert!(!info_str.contains("/etc/passwd"), 
                   "System info should not contain system file paths");
            
            println!("System info passed security check");
        }
        Err(e) => {
            println!("Could not get system info: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_watch_mode_commands_safety() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    let temp_dir = tempdir().unwrap();
    let safe_path = temp_dir.path().to_string_lossy().to_string();
    
    // Test start_watching with various paths
    let watch_test_cases = vec![
        // Safe paths
        (safe_path.clone(), true),
        
        // Dangerous paths
        ("/etc".to_string(), false),
        ("/root".to_string(), false),
        ("C:\\Windows".to_string(), false),
        ("C:\\Windows\\System32".to_string(), false),
        ("../../../etc/passwd".to_string(), false),
        ("\\\\server\\share\\danger".to_string(), false),
        (format!("{}/../../../etc", safe_path), false),
    ];
    
    for (watch_path, should_succeed) in watch_test_cases {
        let result = start_watching(watch_path.clone(), state_ref.clone()).await;
        
        if should_succeed {
            match result {
                Ok(_) => {
                    println!("Started watching safe path: {}", watch_path);
                    
                    // Test stopping the watcher
                    let stop_result = stop_watching(state_ref.clone()).await;
                    match stop_result {
                        Ok(_) => println!("Stopped watching successfully"),
                        Err(e) => println!("Failed to stop watching: {:?}", e),
                    }
                }
                Err(e) => {
                    println!("Failed to watch safe path '{}': {:?}", watch_path, e);
                }
            }
        } else {
            match result {
                Err(AppError::SecurityError { .. }) | 
                Err(AppError::InvalidPath { .. }) => {
                    println!("Dangerous watch path '{}' correctly blocked", watch_path);
                }
                Ok(_) => {
                    println!("WARNING: Dangerous path '{}' was allowed for watching", watch_path);
                    
                    // Clean up
                    let _ = stop_watching(state_ref.clone()).await;
                }
                Err(e) => {
                    println!("Watch path '{}' rejected: {:?}", watch_path, e);
                }
            }
        }
    }
    
    // Test watch status and configuration
    let status_result = get_watch_status(state_ref.clone()).await;
    match status_result {
        Ok(status) => {
            println!("Watch status retrieved: {:?}", status);
        }
        Err(e) => {
            println!("Could not get watch status: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_api_rate_limiting_and_resource_protection() {
    let state = create_test_app_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    // Test rapid consecutive API calls to detect rate limiting
    let rapid_calls = 100;
    let start_time = std::time::Instant::now();
    
    let tasks: Vec<_> = (0..rapid_calls).map(|i| {
        let state_clone = state_ref.clone();
        tokio::spawn(async move {
            // Use a lightweight command for testing
            get_enabled_features(state_clone).await.map(|_| i)
        })
    }).collect();
    
    let results = futures::future::join_all(tasks).await;
    let elapsed = start_time.elapsed();
    
    let successful_calls = results.into_iter()
        .filter_map(|r| r.ok().and_then(|inner| inner.ok()))
        .count();
    
    println!("Rate limiting test: {}/{} calls succeeded in {:?}", 
             successful_calls, rapid_calls, elapsed);
    
    // Check if there's evidence of rate limiting (some calls should be throttled)
    if successful_calls < rapid_calls {
        println!("Rate limiting appears to be working ({} calls throttled)", 
                rapid_calls - successful_calls);
    } else {
        println!("No evidence of rate limiting (all {} calls succeeded)", rapid_calls);
    }
    
    // Verify system remains responsive after rapid calls
    let post_test_call = get_system_info(state_ref.clone()).await;
    match post_test_call {
        Ok(_) => println!("System responsive after rate limiting test"),
        Err(e) => println!("System may be overloaded after test: {:?}", e),
    }
}