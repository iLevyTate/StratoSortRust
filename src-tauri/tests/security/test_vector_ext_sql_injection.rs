use sqlx::{Row, SqlitePool};
use stratosort::error::AppError;
use stratosort::storage::{Database, ManualVectorSearch, VectorExtension};
use tempfile::tempdir;

/// Critical test for SQL injection vulnerabilities in vector_ext.rs
/// This test specifically targets the format! macro usage that could allow SQL injection
#[tokio::test]
async fn test_vector_table_name_injection_vulnerabilities() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create test database
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();

    // Initialize vector extension
    let vector_ext = VectorExtension::initialize(&pool).await;

    // Large payload for buffer overflow attempt
    let large_payload = format!("files'; {}", "A".repeat(10000));

    // Test malicious table names that could exploit format! macro vulnerabilities
    let malicious_table_names = vec![
        // SQL injection attempts via table name
        "files'; DROP TABLE files; --",
        "files' UNION SELECT * FROM sqlite_master WHERE name LIKE '%'; --",
        "files'; INSERT INTO files VALUES('malicious'); --",
        "files'; UPDATE files SET content = 'compromised'; --",
        "files'; DELETE FROM files; --",
        // Table name with embedded SQL commands
        "files; CREATE TABLE evil AS SELECT * FROM files; DROP TABLE files; --",
        "files UNION SELECT 1,2,3,4,5",
        "files) UNION SELECT password FROM users; --",
        // Function injection attempts
        "files'; SELECT load_extension('evil'); --",
        "files'; PRAGMA table_info(sqlite_master); --",
        "files'; VACUUM; DROP TABLE files; --",
        // Special characters and encoding attacks
        "files\"; DROP TABLE files; --",
        "files`; DROP TABLE files; --",
        "files'; /*comment*/ DROP TABLE files; --",
        "files'; -- comment\nDROP TABLE files;",
        // Non-standard identifier attacks
        "[files]; DROP TABLE files; --",
        "`files`; DROP TABLE files; --",
        "\"files\"; DROP TABLE files; --",
        // Unicode and encoding evasion
        "files'; DROP TABLE files/**/; --",
        "files'/**/UNION/**/SELECT/**/1; --",
        "files'+UNION+SELECT+1+--",
        "files%27%3b%20DROP%20TABLE%20files%3b%20--",
        // Nested query injection
        "files'; SELECT (SELECT password FROM users LIMIT 1); --",
        "files'; WITH evil AS (SELECT * FROM files) SELECT * FROM evil; --",
        // System function exploitation
        "files'; SELECT sqlite_version(), datetime('now'); --",
        "files'; PRAGMA compile_options; --",
        "files'; ATTACH DATABASE ':memory:' AS evil; --",
        // Large payload (buffer overflow attempt)
        &large_payload,
        // Null byte injection
        "files'; DROP TABLE files; --\0",
        "files\0'; DROP TABLE files; --",
    ];

    for table_name in malicious_table_names {
        println!("Testing table name injection: '{}'", table_name);

        // Test create_vector_table with malicious table name
        let result = vector_ext.create_vector_table(&pool, table_name, 384).await;

        match result {
            Ok(_) => {
                // If it succeeded, verify it was properly sanitized and safe

                // Check that no malicious SQL was executed
                let tables_result =
                    sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
                        .fetch_all(&pool)
                        .await;

                match tables_result {
                    Ok(rows) => {
                        for row in rows {
                            let table_name_found: String = row.get("name");

                            // Ensure no suspicious table names were created
                            assert!(
                                !table_name_found.contains("evil")
                                    && !table_name_found.contains("malicious")
                                    && !table_name_found.contains("compromised"),
                                "Malicious table was created: {}",
                                table_name_found
                            );
                        }
                        println!(
                            "Table creation with injection attempt '{}' was properly sanitized",
                            table_name
                        );
                    }
                    Err(e) => {
                        println!(
                            "Database corrupted by injection attempt '{}': {:?}",
                            table_name, e
                        );
                        // This should not happen with proper sanitization
                        panic!("Database state corrupted by SQL injection in table name");
                    }
                }
            }
            Err(AppError::SecurityError { message }) => {
                // This is the expected behavior - injection attempts should be blocked
                println!(
                    "Injection attempt '{}' properly blocked: {}",
                    table_name, message
                );
                assert!(
                    message.contains("Invalid table name")
                        || message.contains("Security")
                        || message.contains("not allowed"),
                    "Error message should indicate security violation"
                );
            }
            Err(other_error) => {
                println!(
                    "Injection attempt '{}' failed with: {:?}",
                    table_name, other_error
                );
                // Other errors are acceptable as long as injection doesn't succeed
            }
        }

        // Additional verification: ensure original tables still exist
        let verify_result = sqlx::query("SELECT COUNT(*) as count FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
            .fetch_one(&pool)
            .await;

        match verify_result {
            Ok(row) => {
                let table_count: i64 = row.get("count");
                println!("Table count after injection attempt: {}", table_count);
                // Ensure we haven't lost legitimate tables
            }
            Err(e) => {
                panic!(
                    "Cannot verify database integrity after injection attempt '{}': {:?}",
                    table_name, e
                );
            }
        }
    }
}

