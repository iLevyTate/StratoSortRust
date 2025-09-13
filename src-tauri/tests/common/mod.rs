// Common test utilities and fixtures
use mockito;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use stratosort::{ai::FileAnalysis, config::Config, error::Result, storage::Database};
use tempfile::{tempdir, TempDir};
use uuid::Uuid;

// Test data builders
pub struct TestDataBuilder;

impl TestDataBuilder {
    pub fn file_analysis() -> FileAnalysisBuilder {
        FileAnalysisBuilder::new()
    }

    pub fn config() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    pub fn test_file() -> TestFileBuilder {
        TestFileBuilder::new()
    }
}

pub struct FileAnalysisBuilder {
    analysis: FileAnalysis,
}

impl FileAnalysisBuilder {
    fn new() -> Self {
        Self {
            analysis: FileAnalysis {
                path: "/test/file.txt".to_string(),
                category: "Documents".to_string(),
                tags: vec![],
                summary: "Test file".to_string(),
                confidence: 0.9,
                extracted_text: None,
                detected_language: None,
                metadata: serde_json::json!({}),
            },
        }
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.analysis.path = path.to_string();
        self
    }

    pub fn with_category(mut self, category: &str) -> Self {
        self.analysis.category = category.to_string();
        self
    }

    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.analysis.tags = tags.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.analysis.confidence = confidence;
        self
    }

    pub fn build(self) -> FileAnalysis {
        self.analysis
    }
}

pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    pub fn with_ollama_host(mut self, host: &str) -> Self {
        self.config.ollama_host = host.to_string();
        self
    }

    pub fn with_ollama_model(mut self, model: &str) -> Self {
        self.config.ollama_model = model.to_string();
        self
    }

    pub fn with_debug_mode(mut self, debug: bool) -> Self {
        self.config.debug_mode = debug;
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}

pub struct TestFileBuilder {
    path: Option<PathBuf>,
    content: String,
    file_type: String,
}

impl TestFileBuilder {
    fn new() -> Self {
        Self {
            path: None,
            content: "Test content".to_string(),
            file_type: "text/plain".to_string(),
        }
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_content(mut self, content: &str) -> Self {
        self.content = content.to_string();
        self
    }

    pub fn with_type(mut self, file_type: &str) -> Self {
        self.file_type = file_type.to_string();
        self
    }

    pub fn create(self) -> Result<PathBuf> {
        let path = self.path.unwrap_or_else(|| {
            let dir = tempdir().unwrap();
            dir.path().join("test_file.txt")
        });

        fs::write(&path, self.content)?;
        Ok(path)
    }
}

// Mock services
pub struct MockAiService {
    responses: Vec<FileAnalysis>,
    current_index: usize,
}

impl MockAiService {
    pub fn new() -> Self {
        Self {
            responses: vec![],
            current_index: 0,
        }
    }

    pub fn with_response(mut self, response: FileAnalysis) -> Self {
        self.responses.push(response);
        self
    }

    pub async fn analyze_file(&mut self, _content: &str, _file_type: &str) -> Result<FileAnalysis> {
        if self.current_index < self.responses.len() {
            let response = self.responses[self.current_index].clone();
            self.current_index += 1;
            Ok(response)
        } else {
            Ok(TestDataBuilder::file_analysis().build())
        }
    }
}

// Test environment setup
pub struct TestEnvironment {
    pub temp_dir: TempDir,
    pub config: Config,
    pub database: Option<Database>,
}

impl TestEnvironment {
    pub async fn new() -> Self {
        let temp_dir = tempdir().unwrap();
        let config = Config::default();

        // Config doesn't have database_path field anymore
        // Database paths are handled separately

        Self {
            temp_dir,
            config,
            database: None,
        }
    }

    pub async fn with_database(mut self) -> Self {
        let db_path = self.temp_dir.path().join("test.db");
        let db_url = format!("sqlite://{}", db_path.display());
        self.database = Some(Database::new_from_url(&db_url).await.unwrap());
        self
    }

    pub fn create_test_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    pub fn create_test_directory(&self, name: &str) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }
}

// Mock Ollama server
pub struct MockOllamaServer {
    server: mockito::ServerGuard,
}

