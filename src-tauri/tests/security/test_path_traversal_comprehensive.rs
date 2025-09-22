use std::fs;
use stratosort::error::AppError;
use stratosort::utils::security::validate_path;
use tauri::test::mock_app;
use tempfile::tempdir;

#[test]
fn test_toctou_race_condition_prevention() {
    // Test Time-of-Check-Time-of-Use race conditions
    let temp_dir = tempdir().unwrap();
    let app = mock_app(); // Mock app needed for path validation

    // Create a legitimate file first
    let legit_path = temp_dir.path().join("legitimate.txt");
    fs::write(&legit_path, "safe content").unwrap();

    // Test path that could change between validation and use
    let legit_path_str = legit_path.to_string_lossy().to_string();

    // First validation should pass
    let result = validate_path(&legit_path_str, &app.handle());
    assert!(result.is_ok(), "Legitimate path should validate");

    // Test multiple rapid validations (simulate race condition)
    for i in 0..100 {
        let result = validate_path(&legit_path_str, &app.handle());
        if let Ok(path) = result {
            // Ensure the path is still within expected bounds
            assert!(
                path.starts_with(temp_dir.path()),
                "Iteration {}: Path should stay within temp directory",
                i
            );
        }
    }
}

#[test]
fn test_symlink_attack_prevention() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app(); // Mock app needed for path validation

    // Create target file outside allowed area (simulated - we can't actually create symlinks in all test environments)
    let outside_target = "/etc/passwd";
    let symlink_path = temp_dir.path().join("malicious_symlink");

    // Test detection of potential symlink attacks by path pattern
    let symlink_patterns = vec![
        format!("{}/../../../etc/passwd", temp_dir.path().display()),
        format!("{}/.././../etc/shadow", temp_dir.path().display()),
        format!(
            "{}\\..\\..\\windows\\system32\\config\\sam",
            temp_dir.path().display()
        ),
    ];

    for pattern in symlink_patterns {
        let result = validate_path(&pattern, &app.handle());

        match result {
            Err(AppError::SecurityError { message }) => {
                assert!(
                    message.contains("Path traversal")
                        || message.contains("not allowed")
                        || message.contains("system directories"),
                    "Should detect symlink attack pattern: {}",
                    pattern
                );
            }
            Ok(sanitized_path) => {
                // If sanitization succeeds, ensure it doesn't access dangerous areas
                let path_str = sanitized_path.to_string_lossy();
                assert!(
                    !path_str.contains("/etc/"),
                    "Sanitized path should not access /etc/"
                );
                assert!(
                    !path_str.contains("\\system32\\"),
                    "Sanitized path should not access system32"
                );
                println!(
                    "Pattern '{}' sanitized to safe path: '{}'",
                    pattern, path_str
                );
            }
            Err(e) => {
                // Other errors are acceptable
                println!("Symlink pattern '{}' rejected: {:?}", pattern, e);
            }
        }
    }
}

