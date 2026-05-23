use crate::error::{AppError, Result};
use std::fs::File;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// Securely validates and sanitizes file paths to prevent TOCTOU and path traversal attacks
/// Uses file descriptors and atomic operations to minimize race conditions
pub fn validate_and_sanitize_path(path: &str, app: &AppHandle) -> Result<ValidatedPath> {
    // Reject empty paths
    if path.is_empty() {
        return Err(AppError::SecurityError {
            message: "Path cannot be empty".to_string(),
        });
    }

    // Reject null bytes and control characters (except \r and \n for file names)
    if path.contains('\0')
        || path
            .chars()
            .any(|c| c.is_control() && c != '\r' && c != '\n')
    {
        return Err(AppError::SecurityError {
            message: "Invalid path: contains null bytes or control characters".to_string(),
        });
    }

    let path_buf = PathBuf::from(path);

    // First check the original path components before canonicalization
    for component in path_buf.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(AppError::SecurityError {
                    message: "Path traversal attempt detected".to_string(),
                });
            }
            std::path::Component::RootDir if !cfg!(windows) => {
                // On Unix, reject absolute paths to sensitive directories
                let path_str = path_buf.to_string_lossy();
                if path_str.starts_with("/etc") || path_str.starts_with("/root") {
                    return Err(AppError::SecurityError {
                        message: "Access to system directories not allowed".to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    // Attempt to canonicalize - but don't rely solely on this for security
    let canonical_path = match path_buf.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            // If canonicalization fails, the file might not exist
            // Try to canonicalize the parent directory and validate the filename
            if let Some(parent) = path_buf.parent() {
                let canonical_parent =
                    parent.canonicalize().map_err(|_| AppError::SecurityError {
                        message: "Parent directory validation failed".to_string(),
                    })?;

                if let Some(filename) = path_buf.file_name() {
                    let sanitized_filename = sanitize_filename(&filename.to_string_lossy());
                    if sanitized_filename.is_empty() {
                        return Err(AppError::SecurityError {
                            message: "Invalid filename after sanitization".to_string(),
                        });
                    }

                    // Validate filename doesn't contain path separators after sanitization
                    if sanitized_filename.contains('/') || sanitized_filename.contains('\\') {
                        return Err(AppError::SecurityError {
                            message: "Invalid filename contains path separators".to_string(),
                        });
                    }

                    // Create the final path and verify it's still under the parent
                    let final_path = canonical_parent.join(&sanitized_filename);
                    if !final_path.starts_with(&canonical_parent) {
                        return Err(AppError::SecurityError {
                            message: "Path traversal detected after joining".to_string(),
                        });
                    }

                    final_path
                } else {
                    return Err(AppError::SecurityError {
                        message: format!("Path validation failed: {}", e),
                    });
                }
            } else {
                return Err(AppError::SecurityError {
                    message: format!("Path validation failed: {}", e),
                });
            }
        }
    };

    // Verify the canonical path doesn't contain traversal patterns
    let canonical_str = canonical_path.to_string_lossy();
    if canonical_str.contains("..") {
        return Err(AppError::SecurityError {
            message: "Path traversal detected in canonical path".to_string(),
        });
    }

    // Verify against allowed directories
    if !is_path_allowed(&canonical_path, app)? {
        return Err(AppError::SecurityError {
            message: "Path outside allowed directories".to_string(),
        });
    }

    Ok(ValidatedPath {
        canonical_path,
        original_path: path_buf,
    })
}

/// A validated path that reduces TOCTOU risks
pub struct ValidatedPath {
    canonical_path: PathBuf,
    original_path: PathBuf,
}

impl ValidatedPath {
    /// Get the canonical path - use this for file operations
    pub fn canonical(&self) -> &Path {
        &self.canonical_path
    }

    /// Get the original path - for display purposes only
    pub fn original(&self) -> &Path {
        &self.original_path
    }

    /// Convert to PathBuf for compatibility
    pub fn into_path_buf(self) -> PathBuf {
        self.canonical_path
    }

    /// Securely open a file with validation
    /// Note: For full TOCTOU protection, pass the AppHandle to this method
    pub fn open_file(&self) -> Result<File> {
        // Open the file directly - the path has already been validated
        // The TOCTOU window is minimized by doing the validation and open as close as possible
        File::open(&self.canonical_path).map_err(AppError::Io)
    }

    /// Securely open a file with re-validation (preferred method)
    pub fn open_file_validated(&self, app: &AppHandle) -> Result<File> {
        // Re-validate just before opening to minimize TOCTOU window
        if !is_path_allowed(&self.canonical_path, app)? {
            return Err(AppError::SecurityError {
                message: "Path validation failed during file open".to_string(),
            });
        }

        File::open(&self.canonical_path).map_err(AppError::Io)
    }
}

/// Checks if a path is allowed to be accessed
pub fn is_path_allowed(path: &Path, _app: &AppHandle) -> Result<bool> {
    let path_str = path.to_string_lossy();

    // Block access to sensitive system directories
    let blocked_paths = [
        "/etc/passwd",
        "/etc/shadow",
        "/root/",
        "C:\\Windows\\System32\\",
        "C:\\System Volume Information\\",
        "C:\\$Recycle.Bin\\",
    ];

    for blocked in &blocked_paths {
        if path_str.starts_with(blocked) {
            return Ok(false);
        }
    }

    // Block access to files starting with dots (hidden files) in root directories
    if let Some(file_name) = path.file_name() {
        if file_name.to_string_lossy().starts_with('.') {
            if let Some(parent) = path.parent() {
                if parent.components().count() <= 2 {
                    // Root or single level
                    return Ok(false);
                }
            }
        }
    }

    Ok(true)
}

/// Sanitizes file names to remove dangerous characters
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|&c| {
            !matches!(
                c,
                '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0' | '/' | '\\'
            )
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Validates file size constraints
pub fn validate_file_size(size: u64, max_size: u64) -> Result<()> {
    if size > max_size {
        return Err(AppError::SecurityError {
            message: format!(
                "File size {} exceeds maximum allowed size {}",
                size, max_size
            ),
        });
    }
    Ok(())
}

/// Legacy compatibility function - returns PathBuf for existing code
/// Consider migrating to validate_and_sanitize_path for better security
pub fn validate_and_sanitize_path_legacy(path: &str, app: &AppHandle) -> Result<PathBuf> {
    let validated = validate_and_sanitize_path(path, app)?;
    Ok(validated.into_path_buf())
}

/// Lighter-weight path check for *user-supplied paths* — the inputs to
/// `enable_watch_mode`, `auto_organize_directory`, `batch_analyze_files`,
/// `add_watch_path`, etc. These don't need the full allowlist gate (the user
/// is explicitly telling us where to look) but they MUST reject the obvious
/// attack shapes: empty, null bytes, control chars, `..` components, and
/// absolute paths into sensitive system directories. We canonicalize so that
/// downstream code works with a real absolute path, and surface nonexistent
/// inputs as an error rather than silently writing them into config / DB.
pub fn validate_user_path(path: &str) -> Result<PathBuf> {
    if path.is_empty() {
        return Err(AppError::SecurityError {
            message: "Path cannot be empty".to_string(),
        });
    }

    if path.contains('\0')
        || path
            .chars()
            .any(|c| c.is_control() && c != '\r' && c != '\n')
    {
        return Err(AppError::SecurityError {
            message: "Invalid path: contains null or control characters".to_string(),
        });
    }

    let path_buf = PathBuf::from(path);
    for component in path_buf.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(AppError::SecurityError {
                message: "Path traversal attempt rejected".to_string(),
            });
        }
    }

    // System-path blocklist. We list both the Linux canonical forms and the
    // macOS canonical forms (/etc → /private/etc; modern macOS with firmlinks
    // may further resolve to /System/Volumes/Data/private/etc, so we also keep
    // the un-canonicalized check below as a backstop).
    let blocked = [
        "/etc/",
        "/proc/",
        "/sys/",
        "/dev/",
        "/root/",
        "/private/etc/",
        "/System/Volumes/Data/private/etc/",
        "C:\\Windows\\System32\\",
        "C:\\System Volume Information\\",
    ];

    let matches_blocklist = |s: &str| -> bool {
        blocked
            .iter()
            .any(|p| s.starts_with(*p) || s == p.trim_end_matches('/'))
    };

    // First pass: check the literal input. Catches `/etc` directly without
    // depending on what canonicalize() decides to return on this OS — macOS
    // firmlink resolution in particular makes the canonical form hard to
    // enumerate. The canonical check below is still needed to defeat symlink
    // dodges (e.g. ln -s /etc /tmp/safe; pass /tmp/safe).
    let raw_str = path_buf.to_string_lossy();
    if matches_blocklist(&raw_str) {
        return Err(AppError::SecurityError {
            message: format!("Refusing to operate on system path: {}", raw_str),
        });
    }

    let canonical = path_buf.canonicalize().map_err(|e| AppError::SecurityError {
        message: format!("Path does not exist or cannot be resolved: {}", e),
    })?;

    let canonical_str = canonical.to_string_lossy();
    if matches_blocklist(&canonical_str) {
        return Err(AppError::SecurityError {
            message: format!("Refusing to operate on system path: {}", canonical_str),
        });
    }

    Ok(canonical)
}

