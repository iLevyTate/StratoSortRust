use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use std::sync::Arc;

/// Security event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEventType {
    // Authentication events
    AuthenticationFailed { reason: String },
    AuthenticationSucceeded,

    // Authorization events
    PermissionDenied { resource: String, action: String },
    PermissionGranted { resource: String, action: String },

    // Path security events
    PathTraversalAttempt { path: String },
    InvalidPathAccess { path: String },

    // Rate limiting events
    RateLimitExceeded { endpoint: String, client: String },
    RateLimitWarning { endpoint: String, client: String, percentage: u8 },

    // Input validation events
    SqlInjectionAttempt { query: String },
    CommandInjectionAttempt { command: String },
    InvalidInputBlocked { field: String, reason: String },

    // File operations
    SensitiveFileAccess { path: String },
    UnauthorizedFileOperation { operation: String, path: String },

    // System events
    ServiceStarted { service: String },
    ServiceStopped { service: String },
    ConfigurationChanged { setting: String },

    // AI/LLM events
    AiServiceFailure { reason: String },
    EmbeddingGenerationFailed { file: String },

    // Database events
    DatabaseConnectionFailed,
    DatabaseBackupCreated { path: String },

    // Anomaly detection
    AnomalyDetected { description: String },
    SuspiciousActivity { description: String },
}

/// Severity levels for audit events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier for the event
    pub id: String,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: SecurityEventType,
    /// Severity level
    pub severity: Severity,
    /// User or client identifier
    pub client_id: Option<String>,
    /// IP address if available
    pub ip_address: Option<String>,
    /// User agent if available
    pub user_agent: Option<String>,
    /// Additional context
    pub context: Option<serde_json::Value>,
    /// Session ID for correlation
    pub session_id: Option<String>,
    /// Process ID
    pub process_id: u32,
}

impl AuditEntry {
    pub fn new(event_type: SecurityEventType, severity: Severity) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type,
            severity,
            client_id: None,
            ip_address: None,
            user_agent: None,
            context: None,
            session_id: None,
            process_id: std::process::id(),
        }
    }

    pub fn with_client(mut self, client_id: String) -> Self {
        self.client_id = Some(client_id);
        self
    }

    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }
}

/// Audit logger configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Directory for audit logs
    pub log_dir: PathBuf,
    /// Maximum log file size in bytes
    pub max_file_size: u64,
    /// Number of log files to retain
    pub max_files: usize,
    /// Whether to log to console as well
    pub console_output: bool,
    /// Minimum severity to log
    pub min_severity: Severity,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("./audit_logs"),
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_files: 100,
            console_output: true,
            min_severity: Severity::Low,
        }
    }
}

/// Audit logger service
pub struct AuditLogger {
    config: Arc<RwLock<AuditConfig>>,
    current_file: Arc<RwLock<Option<PathBuf>>>,
    alerts: Arc<RwLock<Vec<Box<dyn Fn(&AuditEntry) + Send + Sync>>>>,
}

