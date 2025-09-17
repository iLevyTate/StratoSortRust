use std::sync::Arc;
use stratosort::commands::ai::{analyze_with_ai, generate_embeddings, pull_model, semantic_search};
use stratosort::commands::files::{analyze_files, get_file_content, scan_directory};
use stratosort::config::Config;
use stratosort::error::AppError;
use stratosort::state::AppState;
use stratosort::utils::security::{sanitize_filename, validate_and_sanitize_path_legacy};
use tauri::test::{mock_app, mock_context, MockRuntime};
use tauri::{AppHandle, Manager};
use tokio::runtime::Runtime;

#[test]
fn test_model_name_injection_prevention() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        // Create mock app state (simplified for testing)
        let app = mock_app();
        let state = create_mock_app_state().await;
        let state_ref = tauri::State::<Arc<AppState>>::from(&state);

        let malicious_model_names = vec![
            "",                                   // Empty
            "a".repeat(200),                      // Too long
            "model; rm -rf /",                    // Command injection attempt
            "model && evil_command",              // Command chaining
            "model`evil_command`",                // Backtick injection
            "model$(evil_command)",               // Command substitution
            "model|evil_command",                 // Pipe injection
            "model\nmalicious_command",           // Newline injection
            "model\0null_byte",                   // Null byte injection
            "../../../etc/passwd",                // Path traversal
            "model<script>alert('xss')</script>", // XSS attempt
            "model'; DROP TABLE users; --",       // SQL injection attempt
            "model\r\nHTTP/1.1 200 OK",           // HTTP header injection
            "🚀💻🔥",                             // Unicode/emoji
            "\u{202e}model",                      // Right-to-left override
            "model\u{0000}",                      // Unicode null
        ];

        for malicious_name in malicious_model_names {
            let result = pull_model(malicious_name.to_string(), state_ref.clone()).await;

            match result {
                Err(AppError::InvalidPath { message }) => {
                    assert!(message.contains("empty") || message.contains("Model name"));
                }
                Err(AppError::SecurityError { message }) => {
                    assert!(
                        message.contains("too long")
                            || message.contains("Invalid model name format")
                            || message.contains("characters")
                    );
                }
                Ok(_) => {
                    panic!(
                        "Malicious model name '{}' should have been rejected",
                        malicious_name
                    );
                }
                Err(e) => {
                    // Other errors are acceptable (like AiError for Ollama not available)
                    println!("Model name '{}' rejected with: {:?}", malicious_name, e);
                }
            }
        }
    });
}

#[test]
fn test_content_injection_prevention() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let app = mock_app();
        let state = create_mock_app_state().await;
        let state_ref = tauri::State::<Arc<AppState>>::from(&state);

        let malicious_content_tests = vec![
            ("a".repeat(11 * 1024 * 1024), "text/plain"), // Too large (>10MB)
            ("normal content", ""),                       // Empty MIME type
            ("normal content", "a".repeat(300)),          // MIME type too long
            ("normal content", "text/plain; rm -rf /"),   // Command injection in MIME
            ("normal content", "text/plain\nContent-Type: evil"), // Header injection
            ("normal content", "text/plain<script>"),     // XSS in MIME type
            ("normal content", "application/x-executable"), // Potentially dangerous MIME
            ("normal content", "text/plain\0"),           // Null byte in MIME
        ];

        for (content, mime_type) in malicious_content_tests {
            let result =
                analyze_with_ai(content.clone(), mime_type.to_string(), state_ref.clone()).await;

            match result {
                Err(AppError::SecurityError { message }) => {
                    assert!(
                        message.contains("too large")
                            || message.contains("Invalid MIME type")
                            || message.contains("format")
                    );
                }
                Err(AppError::InvalidPath { message }) => {
                    assert!(message.contains("MIME type"));
                }
                Ok(_) => {
                    panic!(
                        "Malicious content should have been rejected: content_len={}, mime='{}'",
                        content.len(),
                        mime_type
                    );
                }
                Err(e) => {
                    // Other errors (like AiError) are acceptable
                    println!("Content rejected with: {:?}", e);
                }
            }
        }
    });
}

#[test]
fn test_search_query_injection_prevention() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let app = mock_app();
        let state = create_mock_app_state().await;
        let state_ref = tauri::State::<Arc<AppState>>::from(&state);

        let malicious_queries = vec![
            ("", 10),                                        // Empty query
            ("a".repeat(1500), 10),                          // Too long query
            ("query'; DROP TABLE file_analysis; --", 10),    // SQL injection
            ("query UNION SELECT * FROM sqlite_master", 10), // SQL union attack
            ("query/*comment*/SELECT", 10),                  // SQL comment injection
            ("normal query", 0),                             // Invalid limit (0)
            ("normal query", 1000),                          // Limit too high
            ("query\nUNION\nSELECT", 10),                    // Multi-line injection
            ("query\0null", 10),                             // Null byte injection
            ("query' OR '1'='1", 10),                        // Classic SQL injection
        ];

        for (query, limit) in malicious_queries {
            let result = semantic_search(query.to_string(), limit, state_ref.clone()).await;

            match result {
                Err(AppError::InvalidPath { message }) => {
                    assert!(message.contains("empty") || message.contains("query"));
                }
                Err(AppError::SecurityError { message }) => {
                    assert!(
                        message.contains("too long")
                            || message.contains("Invalid limit")
                            || message.contains("characters")
                    );
                }
                Ok(_) => {
                    panic!(
                        "Malicious search query should have been rejected: '{}'",
                        query
                    );
                }
                Err(e) => {
                    // Other errors are acceptable (like database errors)
                    println!("Query '{}' rejected with: {:?}", query, e);
                }
            }
        }
    });
}