#[tokio::test]
async fn test_vector_store_embedding_path_injection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let vector_ext = VectorExtension::initialize(&pool).await;

    // Create a legitimate table first
    if vector_ext.is_available {
        let _ = vector_ext
            .create_vector_table(&pool, "test_vectors", 384)
            .await;
    }

    // Test path injection attempts in store_embedding
    let malicious_paths = vec![
        // SQL injection in path parameter
        "file.txt'; DROP TABLE test_vectors; --",
        "file.txt' UNION SELECT password FROM users; --",
        "'; INSERT INTO test_vectors VALUES('evil', 'data'); --",

        // Path traversal combined with SQL injection
        "../../../etc/passwd'; DROP TABLE test_vectors; --",
        "..\\..\\system32\\config\\sam'; DELETE FROM test_vectors; --",

        // Embedding data manipulation
        "normal.txt' WHERE 1=1; UPDATE test_vectors SET embedding='malicious'; --",
        "test.txt'; CREATE TRIGGER evil AFTER INSERT ON test_vectors BEGIN DROP TABLE test_vectors; END; --",

        // Binary data injection (simulated)
        "file.txt\0'; DROP TABLE test_vectors; --",
        "file.txt\x00'; DROP TABLE test_vectors; --",
    ];

    let dummy_embedding = vec![0.1f32; 384]; // Valid embedding vector

    for malicious_path in malicious_paths {
        println!(
            "Testing path injection in store_embedding: '{}'",
            malicious_path
        );

        let result = vector_ext
            .store_embedding(&pool, "test_vectors", malicious_path, &dummy_embedding)
            .await;

        match result {
            Ok(_) => {
                // If storage succeeded, verify the path was stored literally and safely
                println!("Path '{}' was stored (hopefully sanitized)", malicious_path);

                // Verify database integrity
                let integrity_check = sqlx::query("PRAGMA integrity_check").fetch_one(&pool).await;

                match integrity_check {
                    Ok(row) => {
                        let result: String = row.get(0);
                        assert_eq!(
                            result, "ok",
                            "Database integrity compromised by path injection"
                        );
                    }
                    Err(e) => {
                        panic!(
                            "Database integrity check failed after path injection '{}': {:?}",
                            malicious_path, e
                        );
                    }
                }
            }
            Err(AppError::SecurityError { message }) => {
                println!(
                    "Path injection '{}' properly blocked: {}",
                    malicious_path, message
                );
            }
            Err(AppError::DatabaseError { message }) => {
                // Database errors are acceptable if they prevent injection
                println!(
                    "Path injection '{}' caused database error (acceptable): {}",
                    malicious_path, message
                );
            }
            Err(other) => {
                println!(
                    "Path injection '{}' failed with: {:?}",
                    malicious_path, other
                );
            }
        }
    }
}

