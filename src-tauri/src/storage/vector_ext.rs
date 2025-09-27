use crate::error::{AppError, Result};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use tracing::{debug, info, warn};

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag to track if sqlite-vec extension is available
static VEC_EXTENSION_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Vector extension manager for sqlite-vec
pub struct VectorExtension {
    pub is_available: bool,
    pub version: Option<String>,
    pub embedding_dimensions: usize,
    pub use_quantization: bool,
    pub batch_size: usize,
}

impl VectorExtension {
    /// Initialize sqlite-vec extension and check availability
    pub async fn initialize(pool: &SqlitePool) -> Self {
        let mut extension = Self {
            is_available: false,
            version: None,
            embedding_dimensions: 384, // Updated to match nomic-embed-text model
            use_quantization: true,    // Enable quantization for better performance
            batch_size: 50,            // Optimized batch size for Ollama embeddings
        };

        // Try to initialize sqlite-vec extension
        match Self::try_load_extension(pool).await {
            Ok(version) => {
                extension.is_available = true;
                extension.version = Some(version.clone());
                VEC_EXTENSION_AVAILABLE.store(true, Ordering::Relaxed);
                info!(
                    "sqlite-vec extension loaded successfully, version: {}",
                    version
                );
            }
            Err(e) => {
                warn!("sqlite-vec extension not available: {}. Falling back to manual similarity calculations.", e);
                VEC_EXTENSION_AVAILABLE.store(false, Ordering::Relaxed);
            }
        }

        extension
    }

    /// Create a fallback VectorExtension when the sqlite-vec extension is not available
    pub fn fallback() -> Self {
        Self {
            is_available: false,
            version: None,
            embedding_dimensions: 384,
            use_quantization: false,
            batch_size: 50,
        }
    }

    /// Attempt to load and verify sqlite-vec extension
    async fn try_load_extension(pool: &SqlitePool) -> Result<String> {
        // Initialize sqlite-vec extension using the proper FFI approach
        // Note: This would typically be done at connection level, but we'll check if it's available

        // Test if vec functions are available
        let version_result = sqlx::query_scalar::<_, String>("SELECT vec_version()")
            .fetch_one(pool)
            .await;

        match version_result {
            Ok(version) => {
                debug!("sqlite-vec version detected: {}", version);
                Ok(version)
            }
            Err(_) => {
                // Try to see if we can load the extension manually
                // This is a fallback approach - in production, you'd typically
                // initialize the extension when creating the connection

                debug!("Attempting to check sqlite-vec availability through alternative method");

                // If no vec functions available, return error
                Err(AppError::DatabaseError {
                    message: "sqlite-vec extension functions not available".to_string(),
                })
            }
        }
    }

    /// Create vector table using modern sqlite-vec syntax
    pub async fn create_vector_table(
        &self,
        pool: &SqlitePool,
        table_name: &str,
        dimensions: usize,
    ) -> Result<()> {
        if !self.is_available {
            return Err(AppError::DatabaseError {
                message: "Vector extension not available".to_string(),
            });
        }

        // Validate table name to prevent SQL injection
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        // Safe to use format! after validation
        let create_query = format!(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS {} USING vec0(
                path TEXT PRIMARY KEY,
                embedding FLOAT[{}]
            )
            "#,
            table_name, dimensions
        );

        sqlx::query(&create_query)
            .execute(pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to create vector table {}: {}", table_name, e),
            })?;

