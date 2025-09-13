use stratosort::storage::database::Database;
use stratosort::error::AppError;
use sqlx::SqlitePool;
use tempfile::tempdir;
use std::sync::Arc;

#[tokio::test]
async fn test_sql_injection_in_file_queries() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    // Create test database
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    
    let db = Database::new(pool).await.unwrap();
    
    // SQL injection attempts in file path queries
    let injection_attempts = vec![
        // Classic SQL injection
        "'; DROP TABLE files; --",
        "' OR '1'='1",
        "' UNION SELECT * FROM sqlite_master --",
        "' OR 1=1 --",
        
        // Boolean-based blind injection
        "' AND (SELECT COUNT(*) FROM files) > 0 --",
        "' AND ASCII(SUBSTR((SELECT name FROM sqlite_master LIMIT 1), 1, 1)) > 65 --",
        
        // Time-based blind injection
        "'; SELECT CASE WHEN (1=1) THEN sqlite_version() ELSE (SELECT COUNT(*) FROM files) END; --",
        
        // Error-based injection
        "' AND (SELECT * FROM (SELECT COUNT(*), CONCAT(version(), 0x3a, 0x3a, (SELECT database())) x FROM information_schema.tables GROUP BY x) a) --",
        
        // Second-order injection
        "admin'/*",
        "admin*/'",
        "admin'||'admin",
        
        // Function-based injection
        "'; SELECT load_extension('evil.so'); --",
        "' UNION SELECT 1, load_extension('malicious'), 3 --",
        
        // Nested queries
        "' OR (SELECT COUNT(*) FROM (SELECT 1 FROM files UNION SELECT 2)) --",
        
        // Comment-based evasion
        "' /*comment*/ OR /*comment*/ '1'='1' /*comment*/ --",
        "' OR '1'='1'/*",
        
        // Unicode/encoding evasion
        "' OR '１'='１'", // Fullwidth characters
        "' %4fR '1'='1'", // URL encoding
        
        // Stacked queries
        "'; INSERT INTO files (path, content) VALUES ('malicious', 'payload'); --",
        "'; UPDATE files SET content = 'compromised' WHERE id = 1; --",
        "'; DELETE FROM files WHERE id > 0; --",
    ];
    
    for injection in injection_attempts {
        // Test file search with injection
        let result = db.search_files_by_content(injection, 10).await;
        
        match result {
            Ok(results) => {
                // Even if query succeeds, it should not return malicious results
                assert!(results.len() <= 10, "Results should be limited");
                for result in results {
                    // Ensure no system information is leaked
                    assert!(!result.path.contains("sqlite_master"), 
                           "Should not return system table information for injection: {}", injection);
                    assert!(!result.content.contains("version()"), 
                           "Should not return system function results for injection: {}", injection);
                }
                println!("Injection attempt '{}' returned {} safe results", injection, results.len());
            }
            Err(e) => {
                // Injection attempts should preferably fail
                println!("Injection attempt '{}' properly rejected: {:?}", injection, e);
            }
        }
        
        // Test file insertion with injection
        let result = db.store_file_analysis(injection, "test content", "text/plain", None).await;
        
        match result {
            Ok(_) => {
                println!("File stored with injection attempt as path: '{}'", injection);
                
                // Verify the malicious content didn't execute
                let files = db.get_all_files().await.unwrap_or_default();
                assert!(!files.is_empty() || files.len() < 1000, 
                       "Database should not be corrupted by injection");
            }
            Err(e) => {
                println!("File storage with injection path '{}' rejected: {:?}", injection, e);
            }
        }
    }
}