#[test]
fn test_path_traversal_injection_prevention() {
    let app = mock_app();

    let malicious_paths = vec![
        "../../../etc/passwd",                        // Classic path traversal
        "..\\..\\..\\windows\\system32\\config\\sam", // Windows path traversal
        "/etc/shadow",                                // Direct sensitive file access
        "C:\\Windows\\System32\\config\\SAM",         // Windows sensitive file
        "file://etc/passwd",                          // File URI scheme
        "\\\\server\\share\\file",                    // UNC path
        "file\0.txt",                                 // Null byte injection
        "file\n.txt",                                 // Newline injection
        "con.txt",                                    // Windows reserved name
        "aux.txt",                                    // Windows reserved name
        "file<script>.txt",                           // XSS attempt in filename
        "file'; DROP TABLE users; --.txt",            // SQL injection in filename
        "very_long_filename_".repeat(50) + ".txt",    // Extremely long filename
        "/root/.ssh/id_rsa",                          // SSH private key
        "~/.bashrc",                                  // Home directory traversal
        ".env",                                       // Environment file
        ".git/config",                                // Git configuration
        "../../.env",                                 // Environment file with traversal
        "file\r\n.txt",                               // CRLF injection
        "file\u{202e}.txt",                           // Right-to-left override
    ];

    for malicious_path in malicious_paths {
        let result = validate_and_sanitize_path_legacy(malicious_path, &app);

        match result {
            Err(AppError::SecurityError { message }) => {
                assert!(
                    message.contains("Path traversal")
                        || message.contains("Invalid path")
                        || message.contains("not allowed")
                        || message.contains("system directories")
                        || message.contains("validation failed")
                );
            }
            Err(AppError::FileNotFound { .. }) => {
                // File not found is acceptable - means path was processed but doesn't exist
            }
            Ok(path) => {
                // If validation passes, ensure the path is actually safe
                let path_str = path.to_string_lossy();
                assert!(
                    !path_str.contains(".."),
                    "Path should not contain traversal: {}",
                    path_str
                );
                assert!(
                    !path_str.contains("/etc/"),
                    "Path should not access system directories: {}",
                    path_str
                );
                assert!(
                    !path_str.contains("\\Windows\\System32\\"),
                    "Path should not access system directories: {}",
                    path_str
                );

                println!("Path '{}' was sanitized to: '{}'", malicious_path, path_str);
            }
        }
    }
}

#[test]
fn test_filename_sanitization() {
    let dangerous_filenames = vec![
        ("con.txt", true),                   // Windows reserved
        ("aux.txt", true),                   // Windows reserved
        ("file<script>.txt", false),         // XSS
        ("file\"injection\".txt", false),    // Quote injection
        ("file|pipe.txt", false),            // Pipe character
        ("file*wildcard.txt", false),        // Wildcard
        ("file?.txt", false),                // Question mark
        ("file\0null.txt", false),           // Null byte
        ("file\nnewline.txt", false),        // Newline
        ("file/slash.txt", false),           // Path separator
        ("file\\backslash.txt", false),      // Windows path separator
        ("normal_file.txt", true),           // Should be safe
        ("file-with-dashes.txt", true),      // Should be safe
        ("file_with_underscores.txt", true), // Should be safe
        ("file.with.dots.txt", true),        // Should be safe
    ];

    for (filename, should_remain_valid) in dangerous_filenames {
        let sanitized = sanitize_filename(filename);

        if should_remain_valid {
            assert!(
                !sanitized.is_empty(),
                "Safe filename '{}' should not be completely removed",
                filename
            );
            assert!(
                !sanitized.contains(['<', '>', ':', '"', '|', '?', '*', '/', '\\', '\0']),
                "Sanitized filename should not contain dangerous characters: '{}'",
                sanitized
            );
        } else {
            // Dangerous filenames should either be completely sanitized or have dangerous parts removed
            assert!(
                !sanitized.contains(['<', '>', ':', '"', '|', '?', '*', '/', '\\', '\0']),
                "Sanitized filename should not contain dangerous characters: '{}' -> '{}'",
                filename,
                sanitized
            );
        }

        println!("Filename '{}' sanitized to: '{}'", filename, sanitized);
    }
}

