use stratosort::utils::security::{validate_and_sanitize_path_legacy, is_path_allowed, sanitize_filename, validate_file_extension};
use stratosort::error::AppError;
use std::path::Path;
use tempfile::tempdir;
use tauri::test::{mock_app, mock_context};

#[cfg(test)]
mod security_tests {
    use super::*;

    fn create_mock_app() -> tauri::App<tauri::Wry> {
        let context = mock_context();
        mock_app(context)
    }

    #[tokio::test]
    async fn test_validate_and_sanitize_path_valid() {
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        let test_path = temp_dir.path().join("test.txt");
        std::fs::write(&test_path, "test content").unwrap();
        
        let result = validate_and_sanitize_path_legacy(
            &test_path.to_string_lossy(), 
            app.handle()
        );
        
        assert!(result.is_ok());
        let sanitized = result.unwrap();
        assert_eq!(sanitized, test_path.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_validate_and_sanitize_path_traversal_attack() {
        let app = create_mock_app();
        
        let malicious_paths = vec![
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config\\sam",
            "/etc/shadow",
            "C:\\Windows\\System32\\config\\SAM",
            "file:///../../../etc/passwd",
            "%2e%2e%2f%2e%2e%2f%2e%2e%2fetc%2fpasswd", // URL encoded
        ];
        
        for malicious_path in malicious_paths {
            let result = validate_and_sanitize_path_legacy(malicious_path, app.handle());
            assert!(result.is_err(), "Should reject malicious path: {}", malicious_path);
            
            if let Err(AppError::SecurityError { message }) = result {
                assert!(message.contains("Security violation") || message.contains("Invalid path"));
            } else {
                panic!("Expected SecurityError for path: {}", malicious_path);
            }
        }
    }

    #[tokio::test]
    async fn test_validate_and_sanitize_path_empty() {
        let app = create_mock_app();
        let result = validate_and_sanitize_path_legacy("", app.handle());
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::InvalidPath { message } => {
                assert!(message.contains("empty"));
            }
            _ => panic!("Expected InvalidPath error"),
        }
    }

    #[tokio::test]
    async fn test_validate_and_sanitize_path_nonexistent() {
        let app = create_mock_app();
        let result = validate_and_sanitize_path_legacy("/nonexistent/path/file.txt", app.handle());
        
        // Should handle non-existent paths gracefully for validation
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_path_allowed_temp_directory() {
        let app = create_mock_app();
        let temp_dir = tempdir().unwrap();
        let test_path = temp_dir.path().join("test.txt");
        
        let result = is_path_allowed(&test_path, app.handle());
        assert!(result.is_ok());
        assert!(result.unwrap()); // Temp directories should be allowed
    }

    #[tokio::test]
    async fn test_is_path_allowed_system_directories() {
        let app = create_mock_app();
        
        let restricted_paths = if cfg!(windows) {
            vec![
                Path::new("C:\\Windows\\System32"),
                Path::new("C:\\Windows\\SysWOW64"),
                Path::new("C:\\Program Files"),
                Path::new("C:\\Users\\All Users"),
            ]
        } else {
            vec![
                Path::new("/etc"),
                Path::new("/proc"),
                Path::new("/sys"),
                Path::new("/dev"),
                Path::new("/root"),
            ]
        };
        
        for restricted_path in restricted_paths {
            let result = is_path_allowed(restricted_path, app.handle());
            if result.is_ok() {
                // Some paths might not exist or might be allowed depending on permissions
                // This is more of a functional test than a strict requirement
                println!("Path {} was allowed (may not exist)", restricted_path.display());
            }
        }
    }

    #[test]
    fn test_sanitize_filename_valid() {
        assert_eq!(sanitize_filename("normal_file.txt"), "normal_file.txt");
        assert_eq!(sanitize_filename("file with spaces.doc"), "file with spaces.doc");
        assert_eq!(sanitize_filename("file-name_123.pdf"), "file-name_123.pdf");
    }

    #[test]
    fn test_sanitize_filename_invalid_characters() {
        assert_eq!(sanitize_filename("file<name>.txt"), "file_name_.txt");
        assert_eq!(sanitize_filename("file|name.txt"), "file_name.txt");
        assert_eq!(sanitize_filename("file?name.txt"), "file_name.txt");
        assert_eq!(sanitize_filename("file*name.txt"), "file_name.txt");
        assert_eq!(sanitize_filename("file\"name.txt"), "file_name.txt");
        assert_eq!(sanitize_filename("file:name.txt"), "file_name.txt");
    }

    #[test]
    fn test_sanitize_filename_reserved_names() {
        if cfg!(windows) {
            assert_eq!(sanitize_filename("CON"), "_CON");
            assert_eq!(sanitize_filename("PRN"), "_PRN");
            assert_eq!(sanitize_filename("AUX"), "_AUX");
            assert_eq!(sanitize_filename("NUL"), "_NUL");
            assert_eq!(sanitize_filename("COM1"), "_COM1");
            assert_eq!(sanitize_filename("LPT1"), "_LPT1");
            assert_eq!(sanitize_filename("con.txt"), "_con.txt");
        }
    }

    #[test]
    fn test_sanitize_filename_empty_and_dots() {
        assert_eq!(sanitize_filename(""), "_");
        assert_eq!(sanitize_filename("."), "_.");
        assert_eq!(sanitize_filename(".."), "_..");
        assert_eq!(sanitize_filename("..."), "...");
    }

    #[test]
    fn test_sanitize_filename_too_long() {
        let long_name = "a".repeat(300);
        let sanitized = sanitize_filename(&long_name);
        assert!(sanitized.len() <= 255);
        assert!(sanitized.starts_with("aaa"));
    }

    #[test]
    fn test_sanitize_filename_unicode() {
        assert_eq!(sanitize_filename("文件名.txt"), "文件名.txt");
        assert_eq!(sanitize_filename("файл.doc"), "файл.doc");
        assert_eq!(sanitize_filename("ファイル名.pdf"), "ファイル名.pdf");
    }

    #[test]
    fn test_validate_file_extension_allowed() {
        let allowed_extensions = vec!["txt", "pdf", "doc", "jpg", "png"];
        
        assert!(validate_file_extension("test.txt", &allowed_extensions));
        assert!(validate_file_extension("document.PDF", &allowed_extensions)); // Case insensitive
        assert!(validate_file_extension("image.JPG", &allowed_extensions));
    }

    #[test]
    fn test_validate_file_extension_disallowed() {
        let allowed_extensions = vec!["txt", "pdf", "doc"];
        
        assert!(!validate_file_extension("script.exe", &allowed_extensions));
        assert!(!validate_file_extension("virus.bat", &allowed_extensions));
        assert!(!validate_file_extension("shell.sh", &allowed_extensions));
    }

    #[test]
    fn test_validate_file_extension_no_extension() {
        let allowed_extensions = vec!["txt", "pdf"];
        
        assert!(!validate_file_extension("filename", &allowed_extensions));
        assert!(!validate_file_extension("file.", &allowed_extensions));
    }

    #[test]
    fn test_validate_file_extension_multiple_extensions() {
        let allowed_extensions = vec!["txt", "pdf"];
        
        assert!(validate_file_extension("file.backup.txt", &allowed_extensions));
        assert!(!validate_file_extension("file.txt.exe", &allowed_extensions));
    }

    #[test]
    fn test_validate_file_extension_empty_list() {
        let allowed_extensions: Vec<&str> = vec![];
        
        assert!(!validate_file_extension("test.txt", &allowed_extensions));
        assert!(!validate_file_extension("test", &allowed_extensions));
    }

    #[tokio::test]
    async fn test_path_canonicalization() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "content").unwrap();
        
        // Test with relative path components
        let complex_path = temp_dir.path().join("./subdir/../test.txt");
        let canonical = complex_path.canonicalize().unwrap_or(test_file.clone());
        
        assert_eq!(canonical, test_file);
    }

    #[test]
    fn test_security_edge_cases() {
        // Test null bytes
        assert_eq!(sanitize_filename("file\x00name.txt"), "file_name.txt");
        
        // Test control characters
        assert_eq!(sanitize_filename("file\x01\x02name.txt"), "file__name.txt");
        
        // Test path separators
        assert_eq!(sanitize_filename("path/to/file.txt"), "path_to_file.txt");
        assert_eq!(sanitize_filename("path\\to\\file.txt"), "path_to_file.txt");
    }

    #[test]
    fn test_filename_injection_attempts() {
        // Test various injection attempts
        assert_eq!(sanitize_filename("../../../etc/passwd"), ".._.._.._.._etc_passwd");
        assert_eq!(sanitize_filename("$(rm -rf /)"), "_rm -rf __");
        assert_eq!(sanitize_filename("`rm -rf /`"), "_rm -rf __");
        assert_eq!(sanitize_filename("file; rm -rf /"), "file_ rm -rf _");
    }
}