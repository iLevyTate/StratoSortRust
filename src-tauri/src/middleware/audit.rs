// Request Audit Logging Middleware
// Provides comprehensive logging of all API requests for security and compliance

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::AppError;

// Audit log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub command: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub ip_address: Option<String>,
    pub parameters: HashMap<String, serde_json::Value>,
    pub result: AuditResult,
    pub duration_ms: u64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    Success,
    Failure { error: String },
    Denied { reason: String },
}

// Audit logger configuration
#[derive(Debug, Clone)]
pub struct AuditLoggerConfig {
    pub enabled: bool,
    pub log_directory: PathBuf,
    pub max_file_size: u64, // bytes
    pub max_files: usize,
    pub sensitive_commands: Vec<String>, // Commands that require extra logging
    pub exclude_commands: Vec<String>, // Commands to exclude from logging
    pub include_parameters: bool,
    pub anonymize_pii: bool,
}

impl Default for AuditLoggerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_directory: PathBuf::from("./audit_logs"),
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_files: 100,
            sensitive_commands: vec![
                "delete_file".to_string(),
                "move_files".to_string(),
                "update_settings".to_string(),
                "apply_organization".to_string(),
                "shutdown_application".to_string(),
            ],
            exclude_commands: vec![
                "get_system_info".to_string(),
                "check_ollama_status".to_string(),
            ],
            include_parameters: true,
            anonymize_pii: false,
        }
    }
}

// Main audit logger
pub struct AuditLogger {
    config: Arc<AuditLoggerConfig>,
    current_file: Arc<Mutex<Option<PathBuf>>>,
    entries_buffer: Arc<Mutex<Vec<AuditLogEntry>>>,
    buffer_size: usize,
}

impl AuditLogger {
    // Create new audit logger instance
    pub fn new(config: AuditLoggerConfig) -> Result<Self, AppError> {
        // Ensure audit directory exists
        if config.enabled {
            fs::create_dir_all(&config.log_directory).map_err(|e| AppError::SystemError {
                message: format!("Failed to create audit directory: {}", e),
            })?;
        }

        Ok(Self {
            config: Arc::new(config),
            current_file: Arc::new(Mutex::new(None)),
            entries_buffer: Arc::new(Mutex::new(Vec::new())),
            buffer_size: 100, // Buffer 100 entries before writing
        })
    }

    // Log an audit entry
    pub async fn log(&self, entry: AuditLogEntry) -> Result<(), AppError> {
        if !self.config.enabled {
            return Ok(());
        }

        // Check if command should be excluded
        if self.config.exclude_commands.contains(&entry.command) {
            return Ok(());
        }

        // Add to buffer
        let mut buffer = self.entries_buffer.lock().await;
        buffer.push(entry.clone());

        // Check if buffer should be flushed
        if buffer.len() >= self.buffer_size {
            self.flush_buffer().await?;
        }

        // Log sensitive commands immediately
        if self.config.sensitive_commands.contains(&entry.command) {
            self.write_entry(&entry).await?;
        }

        Ok(())
    }

    // Flush buffer to disk
    pub async fn flush_buffer(&self) -> Result<(), AppError> {
        let mut buffer = self.entries_buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let entries = buffer.drain(..).collect::<Vec<_>>();
        drop(buffer); // Release lock early

        for entry in entries {
            self.write_entry(&entry).await?;
        }

        Ok(())
    }