impl MockOllamaServer {
    pub fn new() -> Self {
        Self {
            server: mockito::Server::new(),
        }
    }

    pub fn url(&self) -> String {
        self.server.url()
    }

    pub fn mock_list_models(&mut self) -> mockito::Mock {
        self.server
            .mock("GET", "/api/tags")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"models": [{"name": "test-model", "size": 1000}]}"#)
            .create()
    }

    pub fn mock_generate(&mut self, response: &str) -> mockito::Mock {
        self.server
            .mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{"response": "{}", "done": true}}"#, response))
            .create()
    }

    pub fn mock_embeddings(&mut self, embeddings: Vec<f32>) -> mockito::Mock {
        let embedding_str = serde_json::to_string(&embeddings).unwrap();
        self.server
            .mock("POST", "/api/embeddings")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{"embedding": {}}}"#, embedding_str))
            .create()
    }
}

// Test assertions
pub struct TestAssertions;

impl TestAssertions {
    pub fn assert_file_analysis(actual: &FileAnalysis, expected: &FileAnalysis) {
        assert_eq!(actual.path, expected.path, "Path mismatch");
        assert_eq!(actual.category, expected.category, "Category mismatch");
        assert_eq!(actual.tags, expected.tags, "Tags mismatch");
        assert!(
            (actual.confidence - expected.confidence).abs() < 0.01,
            "Confidence mismatch"
        );
    }

    pub fn assert_path_exists(path: &Path) {
        assert!(path.exists(), "Path does not exist: {:?}", path);
    }

    pub fn assert_file_content(path: &Path, expected_content: &str) {
        let content = fs::read_to_string(path).unwrap();
        assert_eq!(content, expected_content, "File content mismatch");
    }
}

// Performance measurement
pub struct PerformanceTimer {
    start: std::time::Instant,
    name: String,
}

impl PerformanceTimer {
    pub fn start(name: &str) -> Self {
        Self {
            start: std::time::Instant::now(),
            name: name.to_string(),
        }
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    pub fn assert_under_ms(&self, max_ms: u128) {
        let elapsed = self.elapsed_ms();
        assert!(
            elapsed < max_ms,
            "{} took {}ms, expected under {}ms",
            self.name,
            elapsed,
            max_ms
        );
    }
}

// Test data generators
pub struct TestDataGenerator;

impl TestDataGenerator {
    pub fn random_path() -> String {
        format!("/test/{}.txt", Uuid::new_v4())
    }

    pub fn random_content(size: usize) -> String {
        (0..size).map(|_| "x").collect()
    }

    pub fn sample_files(count: usize) -> Vec<String> {
        (0..count)
            .map(|i| format!("/test/file_{}.txt", i))
            .collect()
    }

    pub fn sample_embeddings(dimension: usize) -> Vec<f32> {
        (0..dimension).map(|i| (i as f32) * 0.1).collect()
    }
}

// Database test helpers
pub struct DatabaseTestHelper;

impl DatabaseTestHelper {
    pub async fn create_temp_database() -> Result<Database> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite://{}", db_path.display());
        Database::new_from_url(&db_url).await
    }

    pub async fn populate_with_test_data(db: &Database, count: usize) -> Result<()> {
        for i in 0..count {
            let analysis = TestDataBuilder::file_analysis()
                .with_path(&format!("/test/file_{}.txt", i))
                .with_category(if i % 2 == 0 { "Documents" } else { "Images" })
                .with_tags(vec![&format!("tag_{}", i)])
                .build();

            db.save_analysis(&analysis).await?;
        }
        Ok(())
    }
}

// Concurrency test helpers
pub struct ConcurrencyTestHelper;

impl ConcurrencyTestHelper {
    pub async fn run_concurrent_tasks<F, T>(task_count: usize, task_fn: F) -> Vec<T>
    where
        F: Fn(usize) -> T + Send + Sync + 'static,
        T: Send + 'static,
    {
        let mut handles = vec![];
        let task_fn = Arc::new(task_fn);

        for i in 0..task_count {
            let task_fn = task_fn.clone();
            let handle = tokio::spawn(async move { task_fn(i) });
            handles.push(handle);
        }

        let mut results = vec![];
        for handle in handles {
            results.push(handle.await.unwrap());
        }
        results
    }
}