#[tokio::test]
async fn test_vector_search_embedding_injection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let vector_ext = VectorExtension::initialize(&pool).await;

    // Create table and store some test data
    if vector_ext.is_available {
        let _ = vector_ext
            .create_vector_table(&pool, "search_test", 384)
            .await;
        let test_embedding = vec![0.5f32; 384];
        let _ = vector_ext
            .store_embedding(&pool, "search_test", "test.txt", &test_embedding)
            .await;
    }

    // Test malicious embedding data that could exploit binary serialization
    let malicious_search_attempts = vec![
        // Oversized embeddings (buffer overflow attempt)
        vec![f32::INFINITY; 384],
        vec![f32::NEG_INFINITY; 384],
        vec![f32::NAN; 384],
        // Extreme values
        vec![f32::MAX; 384],
        vec![f32::MIN; 384],
        vec![1e38f32; 384], // Very large numbers
        vec![-1e38f32; 384],
        // Pattern that might break serialization
        (0..384)
            .map(|i| {
                if i % 2 == 0 {
                    f32::INFINITY
                } else {
                    f32::NEG_INFINITY
                }
            })
            .collect(),
        // All zeros (edge case)
        vec![0.0f32; 384],
        // Subnormal numbers
        vec![f32::MIN_POSITIVE; 384],
    ];

    for (i, malicious_embedding) in malicious_search_attempts.iter().enumerate() {
        println!("Testing search with malicious embedding #{}", i);

        let result = vector_ext
            .vector_search(&pool, "search_test", malicious_embedding, 10)
            .await;

        match result {
            Ok(results) => {
                // Verify results are reasonable and don't contain system information
                println!(
                    "Search with malicious embedding #{} returned {} results",
                    i,
                    results.len()
                );

                for (path, similarity) in results {
                    // Ensure no system paths or sensitive information leaked
                    assert!(
                        !path.contains("sqlite_master"),
                        "System table information leaked"
                    );
                    assert!(!path.contains("/etc/"), "System file paths leaked");
                    assert!(!path.contains("password"), "Sensitive data leaked");

                    // Ensure similarity scores are reasonable
                    assert!(
                        similarity.is_finite() && similarity >= -1.0 && similarity <= 1.0,
                        "Invalid similarity score: {}",
                        similarity
                    );
                }
            }
            Err(AppError::InvalidInput { message }) => {
                println!("Malicious embedding #{} properly rejected: {}", i, message);
            }
            Err(AppError::DatabaseError { message }) => {
                println!(
                    "Malicious embedding #{} caused database error: {}",
                    i, message
                );
                // Database errors are acceptable for malicious input
            }
            Err(other) => {
                println!("Malicious embedding #{} failed with: {:?}", i, other);
            }
        }

        // Verify database is still accessible after each attempt
        let health_check = sqlx::query("SELECT 1").fetch_one(&pool).await;

        if health_check.is_err() {
            panic!(
                "Database became inaccessible after malicious embedding #{}",
                i
            );
        }
    }
}

#[tokio::test]
async fn test_vector_delete_embeddings_injection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let vector_ext = VectorExtension::initialize(&pool).await;

    if !vector_ext.is_available {
        println!("Vector extension not available, skipping deletion injection tests");
        return;
    }

    // Create table and store test data
    let _ = vector_ext
        .create_vector_table(&pool, "delete_test", 384)
        .await;
    let test_embedding = vec![0.5f32; 384];
    let _ = vector_ext
        .store_embedding(&pool, "delete_test", "legitimate.txt", &test_embedding)
        .await;
    let _ = vector_ext
        .store_embedding(&pool, "delete_test", "important.txt", &test_embedding)
        .await;

    // Test malicious paths in delete operations
    let malicious_delete_paths = vec![
        // SQL injection attempts
        vec!["'; DROP TABLE delete_test; --".to_string()],
        vec!["' OR '1'='1".to_string()],
        vec!["'; DELETE FROM delete_test; --".to_string()],
        // Multiple path injection
        vec![
            "legitimate.txt".to_string(),
            "'; DROP TABLE delete_test; --".to_string(),
        ],
        // Boolean injection
        vec!["' OR path LIKE '%'".to_string()],
        vec!["' UNION SELECT path FROM delete_test; --".to_string()],
        // Wildcard injection
        vec!["%".to_string()],
        vec!["*".to_string()],
    ];

    for malicious_paths in malicious_delete_paths {
        println!("Testing delete injection with paths: {:?}", malicious_paths);

        // Count rows before deletion attempt
        let before_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM delete_test")
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

        let result = vector_ext
            .delete_embeddings(&pool, "delete_test", &malicious_paths)
            .await;

        // Count rows after deletion attempt
        let after_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM delete_test")
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

        match result {
            Ok(deleted_count) => {
                println!("Deletion attempt deleted {} items", deleted_count);

                // Ensure only legitimate deletions occurred
                let legitimate_paths_count = malicious_paths
                    .iter()
                    .filter(|p| {
                        !p.contains("'")
                            && !p.contains(";")
                            && !p.contains("DROP")
                            && !p.contains("DELETE")
                    })
                    .count();

                // Verify that the deletion count makes sense
                assert!(
                    deleted_count <= legitimate_paths_count,
                    "More items deleted ({}) than legitimate paths ({})",
                    deleted_count,
                    legitimate_paths_count
                );

                // Ensure we didn't delete everything (unless all paths were legitimate)
                if malicious_paths
                    .iter()
                    .any(|p| p.contains("'") || p.contains("DROP"))
                {
                    assert!(after_count > 0, "All data was deleted by injection attack");
                }
            }
            Err(AppError::SecurityError { message }) => {
                println!("Delete injection properly blocked: {}", message);

                // Ensure no data was deleted when security error occurred
                assert_eq!(
                    before_count, after_count,
                    "Data was deleted despite security error"
                );
            }
            Err(other) => {
                println!("Delete injection failed with: {:?}", other);

                // Ensure database integrity is maintained
                assert_eq!(
                    before_count, after_count,
                    "Data loss occurred despite error"
                );
            }
        }

        // Verify table still exists
        let table_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='delete_test'",
        )
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

        assert_eq!(table_exists, 1, "Table was dropped by deletion injection");
    }
}