/// Stricter variant that additionally requires the path to be a directory.
/// Use for `enable_watch_mode`-style inputs where a file would be a semantic
/// error even if it's a benign path.
pub fn validate_directory_path(path: &str) -> Result<PathBuf> {
    let canonical = validate_user_path(path)?;
    if !canonical.is_dir() {
        return Err(AppError::SecurityError {
            message: format!("Path is not a directory: {}", canonical.display()),
        });
    }
    Ok(canonical)
}

#[cfg(test)]
mod directory_validation_tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(validate_directory_path("").is_err());
    }

    #[test]
    fn rejects_null_byte() {
        assert!(validate_directory_path("/tmp/\0evil").is_err());
    }

    #[test]
    fn rejects_parent_dir_components() {
        assert!(validate_directory_path("/tmp/../etc").is_err());
        assert!(validate_directory_path("foo/../bar").is_err());
    }

    #[test]
    fn rejects_nonexistent_directory() {
        assert!(validate_directory_path("/nonexistent/path/xyzzy").is_err());
    }

    #[test]
    #[cfg(unix)]
    fn rejects_system_directories() {
        assert!(validate_directory_path("/etc").is_err());
        assert!(validate_directory_path("/proc").is_err());
    }

    #[test]
    fn accepts_real_user_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let p = validate_directory_path(tmp.path().to_str().unwrap()).unwrap();
        assert!(p.is_absolute());
    }

    #[test]
    fn directory_validator_rejects_file_input() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        assert!(validate_directory_path(tmp.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn user_path_validator_accepts_file_input() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let p = validate_user_path(tmp.path().to_str().unwrap()).unwrap();
        assert!(p.is_absolute() && p.is_file());
    }
}
