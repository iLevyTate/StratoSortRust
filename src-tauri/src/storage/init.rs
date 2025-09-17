use crate::error::Result;
use tracing::{debug, info, warn};

/// Initialize sqlite-vec extension at the SQLite connection level
/// This should be called early in the application lifecycle
pub fn initialize_sqlite_vec() -> Result<()> {
    // Register sqlite-vec extension with SQLite
    // Note: sqlite-vec init function returns void, so we handle errors differently
    // SAFETY: This call initializes the sqlite-vec extension in the SQLite library.
    // The function is provided by the sqlite-vec crate and follows SQLite's standard
    // extension initialization pattern. No memory management is required here as
    // SQLite handles the extension lifecycle internally.
    unsafe {
        // This registers the extension globally for all new SQLite connections
        sqlite_vec::sqlite3_vec_init();
        debug!("sqlite-vec extension initialization called");
    }

    // Skip runtime verification since it causes runtime conflicts
    // The extension availability will be verified later during actual database operations

    info!("sqlite-vec extension initialization completed");
    Ok(())
}

/// Check if sqlite-vec extension can be loaded
pub async fn check_vec_extension_availability() -> bool {
    use sqlx::sqlite::SqlitePool;

    // Create a temporary in-memory connection to test
    let pool_result = SqlitePool::connect("sqlite::memory:").await;

    match pool_result {
        Ok(pool) => {
            // Try to call vec_version function
            let version_result = sqlx::query_scalar::<_, String>("SELECT vec_version()")
                .fetch_one(&pool)
                .await;

            match version_result {
                Ok(version) => {
                    debug!("sqlite-vec extension is available, version: {}", version);
                    pool.close().await;
                    true
                }
                Err(_) => {
                    debug!("sqlite-vec extension functions not available");
                    pool.close().await;
                    false
                }
            }
        }
        Err(e) => {
            warn!("Failed to create test SQLite connection: {}", e);
            false
        }
    }
}

/// Alternative initialization method using rusqlite for applications that need it
#[cfg(feature = "rusqlite-init")]
pub fn initialize_with_rusqlite() -> Result<()> {
    use rusqlite::{ffi::sqlite3_auto_extension, Connection};

    // Register sqlite-vec extension to auto-load with new connections
    // SAFETY: This transmute is required for SQLite's C FFI auto-extension mechanism.
    // The sqlite3_vec_init function has the correct signature for SQLite extensions,
    // but Rust's type system requires explicit casting. This is a well-established
    // pattern in SQLite extension loading. The function pointer remains valid for
    // the lifetime of the application.
    unsafe {
        let result = sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));

        if result != 0 {
            return Err(AppError::DatabaseError {
                message: format!("Failed to auto-register sqlite-vec extension: {}", result),
            });
        }
    }

    // Test the extension works
    let conn = Connection::open_in_memory().map_err(|e| AppError::DatabaseError {
        message: format!("Failed to create test connection: {}", e),
    })?;

    let version: Result<String, _> = conn.query_row("SELECT vec_version()", [], |row| row.get(0));

    match version {
        Ok(ver) => {
            info!(
                "sqlite-vec extension verified with rusqlite, version: {}",
                ver
            );
            Ok(())
        }
        Err(e) => Err(AppError::DatabaseError {
            message: format!("sqlite-vec extension not working: {}", e),
        }),
    }
}

/// Configuration for vector extension
#[derive(Debug, Clone)]
pub struct VectorConfig {
    pub default_dimensions: usize,
    pub enable_quantization: bool,
    pub use_experimental_features: bool,
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            default_dimensions: 384, // Standard for nomic-embed-text
            enable_quantization: false,
            use_experimental_features: false,
        }
    }
}

/// Get recommended vector configuration based on the embedding model
pub fn get_vector_config_for_model(model_name: &str) -> VectorConfig {
    let mut config = VectorConfig::default();

    match model_name {
        "nomic-embed-text" => {
            config.default_dimensions = 384;
        }
        "text-embedding-ada-002" => {
            config.default_dimensions = 1536;
        }
        "sentence-transformers/all-MiniLM-L6-v2" => {
            config.default_dimensions = 384;
        }
        "sentence-transformers/all-mpnet-base-v2" => {
            config.default_dimensions = 768;
        }
        _ => {
            warn!(
                "Unknown embedding model: {}, using default config",
                model_name
            );
        }
    }

    config
}
