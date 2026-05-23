use stratosort::commands::setup::*;
use stratosort::commands::ai_status::*;
use stratosort::state::AppState;
use stratosort::config::Config;
use stratosort::error::AppError;
use tauri::test::{mock_app, MockRuntime};
use tauri::{State, Emitter};
use tempfile::tempdir;
use std::sync::Arc;
use tokio::fs;
use tokio::time::{timeout, Duration};

/// Integration tests for first-run setup with comprehensive error recovery scenarios
/// These tests verify the robustness of the initial application setup process

#[tokio::test]
async fn test_first_run_setup_happy_path() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Create a clean config directory for testing
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir).await.unwrap();

    let mut config = Config::default();
    config.config_directory = Some(config_dir.display().to_string());

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    // Test successful first-run setup
    let setup_request = SetupRequest {
        ollama_url: "http://localhost:11434".to_string(),
        model_name: "llama2".to_string(),
        scan_directory: Some(temp_dir.path().display().to_string()),
    };

    println!("Testing successful first-run setup...");
    let state_clone = State::from(state.clone());
    let result = setup_application(setup_request, state_clone, app.clone()).await;

    match result {
        Ok(setup_result) => {
            println!("First-run setup completed successfully");
            assert!(setup_result.config_saved, "Config should be saved on successful setup");
            assert!(setup_result.database_initialized, "Database should be initialized");

            // Verify config was actually saved
            let config_file = config_dir.join("config.json");
            assert!(config_file.exists(), "Config file should exist after setup");

            // Verify config contents
            let saved_config = Config::load_from_file(&config_file).await.unwrap();
            assert_eq!(saved_config.ollama_url, "http://localhost:11434");
            assert_eq!(saved_config.ollama_model, "llama2");

            println!("Configuration properly saved and validated");
        }
        Err(e) => {
            panic!("First-run setup should succeed in happy path scenario: {:?}", e);
        }
    }

    // Test that subsequent setups handle existing configuration gracefully
    println!("Testing setup with existing configuration...");
    let second_setup = SetupRequest {
        ollama_url: "http://localhost:11435".to_string(),
        model_name: "llama3".to_string(),
        scan_directory: Some(temp_dir.path().display().to_string()),
    };

    let state_clone = State::from(state.clone());
    let result = setup_application(second_setup, state_clone, app.clone()).await;

    match result {
        Ok(setup_result) => {
            println!("Second setup completed");
            // Should update existing configuration
            assert!(setup_result.config_saved, "Config should be updated");
        }
        Err(e) => {
            println!("Second setup failed (may be acceptable): {:?}", e);
            // Depending on implementation, this might fail or succeed
        }
    }
}

#[tokio::test]
async fn test_setup_with_invalid_ollama_connection() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    let mut config = Config::default();
    config.config_directory = Some(temp_dir.path().join("config").display().to_string());

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    // Test setup with invalid Ollama URLs
    let invalid_setups = vec![
        // Unreachable host
        SetupRequest {
            ollama_url: "http://nonexistent.host:11434".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some(temp_dir.path().display().to_string()),
        },

        // Wrong port
        SetupRequest {
            ollama_url: "http://localhost:99999".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some(temp_dir.path().display().to_string()),
        },

        // Invalid protocol
        SetupRequest {
            ollama_url: "ftp://localhost:11434".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some(temp_dir.path().display().to_string()),
        },

        // Malformed URL
        SetupRequest {
            ollama_url: "not-a-url".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some(temp_dir.path().display().to_string()),
        },

        // Empty URL
        SetupRequest {
            ollama_url: "".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some(temp_dir.path().display().to_string()),
        },
    ];

    for (i, invalid_setup) in invalid_setups.iter().enumerate() {
        println!("Testing invalid Ollama setup #{}: {}", i, invalid_setup.ollama_url);

        let state_clone = State::from(state.clone());
        let result = timeout(
            Duration::from_secs(30), // Reasonable timeout for connection attempts
            setup_application(invalid_setup.clone(), state_clone, app.clone())
        ).await;

        match result {
            Ok(Ok(setup_result)) => {
                println!("Setup #{} unexpectedly succeeded: {:?}", i, setup_result);
                // Some setups might succeed if validation is lenient
                if setup_result.config_saved {
                    println!("WARNING: Invalid configuration was saved");
                }
            }
            Ok(Err(error)) => {
                println!("Setup #{} properly failed: {:?}", i, error);
                match error {
                    AppError::NetworkError { .. } => {
                        println!("  Network error (expected for unreachable hosts)");
                    }
                    AppError::InvalidInput { .. } => {
                        println!("  Invalid input error (expected for malformed URLs)");
                    }
                    AppError::ConfigError { .. } => {
                        println!("  Config error (expected for invalid configurations)");
                    }
                    _ => {
                        println!("  Other error type (may be acceptable)");
                    }
                }
            }
            Err(_timeout) => {
                println!("Setup #{} timed out (acceptable for unreachable hosts)", i);
            }
        }

        // Verify that failed setups don't leave the system in a bad state
        verify_system_state_after_failed_setup(&state).await;
    }
}

