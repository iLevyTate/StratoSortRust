// Database Migration System
// Provides version control and automatic migration for database schema changes

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, SqlitePool};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::AppError;

// Migration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub version: i32,
    pub name: String,
    pub description: String,
    pub sql_up: String,
    pub sql_down: Option<String>,
    pub checksum: String,
    pub requires_data_migration: bool,
}

// Migration history entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MigrationHistory {
    pub id: i32,
    pub version: i32,
    pub name: String,
    pub checksum: String,
    pub applied_at: DateTime<Utc>,
    pub execution_time_ms: i64,
    pub success: bool,
    pub error_message: Option<String>,
}

// Migration manager
pub struct MigrationManager {
    pool: SqlitePool,
    migrations_dir: PathBuf,
    migrations: Vec<Migration>,
    dry_run: bool,
}

impl MigrationManager {
    // Initialize migration manager
    pub async fn new(pool: SqlitePool, migrations_dir: PathBuf) -> Result<Self, AppError> {
        let mut manager = Self {
            pool,
            migrations_dir,
            migrations: Vec::new(),
            dry_run: false,
        };

        // Ensure migration history table exists
        manager.create_migration_table().await?;

        // Load all migrations
        manager.load_migrations()?;

        Ok(manager)
    }

    // Create migration history table if it doesn't exist
    async fn create_migration_table(&self) -> Result<(), AppError> {
        let query = r#"
            CREATE TABLE IF NOT EXISTS _migration_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL UNIQUE,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                execution_time_ms INTEGER NOT NULL,
                success INTEGER NOT NULL,
                error_message TEXT,
                UNIQUE(version)
            )
        "#;