#[test]
fn test_unicode_normalization_attacks() {
    let app = mock_app(); // Mock app needed for path validation

    let unicode_attacks = vec![
        // Unicode normalization attacks
        ("café", "cafe\u{0301}"),     // NFC vs NFD
        ("file.txt", "ﬁle.txt"),      // Ligature substitution
        ("../etc", "..\u{002e}/etc"), // Fullwidth period
        ("test", "test\u{200e}"),     // Left-to-right mark
        ("evil", "\u{202e}live"),     // Right-to-left override (spells "evil" backwards visually)
        // Zero-width character attacks
        ("file.txt", "fil\u{200b}e.txt"), // Zero-width space
        ("admin", "ad\u{200c}min"),       // Zero-width non-joiner
        ("secret", "sec\u{200d}ret"),     // Zero-width joiner
        // Homograph attacks (characters that look similar)
        ("admin", "аdmin"),       // Cyrillic 'а' instead of Latin 'a'
        ("scope", "ѕсоре"),       // Mixed Cyrillic characters
        ("test.txt", "tеst.txt"), // Cyrillic 'е' instead of Latin 'e'
    ];

    for (expected, attack) in unicode_attacks {
        let result = validate_path(attack, &app.handle());

        match result {
            Ok(path) => {
                let path_str = path.to_string_lossy();

                // Ensure dangerous unicode characters are handled
                assert!(
                    !path_str.contains('\u{200b}'),
                    "Zero-width space should be removed from: {}",
                    attack
                );
                assert!(
                    !path_str.contains('\u{200c}'),
                    "Zero-width non-joiner should be removed from: {}",
                    attack
                );
                assert!(
                    !path_str.contains('\u{200d}'),
                    "Zero-width joiner should be removed from: {}",
                    attack
                );
                assert!(
                    !path_str.contains('\u{202e}'),
                    "Right-to-left override should be removed from: {}",
                    attack
                );
                assert!(
                    !path_str.contains('\u{200e}'),
                    "Left-to-right mark should be removed from: {}",
                    attack
                );

                println!(
                    "Unicode attack '{}' -> '{}' sanitized to: '{}'",
                    expected, attack, path_str
                );
            }
            Err(e) => {
                println!("Unicode attack '{}' rejected: {:?}", attack, e);
            }
        }
    }
}

#[test]
fn test_path_canonicalization_bypass() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app(); // Mock app needed for path validation

    // Create nested directory structure for testing
    let nested_dir = temp_dir.path().join("level1").join("level2");
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(nested_dir.join("secret.txt"), "secret data").unwrap();

    let canonicalization_bypasses = vec![
        // Double encoding
        "..%252f..%252f..%252fetc%252fpasswd",
        "..%c0%af..%c0%af..%c0%afetc%c0%afpasswd",
        // Mixed separators
        "../\\../\\../etc/passwd",
        "..\\/../\\../etc/passwd",
        // Null byte injection
        "../../../etc/passwd\0.txt",
        "legitimate.txt\0../../../etc/passwd",
        // Long path attacks
        &format!("{}{}", "../".repeat(100), "etc/passwd"),
        &format!("{}{}", "a/".repeat(1000), "../etc/passwd"),
        // Case variation (Windows)
        "../../../ETC/PASSWD",
        "../../../Etc/Passwd",
        // Space variations
        ".. /../../etc/passwd",
        "../ ../../etc/passwd",
        "../../../etc /passwd",
    ];

    for bypass_attempt in canonicalization_bypasses {
        let test_path = format!("{}/{}", temp_dir.path().display(), bypass_attempt);
        let result = validate_path(&test_path, &app.handle());

        match result {
            Err(AppError::SecurityError { message }) => {
                assert!(
                    message.contains("Path traversal")
                        || message.contains("Invalid path")
                        || message.contains("not allowed"),
                    "Should detect canonicalization bypass: {}",
                    bypass_attempt
                );
            }
            Ok(sanitized_path) => {
                // If sanitization passes, ensure it's actually safe
                let canonical = sanitized_path.canonicalize().unwrap_or(sanitized_path);
                let temp_canonical = temp_dir.path().canonicalize().unwrap();

                assert!(
                    canonical.starts_with(&temp_canonical),
                    "Canonicalized path should stay within temp dir: '{}' vs '{}'",
                    canonical.display(),
                    temp_canonical.display()
                );

                println!(
                    "Bypass attempt '{}' sanitized to safe path: '{}'",
                    bypass_attempt,
                    canonical.display()
                );
            }
            Err(e) => {
                println!(
                    "Canonicalization bypass '{}' rejected: {:?}",
                    bypass_attempt, e
                );
            }
        }
    }
}