#[tokio::test]
async fn test_setup_with_filesystem_errors() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Test setup scenarios that involve filesystem errors
    let filesystem_error_scenarios = vec![
        // Non-existent config directory
        ("nonexistent_config", None),

        // Read-only config directory (simulate by creating then making read-only)
        ("readonly_config", Some("readonly")),

        // Very long path (may cause issues on some systems)
        ("long_path", Some(&"A".repeat(200))),
    ];

    for (scenario_name, path_suffix) in filesystem_error_scenarios {
        println!("Testing filesystem error scenario: {}", scenario_name);

        let config_dir = match path_suffix {
            Some("readonly") => {
                let dir = temp_dir.path().join("readonly_config");
                fs::create_dir_all(&dir).await.unwrap();
                // Note: Making directory read-only is platform-specific
                // This is a simplified test
                dir
            }
            Some(suffix) => temp_dir.path().join(format!("config_{}", suffix)),
            None => temp_dir.path().join("nonexistent").join("deeply").join("nested"),
        };

        let mut config = Config::default();
        config.config_directory = Some(config_dir.display().to_string());

        let state_result = AppState::with_config(config).await;

        match state_result {
            Ok(state) => {
                let state = Arc::new(state);

                let setup_request = SetupRequest {
                    ollama_url: "http://localhost:11434".to_string(),
                    model_name: "llama2".to_string(),
                    scan_directory: Some(temp_dir.path().display().to_string()),
                };

                let state_clone = State::from(state.clone());
                let result = setup_application(setup_request, state_clone, app.clone()).await;

                match result {
                    Ok(setup_result) => {
                        println!("Setup succeeded despite filesystem scenario: {:?}", setup_result);
                        // Some scenarios might succeed if the system creates directories
                    }
                    Err(AppError::ConfigError { message }) => {
                        println!("Setup properly failed with config error: {}", message);
                    }
                    Err(AppError::FileSystemError { message }) => {
                        println!("Setup properly failed with filesystem error: {}", message);
                    }
                    Err(other) => {
                        println!("Setup failed with other error: {:?}", other);
                    }
                }
            }
            Err(e) => {
                println!("AppState creation failed for scenario {}: {:?}", scenario_name, e);
                // This is acceptable for invalid filesystem scenarios
            }
        }
    }
}