    // Write single entry to file
    async fn write_entry(&self, entry: &AuditLogEntry) -> Result<(), AppError> {
        let file_path = self.get_or_create_file().await?;

        // Serialize entry to JSON
        let mut json = serde_json::to_string(entry).map_err(|e| AppError::SystemError {
            message: format!("Failed to serialize audit entry: {}", e),
        })?;

        // Anonymize PII if configured
        if self.config.anonymize_pii {
            json = self.anonymize_data(json);
        }

        // Write to file with newline
        json.push('\n');

        // Append to file (synchronous write for reliability)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| AppError::SystemError {
                message: format!("Failed to open audit file: {}", e),
            })?;

        file.write_all(json.as_bytes())
            .map_err(|e| AppError::SystemError {
                message: format!("Failed to write audit entry: {}", e),
            })?;

        Ok(())
    }

    // Get current file or create new one if needed
    async fn get_or_create_file(&self) -> Result<PathBuf, AppError> {
        let mut current = self.current_file.lock().await;

        // Check if we need a new file
        let needs_new_file = match &*current {
            None => true,
            Some(path) => {
                let metadata = fs::metadata(path).map_err(|e| AppError::SystemError {
                    message: format!("Failed to get file metadata: {}", e),
                })?;
                metadata.len() >= self.config.max_file_size
            }
        };

        if needs_new_file {
            // Rotate logs if needed
            self.rotate_logs().await?;

            // Create new file with timestamp
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let filename = format!("audit_{}.jsonl", timestamp);
            let file_path = self.config.log_directory.join(filename);

            *current = Some(file_path.clone());
            Ok(file_path)
        } else {
            current.as_ref().cloned().ok_or_else(|| {
                AppError::SystemError {
                    message: "Current audit file path is None".to_string()
                }
            })
        }
    }

    // Rotate old log files
    async fn rotate_logs(&self) -> Result<(), AppError> {
        let mut entries = fs::read_dir(&self.config.log_directory)
            .map_err(|e| AppError::SystemError {
                message: format!("Failed to read audit directory: {}", e),
            })?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| name.starts_with("audit_") && name.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        // Sort by modification time (oldest first)
        entries.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        // Remove old files if we exceed max_files
        while entries.len() >= self.config.max_files {
            if let Some(oldest) = entries.first() {
                fs::remove_file(oldest.path()).map_err(|e| AppError::SystemError {
                    message: format!("Failed to remove old audit file: {}", e),
                })?;
                entries.remove(0);
            } else {
                break;
            }
        }

        Ok(())
    }

    // Anonymize sensitive data
    fn anonymize_data(&self, data: String) -> String {
        // Simple anonymization - replace email-like patterns
        let email_regex = regex::Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b");

        let mut result = data;
        if let Ok(regex) = email_regex {
            result = regex.replace_all(&result, "***@***.***").to_string();
        }

        // Replace IP addresses
        if let Ok(ip_regex) = regex::Regex::new(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b") {
            result = ip_regex.replace_all(&result, "***.***.***").to_string();
        }


        result
    }

    // Query audit logs
    pub async fn query(
        &self,
        filter: AuditQueryFilter,
    ) -> Result<Vec<AuditLogEntry>, AppError> {
        let mut results = Vec::new();

        // Read all audit files
        let entries = fs::read_dir(&self.config.log_directory)
            .map_err(|e| AppError::SystemError {
                message: format!("Failed to read audit directory: {}", e),
            })?;

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !filename.starts_with("audit_") || !filename.ends_with(".jsonl") {
                continue;
            }

            // Read and parse file
            let content = fs::read_to_string(&path).map_err(|e| AppError::SystemError {
                message: format!("Failed to read audit file: {}", e),
            })?;

            for line in content.lines() {
                if line.is_empty() {
                    continue;
                }

                if let Ok(entry) = serde_json::from_str::<AuditLogEntry>(line) {
                    if filter.matches(&entry) {
                        results.push(entry);
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(results)
    }

    // Generate audit report
    pub async fn generate_report(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<AuditReport, AppError> {
        let filter = AuditQueryFilter {
            start_date: Some(start_date),
            end_date: Some(end_date),
            ..Default::default()
        };

        let entries = self.query(filter).await?;

        // Analyze entries
        let mut command_counts = HashMap::new();
        let mut user_activity = HashMap::new();
        let mut error_count = 0;
        let mut total_duration = 0u64;

        for entry in &entries {
            // Count commands
            *command_counts.entry(entry.command.clone()).or_insert(0) += 1;

            // Count user activity
            if let Some(user) = &entry.user_id {
                *user_activity.entry(user.clone()).or_insert(0) += 1;
            }

            // Count errors
            if matches!(entry.result, AuditResult::Failure { .. }) {
                error_count += 1;
            }

            total_duration += entry.duration_ms;
        }

        Ok(AuditReport {
            start_date,
            end_date,
            total_requests: entries.len(),
            error_count,
            average_duration_ms: if entries.is_empty() {
                0
            } else {
                total_duration / entries.len() as u64
            },
            command_counts,
            user_activity,
            sensitive_operations: entries
                .iter()
                .filter(|e| self.config.sensitive_commands.contains(&e.command))
                .count(),
        })
    }
}

// Query filter for audit logs
#[derive(Debug, Default)]
pub struct AuditQueryFilter {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub command: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub success_only: bool,
}

impl AuditQueryFilter {
    fn matches(&self, entry: &AuditLogEntry) -> bool {
        // Check date range
        if let Some(start) = &self.start_date {
            if entry.timestamp < *start {
                return false;
            }
        }

        if let Some(end) = &self.end_date {
            if entry.timestamp > *end {
                return false;
            }
        }

        // Check command
        if let Some(cmd) = &self.command {
            if entry.command != *cmd {
                return false;
            }
        }

        // Check user
        if let Some(user) = &self.user_id {
            if entry.user_id.as_ref() != Some(user) {
                return false;
            }
        }

        // Check session
        if let Some(session) = &self.session_id {
            if entry.session_id.as_ref() != Some(session) {
                return false;
            }
        }

        // Check success
        if self.success_only && !matches!(entry.result, AuditResult::Success) {
            return false;
        }

        true
    }
}

// Audit report structure
#[derive(Debug, Serialize)]
pub struct AuditReport {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub total_requests: usize,
    pub error_count: usize,
    pub average_duration_ms: u64,
    pub command_counts: HashMap<String, usize>,
    pub user_activity: HashMap<String, usize>,
    pub sensitive_operations: usize,
}

// Helper to create audit entry from command invocation
pub fn create_audit_entry(
    command: &str,
    parameters: HashMap<String, serde_json::Value>,
    user_id: Option<String>,
    session_id: Option<String>,
) -> AuditLogEntry {
    AuditLogEntry {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: Utc::now(),
        command: command.to_string(),
        user_id,
        session_id,
        ip_address: None, // Would be set by the server
        parameters,
        result: AuditResult::Success, // To be updated after execution
        duration_ms: 0, // To be updated after execution
        metadata: HashMap::new(),
    }
}