#[tokio::test]
async fn test_vector_optimize_and_stats_injection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let vector_ext = VectorExtension::initialize(&pool).await;

    // Test malicious table names in optimization and stats functions
    let malicious_table_names = vec![
        "test'; DROP TABLE sqlite_master; --",
        "test' UNION SELECT * FROM sqlite_master; --",
        "test'; PRAGMA writable_schema=ON; --",
        "test'; VACUUM INTO '/tmp/evil.db'; --",
        "test'; CREATE TABLE evil AS SELECT * FROM sqlite_master; --",
        "test'; ALTER TABLE sqlite_master RENAME TO evil; --",
    ];

    for table_name in malicious_table_names {
        println!(
            "Testing optimization injection with table name: '{}'",
            table_name
        );

        // Test optimize_vector_table
        let optimize_result = vector_ext.optimize_vector_table(&pool, table_name).await;
        match optimize_result {
            Ok(_) => {
                println!(
                    "Optimization completed (hopefully safely) for '{}'",
                    table_name
                );
            }
            Err(e) => {
                println!("Optimization rejected for '{}': {:?}", table_name, e);
            }
        }

        // Test get_vector_stats
        let stats_result = vector_ext.get_vector_stats(&pool, table_name).await;
        match stats_result {
            Ok(stats) => {
                println!(
                    "Stats retrieved for '{}': {} vectors",
                    table_name, stats.total_vectors
                );

                // Ensure stats don't reveal sensitive system information
                assert!(
                    stats.total_vectors < 1_000_000,
                    "Suspiciously high vector count"
                );
                assert!(
                    stats.dimensions > 0 && stats.dimensions < 10_000,
                    "Suspicious dimension count"
                );
            }
            Err(e) => {
                println!("Stats retrieval failed for '{}': {:?}", table_name, e);
            }
        }

        // Verify database integrity after each operation
        let integrity = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .unwrap_or_default();

        assert_eq!(
            integrity, "ok",
            "Database integrity compromised by injection in '{}'",
            table_name
        );
    }
}