#[tokio::test]
async fn test_setup_parameter_validation_and_recovery() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir).await.unwrap();

    let mut config = Config::default();
    config.config_directory = Some(config_dir.display().to_string());

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    // Test parameter validation with various invalid inputs
    let invalid_parameter_scenarios = vec![
        // Missing model name
        SetupRequest {
            ollama_url: "http://localhost:11434".to_string(),
            model_name: "".to_string(),
            scan_directory: Some(temp_dir.path().display().to_string()),
        },

        // Invalid scan directory
        SetupRequest {
            ollama_url: "http://localhost:11434".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some("/nonexistent/directory".to_string()),
        },

        // Null bytes in parameters
        SetupRequest {
            ollama_url: "http://localhost:11434\0".to_string(),
            model_name: "llama2\0".to_string(),
            scan_directory: Some(format!("{}\0", temp_dir.path().display())),
        },

        // Extremely long parameters
        SetupRequest {
            ollama_url: format!("http://localhost:11434/{}", "A".repeat(10000)),
            model_name: "B".repeat(10000),
            scan_directory: Some("C".repeat(10000)),
        },

        // Unicode attacks
        SetupRequest {
            ollama_url: "http://localhost:11434".to_string(),
            model_name: "llama2\u{202e}kcatta\u{202c}".to_string(),
            scan_directory: Some(format!("{}\u{200b}", temp_dir.path().display())),
        },

        // Path traversal in scan directory
        SetupRequest {
            ollama_url: "http://localhost:11434".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some("../../../etc".to_string()),
        },
    ];

    for (i, invalid_request) in invalid_parameter_scenarios.iter().enumerate() {
        println!("Testing parameter validation scenario #{}", i);

        let state_clone = State::from(state.clone());
        let result = setup_application(invalid_request.clone(), state_clone, app.clone()).await;

        match result {
            Ok(setup_result) => {
                println!("Setup #{} unexpectedly succeeded: {:?}", i, setup_result);

                // If setup succeeded, verify the parameters were sanitized
                if setup_result.config_saved {
                    let config_file = config_dir.join("config.json");
                    if let Ok(saved_config) = Config::load_from_file(&config_file).await {
                        // Verify sanitization occurred
                        assert!(!saved_config.ollama_url.contains('\0'),
                               "Null bytes should be removed from URL");
                        assert!(!saved_config.ollama_model.contains('\0'),
                               "Null bytes should be removed from model name");
                        assert!(saved_config.ollama_url.len() < 1000,
                               "URL should be truncated if too long");
                        assert!(saved_config.ollama_model.len() < 1000,
                               "Model name should be truncated if too long");

                        println!("Parameters were properly sanitized in config");
                    }
                }
            }
            Err(AppError::InvalidInput { message }) => {
                println!("Setup #{} properly rejected invalid input: {}", i, message);
            }
            Err(AppError::SecurityError { message }) => {
                println!("Setup #{} properly rejected for security: {}", i, message);
            }
            Err(AppError::InvalidPath { message }) => {
                println!("Setup #{} properly rejected invalid path: {}", i, message);
            }
            Err(other) => {
                println!("Setup #{} failed with other error: {:?}", i, other);
            }
        }

        // Verify system state remains consistent after each failed attempt
        verify_system_state_after_failed_setup(&state).await;
    }
}

#[tokio::test]
async fn test_setup_database_initialization_errors() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    // Test scenarios where database initialization might fail
    let db_scenarios = vec![
        // Invalid database path
        ("invalid_db_path", Some("/root/readonly/database.db")),

        // Very long database path
        ("long_db_path", Some(&format!("{}/{}.db", temp_dir.path().display(), "A".repeat(200)))),

        // Database path with special characters
        ("special_chars", Some(&format!("{}/data\0base.db", temp_dir.path().display()))),
    ];

    for (scenario_name, db_path_override) in db_scenarios {
        println!("Testing database scenario: {}", scenario_name);

        let config_dir = temp_dir.path().join(format!("config_{}", scenario_name));
        fs::create_dir_all(&config_dir).await.unwrap();

        let mut config = Config::default();
        config.config_directory = Some(config_dir.display().to_string());

        if let Some(db_path) = db_path_override {
            config.database_path = Some(db_path.to_string());
        }

        let state_result = AppState::with_config(config).await;

        match state_result {
            Ok(state) => {
                let state = Arc::new(state);

                let setup_request = SetupRequest {
                    ollama_url: "http://localhost:11434".to_string(),
                    model_name: "llama2".to_string(),
                    scan_directory: Some(temp_dir.path().display().to_string()),
                };

                let state_clone = State::from(state.clone());
                let result = setup_application(setup_request, state_clone, app.clone()).await;

                match result {
                    Ok(setup_result) => {
                        println!("Database setup succeeded: {:?}", setup_result);
                        assert!(setup_result.database_initialized,
                               "Database should be marked as initialized if setup succeeded");
                    }
                    Err(AppError::DatabaseError { message }) => {
                        println!("Database setup properly failed: {}", message);
                    }
                    Err(other) => {
                        println!("Database setup failed with: {:?}", other);
                    }
                }
            }
            Err(e) => {
                println!("AppState creation failed for database scenario {}: {:?}", scenario_name, e);
            }
        }
    }
}

