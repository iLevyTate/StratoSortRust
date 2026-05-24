// Plugin test fixtures and utilities
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};
use tokio::sync::RwLock;

/// Mock application handle for testing plugins
pub struct MockAppHandle {
    #[allow(dead_code)]
    pub temp_dir: TempDir,
    pub data_dir: PathBuf,
    #[allow(dead_code)]
    pub config_dir: PathBuf,
    #[allow(dead_code)]
    pub cache_dir: PathBuf,
}

impl MockAppHandle {
    pub fn new() -> Self {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let base_path = temp_dir.path();

        let data_dir = base_path.join("data");
        let config_dir = base_path.join("config");
        let cache_dir = base_path.join("cache");

        std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
        std::fs::create_dir_all(&cache_dir).expect("Failed to create cache dir");

        Self {
            temp_dir,
            data_dir,
            config_dir,
            cache_dir,
        }
    }

    pub fn create_test_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.data_dir.join(name);
        std::fs::write(&path, content).expect("Failed to write test file");
        path
    }

    pub fn create_test_files(&self, count: usize) -> Vec<PathBuf> {
        (0..count)
            .map(|i| {
                let name = format!("test_file_{}.txt", i);
                let content = format!("Test content {}", i);
                self.create_test_file(&name, &content)
            })
            .collect()
    }
}

/// Mock window state for testing window-related plugins
#[derive(Debug, Clone, PartialEq)]
pub struct MockWindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub fullscreen: bool,
    pub focused: bool,
}

impl Default for MockWindowState {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 1024,
            height: 768,
            maximized: false,
            fullscreen: false,
            focused: true,
        }
    }
}

/// Mock process info for testing process plugin
#[derive(Debug, Clone)]
pub struct MockProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cmd: Vec<String>,
    pub memory_usage: u64,
    pub cpu_usage: f32,
}

impl MockProcessInfo {
    pub fn new(name: &str) -> Self {
        Self {
            pid: 1234,
            name: name.to_string(),
            cmd: vec![name.to_string()],
            memory_usage: 50_000_000, // 50MB
            cpu_usage: 5.0,
        }
    }
}

/// Mock OS info for testing OS plugin
#[derive(Debug, Clone)]
pub struct MockOsInfo {
    pub platform: String,
    pub version: String,
    pub arch: String,
    pub hostname: String,
    pub total_memory: u64,
    pub available_memory: u64,
}

impl Default for MockOsInfo {
    fn default() -> Self {
        Self {
            platform: "windows".to_string(),
            version: "10.0.19045".to_string(),
            arch: "x86_64".to_string(),
            hostname: "test-machine".to_string(),
            total_memory: 16_000_000_000,    // 16GB
            available_memory: 8_000_000_000, // 8GB
        }
    }
}

/// Mock HTTP response for testing HTTP plugin
#[derive(Debug, Clone)]
pub struct MockHttpResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
}

impl MockHttpResponse {
    pub fn ok_json(data: serde_json::Value) -> Self {
        let mut headers = std::collections::HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        Self {
            status: 200,
            headers,
            body: serde_json::to_vec(&data).unwrap(),
        }
    }

    pub fn error(status: u16, message: &str) -> Self {
        Self {
            status,
            headers: std::collections::HashMap::new(),
            body: message.as_bytes().to_vec(),
        }
    }
}

/// Mock update info for testing updater plugin
#[derive(Debug, Clone)]
pub struct MockUpdateInfo {
    pub version: String,
    #[allow(dead_code)]
    pub notes: String,
    #[allow(dead_code)]
    pub pub_date: String,
    pub download_url: String,
    pub signature: String,
}

impl Default for MockUpdateInfo {
    fn default() -> Self {
        Self {
            version: "0.2.0".to_string(),
            notes: "Bug fixes and improvements".to_string(),
            pub_date: "2024-01-20T10:00:00Z".to_string(),
            download_url: "https://example.com/download/v0.2.0".to_string(),
            signature: "mock_signature_12345".to_string(),
        }
    }
}

/// Mock localhost server for testing localhost plugin
pub struct MockLocalhostServer {
    pub port: u16,
    pub routes: Arc<RwLock<Vec<(String, String)>>>,
}

impl MockLocalhostServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            routes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn add_route(&self, path: &str, response: &str) {
        let mut routes = self.routes.write().await;
        routes.push((path.to_string(), response.to_string()));
    }

    pub async fn get_route(&self, path: &str) -> Option<String> {
        let routes = self.routes.read().await;
        routes
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, r)| r.clone())
    }
}

/// Test data generator for plugin tests
#[allow(dead_code)]
pub struct PluginTestDataGenerator;

#[allow(dead_code)]
impl PluginTestDataGenerator {
    pub fn sample_file_paths(count: usize) -> Vec<PathBuf> {
        (0..count)
            .map(|i| PathBuf::from(format!("/test/file_{}.txt", i)))
            .collect()
    }

    pub fn sample_file_organization() -> serde_json::Value {
        json!({
            "documents": [
                "/test/report.pdf",
                "/test/invoice.docx"
            ],
            "images": [
                "/test/photo1.jpg",
                "/test/screenshot.png"
            ],
            "archives": [
                "/test/backup.zip",
                "/test/data.tar.gz"
            ]
        })
    }

    pub fn sample_ai_analysis() -> serde_json::Value {
        json!({
            "category": "Documents",
            "confidence": 0.95,
            "tags": ["invoice", "financial", "2024"],
            "summary": "Financial invoice document",
            "suggested_location": "/organized/financial/2024/"
        })
    }
}

/// Assertion helpers for plugin tests
pub struct PluginAssertions;

impl PluginAssertions {
    pub fn assert_process_running(process: &MockProcessInfo) {
        assert!(process.pid > 0, "Process should have valid PID");
        assert!(!process.name.is_empty(), "Process should have name");
        assert!(process.memory_usage > 0, "Process should use memory");
    }

    pub fn assert_window_state_valid(state: &MockWindowState) {
        assert!(state.width > 0, "Window width should be positive");
        assert!(state.height > 0, "Window height should be positive");
        if state.maximized {
            assert!(
                !state.fullscreen,
                "Window cannot be both maximized and fullscreen"
            );
        }
    }

    pub fn assert_os_info_valid(info: &MockOsInfo) {
        assert!(!info.platform.is_empty(), "OS platform should be specified");
        assert!(!info.version.is_empty(), "OS version should be specified");
        assert!(info.total_memory > 0, "Total memory should be positive");
        assert!(
            info.available_memory <= info.total_memory,
            "Available memory should not exceed total memory"
        );
    }

    pub fn assert_http_response_ok(response: &MockHttpResponse) {
        assert!(
            response.status >= 200 && response.status < 300,
            "HTTP response should have success status"
        );
        assert!(
            !response.body.is_empty(),
            "Response body should not be empty"
        );
    }

    pub fn assert_update_available(info: &MockUpdateInfo) {
        assert!(
            !info.version.is_empty(),
            "Update version should be specified"
        );
        assert!(
            !info.download_url.is_empty(),
            "Download URL should be provided"
        );
        assert!(
            !info.signature.is_empty(),
            "Update signature should be provided"
        );
    }
}
