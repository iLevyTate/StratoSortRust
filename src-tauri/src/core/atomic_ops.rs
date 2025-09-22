use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Represents a file operation type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpType {
    Move,
    Copy,
    Rename,
    Delete,
    Create,
}

/// Represents a single file operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOp {
    pub id: String,
    pub op_type: OpType,
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub backup_path: Option<PathBuf>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Represents a rollback operation
#[derive(Debug, Clone)]
struct RollbackOp {
    pub op_type: OpType,
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub backup_path: Option<PathBuf>,
}

/// Transaction state
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionState {
    Pending,
    InProgress,
    Committed,
    RolledBack,
    Failed,
}

/// Atomic file operation manager with transaction support
pub struct AtomicFileOperation {
    id: String,
    operations: Vec<FileOp>,
    rollback_stack: Vec<RollbackOp>,
    backup_dir: PathBuf,
    state: TransactionState,
}

impl AtomicFileOperation {
    /// Create a new atomic file operation transaction
    pub fn new() -> Result<Self> {
        let id = Uuid::new_v4().to_string();
        let backup_dir = std::env::temp_dir()
            .join("stratosort_backup")
            .join(&id);

        // Create backup directory
        std::fs::create_dir_all(&backup_dir).map_err(|e| AppError::ProcessingError {
            message: format!("Failed to create backup directory: {}", e),
        })?;

        Ok(Self {
            id,
            operations: Vec::new(),
            rollback_stack: Vec::new(),
            backup_dir,
            state: TransactionState::Pending,
        })
    }

    /// Add an operation to the transaction
    pub fn add_operation(&mut self, op: FileOp) -> Result<()> {
        if self.state != TransactionState::Pending {
            return Err(AppError::InvalidInput {
                message: "Cannot add operations to a transaction that has started".to_string(),
            });
        }

        self.operations.push(op);
        Ok(())
    }