        sqlx::query(query)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to create migration table: {}", e),
            })?;

        // Create index for faster lookups
        let index_query = r#"
            CREATE INDEX IF NOT EXISTS idx_migration_version
            ON _migration_history(version)
        "#;

        sqlx::query(index_query)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to create migration index: {}", e),
            })?;

        Ok(())
    }

    // Load all migration definitions
    fn load_migrations(&mut self) -> Result<(), AppError> {
        // Define migrations inline for reliability
        // In production, these could be loaded from SQL files

        self.migrations = vec![
            Migration {
                version: 1,
                name: "initial_schema".to_string(),
                description: "Create initial database schema".to_string(),
                sql_up: include_str!("../../migrations/001_initial_schema.sql").to_string(),
                sql_down: Some(include_str!("../../migrations/001_initial_schema_down.sql").to_string()),
                checksum: self.calculate_checksum(include_str!("../../migrations/001_initial_schema.sql")),
                requires_data_migration: false,
            },
            Migration {
                version: 2,
                name: "add_smart_folders".to_string(),
                description: "Add smart folders support".to_string(),
                sql_up: r#"
                    CREATE TABLE IF NOT EXISTS smart_folders (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL,
                        description TEXT,
                        criteria TEXT NOT NULL,
                        color TEXT,
                        icon TEXT,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL,
                        is_active BOOLEAN DEFAULT 1
                    );

                    CREATE INDEX IF NOT EXISTS idx_smart_folders_active
                    ON smart_folders(is_active);
                "#.to_string(),
                sql_down: Some("DROP TABLE IF EXISTS smart_folders;".to_string()),
                checksum: self.calculate_checksum("smart_folders_v2"),
                requires_data_migration: false,
            },
            Migration {
                version: 3,
                name: "add_file_tags".to_string(),
                description: "Add file tagging system".to_string(),
                sql_up: r#"
                    CREATE TABLE IF NOT EXISTS tags (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL UNIQUE,
                        color TEXT,
                        created_at TEXT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS file_tags (
                        file_id INTEGER NOT NULL,
                        tag_id INTEGER NOT NULL,
                        assigned_at TEXT NOT NULL,
                        PRIMARY KEY (file_id, tag_id),
                        FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE,
                        FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
                    );

                    CREATE INDEX IF NOT EXISTS idx_file_tags_file
                    ON file_tags(file_id);

                    CREATE INDEX IF NOT EXISTS idx_file_tags_tag
                    ON file_tags(tag_id);
                "#.to_string(),
                sql_down: Some("DROP TABLE IF EXISTS file_tags; DROP TABLE IF EXISTS tags;".to_string()),
                checksum: self.calculate_checksum("tags_system_v3"),
                requires_data_migration: false,
            },
            Migration {
                version: 4,
                name: "add_performance_indexes".to_string(),
                description: "Add indexes for better query performance".to_string(),
                sql_up: r#"
                    CREATE INDEX IF NOT EXISTS idx_files_path
                    ON files(path);

                    CREATE INDEX IF NOT EXISTS idx_files_modified
                    ON files(modified_at);

                    CREATE INDEX IF NOT EXISTS idx_files_size
                    ON files(size);

                    CREATE INDEX IF NOT EXISTS idx_folders_parent
                    ON folders(parent_id);

                    CREATE INDEX IF NOT EXISTS idx_organization_history_date
                    ON organization_history(organized_at);
                "#.to_string(),
                sql_down: Some(r#"
                    DROP INDEX IF EXISTS idx_files_path;
                    DROP INDEX IF EXISTS idx_files_modified;
                    DROP INDEX IF EXISTS idx_files_size;
                    DROP INDEX IF EXISTS idx_folders_parent;
                    DROP INDEX IF EXISTS idx_organization_history_date;
                "#.to_string()),
                checksum: self.calculate_checksum("performance_indexes_v4"),
                requires_data_migration: false,
            },
            Migration {
                version: 5,
                name: "add_file_metadata_cache".to_string(),
                description: "Add metadata caching for faster file operations".to_string(),
                sql_up: r#"
                    CREATE TABLE IF NOT EXISTS file_metadata_cache (
                        file_id INTEGER PRIMARY KEY,
                        metadata TEXT NOT NULL,
                        computed_hash TEXT,
                        last_analyzed TEXT,
                        cache_version INTEGER DEFAULT 1,
                        FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
                    );

                    CREATE INDEX IF NOT EXISTS idx_metadata_cache_analyzed
                    ON file_metadata_cache(last_analyzed);
                "#.to_string(),
                sql_down: Some("DROP TABLE IF EXISTS file_metadata_cache;".to_string()),
                checksum: self.calculate_checksum("metadata_cache_v5"),
                requires_data_migration: false,
            },
        ];

        // Sort migrations by version
        self.migrations.sort_by_key(|m| m.version);

        Ok(())
    }

    // Calculate checksum for migration
    fn calculate_checksum(&self, content: &str) -> String {
        // Note: Would use sha2::Sha256 for secure hashing in production
        // For now, using simple hash based on std library
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    // Get current database version
    pub async fn get_current_version(&self) -> Result<i32, AppError> {
        let query = r#"
            SELECT MAX(version) as version
            FROM _migration_history
            WHERE success = 1
        "#;

        let result = sqlx::query_scalar::<_, Option<i32>>(query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to get current version: {}", e),
            })?;

        Ok(result.unwrap_or(0))
    }

    // Get pending migrations
    pub async fn get_pending_migrations(&self) -> Result<Vec<&Migration>, AppError> {
        let current_version = self.get_current_version().await?;

        Ok(self.migrations
            .iter()
            .filter(|m| m.version > current_version)
            .collect())
    }

    // Run all pending migrations
    pub async fn migrate(&mut self) -> Result<MigrationReport, AppError> {
        let start_time = std::time::Instant::now();
        let current_version = self.get_current_version().await?;
        let pending = self.get_pending_migrations().await?;

        if pending.is_empty() {
            return Ok(MigrationReport {
                from_version: current_version,
                to_version: current_version,
                migrations_applied: 0,
                success: true,
                duration_ms: start_time.elapsed().as_millis() as u64,
                messages: vec!["Database is already up to date".to_string()],
            });
        }

        let mut messages = Vec::new();
        let mut applied_count = 0;
        let mut final_version = current_version;

        // Begin transaction for all migrations
        let mut tx = self.pool.begin().await.map_err(|e| AppError::DatabaseError {
            message: format!("Failed to begin migration transaction: {}", e),
        })?;

        for migration in pending {
            let migration_start = std::time::Instant::now();
            messages.push(format!(
                "Applying migration v{}: {}",
                migration.version, migration.name
            ));

            if self.dry_run {
                messages.push(format!("DRY RUN - Would execute: {}", migration.sql_up));
                applied_count += 1;
                final_version = migration.version;
                continue;
            }

            // Execute migration
            match sqlx::query(&migration.sql_up)
                .execute(&mut *tx)
                .await
            {
                Ok(_) => {
                    // Record successful migration
                    let execution_time = migration_start.elapsed().as_millis() as i64;

                    let history_query = r#"
                        INSERT INTO _migration_history
                        (version, name, checksum, applied_at, execution_time_ms, success, error_message)
                        VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#;

                    sqlx::query(history_query)
                        .bind(migration.version)
                        .bind(&migration.name)
                        .bind(&migration.checksum)
                        .bind(Utc::now().to_rfc3339())
                        .bind(execution_time)
                        .bind(true)
                        .bind(None::<String>)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| AppError::DatabaseError {
                            message: format!("Failed to record migration history: {}", e),
                        })?;

                    messages.push(format!(
                        "✓ Migration v{} applied successfully in {}ms",
                        migration.version, execution_time
                    ));

                    applied_count += 1;
                    final_version = migration.version;
                }
                Err(e) => {
                    // Rollback transaction
                    tx.rollback().await.map_err(|e| AppError::DatabaseError {
                        message: format!("Failed to rollback migration: {}", e),
                    })?;

                    return Err(AppError::DatabaseError {
                        message: format!(
                            "Migration v{} failed: {}",
                            migration.version, e
                        ),
                    });
                }
            }
        }

        // Commit all migrations
        if !self.dry_run {
            tx.commit().await.map_err(|e| AppError::DatabaseError {
                message: format!("Failed to commit migrations: {}", e),
            })?;
        }

        Ok(MigrationReport {
            from_version: current_version,
            to_version: final_version,
            migrations_applied: applied_count,
            success: true,
            duration_ms: start_time.elapsed().as_millis() as u64,
            messages,
        })
    }

    // Rollback to a specific version
    pub async fn rollback_to(&mut self, target_version: i32) -> Result<MigrationReport, AppError> {
        let current_version = self.get_current_version().await?;

        if target_version >= current_version {
            return Err(AppError::InvalidInput {
                message: format!(
                    "Cannot rollback to version {} (current: {})",
                    target_version, current_version
                ),
            });
        }

        let mut messages = Vec::new();
        let start_time = std::time::Instant::now();

        // Get migrations to rollback
        let to_rollback: Vec<_> = self.migrations
            .iter()
            .filter(|m| m.version > target_version && m.version <= current_version)
            .rev()
            .collect();

        if to_rollback.is_empty() {
            return Ok(MigrationReport {
                from_version: current_version,
                to_version: target_version,
                migrations_applied: 0,
                success: true,
                duration_ms: start_time.elapsed().as_millis() as u64,
                messages: vec!["No migrations to rollback".to_string()],
            });
        }

        // Begin transaction
        let mut tx = self.pool.begin().await.map_err(|e| AppError::DatabaseError {
            message: format!("Failed to begin rollback transaction: {}", e),
        })?;

        for migration in to_rollback {
            if let Some(sql_down) = &migration.sql_down {
                messages.push(format!(
                    "Rolling back migration v{}: {}",
                    migration.version, migration.name
                ));

                if self.dry_run {
                    messages.push(format!("DRY RUN - Would execute: {}", sql_down));
                    continue;
                }

                // Execute rollback
                sqlx::query(sql_down)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| AppError::DatabaseError {
                        message: format!("Rollback failed for v{}: {}", migration.version, e),
                    })?;

                // Remove from history
                let delete_query = "DELETE FROM _migration_history WHERE version = ?";
                sqlx::query(delete_query)
                    .bind(migration.version)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| AppError::DatabaseError {
                        message: format!("Failed to remove migration history: {}", e),
                    })?;

                messages.push(format!("✓ Rolled back migration v{}", migration.version));
            } else {
                return Err(AppError::DatabaseError {
                    message: format!(
                        "Migration v{} cannot be rolled back (no down migration)",
                        migration.version
                    ),
                });
            }
        }

        if !self.dry_run {
            tx.commit().await.map_err(|e| AppError::DatabaseError {
                message: format!("Failed to commit rollback: {}", e),
            })?;
        }

        Ok(MigrationReport {
            from_version: current_version,
            to_version: target_version,
            migrations_applied: to_rollback.len(),
            success: true,
            duration_ms: start_time.elapsed().as_millis() as u64,
            messages,
        })
    }

    // Verify migration integrity
    pub async fn verify_integrity(&self) -> Result<Vec<IntegrityCheck>, AppError> {
        let mut checks = Vec::new();

        // Check migration history table
        let history_query = "SELECT version, name, checksum FROM _migration_history WHERE success = 1";
        let history: Vec<(i32, String, String)> = sqlx::query_as(history_query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to query migration history: {}", e),
            })?;

        for (version, name, stored_checksum) in history {
            if let Some(migration) = self.migrations.iter().find(|m| m.version == version) {
                let checksum_match = migration.checksum == stored_checksum;
                let name_match = migration.name == name;

                checks.push(IntegrityCheck {
                    version,
                    name: name.clone(),
                    checksum_valid: checksum_match,
                    name_valid: name_match,
                    message: if !checksum_match {
                        Some("Checksum mismatch - migration may have been modified".to_string())
                    } else if !name_match {
                        Some("Name mismatch - migration renamed".to_string())
                    } else {
                        None
                    },
                });
            } else {
                checks.push(IntegrityCheck {
                    version,
                    name: name.clone(),
                    checksum_valid: false,
                    name_valid: false,
                    message: Some("Migration not found in current definitions".to_string()),
                });
            }
        }

        Ok(checks)
    }

    // Set dry run mode
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
    }

    // Get migration history
    pub async fn get_history(&self) -> Result<Vec<MigrationHistory>, AppError> {
        let query = r#"
            SELECT id, version, name, checksum, applied_at, execution_time_ms, success, error_message
            FROM _migration_history
            ORDER BY version DESC
        "#;

        let history = sqlx::query_as::<_, MigrationHistory>(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to get migration history: {}", e),
            })?;

        Ok(history)
    }
}

// Migration report
#[derive(Debug, Serialize)]
pub struct MigrationReport {
    pub from_version: i32,
    pub to_version: i32,
    pub migrations_applied: usize,
    pub success: bool,
    pub duration_ms: u64,
    pub messages: Vec<String>,
}

// Integrity check result
#[derive(Debug, Serialize)]
pub struct IntegrityCheck {
    pub version: i32,
    pub name: String,
    pub checksum_valid: bool,
    pub name_valid: bool,
    pub message: Option<String>,
}