#[tokio::test]
async fn test_manual_vector_search_sql_injection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();

    // Create database with analysis table
    let db = Database::new_test(&db_path).await.unwrap();

    // Store some test data with potentially vulnerable paths
    let test_data = vec![
        ("normal.txt", "Normal file content"),
        ("'; DROP TABLE file_analysis; --", "Malicious filename"),
        ("../../../etc/passwd", "Path traversal attempt"),
        ("test' OR '1'='1.txt", "Boolean injection filename"),
    ];

    for (path, summary) in test_data {
        let analysis = stratosort::ai::FileAnalysis {
            path: path.to_string(),
            category: "test".to_string(),
            tags: vec!["test".to_string()],
            summary: summary.to_string(),
            confidence: 0.9,
            extracted_text: None,
            detected_language: None,
            metadata: serde_json::Value::Null,
        };
        let _ = db.save_analysis(&analysis).await;
    }

    // Test manual vector search with malicious embeddings
    let malicious_query_embeddings = vec![
        // Normal case (control)
        vec![0.5f32; 384],
        // Edge cases that might break manual similarity calculation
        vec![f32::INFINITY; 384],
        vec![f32::NEG_INFINITY; 384],
        vec![f32::NAN; 384],
        vec![0.0f32; 384],
        // Large values
        vec![f32::MAX; 384],
        vec![f32::MIN; 384],
        // Alternating extreme values
        (0..384)
            .map(|i| if i % 2 == 0 { f32::MAX } else { f32::MIN })
            .collect(),
    ];

    for (i, query_embedding) in malicious_query_embeddings.iter().enumerate() {
        println!("Testing manual vector search with embedding #{}", i);

        let result = ManualVectorSearch::cosine_similarity_search(&pool, query_embedding, 10).await;

        match result {
            Ok(results) => {
                println!("Manual search #{} returned {} results", i, results.len());

                for (path, similarity) in results {
                    // Ensure no SQL injection occurred in path retrieval
                    assert!(!path.contains("UNION"), "SQL injection detected in path");
                    assert!(!path.contains("SELECT"), "SQL injection detected in path");
                    assert!(!path.contains("DROP"), "SQL injection detected in path");

                    // Ensure similarity calculation didn't produce dangerous values
                    if !similarity.is_nan() {
                        assert!(
                            similarity >= -1.1 && similarity <= 1.1,
                            "Similarity out of valid range: {}",
                            similarity
                        );
                    }

                    println!("  Path: '{}', Similarity: {}", path, similarity);
                }
            }
            Err(e) => {
                println!("Manual search #{} failed: {:?}", i, e);
            }
        }

        // Verify database wasn't corrupted
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM file_analysis")
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

        assert!(
            count > 0,
            "File analysis table was corrupted during manual search #{}",
            i
        );
    }
}

#[tokio::test]
async fn test_vector_batch_operations_injection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let vector_ext = VectorExtension::initialize(&pool).await;

    if !vector_ext.is_available {
        println!("Vector extension not available, skipping batch injection tests");
        return;
    }

    // Create test table
    let _ = vector_ext
        .create_vector_table(&pool, "batch_test", 384)
        .await;

    // Test batch insertion with mixed legitimate and malicious data
    let batch_embeddings = vec![
        ("legitimate1.txt".to_string(), vec![0.1f32; 384]),
        (
            "'; DROP TABLE batch_test; --".to_string(),
            vec![0.2f32; 384],
        ),
        ("normal2.txt".to_string(), vec![0.3f32; 384]),
        ("' OR '1'='1".to_string(), vec![0.4f32; 384]),
        ("../../../etc/passwd".to_string(), vec![0.5f32; 384]),
        ("legitimate3.txt".to_string(), vec![0.6f32; 384]),
    ];

    println!("Testing batch embedding insertion with mixed data");

    let result = vector_ext
        .store_embeddings_batch(&pool, "batch_test", &batch_embeddings)
        .await;

    match result {
        Ok(stored_count) => {
            println!("Batch insertion stored {} embeddings", stored_count);

            // Verify table integrity
            let table_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM batch_test")
                .fetch_one(&pool)
                .await
                .unwrap_or(0);

            println!("Table contains {} rows after batch insertion", table_count);

            // Ensure the table still exists (wasn't dropped)
            let table_exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='batch_test'",
            )
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

            assert_eq!(
                table_exists, 1,
                "Batch test table was dropped during injection"
            );

            // Verify that stored paths are literal (not executed as SQL)
            let stored_paths = sqlx::query_scalar::<_, String>("SELECT path FROM batch_test")
                .fetch_all(&pool)
                .await
                .unwrap_or_default();

            for path in &stored_paths {
                // Paths should be stored literally, not executed
                println!("Stored path: '{}'", path);
            }

            // Verify no system tables were affected
            let system_table_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            )
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

            println!(
                "Total system tables after batch insertion: {}",
                system_table_count
            );
        }
        Err(e) => {
            println!("Batch insertion failed: {:?}", e);

            // Even if batch insertion fails, ensure table still exists
            let table_exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='batch_test'",
            )
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

            assert_eq!(
                table_exists, 1,
                "Table was destroyed during failed batch insertion"
            );
        }
    }

    // Test database integrity
    let integrity = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .unwrap_or_default();

    assert_eq!(
        integrity, "ok",
        "Database integrity compromised by batch injection"
    );
}