    /// Add a move operation
    pub fn add_move(&mut self, source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<()> {
        self.add_operation(FileOp {
            id: Uuid::new_v4().to_string(),
            op_type: OpType::Move,
            source: source.as_ref().to_path_buf(),
            destination: Some(destination.as_ref().to_path_buf()),
            backup_path: None,
            metadata: None,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Add a copy operation
    pub fn add_copy(&mut self, source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<()> {
        self.add_operation(FileOp {
            id: Uuid::new_v4().to_string(),
            op_type: OpType::Copy,
            source: source.as_ref().to_path_buf(),
            destination: Some(destination.as_ref().to_path_buf()),
            backup_path: None,
            metadata: None,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Add a delete operation
    pub fn add_delete(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.add_operation(FileOp {
            id: Uuid::new_v4().to_string(),
            op_type: OpType::Delete,
            source: path.as_ref().to_path_buf(),
            destination: None,
            backup_path: None,
            metadata: None,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Execute all operations in the transaction
    pub async fn execute(&mut self) -> Result<()> {
        if self.state != TransactionState::Pending {
            return Err(AppError::InvalidInput {
                message: "Transaction has already been executed".to_string(),
            });
        }

        self.state = TransactionState::InProgress;
        info!("Starting atomic transaction {} with {} operations", self.id, self.operations.len());

        let operations = self.operations.clone();
        for op in &operations {
            match self.execute_single_operation(op).await {
                Ok(rollback) => {
                    self.rollback_stack.push(rollback);
                    debug!("Successfully executed operation: {:?}", op.op_type);
                }
                Err(e) => {
                    error!("Operation failed: {:?}. Starting rollback...", e);
                    self.state = TransactionState::Failed;

                    // Attempt rollback
                    if let Err(rollback_err) = self.rollback().await {
                        error!("Rollback failed: {:?}", rollback_err);
                    }

                    return Err(e);
                }
            }
        }

        self.state = TransactionState::Committed;
        info!("Transaction {} committed successfully", self.id);

        // Clean up backup directory
        self.cleanup_backups().await;

        Ok(())
    }

    /// Execute a single operation and return its rollback operation
    async fn execute_single_operation(&mut self, op: &FileOp) -> Result<RollbackOp> {
        match op.op_type {
            OpType::Move => self.execute_move(op).await,
            OpType::Copy => self.execute_copy(op).await,
            OpType::Delete => self.execute_delete(op).await,
            OpType::Rename => self.execute_rename(op).await,
            OpType::Create => self.execute_create(op).await,
        }
    }

    /// Execute a move operation
    async fn execute_move(&mut self, op: &FileOp) -> Result<RollbackOp> {
        let dest = op.destination.as_ref().ok_or_else(|| AppError::InvalidInput {
            message: "Move operation requires a destination".to_string(),
        })?;

        // Ensure source exists
        if !op.source.exists() {
            return Err(AppError::FileNotFound {
                path: op.source.display().to_string(),
            });
        }

        // Create parent directory if needed
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Handle existing destination
        let mut backup_path = None;
        if dest.exists() {
            let backup = self.create_backup(dest).await?;
            backup_path = Some(backup);
        }

        // Perform the move
        fs::rename(&op.source, dest).await.map_err(|e| AppError::ProcessingError {
            message: format!("Failed to move file: {}", e),
        })?;

        Ok(RollbackOp {
            op_type: OpType::Move,
            source: dest.clone(),
            destination: Some(op.source.clone()),
            backup_path,
        })
    }

    /// Execute a copy operation
    async fn execute_copy(&mut self, op: &FileOp) -> Result<RollbackOp> {
        let dest = op.destination.as_ref().ok_or_else(|| AppError::InvalidInput {
            message: "Copy operation requires a destination".to_string(),
        })?;

        // Ensure source exists
        if !op.source.exists() {
            return Err(AppError::FileNotFound {
                path: op.source.display().to_string(),
            });
        }

        // Create parent directory if needed
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Handle existing destination
        let mut backup_path = None;
        if dest.exists() {
            let backup = self.create_backup(dest).await?;
            backup_path = Some(backup);
        }

        // Perform the copy
        if op.source.is_dir() {
            self.copy_dir_recursive(&op.source, dest).await?;
        } else {
            fs::copy(&op.source, dest).await?;
        }

        Ok(RollbackOp {
            op_type: OpType::Delete,
            source: dest.clone(),
            destination: None,
            backup_path,
        })
    }

    /// Execute a delete operation
    async fn execute_delete(&mut self, op: &FileOp) -> Result<RollbackOp> {
        if !op.source.exists() {
            return Err(AppError::FileNotFound {
                path: op.source.display().to_string(),
            });
        }

        // Create backup before deletion
        let backup_path = self.create_backup(&op.source).await?;

        // Perform the delete
        if op.source.is_dir() {
            fs::remove_dir_all(&op.source).await?;
        } else {
            fs::remove_file(&op.source).await?;
        }

        Ok(RollbackOp {
            op_type: OpType::Create,
            source: op.source.clone(),
            destination: None,
            backup_path: Some(backup_path),
        })
    }

    /// Execute a rename operation (similar to move but in same directory)
    async fn execute_rename(&mut self, op: &FileOp) -> Result<RollbackOp> {
        self.execute_move(op).await
    }

    /// Execute a create operation
    async fn execute_create(&mut self, op: &FileOp) -> Result<RollbackOp> {
        if op.source.exists() {
            return Err(AppError::InvalidPath {
                message: format!("File already exists: {}", op.source.display()),
            });
        }

        // Create parent directory if needed
        if let Some(parent) = op.source.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Create the file
        fs::File::create(&op.source).await?;

        Ok(RollbackOp {
            op_type: OpType::Delete,
            source: op.source.clone(),
            destination: None,
            backup_path: None,
        })
    }

    /// Rollback all executed operations
    pub async fn rollback(&mut self) -> Result<()> {
        if self.state == TransactionState::RolledBack {
            return Ok(());
        }

        warn!("Starting rollback for transaction {}", self.id);
        self.state = TransactionState::RolledBack;

        while let Some(rollback_op) = self.rollback_stack.pop() {
            if let Err(e) = self.execute_rollback_operation(&rollback_op).await {
                error!("Failed to rollback operation: {:?}", e);
                // Continue with other rollbacks even if one fails
            }
        }

        info!("Rollback completed for transaction {}", self.id);
        Ok(())
    }

    /// Execute a single rollback operation
    async fn execute_rollback_operation(&self, op: &RollbackOp) -> Result<()> {
        match op.op_type {
            OpType::Move => {
                if let Some(dest) = &op.destination {
                    fs::rename(&op.source, dest).await?;
                }
            }
            OpType::Delete => {
                if op.source.exists() {
                    if op.source.is_dir() {
                        fs::remove_dir_all(&op.source).await?;
                    } else {
                        fs::remove_file(&op.source).await?;
                    }
                }
            }
            OpType::Create => {
                if let Some(backup) = &op.backup_path {
                    // Restore from backup
                    if backup.is_dir() {
                        self.copy_dir_recursive(backup, &op.source).await?;
                    } else {
                        fs::copy(backup, &op.source).await?;
                    }
                }
            }
            _ => {}
        }

        // Restore original file if it was overwritten
        if let Some(backup) = &op.backup_path {
            if let Some(dest) = &op.destination {
                if backup.exists() {
                    if backup.is_dir() {
                        self.copy_dir_recursive(backup, dest).await?;
                    } else {
                        fs::copy(backup, dest).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Create a backup of a file or directory
    async fn create_backup(&self, path: &Path) -> Result<PathBuf> {
        let backup_name = format!("{}_{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("backup"),
            chrono::Utc::now().timestamp()
        );

        let backup_path = self.backup_dir.join(backup_name);

        if path.is_dir() {
            self.copy_dir_recursive(path, &backup_path).await?;
        } else {
            fs::copy(path, &backup_path).await?;
        }

        debug!("Created backup: {} -> {}", path.display(), backup_path.display());
        Ok(backup_path)
    }

    /// Recursively copy a directory
    async fn copy_dir_recursive(&self, src: &Path, dst: &Path) -> Result<()> {
        fs::create_dir_all(dst).await?;

        let mut entries = fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let dest_path = dst.join(entry.file_name());

            if entry_path.is_dir() {
                Box::pin(self.copy_dir_recursive(&entry_path, &dest_path)).await?;
            } else {
                fs::copy(&entry_path, &dest_path).await?;
            }
        }

        Ok(())
    }

    /// Clean up backup files after successful commit
    async fn cleanup_backups(&self) {
        if self.backup_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&self.backup_dir).await {
                warn!("Failed to clean up backup directory: {}", e);
            }
        }
    }

    /// Get the transaction ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the current state
    pub fn state(&self) -> &TransactionState {
        &self.state
    }

    /// Get the number of operations
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }
}

impl Drop for AtomicFileOperation {
    fn drop(&mut self) {
        // Clean up backup directory if transaction was committed
        if self.state == TransactionState::Committed && self.backup_dir.exists() {
            let backup_dir = self.backup_dir.clone();
            // Fire and forget cleanup
            tokio::spawn(async move {
                let _ = fs::remove_dir_all(&backup_dir).await;
            });
        }
    }
}

/// Global file lock manager to prevent concurrent operations on the same files
use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

static FILE_LOCKS: Lazy<DashMap<PathBuf, Arc<Mutex<()>>>> =
    Lazy::new(DashMap::new);

/// Perform atomic file move with proper locking to prevent race conditions
pub async fn move_file_atomic(src: &Path, dst: &Path) -> Result<()> {
    use std::sync::Arc;

    // Acquire exclusive lock on source file
    let src_lock = FILE_LOCKS
        .entry(src.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();

    let _guard = src_lock.lock().await;

    // Check source still exists under lock
    if !src.exists() {
        FILE_LOCKS.remove(&src.to_path_buf());
        return Err(AppError::FileNotFound {
            path: src.to_string_lossy().into()
        });
    }

    // Ensure destination directory exists
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Atomic rename (same filesystem) or copy+delete (cross-filesystem)
    match tokio::fs::rename(src, dst).await {
        Ok(_) => {
            FILE_LOCKS.remove(&src.to_path_buf());
            Ok(())
        }
        Err(_) => {
            // Cross-filesystem move requires copy + delete
            tokio::fs::copy(src, dst).await?;
            tokio::fs::remove_file(src).await?;
            FILE_LOCKS.remove(&src.to_path_buf());
            Ok(())
        }
    }
}

/// Perform atomic file copy with proper locking
pub async fn copy_file_atomic(src: &Path, dst: &Path) -> Result<()> {
    use std::sync::Arc;

    // Acquire shared lock on source file (multiple readers allowed)
    let src_lock = FILE_LOCKS
        .entry(src.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();

    let _guard = src_lock.lock().await;

    // Check source still exists under lock
    if !src.exists() {
        return Err(AppError::FileNotFound {
            path: src.to_string_lossy().into()
        });
    }

    // Ensure destination directory exists
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::copy(src, dst).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_atomic_move() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");

        // Create source file
        fs::write(&source, "test content").await.unwrap();

        // Execute atomic move
        let mut atomic_op = AtomicFileOperation::new().unwrap();
        atomic_op.add_move(&source, &dest).unwrap();
        atomic_op.execute().await.unwrap();

        // Verify
        assert!(!source.exists());
        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).await.unwrap(), "test content");
    }

    #[tokio::test]
    async fn test_rollback_on_failure() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        let dest = temp_dir.path().join("dest.txt");
        let invalid = PathBuf::from("/invalid/path/that/does/not/exist.txt");

        // Create source file
        fs::write(&source, "test content").await.unwrap();

        // Create atomic operation that will fail
        let mut atomic_op = AtomicFileOperation::new().unwrap();
        atomic_op.add_move(&source, &dest).unwrap();
        atomic_op.add_move(&dest, &invalid).unwrap(); // This will fail

        // Execute should fail and rollback
        assert!(atomic_op.execute().await.is_err());

        // Verify rollback worked - source should still exist
        assert!(source.exists());
        assert!(!dest.exists());
        assert_eq!(fs::read_to_string(&source).await.unwrap(), "test content");
    }
}