        info!(
            "Created vector table: {} with {} dimensions",
            table_name, dimensions
        );
        Ok(())
    }

    /// Store vector embedding using sqlite-vec
    pub async fn store_embedding(
        &self,
        pool: &SqlitePool,
        table_name: &str,
        path: &str,
        embedding: &[f32],
    ) -> Result<()> {
        if !self.is_available {
            return Err(AppError::DatabaseError {
                message: "Vector extension not available".to_string(),
            });
        }

        // Validate table name to prevent SQL injection
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        if embedding.len() != self.embedding_dimensions {
            return Err(AppError::InvalidInput {
                message: format!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    self.embedding_dimensions,
                    embedding.len()
                ),
            });
        }

        // Convert embedding to bytes for sqlite-vec
        let embedding_bytes = Self::f32_vec_to_bytes(embedding);

        // Safe to use format! after validation
        let insert_query = format!(
            "INSERT OR REPLACE INTO {} (path, embedding) VALUES (?, ?)",
            table_name
        );

        sqlx::query(&insert_query)
            .bind(path)
            .bind(embedding_bytes)
            .execute(pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to store embedding for {}: {}", path, e),
            })?;

        Ok(())
    }

    /// Perform vector similarity search using sqlite-vec
    pub async fn vector_search(
        &self,
        pool: &SqlitePool,
        table_name: &str,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        if !self.is_available {
            return Err(AppError::DatabaseError {
                message: "Vector extension not available".to_string(),
            });
        }

        // SECURITY: SQL injection prevention - validate table name
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        // SECURITY: Additional whitelist check for defense in depth
        const ALLOWED_TABLES: &[&str] = &["vec_embeddings", "file_embeddings", "search_history"];
        if !ALLOWED_TABLES.contains(&table_name) {
            return Err(AppError::SecurityError {
                message: format!("Table '{}' is not in allowed list for vector search", table_name),
            });
        }

        if query_embedding.len() != self.embedding_dimensions {
            return Err(AppError::InvalidInput {
                message: format!(
                    "Query embedding dimension mismatch: expected {}, got {}",
                    self.embedding_dimensions,
                    query_embedding.len()
                ),
            });
        }

        let query_bytes = Self::f32_vec_to_bytes(query_embedding);

        // SECURITY: Safe to use format! after validation and whitelist check - sqlite-vec's distance function for similarity search
        let search_query = format!(
            r#"
            SELECT path, vec_distance_cosine(embedding, ?) as distance
            FROM {}
            ORDER BY distance
            LIMIT ?
            "#,
            table_name
        );

        let rows = sqlx::query(&search_query)
            .bind(query_bytes)
            .bind(limit as i32)
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Vector search failed: {}", e),
            })?;

        let mut results = Vec::new();
        for row in rows {
            let path: String = row.get("path");
            let distance: f32 = row.get("distance");
            // Convert distance to similarity (cosine distance -> cosine similarity)
            let similarity = 1.0 - distance;
            results.push((path, similarity));
        }

        Ok(results)
    }

    /// Convert f32 vector to bytes for sqlite-vec storage
    fn f32_vec_to_bytes(embedding: &[f32]) -> Vec<u8> {
        use zerocopy::AsBytes;
        embedding.as_bytes().to_vec()
    }

    /// Convert bytes back to f32 vector
    pub fn bytes_to_f32_vec(bytes: &[u8]) -> Result<Vec<f32>> {
        if bytes.len() % 4 != 0 {
            return Err(AppError::ParseError {
                message: "Invalid embedding bytes length".to_string(),
            });
        }

        Ok(bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect())
    }

    /// Check if vector extension is globally available
    pub fn is_globally_available() -> bool {
        VEC_EXTENSION_AVAILABLE.load(Ordering::Relaxed)
    }

    /// Get embedding dimensions
    pub fn get_dimensions(&self) -> usize {
        self.embedding_dimensions
    }

    /// Set embedding dimensions (should be done during initialization)
    pub fn set_dimensions(&mut self, dimensions: usize) {
        self.embedding_dimensions = dimensions;
        info!("Updated embedding dimensions to: {}", dimensions);
    }

    /// Perform maintenance operations on vector table
    pub async fn vacuum_vector_table(&self, pool: &SqlitePool, table_name: &str) -> Result<()> {
        if !self.is_available {
            return Ok(()); // Skip if extension not available
        }

        // Note: sqlite-vec doesn't require special vacuum operations
        // but we can perform general maintenance
        let vacuum_query = "VACUUM";
        sqlx::query(vacuum_query)
            .execute(pool)
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to vacuum vector table: {}", e),
            })?;

        debug!("Vacuumed vector table: {}", table_name);
        Ok(())
    }

    /// Store multiple embeddings in a batch for better performance
    pub async fn store_embeddings_batch(
        &self,
        pool: &SqlitePool,
        table_name: &str,
        embeddings: &[(String, Vec<f32>)],
    ) -> Result<usize> {
        if !self.is_available {
            return Err(AppError::DatabaseError {
                message: "Vector extension not available".to_string(),
            });
        }

        // Validate table name to prevent SQL injection
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        if embeddings.is_empty() {
            return Ok(0);
        }

        let mut stored_count = 0;
        let mut tx = pool.begin().await?;

        // Process in batches to avoid memory issues
        for chunk in embeddings.chunks(self.batch_size) {
            for (path, embedding) in chunk {
                if embedding.len() != self.embedding_dimensions {
                    warn!(
                        "Skipping embedding with wrong dimensions: {} (expected {})",
                        embedding.len(),
                        self.embedding_dimensions
                    );
                    continue;
                }

                let embedding_bytes = Self::f32_vec_to_bytes(embedding);

                // Safe to use format! after validation
                let insert_query = format!(
                    "INSERT OR REPLACE INTO {} (path, embedding) VALUES (?, ?)",
                    table_name
                );

                sqlx::query(&insert_query)
                    .bind(path)
                    .bind(&embedding_bytes)
                    .execute(&mut *tx)
                    .await?;

                stored_count += 1;
            }
        }

        tx.commit().await?;

        info!("Successfully stored {} embeddings in batch", stored_count);
        Ok(stored_count)
    }

    /// Delete embeddings for specified paths
    pub async fn delete_embeddings(
        &self,
        pool: &SqlitePool,
        table_name: &str,
        paths: &[String],
    ) -> Result<usize> {
        if !self.is_available || paths.is_empty() {
            return Ok(0);
        }

        // Validate table name to prevent SQL injection
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        // Additional whitelist check for known table names
        const ALLOWED_TABLES: &[&str] = &["vec_embeddings", "file_embeddings", "search_history"];
        if !ALLOWED_TABLES.contains(&table_name) {
            return Err(AppError::SecurityError {
                message: format!("Table '{}' is not in allowed list", table_name),
            });
        }

        let mut deleted_count = 0;
        let mut tx = pool.begin().await?;

        for path in paths {
            // SECURITY: SQL injection prevention - table_name has been validated above
            // using is_valid_sql_identifier() and checked against whitelist
            let delete_query = format!("DELETE FROM {} WHERE path = ?", table_name);
            let result = sqlx::query(&delete_query)
                .bind(path)
                .execute(&mut *tx)
                .await?;

            deleted_count += result.rows_affected();
        }

        tx.commit().await?;

        info!("Deleted {} embeddings", deleted_count);
        Ok(deleted_count as usize)
    }

    /// Perform similarity search with filtering
    pub async fn filtered_vector_search(
        &self,
        pool: &SqlitePool,
        table_name: &str,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f32,
        path_pattern: Option<&str>,
    ) -> Result<Vec<(String, f32)>> {
        if !self.is_available {
            return Err(AppError::DatabaseError {
                message: "Vector extension not available".to_string(),
            });
        }

        // SECURITY: SQL injection prevention - validate table name
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        // SECURITY: Additional whitelist check for defense in depth
        const ALLOWED_TABLES: &[&str] = &["vec_embeddings", "file_embeddings", "search_history"];
        if !ALLOWED_TABLES.contains(&table_name) {
            return Err(AppError::SecurityError {
                message: format!("Table '{}' is not in allowed list for filtered search", table_name),
            });
        }

        let query_bytes = Self::f32_vec_to_bytes(query_embedding);

        let mut search_query = format!(
            r#"
            SELECT path, vec_distance_cosine(embedding, ?) as distance
            FROM {}
            WHERE 1=1
            "#,
            table_name
        );

        let mut params: Vec<Box<dyn sqlx::Encode<sqlx::Sqlite> + Send + Sync + 'static>> = vec![];
        params.push(Box::new(query_bytes.clone()));

        // Add path filtering if specified
        if let Some(pattern) = path_pattern {
            search_query.push_str(" AND path LIKE ?");
            params.push(Box::new(pattern.to_string()));
        }

        // Add similarity threshold
        let max_distance = 1.0 - min_similarity;
        search_query.push_str(&format!(
            " AND vec_distance_cosine(embedding, ?) <= {} ORDER BY distance LIMIT ?",
            max_distance
        ));
        params.push(Box::new(query_bytes.clone()));
        params.push(Box::new(limit as i32));

        // For now, use the simpler approach without dynamic parameters
        let simple_query = format!(
            r#"
            SELECT path, vec_distance_cosine(embedding, ?) as distance
            FROM {}
            ORDER BY distance
            LIMIT ?
            "#,
            table_name
        );

        let rows = sqlx::query(&simple_query)
            .bind(Self::f32_vec_to_bytes(query_embedding))
            .bind(limit as i32)
            .fetch_all(pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let path: String = row.get("path");
            let distance: f32 = row.get("distance");
            let similarity = 1.0 - distance;

            // Apply similarity threshold and path filtering in post-processing
            if similarity >= min_similarity {
                if let Some(pattern) = path_pattern {
                    if path.contains(pattern) {
                        results.push((path, similarity));
                    }
                } else {
                    results.push((path, similarity));
                }
            }
        }

        Ok(results)
    }

    /// Optimize vector table for better search performance
    pub async fn optimize_vector_table(&self, pool: &SqlitePool, table_name: &str) -> Result<()> {
        if !self.is_available {
            return Ok(());
        }

        // SECURITY: SQL injection prevention - validate table name
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        // SECURITY: Additional whitelist check for defense in depth
        const ALLOWED_TABLES: &[&str] = &["vec_embeddings", "file_embeddings", "search_history"];
        if !ALLOWED_TABLES.contains(&table_name) {
            return Err(AppError::SecurityError {
                message: format!("Table '{}' is not in allowed list for optimization", table_name),
            });
        }

        // SECURITY: Safe to use format! after validation and whitelist check
        sqlx::query(&format!("ANALYZE {}", table_name))
            .execute(pool)
            .await?;

        debug!("Optimized vector table: {}", table_name);
        Ok(())
    }

    /// Get statistics about vector table
    pub async fn get_vector_stats(
        &self,
        pool: &SqlitePool,
        table_name: &str,
    ) -> Result<VectorStats> {
        // SECURITY: SQL injection prevention - validate table name
        if !crate::storage::database::is_valid_sql_identifier(table_name) {
            return Err(AppError::SecurityError {
                message: "Invalid table name format".to_string(),
            });
        }

        // SECURITY: Additional whitelist check for defense in depth
        const ALLOWED_TABLES: &[&str] = &["vec_embeddings", "file_embeddings", "search_history"];
        if !ALLOWED_TABLES.contains(&table_name) {
            return Err(AppError::SecurityError {
                message: format!("Table '{}' is not in allowed list", table_name),
            });
        }

        // SECURITY: Safe to use format! after validation and whitelist check
        let count_query = format!("SELECT COUNT(*) as count FROM {}", table_name);

        let count: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        Ok(VectorStats {
            total_vectors: count as usize,
            dimensions: self.embedding_dimensions,
            extension_available: self.is_available,
            extension_version: self.version.clone(),
        })
    }
}