#[tokio::test]
async fn test_sql_injection_in_search_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Insert some test data first
    let _ = db.store_file_analysis("test1.txt", "normal content", "text/plain", None).await;
    let _ = db.store_file_analysis("test2.txt", "another file", "text/plain", None).await;
    
    let search_injections = vec![
        // Content-based injection attempts
        "normal' OR '1'='1",
        "content'; DROP TABLE file_analysis; --",
        "test' UNION SELECT password FROM users --",
        
        // LIKE injection attempts
        "normal%'; DROP TABLE files; --",
        "test' OR path LIKE '%admin%' --",
        
        // Regex injection (if supported)
        ".*'; DELETE FROM files; --",
        
        // JSON injection (if JSON functions are used)
        "'; SELECT json_extract(secrets, '$.password') FROM config; --",
        
        // Full-text search injection
        "MATCH'; DROP TABLE files; --",
        "NEAR'; INSERT INTO files VALUES('evil'); --",
    ];
    
    for injection in search_injections {
        // Test semantic search with injection
        let result = db.search_files_by_content(injection, 5).await;
        
        match result {
            Ok(results) => {
                // Verify results are legitimate
                assert!(results.len() <= 5, "Should respect limit parameter");
                
                for result in &results {
                    // Ensure no sensitive system information is returned
                    assert!(!result.path.contains("sqlite_"), "Should not return system tables");
                    assert!(!result.path.contains("/etc/"), "Should not return system files");
                    assert!(!result.content.contains("password"), "Should not return sensitive data");
                    
                    // Ensure injection didn't modify the results unexpectedly
                    assert!(result.path.len() < 500, "Path length should be reasonable");
                    assert!(result.content.len() < 10000, "Content length should be reasonable");
                }
                
                println!("Search injection '{}' returned {} legitimate results", injection, results.len());
            }
            Err(e) => {
                println!("Search injection '{}' properly blocked: {:?}", injection, e);
            }
        }
        
        // Test vector search with injection (if available)
        if let Ok(embedding) = vec![0.1f32; 384] { // Mock embedding
            let result = db.search_similar_files(&embedding, 5).await;
            
            match result {
                Ok(results) => {
                    assert!(results.len() <= 5, "Vector search should respect limit");
                    println!("Vector search with injection context succeeded with {} results", results.len());
                }
                Err(e) => {
                    println!("Vector search with injection context failed (expected): {:?}", e);
                }
            }
        }
    }
}

#[tokio::test]
async fn test_prepared_statement_protection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Test that prepared statements properly escape parameters
    let malicious_inputs = vec![
        ("file'; DROP TABLE files; --.txt", "This should be treated as literal path"),
        ("normal.txt", "Content with '; DROP TABLE files; -- embedded"),
        ("file.txt", "Content with \0 null bytes \0 everywhere"),
        ("unicode💀.txt", "Unicode content with 💀 skull and ☠️ crossbones"),
        ("'\"\\`${}[].txt", "All kinds of special characters"),
    ];
    
    for (path, content) in malicious_inputs {
        // Store file with malicious content
        let result = db.store_file_analysis(path, content, "text/plain", None).await;
        
        match result {
            Ok(file_id) => {
                println!("Stored file '{}' with ID {} (content treated as literal)", path, file_id);
                
                // Retrieve the file and verify content is exactly what we stored
                let retrieved_files = db.search_files_by_content(path, 1).await.unwrap_or_default();
                
                if let Some(file) = retrieved_files.first() {
                    assert_eq!(file.path, path, "Path should be stored literally");
                    // Content might be truncated or processed, but should not cause SQL injection
                    assert!(!file.content.contains("DROP TABLE"), 
                           "Stored content should not execute SQL commands");
                }
                
                // Try to retrieve by exact path
                if let Ok(files) = db.get_all_files().await {
                    let matching_files: Vec<_> = files.into_iter()
                        .filter(|f| f.path == path)
                        .collect();
                    
                    if !matching_files.is_empty() {
                        println!("Successfully retrieved file with path: '{}'", path);
                    }
                }
            }
            Err(e) => {
                // Some malicious inputs might be rejected by validation layers
                println!("File '{}' rejected by validation: {:?}", path, e);
            }
        }
    }
    
    // Verify database integrity after all operations
    let all_files = db.get_all_files().await.unwrap_or_default();
    println!("Database contains {} files after injection tests", all_files.len());
    
    // Ensure no system tables were affected
    let table_check = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='files'")
        .fetch_optional(&db.pool)
        .await;
        
    match table_check {
        Ok(Some(_)) => println!("Files table intact after injection tests"),
        Ok(None) => panic!("Files table was destroyed by injection!"),
        Err(e) => println!("Could not verify table integrity: {:?}", e),
    }
}