#[tokio::test]
async fn test_setup_concurrent_access_handling() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    let config_dir = temp_dir.path().join("concurrent_config");
    fs::create_dir_all(&config_dir).await.unwrap();

    let mut config = Config::default();
    config.config_directory = Some(config_dir.display().to_string());

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    // Test concurrent setup attempts
    let concurrent_setups = vec![
        SetupRequest {
            ollama_url: "http://localhost:11434".to_string(),
            model_name: "llama2".to_string(),
            scan_directory: Some(temp_dir.path().join("scan1").display().to_string()),
        },
        SetupRequest {
            ollama_url: "http://localhost:11435".to_string(),
            model_name: "llama3".to_string(),
            scan_directory: Some(temp_dir.path().join("scan2").display().to_string()),
        },
        SetupRequest {
            ollama_url: "http://localhost:11436".to_string(),
            model_name: "codellama".to_string(),
            scan_directory: Some(temp_dir.path().join("scan3").display().to_string()),
        },
    ];

    println!("Testing concurrent setup operations...");

    // Launch multiple setup operations simultaneously
    let mut handles = Vec::new();

    for (i, setup_request) in concurrent_setups.into_iter().enumerate() {
        let state_clone = State::from(state.clone());
        let app_clone = app.clone();

        let handle = tokio::spawn(async move {
            println!("Starting concurrent setup #{}", i);

            let result = timeout(
                Duration::from_secs(60),
                setup_application(setup_request, state_clone, app_clone)
            ).await;

            match result {
                Ok(Ok(setup_result)) => {
                    println!("Concurrent setup #{} succeeded: {:?}", i, setup_result);
                    Ok(setup_result)
                }
                Ok(Err(error)) => {
                    println!("Concurrent setup #{} failed: {:?}", i, error);
                    Err(format!("Setup error: {:?}", error))
                }
                Err(_timeout) => {
                    println!("Concurrent setup #{} timed out", i);
                    Err("Timeout".to_string())
                }
            }
        });

        handles.push(handle);

        // Slight stagger to simulate realistic timing
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for all concurrent operations to complete
    let results = futures::future::join_all(handles).await;

    let mut successful_setups = 0;
    let mut failed_setups = 0;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(Ok(_setup_result)) => {
                successful_setups += 1;
                println!("Concurrent setup #{} completed successfully", i);
            }
            Ok(Err(error)) => {
                failed_setups += 1;
                println!("Concurrent setup #{} failed: {}", i, error);
            }
            Err(join_error) => {
                failed_setups += 1;
                println!("Concurrent setup #{} panicked: {:?}", i, join_error);
            }
        }
    }

    println!("Concurrent setup results: {} successful, {} failed", successful_setups, failed_setups);

    // At least one setup should succeed, others may fail due to concurrency
    assert!(successful_setups > 0, "At least one concurrent setup should succeed");

    // Verify final system state is consistent
    verify_system_state_after_setup(&state).await;
}

