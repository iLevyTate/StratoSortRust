// Test Fixtures
// Provides reusable test data and mock services

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use tempfile::TempDir;

use crate::error::AppError;
use crate::storage::Database;
use crate::models::{File, SmartFolder, Tag};

// Base fixture trait
#[async_trait::async_trait]
pub trait TestFixture: Send + Sync {
    async fn setup(&self) -> Result<(), AppError>;
    async fn teardown(&self) -> Result<(), AppError>;
}

// Database fixture for test data
pub struct DatabaseFixture {
    database: Arc<RwLock<Database>>,
    temp_dir: TempDir,
    sample_data: SampleDataConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleDataConfig {
    pub file_count: usize,
    pub folder_count: usize,
    pub tag_count: usize,
    pub with_embeddings: bool,
    pub with_relationships: bool,
}

impl Default for SampleDataConfig {
    fn default() -> Self {
        Self {
            file_count: 100,
            folder_count: 10,
            tag_count: 20,
            with_embeddings: false,
            with_relationships: true,
        }
    }
}

impl DatabaseFixture {
    pub async fn new(config: SampleDataConfig) -> Result<Self, AppError> {
        // Create temporary directory
        let temp_dir = tempfile::tempdir()
            .map_err(|e| AppError::IoError {
                message: format!("Failed to create temp dir: {}", e)
            })?;

        // Create database
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new_with_path(db_path.to_str().unwrap()).await?;

        Ok(Self {
            database: Arc::new(RwLock::new(database)),
            temp_dir,
            sample_data: config,
        })
    }