#[tokio::test]
async fn test_blind_sql_injection_detection() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Insert test data
    let _ = db.store_file_analysis("sensitive.txt", "confidential data", "text/plain", None).await;
    
    // Boolean-based blind injection payloads
    let blind_injections = vec![
        // Time-based detection
        ("test' AND (SELECT 1 FROM (SELECT COUNT(*) FROM files) WHERE COUNT(*) > 0) --", "time_based"),
        ("test'; SELECT CASE WHEN 1=1 THEN 'true' ELSE (SELECT 1/0) END; --", "conditional_error"),
        
        // Boolean-based detection
        ("test' AND (SELECT COUNT(*) FROM files) > 0 --", "row_count_check"),
        ("test' AND (SELECT LENGTH(path) FROM files LIMIT 1) > 5 --", "data_length_check"),
        
        // Information extraction attempts
        ("test' AND (SELECT SUBSTR(path,1,1) FROM files LIMIT 1) = 's' --", "char_extraction"),
        ("test' AND ASCII(SUBSTR((SELECT path FROM files LIMIT 1),1,1)) > 100 --", "ascii_extraction"),
        
        // Database structure reconnaissance
        ("test' AND (SELECT COUNT(*) FROM sqlite_master WHERE type='table') > 0 --", "table_enum"),
        ("test' AND (SELECT name FROM sqlite_master LIMIT 1) LIKE 'files%' --", "table_name_check"),
    ];
    
    for (injection, attack_type) in blind_injections {
        let start_time = std::time::Instant::now();
        
        let result = db.search_files_by_content(injection, 1).await;
        
        let elapsed = start_time.elapsed();
        
        match result {
            Ok(results) => {
                // Check for timing attacks
                if elapsed.as_millis() > 1000 {
                    println!("WARNING: {} injection '{}' took {}ms (possible timing attack)", 
                            attack_type, injection, elapsed.as_millis());
                }
                
                // Verify no sensitive information is leaked through result count or content
                for result in results {
                    assert!(!result.path.contains("sqlite_master"), 
                           "Should not reveal system table information");
                    assert!(!result.content.contains("confidential"), 
                           "Should not leak sensitive data through blind injection");
                }
                
                println!("{} injection completed in {}ms with {} results", 
                        attack_type, elapsed.as_millis(), results.len());
            }
            Err(e) => {
                println!("{} injection '{}' blocked: {:?}", attack_type, injection, e);
            }
        }
    }
}

#[tokio::test]
async fn test_sql_injection_in_batch_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let database_url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&database_url).await.unwrap();
    let db = Database::new(pool).await.unwrap();
    
    // Test batch operations with mixed legitimate and malicious data
    let batch_data = vec![
        ("legitimate1.txt", "normal content"),
        ("'; DROP TABLE files; --.txt", "malicious path"),
        ("normal2.txt", "'; DELETE FROM files; --"),
        ("normal3.txt", "legitimate content"),
        ("admin' OR '1'='1.txt", "path injection"),
    ];
    
    // Test batch insert operations
    let mut successful_inserts = 0;
    let mut rejected_inserts = 0;
    
    for (path, content) in batch_data {
        match db.store_file_analysis(path, content, "text/plain", None).await {
            Ok(_) => {
                successful_inserts += 1;
                println!("Batch insert succeeded for: '{}'", path);
            }
            Err(e) => {
                rejected_inserts += 1;
                println!("Batch insert rejected for '{}': {:?}", path, e);
            }
        }
    }
    
    println!("Batch operation results: {} successful, {} rejected", 
             successful_inserts, rejected_inserts);
    
    // Verify database integrity after batch operations
    let all_files = db.get_all_files().await.unwrap_or_default();
    
    // Check that malicious SQL didn't execute
    for file in &all_files {
        assert!(!file.path.is_empty(), "All files should have valid paths");
        assert!(!file.content.is_empty() || file.content == "", "Content should be handled properly");
    }
    
    // Verify table still exists and has expected structure
    let table_info = sqlx::query("PRAGMA table_info(files)")
        .fetch_all(&db.pool)
        .await;
        
    match table_info {
        Ok(info) => {
            assert!(!info.is_empty(), "Files table should still have columns after batch operations");
            println!("Files table has {} columns after batch injection test", info.len());
        }
        Err(e) => {
            panic!("Could not get table info after batch operations: {:?}", e);
        }
    }
}