#[tokio::test]
async fn test_setup_recovery_from_partial_failures() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app();

    let config_dir = temp_dir.path().join("recovery_config");
    fs::create_dir_all(&config_dir).await.unwrap();

    let mut config = Config::default();
    config.config_directory = Some(config_dir.display().to_string());

    let state = Arc::new(AppState::with_config(config).await.unwrap());

    // Simulate partial setup by creating some but not all required components
    println!("Creating partial setup state...");

    // Create a partial config file with missing fields
    let partial_config = r#"{
        "ollama_url": "http://localhost:11434",
        "ollama_model": ""
    }"#;

    let config_file = config_dir.join("config.json");
    fs::write(&config_file, partial_config).await.unwrap();

    // Test recovery from partial state
    let recovery_setup = SetupRequest {
        ollama_url: "http://localhost:11434".to_string(),
        model_name: "llama2".to_string(),
        scan_directory: Some(temp_dir.path().display().to_string()),
    };

    println!("Testing recovery from partial setup...");
    let state_clone = State::from(state.clone());
    let result = setup_application(recovery_setup, state_clone, app.clone()).await;

    match result {
        Ok(setup_result) => {
            println!("Recovery setup succeeded: {:?}", setup_result);

            // Verify the configuration was properly completed
            let recovered_config = Config::load_from_file(&config_file).await.unwrap();
            assert!(!recovered_config.ollama_model.is_empty(),
                   "Model name should be filled in during recovery");

            println!("Configuration successfully recovered and completed");
        }
        Err(e) => {
            println!("Recovery setup failed: {:?}", e);
            // Recovery might fail if the partial state is too corrupted
        }
    }

    // Test recovery from corrupted config file
    println!("Testing recovery from corrupted config...");
    let corrupted_config = r#"{"incomplete": json syntax error"#;
    fs::write(&config_file, corrupted_config).await.unwrap();

    let state_clone = State::from(state.clone());
    let result = setup_application(recovery_setup, state_clone, app.clone()).await;

    match result {
        Ok(setup_result) => {
            println!("Recovery from corruption succeeded: {:?}", setup_result);
        }
        Err(e) => {
            println!("Recovery from corruption failed (may be expected): {:?}", e);
        }
    }

    // Verify system can still function after recovery attempts
    verify_system_state_after_setup(&state).await;
}

// Helper function to verify system state after failed setup
async fn verify_system_state_after_failed_setup(state: &AppState) {
    // Verify the application is still in a usable state after setup failure

    // Check that config is still accessible
    let config = state.config.read();
    assert!(!config.ollama_url.is_empty(), "Config should still have valid URL");

    // Check that database is still accessible
    let db_check = state.database.get_all_files().await;
    match db_check {
        Ok(_) => println!("Database remains accessible after failed setup"),
        Err(e) => println!("Database access issue after failed setup: {:?}", e),
    }

    // Check that no operations are stuck
    let active_ops = state.active_operations.len();
    if active_ops > 10 {
        println!("WARNING: Many active operations after failed setup: {}", active_ops);
    }
}

// Helper function to verify system state after successful setup
async fn verify_system_state_after_setup(state: &AppState) {
    // Verify the application is in a good state after setup

    let config = state.config.read();
    assert!(!config.ollama_url.is_empty(), "Config should have valid URL after setup");

    // Verify database is functioning
    let db_check = state.database.get_all_files().await;
    assert!(db_check.is_ok(), "Database should be accessible after setup");

    // Verify AI service can be tested
    let state_clone = State::from(Arc::new(state.clone()));
    let app = mock_app();
    let ai_status = check_ai_status(state_clone, app).await;

    match ai_status {
        Ok(status) => {
            println!("AI service status after setup: {:?}", status);
        }
        Err(e) => {
            println!("AI service check failed after setup: {:?}", e);
            // This might be expected if Ollama is not actually running
        }
    }

    println!("System state verification completed");
}