impl AuditLogger {
    pub async fn new(config: AuditConfig) -> Result<Self> {
        // Ensure log directory exists
        fs::create_dir_all(&config.log_dir).await?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            current_file: Arc::new(RwLock::new(None)),
            alerts: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Log an audit entry
    pub async fn log(&self, entry: AuditEntry) -> Result<()> {
        let config = self.config.read().await;

        // Check severity threshold
        if !self.should_log(&entry.severity, &config.min_severity) {
            return Ok(());
        }

        // Log to console if enabled
        if config.console_output {
            self.log_to_console(&entry);
        }

        // Log to file
        self.log_to_file(&entry, &config).await?;

        // Trigger alerts for critical events
        if matches!(entry.severity, Severity::Critical | Severity::High) {
            self.trigger_alerts(&entry).await;
        }

        Ok(())
    }

    /// Log a security event
    pub async fn log_security_event(
        &self,
        event_type: SecurityEventType,
        severity: Severity,
        client_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut entry = AuditEntry::new(event_type, severity);

        if let Some(client) = client_id {
            entry = entry.with_client(client);
        }

        if let Some(ctx) = context {
            entry = entry.with_context(ctx);
        }

        self.log(entry).await
    }

    /// Check if event should be logged based on severity
    fn should_log(&self, event_severity: &Severity, min_severity: &Severity) -> bool {
        match min_severity {
            Severity::Critical => matches!(event_severity, Severity::Critical),
            Severity::High => matches!(event_severity, Severity::Critical | Severity::High),
            Severity::Medium => matches!(
                event_severity,
                Severity::Critical | Severity::High | Severity::Medium
            ),
            Severity::Low => !matches!(event_severity, Severity::Info),
            Severity::Info => true,
        }
    }

    /// Log to console
    fn log_to_console(&self, entry: &AuditEntry) {
        let message = format!(
            "[AUDIT] {} - {:?} - {:?}: {:?}",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            entry.severity,
            entry.event_type,
            entry.client_id.as_ref().unwrap_or(&"system".to_string())
        );

        match entry.severity {
            Severity::Critical | Severity::High => error!("{}", message),
            Severity::Medium => warn!("{}", message),
            Severity::Low | Severity::Info => info!("{}", message),
        }
    }

    /// Log to file
    async fn log_to_file(&self, entry: &AuditEntry, config: &AuditConfig) -> Result<()> {
        // Get or create current log file
        let file_path = self.get_or_create_log_file(&config.log_dir).await?;

        // Check file size and rotate if necessary
        if let Ok(metadata) = fs::metadata(&file_path).await {
            if metadata.len() >= config.max_file_size {
                self.rotate_log_file(&config.log_dir, config.max_files).await?;
            }
        }

        // Write entry as JSON line
        let json = serde_json::to_string(&entry)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await?;

        file.write_all(format!("{}\n", json).as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// Get or create current log file
    async fn get_or_create_log_file(&self, log_dir: &Path) -> Result<PathBuf> {
        let mut current = self.current_file.write().await;

        if current.is_none() {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let filename = format!("audit_{}.jsonl", timestamp);
            let path = log_dir.join(filename);
            *current = Some(path.clone());
            return Ok(path);
        }

        Ok(current.as_ref().unwrap().clone())
    }

    /// Rotate log files
    async fn rotate_log_file(&self, log_dir: &Path, max_files: usize) -> Result<()> {
        let mut current = self.current_file.write().await;

        // Create new file
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("audit_{}.jsonl", timestamp);
        let new_path = log_dir.join(filename);
        *current = Some(new_path);

        // Clean up old files if exceeded max
        self.cleanup_old_files(log_dir, max_files).await?;

        Ok(())
    }

    /// Clean up old log files
    async fn cleanup_old_files(&self, log_dir: &Path, max_files: usize) -> Result<()> {
        let mut entries = fs::read_dir(log_dir).await?;
        let mut files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("audit_") && name.ends_with(".jsonl") {
                    files.push(entry.path());
                }
            }
        }

        // Sort by modification time
        files.sort_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        // Remove oldest files
        while files.len() > max_files {
            if let Some(old_file) = files.first() {
                let _ = fs::remove_file(old_file).await;
                files.remove(0);
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Trigger alerts for critical events
    async fn trigger_alerts(&self, entry: &AuditEntry) {
        let alerts = self.alerts.read().await;
        for alert in alerts.iter() {
            alert(entry);
        }
    }

    /// Register an alert handler
    pub async fn register_alert_handler<F>(&self, handler: F)
    where
        F: Fn(&AuditEntry) + Send + Sync + 'static,
    {
        let mut alerts = self.alerts.write().await;
        alerts.push(Box::new(handler));
    }

    /// Search audit logs
    pub async fn search(
        &self,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        event_types: Option<Vec<SecurityEventType>>,
        severity: Option<Severity>,
        client_id: Option<String>,
    ) -> Result<Vec<AuditEntry>> {
        let config = self.config.read().await;
        let mut results = Vec::new();

        let mut entries = fs::read_dir(&config.log_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("audit_") && name.ends_with(".jsonl") {
                    let content = fs::read_to_string(entry.path()).await?;
                    for line in content.lines() {
                        if let Ok(entry) = serde_json::from_str::<AuditEntry>(line) {
                            // Apply filters
                            if let Some(start) = start_time {
                                if entry.timestamp < start {
                                    continue;
                                }
                            }

                            if let Some(end) = end_time {
                                if entry.timestamp > end {
                                    continue;
                                }
                            }

                            if let Some(client) = &client_id {
                                if entry.client_id.as_ref() != Some(client) {
                                    continue;
                                }
                            }

                            if let Some(sev) = &severity {
                                if !self.should_log(sev, &entry.severity) {
                                    continue;
                                }
                            }

                            results.push(entry);
                        }
                    }
                }
            }
        }

        results.sort_by_key(|e| e.timestamp);
        Ok(results)
    }

    /// Get statistics
    pub async fn get_stats(&self) -> Result<AuditStats> {
        let config = self.config.read().await;
        let mut total_entries = 0;
        let mut total_size = 0;
        let mut file_count = 0;

        let mut entries = fs::read_dir(&config.log_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("audit_") && name.ends_with(".jsonl") {
                    file_count += 1;
                    if let Ok(metadata) = entry.metadata().await {
                        total_size += metadata.len();

                        // Count lines
                        if let Ok(content) = fs::read_to_string(entry.path()).await {
                            total_entries += content.lines().count();
                        }
                    }
                }
            }
        }

        Ok(AuditStats {
            total_entries,
            total_size,
            file_count,
            oldest_entry: None, // Could be populated by reading first entry
            newest_entry: None, // Could be populated by reading last entry
        })
    }
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub total_entries: usize,
    pub total_size: u64,
    pub file_count: usize,
    pub oldest_entry: Option<DateTime<Utc>>,
    pub newest_entry: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_logging() {
        let config = AuditConfig {
            log_dir: PathBuf::from("./test_audit_logs"),
            console_output: false,
            ..Default::default()
        };

        let logger = AuditLogger::new(config).await.unwrap();

        // Log various events
        logger.log_security_event(
            SecurityEventType::PathTraversalAttempt {
                path: "/etc/passwd".to_string(),
            },
            Severity::High,
            Some("test_client".to_string()),
            None,
        ).await.unwrap();

        logger.log_security_event(
            SecurityEventType::RateLimitExceeded {
                endpoint: "delete_file".to_string(),
                client: "abusive_client".to_string(),
            },
            Severity::Medium,
            Some("abusive_client".to_string()),
            None,
        ).await.unwrap();

        // Search logs
        let results = logger.search(None, None, None, None, Some("test_client".to_string()))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);

        // Cleanup
        let _ = tokio::fs::remove_dir_all("./test_audit_logs").await;
    }
}