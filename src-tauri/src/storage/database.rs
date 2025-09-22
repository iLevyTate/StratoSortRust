use crate::ai::FileAnalysis;
use crate::error::{AppError, Result};
use crate::storage::{VectorExtension, VectorStats};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Row};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};
use tracing::{debug, info, warn};

pub const CURRENT_SCHEMA_VERSION: i32 = 3;

pub struct Database {
    pool: SqlitePool,
    vector_ext: Arc<VectorExtension>,
}

/// Validates SQL identifiers to prevent injection attacks
/// Only allows alphanumeric characters, underscores, and limits length
pub fn is_valid_sql_identifier(identifier: &str) -> bool {
    if identifier.is_empty() || identifier.len() > 64 {
        return false;
    }

    // Must start with letter or underscore
    if !identifier
        .chars()
        .next()
        .unwrap_or(' ')
        .is_ascii_alphabetic()
        && !identifier.starts_with('_')
    {
        return false;
    }

    // Only allow alphanumeric characters and underscores
    identifier
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// Helper function to escape LIKE special characters to prevent SQL injection
#[allow(dead_code)]
fn escape_like_pattern(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

impl Database {
    pub async fn new<R: Runtime>(handle: &AppHandle<R>) -> Result<Self> {
        let db_path = Self::database_path_with_fallbacks(handle).await?;

        // Robust directory creation with multiple fallbacks
        if let Some(parent) = db_path.parent() {
            // Create directory with comprehensive error handling
            if let Err(e) = Self::ensure_database_directory(parent).await {
                tracing::error!("Failed to create database directory {:?}: {}", parent, e);

                // Try fallback directory locations
                let fallback_paths = Self::get_fallback_database_paths();
                for fallback_path in fallback_paths {
                    if let Ok(fallback_db_path) = Self::try_fallback_database(&fallback_path).await
                    {
                        tracing::warn!("Using fallback database path: {:?}", fallback_db_path);
                        return Self::initialize_database_at_path(fallback_db_path).await;
                    }
                }

                return Err(AppError::DatabaseError {
                    message: format!("Failed to create database directory '{}': {}. All fallback locations failed.", parent.display(), e),
                });
            }
        }

        Self::initialize_database_at_path(db_path).await
    }

    async fn initialize_database_at_path(db_path: PathBuf) -> Result<Self> {
        // Log the database path for debugging
        tracing::info!("Initializing database at: {:?}", db_path);

        // Create connection with robust retry logic
        let pool = Self::create_database_connection_with_retry(&db_path).await?;

        // Verify connection with enhanced retry logic
        Self::verify_database_connection(&pool).await?;

        // Initialize vector extension with non-blocking approach
        let vector_ext = Arc::new(Self::initialize_vector_extension_safely(&pool).await);

        let db = Self { pool, vector_ext };

        // Check database integrity with recovery options
        if let Err(e) = db.check_integrity_with_recovery().await {
            tracing::warn!(
                "Database integrity check failed, attempting recovery: {}",
                e
            );
            db.attempt_database_recovery().await?;
        }

        // Run migrations with atomic transactions
        db.run_migrations_atomically().await?;

        info!("Database initialized successfully");
        Ok(db)
    }

    /// Test-specific constructor that takes a path directly
    pub async fn new_test(db_path: &std::path::Path) -> Result<Self> {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let _db_url = format!("sqlite://{}", db_path.display());

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(30))
            .pragma("foreign_keys", "ON");

        let pool = SqlitePoolOptions::new()
            .max_connections(20) // Increased for concurrent ops
            .min_connections(5)   // Minimum connections to maintain
            .acquire_timeout(Duration::from_secs(3))
            .idle_timeout(Some(Duration::from_secs(10)))
            .max_lifetime(Some(Duration::from_secs(3600)))
            .connect_with(options).await?;

        // Initialize vector extension
        let vector_ext = Arc::new(VectorExtension::initialize(&pool).await);

        let db = Self { pool, vector_ext };

        // Run migrations to ensure schema is up to date
        db.run_migrations().await?;

        Ok(db)
    }

    #[allow(dead_code)]
    async fn initialize_schema(&self) -> Result<()> {
        // Create tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS file_analysis (
                path TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                tags TEXT NOT NULL,
                summary TEXT NOT NULL,
                confidence REAL NOT NULL,
                extracted_text TEXT,
                detected_language TEXT,
                metadata TEXT,
                analyzed_at INTEGER NOT NULL,
                embedding BLOB
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS smart_folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                rules TEXT NOT NULL,
                icon TEXT,
                color TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS operations_history (
                id TEXT PRIMARY KEY,
                operation_type TEXT NOT NULL,
                source TEXT NOT NULL,
                destination TEXT,
                timestamp INTEGER NOT NULL,
                metadata TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes with proper error handling
        self.create_index_safely("idx_analysis_category", "file_analysis", "category")
            .await?;
        self.create_index_safely("idx_analysis_analyzed_at", "file_analysis", "analyzed_at")
            .await?;
        self.create_index_safely(
            "idx_operations_timestamp",
            "operations_history",
            "timestamp",
        )
        .await?;

        // Create vector table using the vector extension if available
        if self.vector_ext.is_available {
            info!("Creating vector table with sqlite-vec extension");
            if let Err(e) = self
                .vector_ext
                .create_vector_table(
                    &self.pool,
                    "vec_embeddings",
                    self.vector_ext.get_dimensions(),
                )
                .await
            {
                warn!(
                    "Failed to create vector table: {}. Will use fallback storage.",
                    e
                );
            }
        } else {
            info!("sqlite-vec not available, using fallback embedding storage in main table");
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn create_index_safely(
        &self,
        index_name: &str,
        table_name: &str,
        column_name: &str,
    ) -> Result<()> {
        // Whitelist of allowed index configurations for maximum security
        const ALLOWED_INDEXES: &[(&str, &str, &str)] = &[
            ("idx_analysis_category", "file_analysis", "category"),
            ("idx_analysis_analyzed_at", "file_analysis", "analyzed_at"),
            ("idx_analysis_path", "file_analysis", "path"),
            ("idx_analysis_confidence", "file_analysis", "confidence"),
            (
                "idx_smart_folders_created_at",
                "smart_folders",
                "created_at",
            ),
            (
                "idx_smart_folders_updated_at",
                "smart_folders",
                "updated_at",
            ),
        ];

        // Only allow predefined index combinations
        if !ALLOWED_INDEXES
            .iter()
            .any(|(idx, tbl, col)| idx == &index_name && tbl == &table_name && col == &column_name)
        {
            return Err(AppError::SecurityError {
                message: format!(
                    "Unauthorized index configuration: {} on {}.{}",
                    index_name, table_name, column_name
                ),
            });
        }

        // Additional validation as defense in depth
        if !is_valid_sql_identifier(index_name)
            || !is_valid_sql_identifier(table_name)
            || !is_valid_sql_identifier(column_name)
        {
            return Err(AppError::SecurityError {
                message: "Invalid SQL identifier format".to_string(),
            });
        }

        // Safe to use format! after whitelist + validation
        let query = format!(
            "CREATE INDEX IF NOT EXISTS {} ON {}({})",
            index_name, table_name, column_name
        );

        match sqlx::query(&query).execute(&self.pool).await {
            Ok(_) => {
                tracing::debug!("Successfully created or verified index: {}", index_name);
                Ok(())
            }
            Err(sqlx::Error::Database(db_err)) => {
                // Check if this is a "table already exists" or similar benign error
                if let Some(code) = db_err.code() {
                    if code == "1" || code == "SQLITE_ERROR" {
                        // Check if error message indicates index already exists
                        let message = db_err.message().to_lowercase();
                        if message.contains("already exists") || message.contains("duplicate") {
                            tracing::debug!("Index {} already exists, continuing", index_name);
                            return Ok(());
                        }
                    }
                }

                // This is a real error that could affect performance
                tracing::error!(
                    "Critical: Failed to create index {}: {}",
                    index_name,
                    db_err
                );
                Err(AppError::DatabaseError {
                    message: format!("Failed to create critical index {}: {}", index_name, db_err),
                })
            }
            Err(e) => {
                tracing::error!("Critical: Failed to create index {}: {}", index_name, e);
                Err(AppError::DatabaseError {
                    message: format!("Failed to create critical index {}: {}", index_name, e),
                })
            }
        }
    }

    async fn check_integrity(&self) -> Result<()> {
        tracing::info!("Checking database integrity...");

        // Run SQLite's built-in integrity check
        let result = sqlx::query("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                tracing::error!("Database integrity check failed: {}", e);
                AppError::DatabaseError {
                    message: format!("Database integrity check failed: {}", e),
                }
            })?;

        let integrity_result: String = result.get(0);

        if integrity_result != "ok" {
            tracing::error!("Database corruption detected: {}", integrity_result);

            // Try to run a quick check to see if we can recover
            match sqlx::query("PRAGMA quick_check")
                .fetch_one(&self.pool)
                .await
            {
                Ok(quick_result) => {
                    let quick_check: String = quick_result.get(0);
                    if quick_check == "ok" {
                        tracing::warn!("Quick check passed, but full integrity check failed. Database may have minor issues.");
                    } else {
                        return Err(AppError::DatabaseError {
                            message: format!(
                                "Database corruption detected and cannot be recovered: {}",
                                integrity_result
                            ),
                        });
                    }
                }
                Err(_) => {
                    return Err(AppError::DatabaseError {
                        message: format!(
                            "Database corruption detected and cannot be recovered: {}",
                            integrity_result
                        ),
                    });
                }
            }
        } else {
            tracing::debug!("Database integrity check passed");
        }

        Ok(())
    }

    pub async fn save_analysis(&self, analysis: &FileAnalysis) -> Result<()> {
        let tags_json = serde_json::to_string(&analysis.tags)?;
        let analyzed_at = chrono::Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO file_analysis
            (path, category, tags, summary, confidence, extracted_text, detected_language, analyzed_at, embedding)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&analysis.path)
        .bind(&analysis.category)
        .bind(&tags_json)
        .bind(&analysis.summary)
        .bind(analysis.confidence)
        .bind(&analysis.extracted_text)
        .bind(&analysis.detected_language)
        .bind(analyzed_at)
        .bind(None::<&[u8]>) // NULL for embedding initially
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_analysis(&self, path: &str) -> Result<Option<FileAnalysis>> {
        let row = sqlx::query(
            r#"
            SELECT category, tags, summary, confidence, extracted_text, detected_language
            FROM file_analysis
            WHERE path = ?
            "#,
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let tags: Vec<String> = serde_json::from_str(row.get("tags"))?;

            Ok(Some(FileAnalysis {
                path: path.to_string(),
                category: row.get("category"),
                tags,
                summary: row.get("summary"),
                confidence: row.get("confidence"),
                extracted_text: row.get("extracted_text"),
                detected_language: row.get("detected_language"),
                metadata: serde_json::Value::Null,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn search_by_category(&self, category: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT path FROM file_analysis
            WHERE category = ?
            ORDER BY analyzed_at DESC
            "#,
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|row| row.get("path")).collect())
    }

    pub async fn search_by_tags(&self, tags: &[String]) -> Result<Vec<String>> {
        let mut paths = Vec::new();

        for tag in tags {
            // Use exact match with JSON extraction for security
            let rows = sqlx::query(
                r#"
                SELECT path FROM file_analysis
                WHERE EXISTS (
                    SELECT 1 FROM json_each(tags) 
                    WHERE value = ? COLLATE NOCASE
                )
                ORDER BY confidence DESC
                "#,
            )
            .bind(tag.trim()) // Trim whitespace but use exact match
            .fetch_all(&self.pool)
            .await?;

            for row in rows {
                paths.push(row.get("path"));
            }
        }

        // Remove duplicates
        paths.sort();
        paths.dedup();

        Ok(paths)
    }

    pub async fn get_recent_analyses(&self, limit: u32) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT path
            FROM file_analysis
            ORDER BY analyzed_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let paths: Vec<String> = rows.into_iter().map(|row| row.get("path")).collect();

        Ok(paths)
    }

    pub async fn save_embedding(
        &self,
        path: &str,
        embedding: &[f32],
        model_name: Option<&str>,
    ) -> Result<()> {
        // Serialize embedding as JSON string then to bytes for consistent storage
        let embedding_json = serde_json::to_string(embedding)?;
        let embedding_bytes = embedding_json.as_bytes().to_vec();

        // Save to main table as fallback/backup
        sqlx::query(
            r#"
            UPDATE file_analysis
            SET embedding = ?
            WHERE path = ?
            "#,
        )
        .bind(&embedding_bytes)
        .bind(path)
        .execute(&self.pool)
        .await?;

        // Save to embeddings_v3 table for improved organization
        let model = model_name.unwrap_or("unknown");
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO embeddings_v3 (path, embedding, model_name, created_at, updated_at)
            VALUES (?, ?, ?, datetime('now'), datetime('now'))
            "#,
        )
        .bind(path)
        .bind(&embedding_bytes)
        .bind(model)
        .execute(&self.pool)
        .await?;

        // Save to vector table using proper extension if available
        if self.vector_ext.is_available {
            if let Err(e) = self
                .vector_ext
                .store_embedding(&self.pool, "vec_embeddings", path, embedding)
                .await
            {
                warn!(
                    "Failed to store embedding in vector table: {}. Using fallback storage only.",
                    e
                );
            }
        }

        Ok(())
    }

    pub async fn get_embedding(&self, path: &str) -> Result<Option<Vec<f32>>> {
        // Try to get from embeddings_v3 table first
        let row = sqlx::query(
            "SELECT embedding FROM embeddings_v3 WHERE path = ? LIMIT 1"
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let embedding_bytes: Vec<u8> = row.get("embedding");
            let embedding_str = String::from_utf8(embedding_bytes).map_err(|e| AppError::DatabaseError {
                message: format!("Failed to convert embedding bytes to string: {}", e),
            })?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_str)?;
            return Ok(Some(embedding));
        }

        // Fallback to file_analysis table
        let row = sqlx::query(
            "SELECT embedding FROM file_analysis WHERE path = ? AND embedding IS NOT NULL LIMIT 1"
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let embedding_bytes: Vec<u8> = row.get("embedding");
            let embedding_str = String::from_utf8(embedding_bytes).map_err(|e| AppError::DatabaseError {
                message: format!("Failed to convert embedding bytes to string: {}", e),
            })?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_str)?;
            return Ok(Some(embedding));
        }

        Ok(None)
    }

    /// Enhanced semantic search with better accuracy and performance
    pub async fn semantic_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        // Validate embedding dimensions
        if query_embedding.len() != self.vector_ext.embedding_dimensions {
            return Err(crate::error::AppError::InvalidInput {
                message: format!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    self.vector_ext.embedding_dimensions,
                    query_embedding.len()
                ),
            });
        }

        // Try vector search using sqlite-vec extension if available
        if self.vector_ext.is_available {
            debug!("Using sqlite-vec for high-performance semantic search");
            match self
                .vector_ext
                .vector_search(&self.pool, "vec_embeddings", query_embedding, limit)
                .await
            {
                Ok(results) => {
                    debug!("sqlite-vec search returned {} results", results.len());
                    return Ok(results);
                }
                Err(e) => {
                    warn!(
                        "sqlite-vec search failed, falling back to manual search: {}",
                        e
                    );
                }
            }
        }

        // Enhanced fallback with better similarity threshold
        debug!("Using enhanced manual cosine similarity for semantic search");
        self.enhanced_cosine_similarity_search(query_embedding, limit)
            .await
    }

    /// Enhanced cosine similarity search with better accuracy
    async fn enhanced_cosine_similarity_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        // Try embeddings_v3 table first (preferred)
        let rows = match sqlx::query(
            "SELECT path as file_path, embedding FROM embeddings_v3 WHERE embedding IS NOT NULL ORDER BY created_at"
        )
        .fetch_all(&self.pool)
        .await {
            Ok(rows) => rows,
            Err(_) => {
                // Fallback to file_analysis table if embeddings_v3 doesn't exist
                warn!("embeddings_v3 table not available, falling back to file_analysis table");
                sqlx::query(
                    "SELECT path as file_path, embedding FROM file_analysis WHERE embedding IS NOT NULL ORDER BY analyzed_at"
                )
                .fetch_all(&self.pool)
                .await?
            }
        };

        let mut results = Vec::new();
        let minimum_similarity = 0.1; // Filter out very low similarity matches

        for row in rows {
            let file_path: String = row.get("file_path");
            let embedding_blob: Vec<u8> = row.get("embedding");

            // Deserialize stored embedding (convert from bytes to string to JSON)
            if let Ok(embedding_json) = String::from_utf8(embedding_blob) {
                if let Ok(stored_embedding) = serde_json::from_str::<Vec<f32>>(&embedding_json) {
                    // Ensure dimensions match
                    if stored_embedding.len() == query_embedding.len() {
                        let similarity = cosine_similarity(query_embedding, &stored_embedding);

                        // Only include results above minimum similarity threshold
                        if similarity > minimum_similarity {
                            results.push((file_path, similarity));
                        }
                    }
                }
            }
        }

        // Sort by similarity (highest first) and limit results
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        debug!("Enhanced search found {} relevant results", results.len());
        Ok(results)
    }

    pub async fn record_operation(&self, operation: &Operation) -> Result<()> {
        let metadata_json = serde_json::to_string(&operation.metadata)?;

        sqlx::query(
            r#"
            INSERT INTO operations_history 
            (id, operation_type, source, destination, timestamp, metadata)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&operation.id)
        .bind(&operation.operation_type)
        .bind(&operation.source)
        .bind(&operation.destination)
        .bind(operation.timestamp)
        .bind(&metadata_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_recent_operations(&self, limit: usize) -> Result<Vec<Operation>> {
        let rows = sqlx::query(
            r#"
            SELECT id, operation_type, source, destination, timestamp, metadata
            FROM operations_history
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        let mut operations = Vec::new();

        for row in rows {
            let metadata: serde_json::Value = serde_json::from_str(row.get("metadata"))?;

            operations.push(Operation {
                id: row.get("id"),
                operation_type: row.get("operation_type"),
                source: row.get("source"),
                destination: row.get("destination"),
                timestamp: row.get("timestamp"),
                metadata: Some(metadata),
            });
        }

        Ok(operations)
    }

    pub async fn get_operation_by_id(&self, operation_id: &str) -> Result<Option<Operation>> {
        let row = sqlx::query(
            r#"
            SELECT id, operation_type, source, destination, timestamp, metadata
            FROM operations_history
            WHERE id = ?
            "#,
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let metadata: serde_json::Value = serde_json::from_str(row.get("metadata"))?;

            Ok(Some(Operation {
                id: row.get("id"),
                operation_type: row.get("operation_type"),
                source: row.get("source"),
                destination: row.get("destination"),
                timestamp: row.get("timestamp"),
                metadata: Some(metadata),
            }))
        } else {
            Ok(None)
        }
    }

    // IDOR Protection: Check if user has permission to access specific file
    pub async fn check_file_permission(&self, path: &str, _user_id: &str) -> Result<bool> {
        // For a desktop application, this is simplified - in a real multi-user system,
        // you would have a proper user_permissions table

        // Check if the file has been analyzed by this user (implying permission)
        let result = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM file_analysis
            WHERE path = ?
            "#,
        )
        .bind(path)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = result.get("count");

        // If file exists in our analysis database, user has access
        // In a real system, you'd check a user_permissions table:
        // SELECT COUNT(*) FROM user_permissions WHERE user_id = ? AND file_path = ? AND permission = 'read'
        Ok(count > 0)
    }

    pub async fn vacuum(&self) -> Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn flush(&self) -> Result<()> {
        // Force WAL checkpoint to prevent disk bloat - TRUNCATE mode is more aggressive
        // This ensures WAL files don't grow indefinitely
        match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await 
        {
            Ok(_) => {
                tracing::debug!("WAL checkpoint completed successfully");
            }
            Err(e) => {
                tracing::warn!("WAL checkpoint failed, attempting fallback: {}", e);
                // Fallback to RESTART mode if TRUNCATE fails
                sqlx::query("PRAGMA wal_checkpoint(RESTART)")
                    .execute(&self.pool)
                    .await
                    .map_err(|fallback_err| AppError::DatabaseError {
                        message: format!("WAL checkpoint failed completely. Original: {}, Fallback: {}", e, fallback_err)
                    })?;
                tracing::info!("WAL checkpoint fallback to RESTART mode succeeded");
            }
        }
        Ok(())
    }

    /// Perform aggressive WAL cleanup to reclaim disk space
    pub async fn cleanup_wal_files(&self) -> Result<()> {
        tracing::info!("Starting aggressive WAL cleanup to prevent disk bloat");
        
        // Force checkpoint with TRUNCATE to reset WAL files
        self.flush().await?;
        
        // Additional cleanup operations
        let cleanup_queries = [
            "PRAGMA optimize",           // Analyze and optimize database
            "PRAGMA wal_checkpoint(TRUNCATE)", // Second checkpoint attempt
            "VACUUM",                   // Reclaim space from deleted records
        ];
        
        for query in &cleanup_queries {
            match sqlx::query(query).execute(&self.pool).await {
                Ok(_) => tracing::debug!("WAL cleanup query '{}' succeeded", query),
                Err(e) => tracing::warn!("WAL cleanup query '{}' failed: {}", query, e),
            }
            
            // Small delay between operations to prevent overwhelming the database
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        tracing::info!("WAL cleanup completed");
        Ok(())
    }

    /// Get reference to the database pool
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn database_path<R: Runtime>(handle: &AppHandle<R>) -> Result<PathBuf> {
        // Legacy sync method - use database_path_with_fallbacks for new code
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(Self::database_path_with_fallbacks(handle))
        })
    }

    async fn database_path_with_fallbacks<R: Runtime>(handle: &AppHandle<R>) -> Result<PathBuf> {
        // Try multiple directory options with comprehensive fallbacks
        let directory_options = vec![
            handle.path().app_data_dir(),
            handle.path().app_local_data_dir(),
            handle.path().app_cache_dir(),
            Ok(std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("data")),
            Ok(std::path::PathBuf::from("./stratosort_data")),
            Ok(std::env::temp_dir().join("stratosort")),
        ];

        for app_dir in directory_options.into_iter().flatten() {
            tracing::info!("Trying database directory: {:?}", app_dir);

            // Test if we can create and write to this directory
            if (Self::ensure_database_directory(&app_dir).await).is_ok() {
                let db_path = app_dir.join("stratosort.db");
                tracing::info!("Database path resolved to: {:?}", db_path);
                return Ok(db_path);
            }
        }

        // If all else fails, use in-memory database (warning: data will be lost on restart)
        tracing::error!(
            "All database directory options failed, falling back to in-memory database"
        );
        Err(AppError::DatabaseError {
            message: "Failed to find suitable database directory. All locations are inaccessible."
                .to_string(),
        })
    }

    async fn ensure_database_directory(dir: &std::path::Path) -> Result<()> {
        // Create directory if it doesn't exist
        if !dir.exists() {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| AppError::DatabaseError {
                    message: format!("Failed to create directory '{}': {}", dir.display(), e),
                })?
        }

        // Test write permissions with automatic cleanup
        let test_file = dir.join(".write_test");
        tokio::fs::write(&test_file, "test")
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Directory '{}' is not writable: {}", dir.display(), e),
            })?;

        // Clean up test file - ensure it's removed even on error
        if let Err(e) = tokio::fs::remove_file(&test_file).await {
            tracing::warn!("Failed to cleanup database test file {:?}: {}", test_file, e);
        }

        Ok(())
    }

    fn get_fallback_database_paths() -> Vec<PathBuf> {
        vec![
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("data"),
            std::path::PathBuf::from("./stratosort_data"),
            std::env::temp_dir().join("stratosort"),
        ]
    }

    async fn try_fallback_database(path: &Path) -> Result<PathBuf> {
        Self::ensure_database_directory(path).await?;
        Ok(path.join("stratosort.db"))
    }

    async fn create_database_connection_with_retry(db_path: &PathBuf) -> Result<SqlitePool> {
        let database_url = format!(
            "sqlite:{}?mode=rwc&journal_mode=WAL&busy_timeout=5000&synchronous=NORMAL",
            db_path.to_string_lossy()
        );

        let mut retry_count: u64 = 0;
        let max_retries = 5;

        loop {
            // Create connection options with progressive timeout increases - FIXED OVERFLOW
            let timeout_seconds = 5u64.saturating_add(retry_count.saturating_mul(2).min(30));
            let connection_options = database_url
                .parse::<sqlx::sqlite::SqliteConnectOptions>()
                .map_err(|e| AppError::DatabaseError {
                    message: format!("Invalid database URL: {}", e),
                })?
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                .busy_timeout(Duration::from_secs(timeout_seconds))
                .pragma("cache_size", "10000")
                .pragma("temp_store", "memory")
                .pragma("mmap_size", "268435456")
                .pragma("journal_size_limit", "67108864"); // 64MB journal limit

            match SqlitePool::connect_with(connection_options).await {
                Ok(pool) => {
                    tracing::info!(
                        "Database connection established successfully on attempt {}",
                        retry_count + 1
                    );
                    return Ok(pool);
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        tracing::error!(
                            "Database connection failed after {} attempts: {}",
                            max_retries,
                            e
                        );
                        return Err(AppError::DatabaseError {
                            message: format!(
                                "Failed to connect to database at {:?} after {} attempts: {}",
                                db_path, max_retries, e
                            ),
                        });
                    }

                    tracing::warn!(
                        "Database connection attempt {} failed, retrying in {}ms: {}",
                        retry_count,
                        500 * retry_count,
                        e
                    );
                    tokio::time::sleep(Duration::from_millis(500 * retry_count)).await;
                }
            }
        }
    }

    async fn verify_database_connection(pool: &SqlitePool) -> Result<()> {
        let mut attempts = 0;
        let max_attempts = 3;

        loop {
            match sqlx::query("SELECT 1").fetch_one(pool).await {
                Ok(_) => {
                    tracing::info!("Database connection verified successfully");
                    return Ok(());
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        tracing::error!(
                            "Database connection verification failed after {} attempts: {}",
                            max_attempts,
                            e
                        );
                        return Err(AppError::DatabaseError {
                            message: format!("Database connection verification failed after {} attempts: {}. Database may be corrupted.", max_attempts, e),
                        });
                    }
                    tracing::warn!(
                        "Database verification attempt {} failed, retrying: {}",
                        attempts,
                        e
                    );
                    tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
                }
            }
        }
    }

    async fn initialize_vector_extension_safely(pool: &SqlitePool) -> VectorExtension {
        // Non-blocking vector extension initialization
        match tokio::time::timeout(Duration::from_secs(10), VectorExtension::initialize(pool)).await
        {
            Ok(vector_ext) => {
                tracing::info!("Vector extension initialized successfully");
                vector_ext
            }
            Err(_) => {
                tracing::warn!("Vector extension initialization timed out, using fallback");
                VectorExtension::fallback()
            }
        }
    }

    async fn check_integrity_with_recovery(&self) -> Result<()> {
        match self.check_integrity().await {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::warn!("Integrity check failed: {}, attempting recovery", e);
                self.attempt_database_recovery().await
            }
        }
    }

    async fn attempt_database_recovery(&self) -> Result<()> {
        tracing::info!("Attempting database recovery");

        // Try basic recovery steps
        let recovery_steps = [
            "PRAGMA integrity_check",
            "PRAGMA quick_check",
            "PRAGMA wal_checkpoint(RESTART)",
            "VACUUM",
        ];

        for step in &recovery_steps {
            match sqlx::query(step).execute(&self.pool).await {
                Ok(_) => tracing::info!("Recovery step '{}' succeeded", step),
                Err(e) => tracing::warn!("Recovery step '{}' failed: {}", step, e),
            }
        }

        // Final verification
        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => {
                tracing::info!("Database recovery successful");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Database recovery failed: {}", e);
                Err(AppError::DatabaseError {
                    message: format!("Database recovery failed: {}", e),
                })
            }
        }
    }

    async fn run_migrations_atomically(&self) -> Result<()> {
        // Enhanced migration with better error handling and atomicity
        self.run_migrations().await
    }

    /// Run database migrations to keep schema up to date
    async fn run_migrations(&self) -> Result<()> {
        // Create schema_version table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Get current schema version
        let current_version = self.get_schema_version().await?;
        info!("Current database schema version: {}", current_version);

        if current_version == 0 {
            // Initial schema creation with transaction
            info!("Creating initial database schema");
            let mut tx = self.pool.begin().await?;

            match self.initialize_schema_in_transaction(&mut tx).await {
                Ok(_) => {
                    sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
                        .bind(1)
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                    info!("Initial schema created successfully");

                    // Create vector table after transaction commits (requires pool)
                    if self.vector_ext.is_available {
                        if let Err(e) = self
                            .vector_ext
                            .create_vector_table(
                                &self.pool,
                                "vec_embeddings",
                                self.vector_ext.get_dimensions(),
                            )
                            .await
                        {
                            warn!(
                                "Failed to create vector table: {}. Will use fallback storage.",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    tx.rollback().await?;
                    return Err(AppError::DatabaseError {
                        message: format!("Failed to create initial schema: {}", e),
                    });
                }
            }
        }

        // Run incremental migrations with transactions
        if current_version < 2 {
            info!("Running migration to version 2");
            let mut tx = self.pool.begin().await?;

            match self.migrate_to_v2_in_transaction(&mut tx).await {
                Ok(_) => {
                    sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
                        .bind(2)
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                    info!("Migration to v2 completed successfully");
                }
                Err(e) => {
                    tx.rollback().await?;
                    warn!("Migration to v2 failed, rolled back: {}", e);
                }
            }
        }

        // Future migrations go here
        if current_version < 3 {
            info!("Running migration to version 3 - Enhanced operations support");
            let mut tx = self.pool.begin().await?;

            match self.migrate_to_v3_in_transaction(&mut tx).await {
                Ok(_) => {
                    sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
                        .bind(3)
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                    info!("Migration to v3 completed successfully");
                }
                Err(e) => {
                    tx.rollback().await?;
                    warn!("Migration to v3 failed, rolled back: {}", e);
                }
            }
        }

        info!("Database migrations completed successfully");
        Ok(())
    }

    async fn get_schema_version(&self) -> Result<i32> {
        let result = sqlx::query("SELECT MAX(version) as version FROM schema_version")
            .fetch_optional(&self.pool)
            .await?;

        match result {
            Some(row) => {
                let version: Option<i32> = row.get("version");
                Ok(version.unwrap_or(0))
            }
            None => Ok(0),
        }
    }

    #[allow(dead_code)]
    async fn set_schema_version(&self, version: i32) -> Result<()> {
        sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
            .bind(version)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Initialize schema within a transaction for atomicity
    async fn initialize_schema_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<()> {
        // Create tables within transaction
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS file_analysis (
                path TEXT PRIMARY KEY,
                category TEXT NOT NULL,
                tags TEXT NOT NULL,
                summary TEXT NOT NULL,
                confidence REAL NOT NULL,
                extracted_text TEXT,
                detected_language TEXT,
                metadata TEXT,
                analyzed_at INTEGER NOT NULL,
                embedding BLOB
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS smart_folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                rules TEXT NOT NULL,
                icon TEXT,
                color TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS operations_history (
                id TEXT PRIMARY KEY,
                operation_type TEXT NOT NULL,
                source TEXT NOT NULL,
                destination TEXT,
                timestamp INTEGER NOT NULL,
                metadata TEXT
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        // Create indexes within transaction
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_analysis_category ON file_analysis(category)")
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_analysis_analyzed_at ON file_analysis(analyzed_at)",
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_operations_timestamp ON operations_history(timestamp)",
        )
        .execute(&mut **tx)
        .await?;

        // Note: Vector table creation will be done after transaction commits
        // as it requires the pool, not a transaction

        Ok(())
    }

    /// Migrate to version 2 within a transaction
    async fn migrate_to_v2_in_transaction(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<()> {
        // V2 migration logic here
        // This is a placeholder for future v2 changes
        Ok(())
    }

    /// Migrate to version 3 within a transaction
    async fn migrate_to_v3_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<()> {
        info!("Starting migration to v3 - Enhanced operations support");

        // Enhanced smart folders table with new fields
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS smart_folders_v3 (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                rules TEXT NOT NULL,
                target_path TEXT NOT NULL,
                enabled BOOLEAN DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        // Enhanced embeddings table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS embeddings_v3 (
                path TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                model_name TEXT DEFAULT 'unknown',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        // Operation history with enhanced metadata
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS operations_history_v3 (
                id TEXT PRIMARY KEY,
                operation_type TEXT NOT NULL,
                source_paths TEXT NOT NULL,
                target_paths TEXT,
                backup_data BLOB,
                metadata TEXT,
                can_undo BOOLEAN DEFAULT 1,
                can_redo BOOLEAN DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        // Notifications table for user feedback
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                title TEXT NOT NULL,
                message TEXT NOT NULL,
                metadata TEXT,
                read BOOLEAN DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        // Search history table for tracking search queries
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS search_history (
                id TEXT PRIMARY KEY,
                query TEXT NOT NULL,
                search_type TEXT NOT NULL,
                result_count INTEGER DEFAULT 0,
                timestamp INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;

        // Create indexes for v3 tables
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_smart_folders_enabled ON smart_folders_v3(enabled)",
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_embeddings_created ON embeddings_v3(created_at)",
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_operations_created ON operations_history_v3(created_at)")
            .execute(&mut **tx)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(read)")
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_search_history_timestamp ON search_history(timestamp)",
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_search_history_query ON search_history(query)")
            .execute(&mut **tx)
            .await?;

        info!("Migration to v3 completed");
        Ok(())
    }

    #[allow(dead_code)]
    async fn migrate_to_v3(&self) -> Result<()> {
        info!("Starting migration to v3 - Enhanced operations support");

        // Enhanced smart folders table with new fields
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS smart_folders_v3 (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                rules TEXT NOT NULL,
                target_path TEXT NOT NULL,
                enabled BOOLEAN DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Enhanced file analysis table with timestamps
        sqlx::query(
            r#"
            ALTER TABLE file_analysis ADD COLUMN created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            "#,
        )
        .execute(&self.pool)
        .await
        .ok(); // Ignore error if column already exists

        sqlx::query(
            r#"
            ALTER TABLE file_analysis ADD COLUMN updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            "#,
        )
        .execute(&self.pool)
        .await
        .ok();

        // Enhanced embeddings table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS embeddings_v3 (
                path TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                model_name TEXT DEFAULT 'unknown',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Operation history with enhanced metadata
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS operations_history_v3 (
                id TEXT PRIMARY KEY,
                operation_type TEXT NOT NULL,
                source_paths TEXT NOT NULL,
                target_paths TEXT,
                backup_data BLOB,
                metadata TEXT,
                can_undo BOOLEAN DEFAULT 1,
                can_redo BOOLEAN DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Notifications table for user feedback
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                notification_type TEXT NOT NULL,
                title TEXT NOT NULL,
                message TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                read BOOLEAN DEFAULT 0,
                actions TEXT DEFAULT '[]',
                metadata TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for performance
        let indexes = [
            ("idx_smart_folders_enabled", "smart_folders_v3", "enabled"),
            (
                "idx_smart_folders_created",
                "smart_folders_v3",
                "created_at",
            ),
            ("idx_embeddings_created", "embeddings_v3", "created_at"),
            (
                "idx_operations_type",
                "operations_history_v3",
                "operation_type",
            ),
            (
                "idx_operations_created",
                "operations_history_v3",
                "created_at",
            ),
            ("idx_file_analysis_created", "file_analysis", "created_at"),
            ("idx_notifications_timestamp", "notifications", "timestamp"),
            ("idx_notifications_read", "notifications", "read"),
        ];

        for (index_name, table_name, column_name) in &indexes {
            // Validate SQL identifiers before using format!
            if !is_valid_sql_identifier(index_name)
                || !is_valid_sql_identifier(table_name)
                || !is_valid_sql_identifier(column_name)
            {
                warn!(
                    "Skipping invalid SQL identifier: {}.{}.{}",
                    table_name, column_name, index_name
                );
                continue;
            }

            // Now safe to use format! after validation
            if let Err(e) = sqlx::query(&format!(
                "CREATE INDEX IF NOT EXISTS {} ON {}({})",
                index_name, table_name, column_name
            ))
            .execute(&self.pool)
            .await
            {
                warn!("Failed to create index {}: {}", index_name, e);
            }
        }

        info!("Migration to v3 completed - Enhanced operations support ready");
        Ok(())
    }

    // Smart folder operations
    pub async fn save_smart_folder(
        &self,
        folder: &crate::core::smart_folders::SmartFolder,
    ) -> Result<()> {
        let query = r#"
            INSERT OR REPLACE INTO smart_folders_v3 
            (id, name, description, rules, target_path, enabled, created_at, updated_at) 
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let rules_json =
            serde_json::to_string(&folder.rules).map_err(|e| AppError::ParseError {
                message: format!("Failed to serialize rules: {}", e),
            })?;

        sqlx::query(query)
            .bind(&folder.id)
            .bind(&folder.name)
            .bind(&folder.description)
            .bind(&rules_json)
            .bind(&folder.target_path)
            .bind(folder.enabled)
            .bind(folder.created_at)
            .bind(folder.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to save smart folder: {}", e),
            })?;

        Ok(())
    }

    pub async fn get_smart_folder(
        &self,
        id: &str,
    ) -> Result<Option<crate::core::smart_folders::SmartFolder>> {
        let query = "SELECT * FROM smart_folders_v3 WHERE id = ?";

        let row = sqlx::query(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to get smart folder: {}", e),
            })?;

        if let Some(row) = row {
            let rules_json: String = row.get("rules");
            let rules = serde_json::from_str(&rules_json).map_err(|e| AppError::ParseError {
                message: format!("Failed to deserialize rules: {}", e),
            })?;

            Ok(Some(crate::core::smart_folders::SmartFolder {
                id: row.get("id"),
                name: row.get("name"),
                path: row.get("path"),
                target_path: row.get("target_path"),
                description: row.get("description"),
                enabled: row.get("enabled"),
                rules,
                icon: row.get("icon"),
                color: row.get("color"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_smart_folders(
        &self,
    ) -> Result<Vec<crate::core::smart_folders::SmartFolder>> {
        let query = "SELECT * FROM smart_folders_v3 ORDER BY name";

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to list smart folders: {}", e),
            })?;

        let mut folders = Vec::new();
        for row in rows {
            let rules_json: String = row.get("rules");
            let rules = serde_json::from_str(&rules_json).map_err(|e| AppError::ParseError {
                message: format!("Failed to deserialize rules: {}", e),
            })?;

            folders.push(crate::core::smart_folders::SmartFolder {
                id: row.get("id"),
                name: row.get("name"),
                path: row.get("path"),
                target_path: row.get("target_path"),
                description: row.get("description"),
                enabled: row.get("enabled"),
                rules,
                icon: row.get("icon"),
                color: row.get("color"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(folders)
    }

    pub async fn delete_smart_folder(&self, id: &str) -> Result<()> {
        let query = "DELETE FROM smart_folders_v3 WHERE id = ?";

        sqlx::query(query)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to delete smart folder: {}", e),
            })?;

        Ok(())
    }

    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Database health check failed: {}", e),
            })?;
        Ok(())
    }

    pub async fn clear_cache(&self) -> Result<()> {
        let queries = [
            "DELETE FROM file_analysis WHERE created_at < datetime('now', '-7 days')",
            "DELETE FROM embeddings_v3 WHERE created_at < datetime('now', '-7 days')",
            "VACUUM", // Reclaim space
        ];

        for query in &queries {
            sqlx::query(query)
                .execute(&self.pool)
                .await
                .map_err(|e| AppError::DatabaseError {
                    message: format!("Failed to clear cache: {}", e),
                })?;
        }

        Ok(())
    }

    /// Create database from URL (useful for testing and custom setups)
    pub async fn new_from_url(url: &str) -> Result<Self> {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        let options = SqliteConnectOptions::from_str(url)
            .map_err(|e| AppError::DatabaseError {
                message: format!("Invalid database URL: {}", e),
            })?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));

        let pool =
            SqlitePool::connect_with(options)
                .await
                .map_err(|e| AppError::DatabaseError {
                    message: format!("Failed to connect to database: {}", e),
                })?;

        // Initialize vector extension
        let vector_ext = Arc::new(VectorExtension::initialize(&pool).await);

        let db = Self { pool, vector_ext };

        // Check integrity and run migrations
        db.check_integrity().await?;
        db.run_migrations().await?;

        info!("Database initialized from URL successfully");
        Ok(db)
    }

    /// Get vector extension statistics
    pub async fn get_vector_stats(&self) -> Result<VectorStats> {
        self.vector_ext
            .get_vector_stats(&self.pool, "vec_embeddings")
            .await
    }

    /// Check if vector extension is available
    pub fn is_vector_extension_available(&self) -> bool {
        self.vector_ext.is_available
    }

    /// Get vector extension version
    pub fn get_vector_extension_version(&self) -> Option<String> {
        self.vector_ext.version.clone()
    }

    /// Perform vector table maintenance
    pub async fn maintain_vector_table(&self) -> Result<()> {
        if self.vector_ext.is_available {
            self.vector_ext
                .vacuum_vector_table(&self.pool, "vec_embeddings")
                .await?;
        }
        Ok(())
    }

    // Notification-related methods
    pub async fn save_notification(
        &self,
        notification: &crate::commands::notifications::Notification,
    ) -> Result<()> {
        let query = r#"
            INSERT OR REPLACE INTO notifications 
            (id, notification_type, title, message, timestamp, read, actions, metadata) 
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let notification_type = match notification.notification_type {
            crate::commands::notifications::NotificationType::Success => "success",
            crate::commands::notifications::NotificationType::Info => "info",
            crate::commands::notifications::NotificationType::Warning => "warning",
            crate::commands::notifications::NotificationType::Error => "error",
            crate::commands::notifications::NotificationType::Progress => "progress",
        };

        let actions_json =
            serde_json::to_string(&notification.actions).map_err(|e| AppError::ParseError {
                message: format!("Failed to serialize notification actions: {}", e),
            })?;

        let metadata_json = notification
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| AppError::ParseError {
                message: format!("Failed to serialize notification metadata: {}", e),
            })?;

        sqlx::query(query)
            .bind(&notification.id)
            .bind(notification_type)
            .bind(&notification.title)
            .bind(&notification.message)
            .bind(notification.timestamp)
            .bind(notification.read)
            .bind(&actions_json)
            .bind(&metadata_json)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to save notification: {}", e),
            })?;

        Ok(())
    }

    pub async fn get_notifications(
        &self,
        limit: usize,
        unread_only: bool,
    ) -> Result<Vec<crate::commands::notifications::Notification>> {
        let query = if unread_only {
            "SELECT * FROM notifications WHERE read = 0 ORDER BY timestamp DESC LIMIT ?"
        } else {
            "SELECT * FROM notifications ORDER BY timestamp DESC LIMIT ?"
        };

        let rows = sqlx::query(query)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to get notifications: {}", e),
            })?;

        let mut notifications = Vec::new();
        for row in rows {
            let notification_type_str: String = row.get("notification_type");
            let notification_type = match notification_type_str.as_str() {
                "success" => crate::commands::notifications::NotificationType::Success,
                "info" => crate::commands::notifications::NotificationType::Info,
                "warning" => crate::commands::notifications::NotificationType::Warning,
                "error" => crate::commands::notifications::NotificationType::Error,
                "progress" => crate::commands::notifications::NotificationType::Progress,
                _ => crate::commands::notifications::NotificationType::Info,
            };

            let actions_json: String = row.get("actions");
            let actions =
                serde_json::from_str(&actions_json).map_err(|e| AppError::ParseError {
                    message: format!("Failed to deserialize notification actions: {}", e),
                })?;

            let metadata_json: Option<String> = row.get("metadata");
            let metadata = metadata_json
                .map(|json| serde_json::from_str(&json))
                .transpose()
                .map_err(|e| AppError::ParseError {
                    message: format!("Failed to deserialize notification metadata: {}", e),
                })?;

            notifications.push(crate::commands::notifications::Notification {
                id: row.get("id"),
                notification_type,
                title: row.get("title"),
                message: row.get("message"),
                timestamp: row.get("timestamp"),
                read: row.get("read"),
                actions,
                metadata,
            });
        }

        Ok(notifications)
    }

    pub async fn mark_notification_read(&self, notification_id: &str) -> Result<()> {
        let query = "UPDATE notifications SET read = 1 WHERE id = ?";

        sqlx::query(query)
            .bind(notification_id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to mark notification as read: {}", e),
            })?;

        Ok(())
    }

    pub async fn clear_old_notifications(&self, cutoff_timestamp: i64) -> Result<usize> {
        let query = "DELETE FROM notifications WHERE timestamp < ?";

        let result = sqlx::query(query)
            .bind(cutoff_timestamp)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to clear old notifications: {}", e),
            })?;

        Ok(result.rows_affected() as usize)
    }

    /// Clear all data from the database (for testing or reset)
    pub async fn clear_all_data(&self) -> Result<()> {
        tracing::warn!("Clearing all data from database");

        // Delete from all tables in correct order (respecting foreign key constraints)
        // First delete from tables with foreign key references
        sqlx::query("DELETE FROM embeddings_v3")
            .execute(&self.pool)
            .await
            .ok(); // Ignore if table doesn't exist

        sqlx::query("DELETE FROM operations_history_v3")
            .execute(&self.pool)
            .await
            .ok();

        sqlx::query("DELETE FROM operations_history")
            .execute(&self.pool)
            .await
            .ok();

        sqlx::query("DELETE FROM notifications")
            .execute(&self.pool)
            .await
            .ok();

        sqlx::query("DELETE FROM smart_folders_v3")
            .execute(&self.pool)
            .await
            .ok();

        sqlx::query("DELETE FROM smart_folders")
            .execute(&self.pool)
            .await
            .ok();

        sqlx::query("DELETE FROM file_analysis")
            .execute(&self.pool)
            .await
            .ok();

        tracing::info!("All data cleared from database");
        Ok(())
    }

    /// Close database connections gracefully
    pub async fn close_connections(&self) -> Result<()> {
        tracing::info!("Closing database connections");
        self.pool.close().await;
        tracing::info!("Database connections closed");
        Ok(())
    }

    /// Search files by filename (quick search)
    pub async fn search_by_filename(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let search_pattern = format!("%{}%", query.to_lowercase());

        let rows = sqlx::query(
            r#"
            SELECT path FROM file_analysis
            WHERE LOWER(path) LIKE ? COLLATE NOCASE
            ORDER BY 
                CASE 
                    WHEN LOWER(path) = LOWER(?) THEN 0
                    WHEN LOWER(path) LIKE LOWER(?) THEN 1
                    ELSE 2
                END,
                analyzed_at DESC
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(query)
        .bind(format!("{}%", query.to_lowercase())) // Start with query
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|row| row.get("path")).collect())
    }

    /// Save search history entry
    pub async fn save_search_history(
        &self,
        entry: &crate::commands::ai::SearchHistoryEntry,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO search_history 
            (id, query, search_type, result_count, timestamp) 
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&entry.id)
        .bind(&entry.query)
        .bind(&entry.search_type)
        .bind(entry.result_count as i64)
        .bind(entry.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get search history
    pub async fn get_search_history(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::commands::ai::SearchHistoryEntry>> {
        let rows = sqlx::query(
            r#"
            SELECT id, query, search_type, result_count, timestamp 
            FROM search_history 
            ORDER BY timestamp DESC 
            LIMIT ?
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(crate::commands::ai::SearchHistoryEntry {
                id: row.get("id"),
                query: row.get("query"),
                search_type: row.get("search_type"),
                result_count: row.get::<i64, _>("result_count") as usize,
                timestamp: row.get("timestamp"),
            });
        }

        Ok(entries)
    }

    /// Clear search history older than timestamp
    pub async fn clear_search_history(&self, cutoff_timestamp: i64) -> Result<usize> {
        let result = sqlx::query("DELETE FROM search_history WHERE timestamp < ?")
            .bind(cutoff_timestamp)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() as usize)
    }

    /// Create a backup of the database
    pub async fn backup_database(&self) -> Result<PathBuf> {
        use chrono::Utc;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");

        // Get backup directory relative to current database location
        let backup_dir = PathBuf::from("./data/backups");
        tokio::fs::create_dir_all(&backup_dir).await?;

        let backup_path = backup_dir.join(format!("stratosort_{}.db", timestamp));

        // Use VACUUM INTO for atomic backup
        let backup_path_str = backup_path.to_str()
            .ok_or(AppError::DatabaseError {
                message: "Invalid backup path".to_string()
            })?;

        sqlx::query(&format!("VACUUM INTO '{}'", backup_path_str))
            .execute(&self.pool).await?;

        // Verify backup
        let metadata = tokio::fs::metadata(&backup_path).await?;
        if metadata.len() == 0 {
            return Err(AppError::DatabaseError {
                message: "Backup file is empty".to_string()
            });
        }

        // Clean old backups (keep last 5)
        self.clean_old_backups(&backup_dir, 5).await?;

        tracing::info!("Database backup created: {:?}", backup_path);
        Ok(backup_path)
    }

    /// Clean old backup files, keeping only the specified number of most recent ones
    async fn clean_old_backups(&self, dir: &Path, keep: usize) -> Result<()> {
        use std::ffi::OsStr;

        let mut entries = tokio::fs::read_dir(dir).await?;
        let mut backups = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension() == Some(OsStr::new("db")) {
                if let Ok(metadata) = entry.metadata().await {
                    backups.push((entry.path(), metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)));
                }
            }
        }

        // Sort by modification time, newest first
        backups.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove old backups
        for (old_backup, _) in backups.iter().skip(keep) {
            if let Err(e) = tokio::fs::remove_file(old_backup).await {
                tracing::warn!("Failed to remove old backup {:?}: {}", old_backup, e);
            }
        }

        Ok(())
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            vector_ext: Arc::clone(&self.vector_ext),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub operation_type: String,
    pub source: String,
    pub destination: Option<String>,
    pub timestamp: i64,
    pub metadata: Option<serde_json::Value>,
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    // CRITICAL FIX: Proper division by zero prevention
    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }
    
    let denominator = magnitude_a * magnitude_b;
    if denominator == 0.0 || !denominator.is_finite() {
        tracing::warn!("Vector similarity calculation: invalid denominator detected");
        return 0.0;
    }

    let similarity = dot_product / denominator;
    
    // Additional safety check for NaN/infinite results
    if !similarity.is_finite() {
        tracing::warn!("Vector similarity calculation produced non-finite result");
        return 0.0;
    }
    
    similarity
}
