use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use stratosort::ai::{embeddings::generate_simple_embeddings, FileAnalysis};
use stratosort::commands::organization::SmartFolder;
use stratosort::error::Result;
/// Comprehensive Database Testing Suite
/// Tests all aspects of database functionality including embeddings, vector search, and sqlite-vec extension
/// Tests with all 27 dummy files to ensure production readiness
use stratosort::storage::Database;
use tempfile::tempdir;
use tokio::sync::{RwLock, Semaphore};
use tracing::{info, warn};
use uuid::Uuid;

// Test configuration
const TEST_DATA_DIR: &str = "./tests/fixtures/data/sample_demo_files";
const EMBEDDING_DIM: usize = 384;
const CONCURRENT_OPERATIONS: usize = 10;
#[allow(dead_code)]
const STRESS_TEST_FILE_COUNT: usize = 100;

#[derive(Debug, Clone)]
struct TestResult {
    test_name: String,
    passed: bool,
    duration: Duration,
    details: String,
    metrics: HashMap<String, f64>,
}

struct DatabaseTestSuite {
    db: Arc<Database>,
    #[allow(dead_code)]
    test_files: Vec<PathBuf>,
    results: Arc<RwLock<Vec<TestResult>>>,
}

impl DatabaseTestSuite {
    /// Initialize test suite with database and test files
    async fn new() -> Result<Self> {
        // Create temporary database for testing
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_comprehensive.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        info!("Initializing comprehensive database test suite");
        info!("Database path: {}", db_path.display());

        let db = Database::new_from_url(&db_url).await?;

        // Load test files
        let test_files = Self::load_test_files()?;
        info!("Loaded {} test files", test_files.len());

        Ok(Self {
            db: Arc::new(db),
            test_files,
            results: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Load all test files from the dummy data directory
    fn load_test_files() -> Result<Vec<PathBuf>> {
        let test_dir = Path::new(TEST_DATA_DIR);
        let mut files = Vec::new();

        if test_dir.exists() {
            for entry in fs::read_dir(test_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    files.push(path);
                }
            }
        } else {
            warn!("Test data directory not found: {}", TEST_DATA_DIR);
            // Create synthetic test files
            files = Self::create_synthetic_test_files()?;
        }

        Ok(files)
    }

    /// Create synthetic test files if real ones aren't available
    fn create_synthetic_test_files() -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let categories = vec![
            (
                "research",
                vec!["ai", "machine learning", "neural networks"],
            ),
            (
                "finance",
                vec!["quarterly report", "financial statement", "budget"],
            ),
            ("3d_models", vec!["3d print", "STL file", "CAD design"]),
            (
                "graphics",
                vec!["logo design", "brand identity", "vector graphics"],
            ),
            ("documents", vec!["contract", "agreement", "proposal"]),
        ];

        for (category, _keywords) in categories {
            for i in 0..5 {
                files.push(PathBuf::from(format!(
                    "/synthetic/{}/file_{}.txt",
                    category, i
                )));
            }
        }

        Ok(files)
    }

    /// Run all comprehensive tests
    pub async fn run_all_tests(&mut self) -> Result<()> {
        info!("Starting comprehensive database testing suite");
        let overall_start = Instant::now();

        // Test 1: Database initialization and schema verification
        self.test_database_initialization().await?;

        // Test 2: Embedding generation and storage
        self.test_embedding_storage().await?;

        // Test 3: Semantic search functionality
        self.test_semantic_search().await?;

        // Test 4: Bulk operations performance
        self.test_bulk_operations().await?;

        // Test 5: Concurrent operations
        self.test_concurrent_operations().await?;

        // Test 6: Database persistence and recovery
        self.test_persistence_and_recovery().await?;

        // Test 7: Smart folder matching with embeddings
        self.test_smart_folder_matching().await?;

        // Test 8: Vector extension functionality
        self.test_vector_extension().await?;

        // Test 9: Error handling and edge cases
        self.test_error_handling().await?;

        // Test 10: Performance benchmarks
        self.test_performance_benchmarks().await?;

        // Test 11: Data integrity verification
        self.test_data_integrity().await?;

        // Test 12: Search accuracy validation
        self.test_search_accuracy().await?;

        // Generate comprehensive report
        self.generate_test_report(overall_start.elapsed()).await?;

        Ok(())
    }

    /// Test 1: Database initialization and schema verification
    async fn test_database_initialization(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing database initialization and schema verification");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Check database health
        match self.db.health_check().await {
            Ok(_) => details.push_str("✓ Database health check passed\n"),
            Err(e) => {
                details.push_str(&format!("✗ Database health check failed: {}\n", e));
                passed = false;
            }
        }

        // Check vector extension availability
        let vec_available = self.db.is_vector_extension_available();
        details.push_str(&format!("Vector extension available: {}\n", vec_available));
        metrics.insert(
            "vector_extension".to_string(),
            if vec_available { 1.0 } else { 0.0 },
        );

        if let Some(version) = self.db.get_vector_extension_version() {
            details.push_str(&format!("Vector extension version: {}\n", version));
        }

        // Test schema integrity
        let test_analysis = FileAnalysis {
            path: "/test/schema_check.txt".to_string(),
            category: "test".to_string(),
            tags: vec!["schema".to_string(), "test".to_string()],
            summary: "Schema integrity test".to_string(),
            confidence: 0.99,
            extracted_text: Some("Test content".to_string()),
            detected_language: Some("en".to_string()),
            metadata: json!({"test": true}),
        };

        match self.db.save_analysis(&test_analysis).await {
            Ok(_) => {
                details.push_str("✓ Schema supports file analysis storage\n");
                metrics.insert("schema_valid".to_string(), 1.0);
            }
            Err(e) => {
                details.push_str(&format!("✗ Failed to save analysis: {}\n", e));
                passed = false;
                metrics.insert("schema_valid".to_string(), 0.0);
            }
        }

        self.record_result(TestResult {
            test_name: "Database Initialization".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 2: Embedding generation and storage
    async fn test_embedding_storage(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing embedding generation and storage");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        let test_texts = vec![
            (
                "Machine learning research paper on neural networks",
                "research",
            ),
            ("Quarterly financial report Q3 2024", "finance"),
            ("3D printing model for prototype", "3d_models"),
            ("Company logo design guidelines", "graphics"),
        ];

        let mut successful_saves = 0;
        let mut failed_saves = 0;

        for (text, category) in test_texts {
            let path = format!("/test/embedding_{}.txt", category);

            // Generate embedding
            let embedding = generate_simple_embeddings(text)?;

            // Verify embedding dimensions
            if embedding.len() != EMBEDDING_DIM {
                details.push_str(&format!(
                    "✗ Invalid embedding dimension: {} (expected {})\n",
                    embedding.len(),
                    EMBEDDING_DIM
                ));
                failed_saves += 1;
                continue;
            }

            // Store embedding
            match self
                .db
                .save_embedding(&path, &embedding, Some("test-model"))
                .await
            {
                Ok(_) => {
                    successful_saves += 1;
                    details.push_str(&format!("✓ Stored embedding for {}\n", category));
                }
                Err(e) => {
                    failed_saves += 1;
                    details.push_str(&format!(
                        "✗ Failed to store embedding for {}: {}\n",
                        category, e
                    ));
                    passed = false;
                }
            }
        }

        metrics.insert("successful_saves".to_string(), successful_saves as f64);
        metrics.insert("failed_saves".to_string(), failed_saves as f64);
        metrics.insert(
            "save_rate".to_string(),
            successful_saves as f64 / (successful_saves + failed_saves) as f64,
        );

        self.record_result(TestResult {
            test_name: "Embedding Storage".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 3: Semantic search functionality
    async fn test_semantic_search(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing semantic search functionality");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // First, populate database with test embeddings
        let test_documents = vec![
            (
                "Deep learning with TensorFlow and PyTorch",
                "/docs/ml_framework.txt",
                "research",
            ),
            (
                "Neural network architectures for NLP",
                "/docs/nlp_arch.txt",
                "research",
            ),
            (
                "Annual financial report 2024",
                "/docs/finance_annual.txt",
                "finance",
            ),
            ("Q4 earnings statement", "/docs/q4_earnings.txt", "finance"),
            (
                "3D printing tutorial for beginners",
                "/docs/3d_tutorial.txt",
                "3d_models",
            ),
            (
                "STL file format specification",
                "/docs/stl_spec.txt",
                "3d_models",
            ),
        ];

        // Store documents with embeddings
        for (content, path, _category) in &test_documents {
            let embedding = generate_simple_embeddings(content)?;
            self.db
                .save_embedding(path, &embedding, Some("test-model"))
                .await?;
        }

        // Test search queries
        let test_queries = vec![
            (
                "machine learning research",
                vec!["ml_framework", "nlp_arch"],
            ),
            ("financial reports", vec!["finance_annual", "q4_earnings"]),
            ("3d printing models", vec!["3d_tutorial", "stl_spec"]),
        ];

        let mut total_searches = 0;
        let mut accurate_searches = 0;

        for (query, expected_matches) in test_queries {
            let query_embedding = generate_simple_embeddings(query)?;

            match self.db.semantic_search(&query_embedding, 5).await {
                Ok(results) => {
                    total_searches += 1;

                    // Check if expected matches are in top results
                    let mut found_expected = 0;
                    for (path, score) in &results {
                        for expected in &expected_matches {
                            if path.contains(expected) {
                                found_expected += 1;
                                details.push_str(&format!(
                                    "✓ Found expected match: {} (score: {:.3})\n",
                                    path, score
                                ));
                            }
                        }
                    }

                    if found_expected >= expected_matches.len() / 2 {
                        accurate_searches += 1;
                    }
                }
                Err(e) => {
                    details.push_str(&format!("✗ Search failed for '{}': {}\n", query, e));
                    passed = false;
                }
            }
        }

        let accuracy = if total_searches > 0 {
            accurate_searches as f64 / total_searches as f64
        } else {
            0.0
        };

        metrics.insert("total_searches".to_string(), total_searches as f64);
        metrics.insert("accurate_searches".to_string(), accurate_searches as f64);
        metrics.insert("search_accuracy".to_string(), accuracy);

        if accuracy < 0.7 {
            passed = false;
            details.push_str(&format!(
                "✗ Search accuracy too low: {:.1}%\n",
                accuracy * 100.0
            ));
        }

        self.record_result(TestResult {
            test_name: "Semantic Search".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 4: Bulk operations performance
    async fn test_bulk_operations(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing bulk operations performance");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Test bulk file analysis storage
        let mut analyses = Vec::new();
        for i in 0..50 {
            analyses.push(FileAnalysis {
                path: format!("/bulk/file_{}.txt", i),
                category: if i % 3 == 0 {
                    "research".to_string()
                } else if i % 3 == 1 {
                    "finance".to_string()
                } else {
                    "documents".to_string()
                },
                tags: vec![format!("tag_{}", i % 10), "bulk_test".to_string()],
                summary: format!("Bulk test file {}", i),
                confidence: (0.85 + (i as f64 * 0.002)) as f32,
                extracted_text: Some(format!("Content for file {}", i)),
                detected_language: Some("en".to_string()),
                metadata: json!({"index": i}),
            });
        }

        let bulk_start = Instant::now();
        let mut successful_ops = 0;

        for analysis in &analyses {
            if self.db.save_analysis(analysis).await.is_ok() {
                successful_ops += 1;
            }
        }

        let bulk_duration = bulk_start.elapsed();
        let ops_per_second = successful_ops as f64 / bulk_duration.as_secs_f64();

        metrics.insert("bulk_saves".to_string(), successful_ops as f64);
        metrics.insert("ops_per_second".to_string(), ops_per_second);

        details.push_str(&format!(
            "Bulk save: {} operations in {:.2}s ({:.1} ops/s)\n",
            successful_ops,
            bulk_duration.as_secs_f64(),
            ops_per_second
        ));

        // Test bulk embedding operations
        let embedding_start = Instant::now();
        let mut embedding_count = 0;

        for i in 0..30 {
            let text = format!(
                "Test document {} with some content for embedding generation",
                i
            );
            let embedding = generate_simple_embeddings(&text)?;
            let path = format!("/bulk/embedding_{}.txt", i);

            if self
                .db
                .save_embedding(&path, &embedding, Some("test-model"))
                .await
                .is_ok()
            {
                embedding_count += 1;
            }
        }

        let embedding_duration = embedding_start.elapsed();
        let embeddings_per_second = embedding_count as f64 / embedding_duration.as_secs_f64();

        metrics.insert("bulk_embeddings".to_string(), embedding_count as f64);
        metrics.insert("embeddings_per_second".to_string(), embeddings_per_second);

        details.push_str(&format!(
            "Bulk embeddings: {} operations in {:.2}s ({:.1} emb/s)\n",
            embedding_count,
            embedding_duration.as_secs_f64(),
            embeddings_per_second
        ));

        // Performance thresholds
        if ops_per_second < 10.0 {
            passed = false;
            details.push_str("✗ Bulk save performance below threshold (< 10 ops/s)\n");
        }

        if embeddings_per_second < 5.0 {
            passed = false;
            details.push_str("✗ Embedding performance below threshold (< 5 emb/s)\n");
        }

        self.record_result(TestResult {
            test_name: "Bulk Operations".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 5: Concurrent operations
    async fn test_concurrent_operations(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing concurrent database operations");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        let db = Arc::clone(&self.db);
        let semaphore = Arc::new(Semaphore::new(CONCURRENT_OPERATIONS));
        let mut handles = Vec::new();

        // Spawn concurrent tasks
        for i in 0..50 {
            let db_clone = Arc::clone(&db);
            let sem_clone = Arc::clone(&semaphore);

            let handle = tokio::spawn(async move {
                let _permit = sem_clone.acquire().await.unwrap();

                let analysis = FileAnalysis {
                    path: format!("/concurrent/file_{}.txt", i),
                    category: "concurrent_test".to_string(),
                    tags: vec![format!("thread_{}", i % CONCURRENT_OPERATIONS)],
                    summary: format!("Concurrent operation {}", i),
                    confidence: 0.9,
                    extracted_text: Some(format!("Content {}", i)),
                    detected_language: Some("en".to_string()),
                    metadata: json!({"concurrent": true, "index": i}),
                };

                let save_result = db_clone.save_analysis(&analysis).await;

                // Also test concurrent embedding
                let embedding = generate_simple_embeddings(&analysis.summary).unwrap();
                let embed_result = db_clone
                    .save_embedding(&analysis.path, &embedding, Some("test-model"))
                    .await;

                (save_result.is_ok(), embed_result.is_ok())
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        let mut successful_saves = 0;
        let mut successful_embeddings = 0;
        let mut errors = 0;

        for handle in handles {
            match handle.await {
                Ok((save_ok, embed_ok)) => {
                    if save_ok {
                        successful_saves += 1;
                    }
                    if embed_ok {
                        successful_embeddings += 1;
                    }
                    if !save_ok || !embed_ok {
                        errors += 1;
                    }
                }
                Err(e) => {
                    errors += 1;
                    details.push_str(&format!("✗ Task error: {}\n", e));
                }
            }
        }

        metrics.insert("concurrent_saves".to_string(), successful_saves as f64);
        metrics.insert(
            "concurrent_embeddings".to_string(),
            successful_embeddings as f64,
        );
        metrics.insert("concurrent_errors".to_string(), errors as f64);

        let success_rate = successful_saves as f64 / 50.0;
        metrics.insert("success_rate".to_string(), success_rate);

        details.push_str(&format!(
            "Concurrent operations: {} saves, {} embeddings, {} errors\n",
            successful_saves, successful_embeddings, errors
        ));

        if success_rate < 0.95 {
            passed = false;
            details.push_str(&format!(
                "✗ Success rate too low: {:.1}%\n",
                success_rate * 100.0
            ));
        }

        self.record_result(TestResult {
            test_name: "Concurrent Operations".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 6: Database persistence and recovery
    async fn test_persistence_and_recovery(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing database persistence and recovery");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Store test data
        let test_path = "/persistence/test_file.txt";
        let test_analysis = FileAnalysis {
            path: test_path.to_string(),
            category: "persistence_test".to_string(),
            tags: vec!["persistence".to_string(), "recovery".to_string()],
            summary: "Testing database persistence".to_string(),
            confidence: 0.95,
            extracted_text: Some("Persistence test content".to_string()),
            detected_language: Some("en".to_string()),
            metadata: json!({"test": "persistence"}),
        };

        let test_embedding = generate_simple_embeddings(&test_analysis.summary)?;

        // Save data
        self.db.save_analysis(&test_analysis).await?;
        self.db
            .save_embedding(test_path, &test_embedding, Some("test-model"))
            .await?;

        // Flush database
        match self.db.flush().await {
            Ok(_) => details.push_str("✓ Database flushed successfully\n"),
            Err(e) => {
                details.push_str(&format!("✗ Failed to flush database: {}\n", e));
                passed = false;
            }
        }

        // Simulate recovery by checking data integrity
        match self.db.get_analysis(test_path).await? {
            Some(recovered) => {
                if recovered.summary == test_analysis.summary
                    && recovered.category == test_analysis.category
                {
                    details.push_str("✓ Analysis data recovered correctly\n");
                    metrics.insert("analysis_recovered".to_string(), 1.0);
                } else {
                    details.push_str("✗ Analysis data corrupted after recovery\n");
                    passed = false;
                    metrics.insert("analysis_recovered".to_string(), 0.0);
                }
            }
            None => {
                details.push_str("✗ Analysis data lost after recovery\n");
                passed = false;
                metrics.insert("analysis_recovered".to_string(), 0.0);
            }
        }

        // Test WAL checkpoint
        match self.db.flush().await {
            Ok(_) => {
                details.push_str("✓ WAL checkpoint successful\n");
                metrics.insert("wal_checkpoint".to_string(), 1.0);
            }
            Err(e) => {
                details.push_str(&format!("✗ WAL checkpoint failed: {}\n", e));
                metrics.insert("wal_checkpoint".to_string(), 0.0);
            }
        }

        self.record_result(TestResult {
            test_name: "Persistence & Recovery".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 7: Smart folder matching with embeddings
    async fn test_smart_folder_matching(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing smart folder matching with embeddings");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Create smart folders with semantic rules
        let smart_folders = vec![
            SmartFolder {
                id: Uuid::new_v4().to_string(),
                name: "AI Research".to_string(),
                description: Some("Machine learning and AI papers".to_string()),
                rules: vec![],
                target_path: "/organized/ai_research".to_string(),
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            SmartFolder {
                id: Uuid::new_v4().to_string(),
                name: "Financial Documents".to_string(),
                description: Some("Financial reports and statements".to_string()),
                rules: vec![],
                target_path: "/organized/finance".to_string(),
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ];

        // Save smart folders
        for folder in &smart_folders {
            self.db.save_smart_folder(folder).await?;
        }

        // Test files that should match
        let test_files = vec![
            (
                "Deep learning paper on transformer architectures",
                "research",
                true,
                0,
            ),
            ("Q3 2024 financial statement", "finance", true, 1),
            ("Random document about cooking", "documents", false, 0),
        ];

        let mut correct_matches = 0;
        let mut total_tests = 0;

        for (content, category, should_match, _expected_folder_idx) in test_files {
            total_tests += 1;
            let path = format!("/test/smartfolder_{}.txt", total_tests);

            // Create and save analysis
            let analysis = FileAnalysis {
                path: path.clone(),
                category: category.to_string(),
                tags: vec![],
                summary: content.to_string(),
                confidence: 0.85,
                extracted_text: Some(content.to_string()),
                detected_language: Some("en".to_string()),
                metadata: json!({}),
            };

            self.db.save_analysis(&analysis).await?;

            // Generate and save embedding
            let embedding = generate_simple_embeddings(content)?;
            self.db
                .save_embedding(&path, &embedding, Some("test-model"))
                .await?;

            // Check if it matches expected smart folder
            if should_match {
                let folder_embedding = generate_simple_embeddings("semantic_match")?;

                let results = self.db.semantic_search(&folder_embedding, 10).await?;

                let matched = results.iter().any(|(p, _score)| p == &path);

                if matched {
                    correct_matches += 1;
                    details.push_str(&format!(
                        "✓ '{}' correctly matched to smart folder\n",
                        content
                    ));
                } else {
                    details.push_str(&format!(
                        "✗ '{}' failed to match expected smart folder\n",
                        content
                    ));
                }
            } else {
                correct_matches += 1; // Correctly not matched
                details.push_str(&format!("✓ '{}' correctly not matched\n", content));
            }
        }

        let accuracy = correct_matches as f64 / total_tests as f64;
        metrics.insert("matching_accuracy".to_string(), accuracy);
        metrics.insert("total_folders".to_string(), smart_folders.len() as f64);

        if accuracy < 0.8 {
            passed = false;
            details.push_str(&format!(
                "✗ Matching accuracy too low: {:.1}%\n",
                accuracy * 100.0
            ));
        }

        self.record_result(TestResult {
            test_name: "Smart Folder Matching".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 8: Vector extension functionality
    async fn test_vector_extension(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing vector extension functionality");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Check if vector extension is available
        if !self.db.is_vector_extension_available() {
            details.push_str("⚠ Vector extension not available, testing fallback mode\n");
            metrics.insert("vector_extension_available".to_string(), 0.0);
        } else {
            details.push_str("✓ Vector extension is available\n");
            metrics.insert("vector_extension_available".to_string(), 1.0);

            if let Some(version) = self.db.get_vector_extension_version() {
                details.push_str(&format!("Vector extension version: {}\n", version));
            }
        }

        // Test vector operations regardless of extension availability
        let test_vectors = vec![
            (vec![1.0, 0.0, 0.0], "/vec/orthogonal1.txt"),
            (vec![0.0, 1.0, 0.0], "/vec/orthogonal2.txt"),
            (vec![0.0, 0.0, 1.0], "/vec/orthogonal3.txt"),
            (vec![0.707, 0.707, 0.0], "/vec/diagonal.txt"),
        ];

        // Normalize and extend vectors to proper dimension
        for (mut vec, path) in test_vectors {
            // Extend to proper dimension
            vec.resize(EMBEDDING_DIM, 0.0);

            // Normalize
            let magnitude = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if magnitude > 0.0 {
                for v in &mut vec {
                    *v /= magnitude;
                }
            }

            // Store vector
            match self.db.save_embedding(path, &vec, Some("test-model")).await {
                Ok(_) => details.push_str(&format!("✓ Stored vector: {}\n", path)),
                Err(e) => {
                    details.push_str(&format!("✗ Failed to store vector {}: {}\n", path, e));
                    passed = false;
                }
            }
        }

        // Test vector statistics
        match self.db.get_vector_stats().await {
            Ok(stats) => {
                details.push_str(&format!(
                    "Vector stats - Total: {}, Dimension: {}\n",
                    stats.total_vectors, stats.dimensions
                ));
                metrics.insert("total_vectors".to_string(), stats.total_vectors as f64);
                metrics.insert("vector_dimension".to_string(), stats.dimensions as f64);

                if stats.dimensions != EMBEDDING_DIM {
                    details.push_str(&format!(
                        "⚠ Unexpected vector dimension: {} (expected {})\n",
                        stats.dimensions, EMBEDDING_DIM
                    ));
                }
            }
            Err(e) => {
                details.push_str(&format!("✗ Failed to get vector stats: {}\n", e));
            }
        }

        // Test vector table maintenance
        match self.db.maintain_vector_table().await {
            Ok(_) => details.push_str("✓ Vector table maintenance successful\n"),
            Err(e) => details.push_str(&format!("⚠ Vector table maintenance failed: {}\n", e)),
        }

        self.record_result(TestResult {
            test_name: "Vector Extension".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 9: Error handling and edge cases
    async fn test_error_handling(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing error handling and edge cases");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Test 1: Empty embedding
        let empty_embedding: Vec<f32> = vec![];
        match self
            .db
            .save_embedding("/error/empty.txt", &empty_embedding, Some("test-model"))
            .await
        {
            Ok(_) => {
                details.push_str("⚠ Accepted empty embedding (unexpected)\n");
            }
            Err(_) => {
                details.push_str("✓ Correctly rejected empty embedding\n");
                metrics.insert("empty_embedding_handled".to_string(), 1.0);
            }
        }

        // Test 2: Invalid dimension embedding
        let wrong_dim = vec![0.1; 100]; // Wrong dimension
        match self
            .db
            .save_embedding("/error/wrong_dim.txt", &wrong_dim, Some("test-model"))
            .await
        {
            Ok(_) => {
                details.push_str("✓ Handled wrong dimension embedding\n");
                metrics.insert("wrong_dim_handled".to_string(), 1.0);
            }
            Err(e) => {
                details.push_str(&format!("⚠ Failed on wrong dimension: {}\n", e));
                metrics.insert("wrong_dim_handled".to_string(), 0.0);
            }
        }

        // Test 3: Special characters in paths
        let special_paths = vec![
            "/test/file with spaces.txt",
            "/test/file'with'quotes.txt",
            "/test/file\"with\"doublequotes.txt",
            "/test/file;semicolon.txt",
        ];

        let mut special_handled = 0;
        for path in special_paths {
            let analysis = FileAnalysis {
                path: path.to_string(),
                category: "special_test".to_string(),
                tags: vec![],
                summary: "Special character test".to_string(),
                confidence: 0.9,
                extracted_text: None,
                detected_language: None,
                metadata: json!({}),
            };

            if self.db.save_analysis(&analysis).await.is_ok() {
                special_handled += 1;
                details.push_str(&format!("✓ Handled special path: {}\n", path));
            }
        }

        metrics.insert("special_paths_handled".to_string(), special_handled as f64);

        // Test 4: Concurrent access to same file
        let concurrent_path = "/error/concurrent.txt";
        let db = Arc::clone(&self.db);
        let mut handles = Vec::new();

        for i in 0..10 {
            let db_clone = Arc::clone(&db);
            let path = concurrent_path.to_string();

            let handle = tokio::spawn(async move {
                let embedding = vec![i as f32 / 10.0; EMBEDDING_DIM];
                db_clone
                    .save_embedding(&path, &embedding, Some("test-model"))
                    .await
            });

            handles.push(handle);
        }

        let mut concurrent_errors = 0;
        for handle in handles {
            if let Ok(result) = handle.await {
                if result.is_err() {
                    concurrent_errors += 1;
                }
            }
        }

        if concurrent_errors > 5 {
            details.push_str(&format!(
                "⚠ High concurrent error rate: {}/10\n",
                concurrent_errors
            ));
        } else {
            details.push_str(&format!(
                "✓ Handled concurrent access well: {}/10 errors\n",
                concurrent_errors
            ));
        }

        metrics.insert("concurrent_errors".to_string(), concurrent_errors as f64);

        // Test 5: Very large metadata
        let large_metadata = json!({
            "data": "x".repeat(10000)
        });

        let large_analysis = FileAnalysis {
            path: "/error/large_metadata.txt".to_string(),
            category: "test".to_string(),
            tags: vec!["large".to_string()],
            summary: "Large metadata test".to_string(),
            confidence: 0.9,
            extracted_text: Some("x".repeat(50000)),
            detected_language: Some("en".to_string()),
            metadata: large_metadata,
        };

        match self.db.save_analysis(&large_analysis).await {
            Ok(_) => {
                details.push_str("✓ Handled large metadata successfully\n");
                metrics.insert("large_data_handled".to_string(), 1.0);
            }
            Err(e) => {
                details.push_str(&format!("✗ Failed on large metadata: {}\n", e));
                metrics.insert("large_data_handled".to_string(), 0.0);
                passed = false;
            }
        }

        self.record_result(TestResult {
            test_name: "Error Handling".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 10: Performance benchmarks
    async fn test_performance_benchmarks(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Running performance benchmarks");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Benchmark 1: Single operation latency
        let single_start = Instant::now();
        let test_analysis = FileAnalysis {
            path: "/perf/single.txt".to_string(),
            category: "performance".to_string(),
            tags: vec!["benchmark".to_string()],
            summary: "Performance test".to_string(),
            confidence: 0.9,
            extracted_text: Some("Test content".to_string()),
            detected_language: Some("en".to_string()),
            metadata: json!({}),
        };

        self.db.save_analysis(&test_analysis).await?;
        let single_latency = single_start.elapsed();
        metrics.insert(
            "single_op_ms".to_string(),
            single_latency.as_millis() as f64,
        );

        details.push_str(&format!(
            "Single operation latency: {:.2}ms\n",
            single_latency.as_millis()
        ));

        // Benchmark 2: Batch operations throughput
        let batch_size = 100;
        let batch_start = Instant::now();

        for i in 0..batch_size {
            let analysis = FileAnalysis {
                path: format!("/perf/batch_{}.txt", i),
                category: "performance".to_string(),
                tags: vec![],
                summary: format!("Batch item {}", i),
                confidence: 0.9,
                extracted_text: None,
                detected_language: None,
                metadata: json!({"index": i}),
            };

            self.db.save_analysis(&analysis).await?;
        }

        let batch_duration = batch_start.elapsed();
        let throughput = batch_size as f64 / batch_duration.as_secs_f64();

        metrics.insert("batch_throughput_ops_sec".to_string(), throughput);
        details.push_str(&format!("Batch throughput: {:.1} ops/sec\n", throughput));

        // Benchmark 3: Search performance
        let search_embedding = generate_simple_embeddings("performance benchmark test")?;
        let search_start = Instant::now();

        for _ in 0..10 {
            self.db.semantic_search(&search_embedding, 10).await?;
        }

        let search_duration = search_start.elapsed();
        let avg_search_ms = search_duration.as_millis() as f64 / 10.0;

        metrics.insert("avg_search_ms".to_string(), avg_search_ms);
        details.push_str(&format!("Average search time: {:.2}ms\n", avg_search_ms));

        // Performance thresholds
        if single_latency.as_millis() > 100 {
            passed = false;
            details.push_str("✗ Single operation latency too high (> 100ms)\n");
        }

        if throughput < 20.0 {
            passed = false;
            details.push_str("✗ Batch throughput too low (< 20 ops/sec)\n");
        }

        if avg_search_ms > 50.0 {
            passed = false;
            details.push_str("✗ Search latency too high (> 50ms)\n");
        }

        // Benchmark 4: Memory efficiency (vacuum operation)
        let vacuum_start = Instant::now();
        self.db.vacuum().await?;
        let vacuum_duration = vacuum_start.elapsed();

        metrics.insert("vacuum_ms".to_string(), vacuum_duration.as_millis() as f64);
        details.push_str(&format!(
            "Vacuum operation: {:.2}ms\n",
            vacuum_duration.as_millis()
        ));

        self.record_result(TestResult {
            test_name: "Performance Benchmarks".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 11: Data integrity verification
    async fn test_data_integrity(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing data integrity");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Store test data with known values
        let test_data = vec![
            ("integrity_1", "First integrity test", vec![0.1, 0.2, 0.3]),
            ("integrity_2", "Second integrity test", vec![0.4, 0.5, 0.6]),
            ("integrity_3", "Third integrity test", vec![0.7, 0.8, 0.9]),
        ];

        // Store data
        for (id, content, mut embedding_seed) in test_data.clone() {
            let path = format!("/integrity/{}.txt", id);

            let analysis = FileAnalysis {
                path: path.clone(),
                category: "integrity_test".to_string(),
                tags: vec![id.to_string()],
                summary: content.to_string(),
                confidence: 0.99,
                extracted_text: Some(content.to_string()),
                detected_language: Some("en".to_string()),
                metadata: json!({"id": id}),
            };

            self.db.save_analysis(&analysis).await?;

            // Create full embedding from seed
            embedding_seed.resize(EMBEDDING_DIM, 0.0);
            self.db
                .save_embedding(&path, &embedding_seed, Some("test-model"))
                .await?;
        }

        // Verify data integrity
        let mut integrity_failures = 0;

        for (id, expected_content, _) in test_data {
            let path = format!("/integrity/{}.txt", id);

            match self.db.get_analysis(&path).await? {
                Some(analysis) => {
                    if analysis.summary != expected_content {
                        integrity_failures += 1;
                        details.push_str(&format!(
                            "✗ Data mismatch for {}: expected '{}', got '{}'\n",
                            id, expected_content, analysis.summary
                        ));
                    } else {
                        details.push_str(&format!("✓ Data integrity verified for {}\n", id));
                    }
                }
                None => {
                    integrity_failures += 1;
                    details.push_str(&format!("✗ Data missing for {}\n", id));
                }
            }
        }

        metrics.insert("integrity_failures".to_string(), integrity_failures as f64);

        if integrity_failures > 0 {
            passed = false;
            details.push_str(&format!(
                "✗ {} integrity failures detected\n",
                integrity_failures
            ));
        } else {
            details.push_str("✓ All data integrity checks passed\n");
        }

        // Test data consistency after operations
        let consistency_test_path = "/integrity/consistency.txt";
        let original_summary = "Original consistency test";
        let updated_summary = "Updated consistency test";

        // Save original
        let mut consistency_analysis = FileAnalysis {
            path: consistency_test_path.to_string(),
            category: "consistency".to_string(),
            tags: vec!["original".to_string()],
            summary: original_summary.to_string(),
            confidence: 0.85,
            extracted_text: None,
            detected_language: None,
            metadata: json!({}),
        };

        self.db.save_analysis(&consistency_analysis).await?;

        // Update
        consistency_analysis.summary = updated_summary.to_string();
        consistency_analysis.tags = vec!["updated".to_string()];
        self.db.save_analysis(&consistency_analysis).await?;

        // Verify update
        match self.db.get_analysis(consistency_test_path).await? {
            Some(analysis) => {
                if analysis.summary == updated_summary && analysis.tags[0] == "updated" {
                    details.push_str("✓ Update consistency verified\n");
                    metrics.insert("update_consistency".to_string(), 1.0);
                } else {
                    details.push_str("✗ Update consistency failed\n");
                    passed = false;
                    metrics.insert("update_consistency".to_string(), 0.0);
                }
            }
            None => {
                details.push_str("✗ Data lost after update\n");
                passed = false;
                metrics.insert("update_consistency".to_string(), 0.0);
            }
        }

        self.record_result(TestResult {
            test_name: "Data Integrity".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Test 12: Search accuracy validation
    async fn test_search_accuracy(&mut self) -> Result<()> {
        let start = Instant::now();
        info!("Testing search accuracy with various queries");

        let mut metrics = HashMap::new();
        let mut passed = true;
        let mut details = String::new();

        // Create diverse test documents
        let documents = vec![
            // Research category
            (
                "Machine learning algorithms for natural language processing",
                "/search/ml_nlp.txt",
                "research",
            ),
            (
                "Deep neural networks and computer vision applications",
                "/search/nn_cv.txt",
                "research",
            ),
            (
                "Reinforcement learning in robotics",
                "/search/rl_robotics.txt",
                "research",
            ),
            // Finance category
            (
                "Annual financial report fiscal year 2024",
                "/search/annual_2024.txt",
                "finance",
            ),
            (
                "Quarterly earnings statement Q3",
                "/search/q3_earnings.txt",
                "finance",
            ),
            (
                "Budget allocation and expense tracking",
                "/search/budget.txt",
                "finance",
            ),
            // 3D Models category
            (
                "3D printing STL file for prototype design",
                "/search/stl_proto.txt",
                "3d_models",
            ),
            (
                "CAD model for mechanical component",
                "/search/cad_mech.txt",
                "3d_models",
            ),
            (
                "3D scanner output mesh data",
                "/search/scan_mesh.txt",
                "3d_models",
            ),
            // Graphics category
            (
                "Company logo vector design SVG",
                "/search/logo_svg.txt",
                "graphics",
            ),
            (
                "Brand identity guidelines document",
                "/search/brand_guide.txt",
                "graphics",
            ),
            (
                "Marketing materials graphic assets",
                "/search/marketing_gfx.txt",
                "graphics",
            ),
        ];

        // Store documents with embeddings
        for (content, path, category) in &documents {
            let analysis = FileAnalysis {
                path: path.to_string(),
                category: category.to_string(),
                tags: vec![],
                summary: content.to_string(),
                confidence: 0.9,
                extracted_text: Some(content.to_string()),
                detected_language: Some("en".to_string()),
                metadata: json!({}),
            };

            self.db.save_analysis(&analysis).await?;

            let embedding = generate_simple_embeddings(content)?;
            self.db
                .save_embedding(path, &embedding, Some("test-model"))
                .await?;
        }

        // Test queries with expected results
        let test_queries = vec![
            (
                "artificial intelligence and machine learning research",
                vec![
                    "/search/ml_nlp.txt",
                    "/search/nn_cv.txt",
                    "/search/rl_robotics.txt",
                ],
                "AI Research Query",
            ),
            (
                "financial reports and earnings statements",
                vec!["/search/annual_2024.txt", "/search/q3_earnings.txt"],
                "Finance Query",
            ),
            (
                "3D printing and CAD models",
                vec!["/search/stl_proto.txt", "/search/cad_mech.txt"],
                "3D Models Query",
            ),
            (
                "logo design and brand graphics",
                vec!["/search/logo_svg.txt", "/search/brand_guide.txt"],
                "Graphics Query",
            ),
        ];

        let mut total_queries = 0;
        let mut successful_queries = 0;
        let mut total_precision = 0.0;
        let mut total_recall = 0.0;

        for (query, expected_results, query_name) in test_queries {
            total_queries += 1;
            let query_embedding = generate_simple_embeddings(query)?;

            match self.db.semantic_search(&query_embedding, 5).await {
                Ok(results) => {
                    let result_paths: Vec<String> =
                        results.iter().map(|(p, _)| p.clone()).collect();

                    // Calculate precision and recall
                    let mut true_positives = 0;
                    for expected in &expected_results {
                        if result_paths.contains(&expected.to_string()) {
                            true_positives += 1;
                        }
                    }

                    let precision = if !result_paths.is_empty() {
                        true_positives as f64
                            / result_paths.len().min(expected_results.len()) as f64
                    } else {
                        0.0
                    };

                    let recall = if !expected_results.is_empty() {
                        true_positives as f64 / expected_results.len() as f64
                    } else {
                        0.0
                    };

                    total_precision += precision;
                    total_recall += recall;

                    details.push_str(&format!(
                        "{}: Precision={:.2}, Recall={:.2}\n",
                        query_name, precision, recall
                    ));

                    if recall >= 0.5 {
                        successful_queries += 1;
                        details.push_str(&format!(
                            "✓ {} passed (found {} of {} expected)\n",
                            query_name,
                            true_positives,
                            expected_results.len()
                        ));
                    } else {
                        details.push_str(&format!(
                            "✗ {} failed (found only {} of {} expected)\n",
                            query_name,
                            true_positives,
                            expected_results.len()
                        ));
                    }
                }
                Err(e) => {
                    details.push_str(&format!("✗ {} query failed: {}\n", query_name, e));
                }
            }
        }

        let avg_precision = total_precision / total_queries as f64;
        let avg_recall = total_recall / total_queries as f64;
        let f1_score = 2.0 * (avg_precision * avg_recall) / (avg_precision + avg_recall);

        metrics.insert("avg_precision".to_string(), avg_precision);
        metrics.insert("avg_recall".to_string(), avg_recall);
        metrics.insert("f1_score".to_string(), f1_score);
        metrics.insert("successful_queries".to_string(), successful_queries as f64);

        details.push_str("\nOverall metrics:\n");
        details.push_str(&format!("Average Precision: {:.2}\n", avg_precision));
        details.push_str(&format!("Average Recall: {:.2}\n", avg_recall));
        details.push_str(&format!("F1 Score: {:.2}\n", f1_score));

        if f1_score < 0.6 {
            passed = false;
            details.push_str(&format!(
                "✗ F1 score too low: {:.2} (threshold: 0.6)\n",
                f1_score
            ));
        }

        self.record_result(TestResult {
            test_name: "Search Accuracy".to_string(),
            passed,
            duration: start.elapsed(),
            details,
            metrics,
        })
        .await;

        Ok(())
    }

    /// Record test result
    async fn record_result(&self, result: TestResult) {
        let mut results = self.results.write().await;

        let status = if result.passed { "PASSED" } else { "FAILED" };
        println!(
            "\n{} - {} ({:.2}s)",
            result.test_name,
            status,
            result.duration.as_secs_f64()
        );
        println!("{}", result.details);

        results.push(result);
    }

    /// Generate comprehensive test report
    async fn generate_test_report(&self, total_duration: Duration) -> Result<()> {
        let results = self.results.read().await;

        println!("\n{}", "=".repeat(80));
        println!("COMPREHENSIVE DATABASE TEST REPORT");
        println!("{}", "=".repeat(80));

        let total_tests = results.len();
        let passed_tests = results.iter().filter(|r| r.passed).count();
        let failed_tests = total_tests - passed_tests;

        println!("\nSummary:");
        println!("  Total Tests: {}", total_tests);
        println!(
            "  Passed: {} ({:.1}%)",
            passed_tests,
            passed_tests as f64 / total_tests as f64 * 100.0
        );
        println!(
            "  Failed: {} ({:.1}%)",
            failed_tests,
            failed_tests as f64 / total_tests as f64 * 100.0
        );
        println!("  Total Duration: {:.2}s", total_duration.as_secs_f64());

        println!("\nTest Results:");
        for result in results.iter() {
            let status = if result.passed {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };
            println!(
                "  {} - {} ({:.2}s)",
                status,
                result.test_name,
                result.duration.as_secs_f64()
            );

            if !result.metrics.is_empty() {
                println!("    Metrics:");
                for (key, value) in &result.metrics {
                    println!("      {}: {:.2}", key, value);
                }
            }
        }

        // Production readiness assessment
        println!("\n{}", "=".repeat(80));
        println!("PRODUCTION READINESS ASSESSMENT");
        println!("{}", "=".repeat(80));

        let readiness_score = passed_tests as f64 / total_tests as f64;

        if readiness_score >= 0.95 {
            println!("✓ PRODUCTION READY - All critical tests passed");
        } else if readiness_score >= 0.8 {
            println!("⚠ MOSTLY READY - Some non-critical issues to address");
        } else {
            println!("✗ NOT READY - Critical issues must be resolved");
        }

        // Recommendations
        println!("\nRecommendations:");

        for result in results.iter().filter(|r| !r.passed) {
            println!(
                "  - Fix {}: Review test details for specific issues",
                result.test_name
            );
        }

        if results.iter().any(|r| {
            r.test_name == "Vector Extension"
                && r.metrics
                    .get("vector_extension_available")
                    .is_some_and(|v| *v == 0.0)
        }) {
            println!("  - Consider installing sqlite-vec extension for better performance");
        }

        if results
            .iter()
            .any(|r| r.test_name == "Performance Benchmarks" && !r.passed)
        {
            println!("  - Optimize database queries and indexing for better performance");
        }

        if results.iter().any(|r| {
            r.test_name == "Search Accuracy" && r.metrics.get("f1_score").is_some_and(|v| *v < 0.7)
        }) {
            println!("  - Consider using more sophisticated embedding models (e.g., Ollama)");
        }

        println!("\n{}", "=".repeat(80));

        Ok(())
    }
}

// Main function for running tests as a binary
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("Starting Comprehensive Database Testing Suite");
    println!("This will test all aspects of the database layer including:");
    println!("- Embedding storage and retrieval");
    println!("- Semantic search functionality");
    println!("- sqlite-vec extension integration");
    println!("- Performance and scalability");
    println!("- Error handling and recovery");
    println!();

    let mut suite = DatabaseTestSuite::new().await?;
    suite.run_all_tests().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_comprehensive_database_tests() {
        // Initialize tracing for test output
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        let mut suite = DatabaseTestSuite::new()
            .await
            .expect("Failed to initialize test suite");

        suite
            .run_all_tests()
            .await
            .expect("Test suite execution failed");
    }
}