    pub async fn populate_sample_data(&self) -> Result<(), AppError> {
        let db = self.database.write().await;

        // Create sample tags
        let mut tag_ids = Vec::new();
        for i in 0..self.sample_data.tag_count {
            let tag = Tag {
                id: 0, // Will be set by database
                name: format!("tag_{}", i),
                color: format!("#{:06x}", i * 100000 % 0xFFFFFF),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            let id = db.create_tag(tag).await?;
            tag_ids.push(id);
        }

        // Create sample smart folders
        let mut folder_ids = Vec::new();
        for i in 0..self.sample_data.folder_count {
            let folder = SmartFolder {
                id: 0, // Will be set by database
                name: format!("folder_{}", i),
                description: Some(format!("Test folder {}", i)),
                rules: serde_json::json!({
                    "conditions": [{
                        "field": "extension",
                        "operator": "equals",
                        "value": format!("ext{}", i % 5)
                    }]
                }),
                color: Some(format!("#{:06x}", i * 200000 % 0xFFFFFF)),
                icon: Some(format!("icon_{}", i % 10)),
                sort_order: i as i32,
                parent_id: if i > 0 && self.sample_data.with_relationships {
                    Some(folder_ids[(i - 1) / 2])
                } else {
                    None
                },
                is_active: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            let id = db.create_smart_folder(folder).await?;
            folder_ids.push(id);
        }

        // Create sample files
        for i in 0..self.sample_data.file_count {
            let file = File {
                id: 0, // Will be set by database
                path: format!("/test/path/file_{}.ext{}", i, i % 5),
                name: format!("file_{}.ext{}", i, i % 5),
                extension: Some(format!("ext{}", i % 5)),
                size: (i * 1024) as i64,
                mime_type: Some(format!("application/type{}", i % 3)),
                checksum: Some(format!("{:064x}", i)),
                metadata: Some(serde_json::json!({
                    "test": true,
                    "index": i,
                    "category": format!("cat_{}", i % 10)
                })),
                tags: if self.sample_data.with_relationships && !tag_ids.is_empty() {
                    Some(vec![tag_ids[i % tag_ids.len()].to_string()])
                } else {
                    None
                },
                smart_folder_id: if self.sample_data.with_relationships && !folder_ids.is_empty() {
                    Some(folder_ids[i % folder_ids.len()])
                } else {
                    None
                },
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                last_accessed: Some(chrono::Utc::now()),
                is_favorite: i % 10 == 0,
                is_archived: i % 20 == 0,
            };

            db.insert_file(file).await?;

            // Add embeddings if configured
            if self.sample_data.with_embeddings {
                // Generate dummy embedding
                let embedding: Vec<f32> = (0..384).map(|j| ((i + j) as f32).sin()).collect();
                // Store embedding using vector extension method
                // Note: store_embedding is not a direct method, would need VectorExtension
                // For testing, we'll skip embedding storage since it requires VectorExtension
                // db.vector_ext.store_embedding(&format!("/test/path/file_{}.ext{}", i, i % 5), embedding).await?;
                let _ = embedding; // Suppress unused warning
            }
        }

        Ok(())
    }

    pub fn get_database(&self) -> Arc<RwLock<Database>> {
        self.database.clone()
    }

    pub fn get_temp_dir(&self) -> &Path {
        self.temp_dir.path()
    }
}

#[async_trait::async_trait]
impl TestFixture for DatabaseFixture {
    async fn setup(&self) -> Result<(), AppError> {
        self.populate_sample_data().await
    }

    async fn teardown(&self) -> Result<(), AppError> {
        // Database cleanup handled by Drop trait
        Ok(())
    }
}

// File system fixture for test files
pub struct FileSystemFixture {
    root_dir: TempDir,
    file_structure: FileStructureConfig,
    created_files: Arc<RwLock<Vec<PathBuf>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStructureConfig {
    pub directories: Vec<String>,
    pub files: HashMap<String, FileContent>,
    pub symlinks: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileContent {
    Text(String),
    Binary(Vec<u8>),
    Random { size: usize },
}

impl FileSystemFixture {
    pub fn new(config: FileStructureConfig) -> Result<Self, AppError> {
        let root_dir = tempfile::tempdir()
            .map_err(|e| AppError::IoError {
                message: format!("Failed to create temp dir: {}", e)
            })?;

        Ok(Self {
            root_dir,
            file_structure: config,
            created_files: Arc::new(RwLock::new(Vec::new())),
        })
    }

    pub async fn create_structure(&self) -> Result<(), AppError> {
        let root = self.root_dir.path();

        // Create directories
        for dir in &self.file_structure.directories {
            let path = root.join(dir);
            std::fs::create_dir_all(&path)
                .map_err(|e| AppError::IoError {
                    message: format!("Failed to create directory: {}", e)
                })?;
        }

        // Create files
        let mut created = self.created_files.write().await;
        for (relative_path, content) in &self.file_structure.files {
            let path = root.join(relative_path);

            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::IoError {
                        message: format!("Failed to create parent dir: {}", e)
                    })?;
            }

            // Write content
            match content {
                FileContent::Text(text) => {
                    std::fs::write(&path, text)
                        .map_err(|e| AppError::IoError {
                            message: format!("Failed to write text file: {}", e)
                        })?;
                }
                FileContent::Binary(data) => {
                    std::fs::write(&path, data)
                        .map_err(|e| AppError::IoError {
                            message: format!("Failed to write binary file: {}", e)
                        })?;
                }
                FileContent::Random { size } => {
                    // Generate random data
                    let data: Vec<u8> = (0..*size).map(|i| (i % 256) as u8).collect();
                    std::fs::write(&path, data)
                        .map_err(|e| AppError::IoError {
                            message: format!("Failed to write random file: {}", e)
                        })?;
                }
            }

            created.push(path);
        }

        // Create symlinks
        #[cfg(unix)]
        for (link_path, target) in &self.file_structure.symlinks {
            let link = root.join(link_path);
            let target_path = root.join(target);

            std::os::unix::fs::symlink(&target_path, &link)
                .map_err(|e| AppError::IoError {
                    message: format!("Failed to create symlink: {}", e)
                })?;
        }

        Ok(())
    }

    pub fn get_root(&self) -> &Path {
        self.root_dir.path()
    }

    pub fn get_file_path(&self, relative: &str) -> PathBuf {
        self.root_dir.path().join(relative)
    }

    pub async fn add_file(&self, relative_path: &str, content: FileContent) -> Result<PathBuf, AppError> {
        let path = self.root_dir.path().join(relative_path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::IoError {
                    message: format!("Failed to create parent dir: {}", e)
                })?;
        }

        // Write content
        match content {
            FileContent::Text(text) => {
                std::fs::write(&path, text)
                    .map_err(|e| AppError::IoError {
                        message: format!("Failed to write file: {}", e)
                    })?;
            }
            FileContent::Binary(data) => {
                std::fs::write(&path, &data)
                    .map_err(|e| AppError::IoError {
                        message: format!("Failed to write file: {}", e)
                    })?;
            }
            FileContent::Random { size } => {
                let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
                std::fs::write(&path, data)
                    .map_err(|e| AppError::IoError {
                        message: format!("Failed to write file: {}", e)
                    })?;
            }
        }

        let mut created = self.created_files.write().await;
        created.push(path.clone());

        Ok(path)
    }
}

#[async_trait::async_trait]
impl TestFixture for FileSystemFixture {
    async fn setup(&self) -> Result<(), AppError> {
        self.create_structure().await
    }

