use crate::error::{AppError, Result};
use regex::Regex;
use std::path::PathBuf;

/// Input validator for user-provided data
pub struct InputValidator {
    path_regex: Regex,
    sql_injection_regex: Regex,
    command_injection_regex: Regex,
    max_string_length: usize,
    max_array_length: usize,
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl InputValidator {
    pub fn new() -> Self {
        Self {
            // Match suspicious path traversal patterns
            path_regex: Regex::new(r"(\.\.[/\\])|([/\\]\.\.)|(^\.\.$)")
                .expect("Failed to compile path traversal regex"),
            // Match common SQL injection patterns
            sql_injection_regex: Regex::new(r"(?i)(union\s+select|delete\s+from|drop\s+table|insert\s+into|update\s+set|exec\s*\(|execute\s*\(|script\s*>|<\s*script)")
                .expect("Failed to compile SQL injection regex"),
            // Match command injection patterns
            command_injection_regex: Regex::new(r"[;&|`$]|\$\(|\beval\b|\bexec\b")
                .expect("Failed to compile command injection regex"),
            max_string_length: 10000,
            max_array_length: 1000,
        }
    }

    /// Validate and sanitize a file path
    pub fn validate_path(&self, path: &str) -> Result<PathBuf> {
        // Check length
        if path.is_empty() {
            return Err(AppError::InvalidInput {
                message: "Path cannot be empty".to_string(),
            });
        }

        if path.len() > 4096 {
            return Err(AppError::InvalidInput {
                message: "Path exceeds maximum length".to_string(),
            });
        }

        // Check for null bytes
        if path.contains('\0') {
            return Err(AppError::SecurityError {
                message: "Invalid path: contains null bytes".to_string(),
            });
        }

        // Check for path traversal attempts
        if self.path_regex.is_match(path) {
            return Err(AppError::SecurityError {
                message: "Path traversal attempt detected".to_string(),
            });
        }

        let path_buf = PathBuf::from(path);

        // Additional path security checks
        if path_buf.components().any(|c| {
            matches!(c, std::path::Component::RootDir) && path_buf.components().count() == 1
        }) {
            return Err(AppError::SecurityError {
                message: "Access to root directory not allowed".to_string(),
            });
        }

        // Check for suspicious file extensions in executable contexts
        let suspicious_extensions = ["exe", "bat", "cmd", "sh", "ps1", "vbs", "js", "jar"];
        if let Some(ext) = path_buf.extension() {
            if let Some(ext_str) = ext.to_str() {
                if suspicious_extensions.contains(&ext_str.to_lowercase().as_str()) {
                    // Log but don't reject - may be legitimate
                    tracing::warn!("Suspicious file extension detected: {}", ext_str);
                }
            }
        }

        // Normalize the path to prevent bypasses
        match path_buf.canonicalize() {
            Ok(canonical) => Ok(canonical),
            Err(_) => {
                // Path doesn't exist yet, validate parent
                if let Some(parent) = path_buf.parent() {
                    if parent.exists() {
                        // Parent exists, path is probably valid
                        Ok(path_buf)
                    } else {
                        Err(AppError::InvalidPath {
                            message: "Parent directory does not exist".to_string(),
                        })
                    }
                } else {
                    Ok(path_buf)
                }
            }
        }
    }

    /// Validate multiple paths
    pub fn validate_paths(&self, paths: &[String]) -> Result<Vec<PathBuf>> {
        if paths.len() > self.max_array_length {
            return Err(AppError::InvalidInput {
                message: format!("Too many paths: {} (max {})", paths.len(), self.max_array_length),
            });
        }

        paths.iter()
            .map(|p| self.validate_path(p))
            .collect::<Result<Vec<_>>>()
    }

    /// Validate a search query or text input
    pub fn validate_text_input(&self, input: &str, field_name: &str) -> Result<String> {
        // Check length
        if input.len() > self.max_string_length {
            return Err(AppError::InvalidInput {
                message: format!("{} exceeds maximum length of {} characters", field_name, self.max_string_length),
            });
        }

        // Check for SQL injection attempts
        if self.sql_injection_regex.is_match(input) {
            return Err(AppError::SecurityError {
                message: format!("Suspicious pattern detected in {}", field_name),
            });
        }

        // Check for command injection attempts
        if self.command_injection_regex.is_match(input) {
            return Err(AppError::SecurityError {
                message: format!("Invalid characters in {}", field_name),
            });
        }

        // Check for null bytes
        if input.contains('\0') {
            return Err(AppError::SecurityError {
                message: format!("{} contains invalid characters", field_name),
            });
        }

        Ok(input.to_string())
    }

    /// Validate a file name (more restrictive than paths)
    pub fn validate_filename(&self, filename: &str) -> Result<String> {
        if filename.is_empty() {
            return Err(AppError::InvalidInput {
                message: "Filename cannot be empty".to_string(),
            });
        }

        if filename.len() > 255 {
            return Err(AppError::InvalidInput {
                message: "Filename exceeds maximum length of 255 characters".to_string(),
            });
        }

        // Check for invalid characters in filenames
        let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', '\0'];
        if filename.chars().any(|c| invalid_chars.contains(&c)) {
            return Err(AppError::InvalidInput {
                message: "Filename contains invalid characters".to_string(),
            });
        }

        // Check for reserved names on Windows
        let reserved_names = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4",
            "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3",
            "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ];

        let name_upper = filename.to_uppercase();
        let base_name = name_upper.split('.').next().unwrap_or(&name_upper);

        if reserved_names.contains(&base_name) {
            return Err(AppError::InvalidInput {
                message: "Filename uses a reserved system name".to_string(),
            });
        }

        Ok(filename.to_string())
    }

    /// Validate JSON input size
    pub fn validate_json_size(&self, json_str: &str) -> Result<()> {
        const MAX_JSON_SIZE: usize = 10 * 1024 * 1024; // 10MB

        if json_str.len() > MAX_JSON_SIZE {
            return Err(AppError::InvalidInput {
                message: format!("JSON payload too large: {} bytes (max {} bytes)",
                    json_str.len(), MAX_JSON_SIZE),
            });
        }

        Ok(())
    }

    /// Validate array size
    pub fn validate_array_size<T>(&self, array: &[T], name: &str) -> Result<()> {
        if array.len() > self.max_array_length {
            return Err(AppError::InvalidInput {
                message: format!("{} has too many items: {} (max {})",
                    name, array.len(), self.max_array_length),
            });
        }

        Ok(())
    }

    /// Validate numeric input is within reasonable bounds
    pub fn validate_number_range(&self, value: i64, min: i64, max: i64, field_name: &str) -> Result<i64> {
        if value < min || value > max {
            return Err(AppError::InvalidInput {
                message: format!("{} must be between {} and {}", field_name, min, max),
            });
        }

        Ok(value)
    }

    /// Validate percentage value (0.0 - 100.0)
    pub fn validate_percentage(&self, value: f32, field_name: &str) -> Result<f32> {
        if !value.is_finite() {
            return Err(AppError::InvalidInput {
                message: format!("{} must be a valid number", field_name),
            });
        }

        if !(0.0..=100.0).contains(&value) {
            return Err(AppError::InvalidInput {
                message: format!("{} must be between 0 and 100", field_name),
            });
        }

        Ok(value)
    }

    /// Sanitize HTML content to prevent XSS
    pub fn sanitize_html(&self, html: &str) -> String {
        // Basic HTML sanitization - remove script tags and event handlers
        let mut sanitized = html.to_string();

        // Remove script tags
        let script_regex = Regex::new(r"(?i)<script[^>]*>.*?</script>")
            .expect("Failed to compile script regex");
        sanitized = script_regex.replace_all(&sanitized, "").to_string();

        // Remove event handlers
        let event_regex = Regex::new(r#"(?i)\s*on\w+\s*=\s*["'][^"']*["']"#)
            .expect("Failed to compile event handler regex");
        sanitized = event_regex.replace_all(&sanitized, "").to_string();

        // Remove javascript: protocol
        let js_protocol_regex = Regex::new(r"(?i)javascript\s*:")
            .expect("Failed to compile javascript protocol regex");
        sanitized = js_protocol_regex.replace_all(&sanitized, "").to_string();

        sanitized
    }
}

/// Macro for easy validation in command handlers
#[macro_export]
macro_rules! validate_input {
    ($validator:expr, path: $path:expr) => {
        $validator.validate_path($path)?
    };
    ($validator:expr, paths: $paths:expr) => {
        $validator.validate_paths($paths)?
    };
    ($validator:expr, text: $text:expr, $field:literal) => {
        $validator.validate_text_input($text, $field)?
    };
    ($validator:expr, filename: $filename:expr) => {
        $validator.validate_filename($filename)?
    };
    ($validator:expr, array: $array:expr, $name:literal) => {{
        $validator.validate_array_size($array, $name)?;
        $array
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_validation() {
        use std::env;
        let validator = InputValidator::new();

        // Valid paths - using temp directory which should exist
        let temp_file = env::temp_dir().join("test.txt");
        assert!(validator.validate_path(temp_file.to_str().unwrap()).is_ok());

        // Also test simple relative paths
        assert!(validator.validate_path("src").is_ok()); // src directory should exist
        assert!(validator.validate_path("Cargo.toml").is_ok()); // Cargo.toml should exist

        // Invalid paths
        assert!(validator.validate_path("../../../etc/passwd").is_err());
        assert!(validator.validate_path("/home/../../../etc/passwd").is_err());
        assert!(validator.validate_path("C:\\..\\..\\Windows\\System32").is_err());
        assert!(validator.validate_path("file\0.txt").is_err());
        assert!(validator.validate_path("").is_err());
    }

    #[test]
    fn test_sql_injection_detection() {
        let validator = InputValidator::new();

        // Clean inputs
        assert!(validator.validate_text_input("normal search query", "search").is_ok());
        assert!(validator.validate_text_input("user@example.com", "email").is_ok());

        // SQL injection attempts (matching the regex patterns)
        assert!(validator.validate_text_input("'; DROP TABLE users; --", "search").is_err());
        assert!(validator.validate_text_input("1' UNION SELECT * FROM passwords", "id").is_err());
        assert!(validator.validate_text_input("admin' OR '1'='1", "username").is_ok()); // OR alone is not blocked
    }

    #[test]
    fn test_filename_validation() {
        let validator = InputValidator::new();

        // Valid filenames
        assert!(validator.validate_filename("document.pdf").is_ok());
        assert!(validator.validate_filename("my-file_123.txt").is_ok());

        // Invalid filenames
        assert!(validator.validate_filename("file/with/slash.txt").is_err());
        assert!(validator.validate_filename("file:with:colon.txt").is_err());
        assert!(validator.validate_filename("CON.txt").is_err()); // Windows reserved
        assert!(validator.validate_filename("").is_err());
    }

    #[test]
    fn test_html_sanitization() {
        let validator = InputValidator::new();

        let dirty_html = r#"<div onclick="alert('xss')">Hello <script>alert('xss')</script> World</div>"#;
        let clean = validator.sanitize_html(dirty_html);

        assert!(!clean.contains("<script"));
        assert!(!clean.contains("onclick"));
        assert!(clean.contains("Hello"));
        assert!(clean.contains("World"));
    }
}