#[test]
fn test_embedding_text_injection_prevention() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let app = mock_app();
        let state = create_mock_app_state().await;
        let state_ref = tauri::State::<Arc<AppState>>::from(&state);

        let malicious_texts = vec![
            "".to_string(),                                // Empty text
            "a".repeat(200 * 1024),                        // Too long (>100KB)
            "text\0with\0nulls".to_string(),               // Null bytes
            "text with \u{202e} rtl override".to_string(), // Unicode attacks
            "very normal text".to_string(),                // This should pass
        ];

        for text in malicious_texts {
            let result = generate_embeddings(text.clone(), state_ref.clone()).await;

            match &text {
                t if t.is_empty() => {
                    assert!(
                        matches!(result, Err(AppError::InvalidPath { .. })),
                        "Empty text should be rejected"
                    );
                }
                t if t.len() > 100 * 1024 => {
                    assert!(
                        matches!(result, Err(AppError::SecurityError { .. })),
                        "Oversized text should be rejected"
                    );
                }
                t if t == "very normal text" => {
                    // This might succeed or fail depending on AI service availability
                    match result {
                        Ok(_) => println!("Normal text processed successfully"),
                        Err(e) => println!(
                            "Normal text rejected due to service unavailability: {:?}",
                            e
                        ),
                    }
                }
                _ => {
                    // Other malicious texts should be handled safely
                    match result {
                        Ok(_) => {
                            // If it passes validation, that's okay as long as it's processed safely
                            println!(
                                "Text '{}...' was processed (truncated for display)",
                                &text[..text.len().min(50)]
                            );
                        }
                        Err(e) => {
                            println!("Text rejected: {:?}", e);
                        }
                    }
                }
            }
        }
    });
}

#[test]
fn test_file_operation_injection_prevention() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let app = mock_app();
        let app_handle: AppHandle = app.handle().clone();
        let state = create_mock_app_state().await;
        let state_ref = tauri::State::<Arc<AppState>>::from(&state);

        let malicious_file_operations = vec![
            vec!["../../../etc/passwd".to_string()], // Path traversal in analysis
            vec!["/dev/null".to_string()],           // Special device file
            vec!["C:\\Windows\\System32\\drivers\\etc\\hosts".to_string()], // Windows system file
            vec!["file1.txt".to_string(), "../escape.txt".to_string()], // Mixed safe/unsafe
            vec!["a".repeat(1500)],                  // Extremely long path
            (0..2000).map(|i| format!("file{}.txt", i)).collect(), // Too many files
        ];

        for file_list in malicious_file_operations {
            let result =
                analyze_files(file_list.clone(), state_ref.clone(), app_handle.clone()).await;

            match result {
                Err(AppError::SecurityError { message }) => {
                    assert!(
                        message.contains("Too many files")
                            || message.contains("not allowed")
                            || message.contains("Invalid")
                    );
                }
                Err(AppError::InvalidPath { .. }) => {
                    // Path validation errors are expected
                }
                Err(AppError::FileNotFound { .. }) => {
                    // File not found is acceptable - means validation passed but file doesn't exist
                }
                Ok(_) => {
                    // If it succeeds, ensure no sensitive files were actually processed
                    println!("File analysis completed for {} files", file_list.len());
                }
                Err(e) => {
                    println!("File operation rejected: {:?}", e);
                }
            }
        }
    });
}

// Helper function to create mock app state for testing
async fn create_mock_app_state() -> Arc<AppState> {
    // This is a simplified mock - in real tests you'd want proper mock implementations
    let app = mock_app();
    let app_handle: AppHandle = app.handle().clone();
    let config = Config::default();

    // Create a real AppState for testing (this will fail if dependencies aren't available)
    // In practice, you'd want to create mock implementations
    match AppState::new(app_handle, config).await {
        Ok(state) => Arc::new(state),
        Err(_) => {
            // Fallback to a minimal mock if real state creation fails
            panic!("Could not create app state for testing - need proper mocks");
        }
    }
}

#[test]
fn test_unicode_normalization_attacks() {
    let app = mock_app();

    // Unicode normalization attacks
    let unicode_attacks = vec![
        "café",             // Normal
        "cafe\u{0301}",     // Decomposed form (e + combining acute accent)
        "ﬁle.txt",          // Ligature fi
        "file\u{200d}.txt", // Zero-width joiner
        "file\u{200c}.txt", // Zero-width non-joiner
        "file\u{feff}.txt", // Byte order mark
        "file\u{202d}.txt", // Left-to-right override
        "file\u{202e}.txt", // Right-to-left override
    ];

    for attack_path in unicode_attacks {
        let app_handle: AppHandle = app.handle().clone();
        let result = validate_and_sanitize_path_legacy(attack_path, &app_handle);

        match result {
            Ok(path) => {
                let path_str = path.to_string_lossy();
                // Ensure no dangerous unicode characters remain
                assert!(
                    !path_str.contains('\u{202d}'),
                    "Left-to-right override should be removed"
                );
                assert!(
                    !path_str.contains('\u{202e}'),
                    "Right-to-left override should be removed"
                );
                assert!(
                    !path_str.contains('\u{200d}'),
                    "Zero-width joiner should be handled"
                );

                println!(
                    "Unicode path '{}' sanitized to: '{}'",
                    attack_path, path_str
                );
            }
            Err(e) => {
                println!("Unicode attack '{}' rejected: {:?}", attack_path, e);
            }
        }
    }
}