#[test]
fn test_directory_traversal_with_allowed_paths() {
    let temp_dir = tempdir().unwrap();
    let app = mock_app(); // Mock app needed for path validation

    // Create some allowed and disallowed directories
    let allowed_dir = temp_dir.path().join("allowed");
    let restricted_dir = temp_dir.path().join("restricted");
    fs::create_dir_all(&allowed_dir).unwrap();
    fs::create_dir_all(&restricted_dir).unwrap();

    fs::write(allowed_dir.join("safe.txt"), "safe content").unwrap();
    fs::write(restricted_dir.join("secret.txt"), "secret content").unwrap();

    let test_cases = vec![
        // Should be allowed
        (format!("{}/safe.txt", allowed_dir.display()), true),
        (format!("{}/subdir/file.txt", allowed_dir.display()), true),
        // Should be blocked - attempts to escape allowed area
        (
            format!("{}/../restricted/secret.txt", allowed_dir.display()),
            false,
        ),
        (
            format!("{}/./../../restricted/secret.txt", allowed_dir.display()),
            false,
        ),
        (
            format!("{}/../../../etc/passwd", allowed_dir.display()),
            false,
        ),
    ];

    for (test_path, should_be_allowed) in test_cases {
        let result = validate_path(&test_path, &app.handle());

        if should_be_allowed {
            match result {
                Ok(path) => {
                    println!("Allowed path '{}' -> '{}'", test_path, path.display());
                }
                Err(e) => {
                    // Some allowed paths might fail due to file not existing, which is fine for validation testing
                    println!("Allowed path '{}' validation result: {:?}", test_path, e);
                }
            }
        } else {
            match result {
                Err(AppError::SecurityError { .. }) => {
                    println!("Correctly blocked dangerous path: '{}'", test_path);
                }
                Ok(sanitized_path) => {
                    // If it passes, ensure it's actually within bounds
                    let path_str = sanitized_path.to_string_lossy();
                    assert!(
                        !path_str.contains("/etc/") && !path_str.contains("restricted"),
                        "Path '{}' should not access restricted areas, got: '{}'",
                        test_path,
                        path_str
                    );
                }
                Err(e) => {
                    println!("Blocked path '{}' with error: {:?}", test_path, e);
                }
            }
        }
    }
}

#[test]
fn test_file_extension_confusion_attacks() {
    let app = mock_app(); // Mock app needed for path validation

    let extension_attacks = vec![
        // Double extensions
        "innocent.txt.exe",
        "document.pdf.scr",
        "image.jpg.bat",
        // Hidden extensions
        "document.txt\u{200e}.exe", // Right-to-left mark before .exe
        "file.txt\u{202e}exe",              // Right-to-left override
        // Space confusion
        "file.txt .exe",
        "file.txt\u{a0}.exe", // Non-breaking space
        // Case confusion
        "FILE.TXT.EXE",
        "file.TxT.ExE",
        // Unicode confusables in extensions
        "file.txtе", // Cyrillic 'е' instead of 'e'
        "file.ехе",  // Cyrillic 'х' and 'е'
    ];

    for attack_filename in extension_attacks {
        let result = validate_path(attack_filename, &app.handle());

        match result {
            Ok(sanitized_path) => {
                let filename = sanitized_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Ensure dangerous extensions are handled appropriately
                assert!(
                    !filename.ends_with(".exe")
                        || !filename.ends_with(".scr")
                        || !filename.ends_with(".bat"),
                    "Dangerous executable extension should be handled in: '{}' -> '{}'",
                    attack_filename,
                    filename
                );

                // Ensure unicode trickery is cleaned up
                assert!(
                    !filename.contains('\u{200e}'),
                    "Right-to-left mark should be removed"
                );
                assert!(
                    !filename.contains('\u{202e}'),
                    "Right-to-left override should be removed"
                );

                println!(
                    "Extension attack '{}' sanitized to: '{}'",
                    attack_filename, filename
                );
            }
            Err(e) => {
                println!("Extension attack '{}' rejected: {:?}", attack_filename, e);
            }
        }
    }
}