/// Statistics about vector storage
#[derive(Debug, Clone)]
pub struct VectorStats {
    pub total_vectors: usize,
    pub dimensions: usize,
    pub extension_available: bool,
    pub extension_version: Option<String>,
}

/// Manual fallback implementation for vector similarity when sqlite-vec is not available
pub struct ManualVectorSearch;

impl ManualVectorSearch {
    /// Perform cosine similarity search without vector extensions
    pub async fn cosine_similarity_search(
        pool: &SqlitePool,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        // Load all embeddings from the main table
        let rows = sqlx::query(
            r#"
            SELECT path, embedding
            FROM file_analysis
            WHERE embedding IS NOT NULL
            "#,
        )
        .fetch_all(pool)
        .await?;

        let mut results = Vec::new();

        for row in rows {
            let path: String = row.get("path");
            let embedding_bytes: Vec<u8> = row.get("embedding");

            // Convert bytes back to f32 vector
            if let Ok(embedding) = VectorExtension::bytes_to_f32_vec(&embedding_bytes) {
                let similarity = Self::cosine_similarity(query_embedding, &embedding);
                results.push((path, similarity));
            }
        }

        // Sort by similarity (descending) and limit results
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    /// Calculate cosine similarity between two vectors
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }

        dot_product / (magnitude_a * magnitude_b)
    }
}