    async fn teardown(&self) -> Result<(), AppError> {
        // Cleanup handled by TempDir Drop trait
        Ok(())
    }
}

// API fixture for mocking external services
pub struct ApiFixture {
    mock_responses: Arc<RwLock<HashMap<String, MockResponse>>>,
    request_log: Arc<RwLock<Vec<MockRequest>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockResponse {
    pub status: u16,
    pub body: serde_json::Value,
    pub headers: HashMap<String, String>,
    pub delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Option<serde_json::Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for ApiFixture {
    fn default() -> Self {
        Self {
            mock_responses: Arc::new(RwLock::new(HashMap::new())),
            request_log: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl ApiFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_response(&self, endpoint: &str, response: MockResponse) {
        let mut responses = self.mock_responses.write().await;
        responses.insert(endpoint.to_string(), response);
    }

    pub async fn get_response(&self, endpoint: &str) -> Option<MockResponse> {
        let responses = self.mock_responses.read().await;
        responses.get(endpoint).cloned()
    }

    pub async fn log_request(&self, request: MockRequest) {
        let mut log = self.request_log.write().await;
        log.push(request);
    }

    pub async fn get_request_log(&self) -> Vec<MockRequest> {
        self.request_log.read().await.clone()
    }

    pub async fn clear_log(&self) {
        let mut log = self.request_log.write().await;
        log.clear();
    }

    pub async fn verify_request_made(&self, method: &str, path: &str) -> bool {
        let log = self.request_log.read().await;
        log.iter().any(|req| req.method == method && req.path == path)
    }

    pub async fn get_request_count(&self, path: &str) -> usize {
        let log = self.request_log.read().await;
        log.iter().filter(|req| req.path == path).count()
    }
}

#[async_trait::async_trait]
impl TestFixture for ApiFixture {
    async fn setup(&self) -> Result<(), AppError> {
        // Setup default responses if needed
        Ok(())
    }

    async fn teardown(&self) -> Result<(), AppError> {
        self.clear_log().await;
        Ok(())
    }
}

// Mock service for testing
pub struct MockService<T> {
    name: String,
    state: Arc<RwLock<T>>,
    behavior: Arc<RwLock<MockBehavior>>,
}

#[derive(Debug, Clone)]
pub enum MockBehavior {
    Success,
    Failure { error: String },
    Timeout { duration_ms: u64 },
    Intermittent { success_rate: f32 },
}

impl<T: Clone + Send + Sync> MockService<T> {
    pub fn new(name: impl Into<String>, initial_state: T) -> Self {
        Self {
            name: name.into(),
            state: Arc::new(RwLock::new(initial_state)),
            behavior: Arc::new(RwLock::new(MockBehavior::Success)),
        }
    }

    pub async fn set_behavior(&self, behavior: MockBehavior) {
        let mut b = self.behavior.write().await;
        *b = behavior;
    }

    pub async fn get_state(&self) -> T {
        self.state.read().await.clone()
    }

    pub async fn set_state(&self, state: T) {
        let mut s = self.state.write().await;
        *s = state;
    }

    pub async fn execute<F, R>(&self, operation: F) -> Result<R, AppError>
    where
        F: FnOnce(T) -> R,
    {
        let behavior = self.behavior.read().await.clone();

        match behavior {
            MockBehavior::Success => {
                let state = self.state.read().await.clone();
                Ok(operation(state))
            }
            MockBehavior::Failure { error } => {
                Err(AppError::SystemError { message: error })
            }
            MockBehavior::Timeout { duration_ms } => {
                tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
                Err(AppError::Timeout {
                    message: format!("{} timed out", self.name)
                })
            }
            MockBehavior::Intermittent { success_rate } => {
                if rand::random::<f32>() < success_rate {
                    let state = self.state.read().await.clone();
                    Ok(operation(state))
                } else {
                    Err(AppError::SystemError {
                        message: format!("{} intermittent failure", self.name)
                    })
                }
            }
        }
    }
}