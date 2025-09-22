use crate::{
    ai::{AiService, FileAnalysis},
    config::Config,
    core::ContentExtractor,
    error::{AppError, Result},
};
use std::path::Path;
use std::sync::Arc;

pub struct FileAnalyzer {
    ai_service: Arc<AiService>,
    config: Arc<parking_lot::RwLock<Config>>,
    content_extractor: ContentExtractor,
}

impl FileAnalyzer {
    pub fn new(ai_service: Arc<AiService>, config: Arc<parking_lot::RwLock<Config>>) -> Self {
        Self {
            ai_service,
            config,
            content_extractor: ContentExtractor::new(),
        }
    }

    pub async fn analyze_file(&self, path: &str) -> Result<FileAnalysis> {
        // Check file size first
        self.check_file_size(path).await?;

        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        // Check if this is an image file - use vision analysis
        if mime_type.starts_with("image/") {
            // Use vision analysis for images
            return self.ai_service.analyze_image(path).await;
        }

        // For non-image files, use content extractor for better text extraction
        let content = self.content_extractor.extract_content(Path::new(path)).await?;
        self.ai_service.analyze_file(&content, &mime_type).await
    }

    pub async fn analyze_batch(&self, paths: Vec<String>) -> Vec<Result<FileAnalysis>> {
        let mut results = Vec::new();

        for path in paths {
            results.push(self.analyze_file(&path).await);
        }

        results
    }

    async fn check_file_size(&self, path: &str) -> Result<()> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|e| AppError::ProcessingError {
                message: format!("Failed to get file metadata: {}", e),
            })?;

        let file_size = metadata.len();
        let max_file_size = self.config.read().max_file_size;

        if file_size > max_file_size {
            return Err(AppError::ProcessingError {
                message: format!(
                    "File too large for analysis: {} bytes (max: {} bytes). Consider increasing max_file_size in settings.",
                    file_size, max_file_size
                ),
            });
        }

        Ok(())
    }

    #[allow(dead_code)] // Reserved for future content analysis features
    async fn read_file_content(&self, path: &str) -> Result<String> {
        use tokio::io::{AsyncReadExt, BufReader};

        let file = tokio::fs::File::open(path).await?;
        let mut reader = BufReader::new(file);

        // Determine read size based on configuration and file type
        let max_read_size = self.get_max_read_size(path);
        let mut buffer = vec![0u8; max_read_size];

        let bytes_read = reader.read(&mut buffer).await?;
        buffer.truncate(bytes_read);

        // Check if the file appears to be binary
        if self.is_binary_content(&buffer) {
            return Err(AppError::ProcessingError {
                message: "File appears to be binary and cannot be analyzed as text".to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    #[allow(dead_code)] // Reserved for future content analysis features
    fn get_max_read_size(&self, path: &str) -> usize {
        let config = self.config.read();
        let base_size = 10240; // 10KB default

        // For certain file types, we might want to read more
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            // Text files can be read more extensively
            "txt" | "md" | "markdown" | "csv" | "json" | "xml" | "yaml" | "yml" | "toml" => {
                std::cmp::min(config.max_file_size as usize, 100 * 1024) // Up to 100KB for text files
            }
            // Code files
            "rs" | "py" | "js" | "ts" | "html" | "css" | "cpp" | "c" | "java" | "go" => {
                std::cmp::min(config.max_file_size as usize, 50 * 1024) // Up to 50KB for code files
            }
            // Default for other files
            _ => std::cmp::min(config.max_file_size as usize, base_size),
        }
    }

    #[allow(dead_code)] // Reserved for future content analysis features
    fn is_binary_content(&self, buffer: &[u8]) -> bool {
        // Simple heuristic: if more than 10% of the first 512 bytes are non-printable, consider it binary
        let sample_size = std::cmp::min(buffer.len(), 512);
        if sample_size == 0 {
            return false;
        }

        let non_printable_count = buffer[..sample_size]
            .iter()
            .filter(|&&byte| byte < 32 && byte != 9 && byte != 10 && byte != 13) // Allow tab, newline, carriage return
            .count();

        let non_printable_ratio = non_printable_count as f64 / sample_size as f64;
        non_printable_ratio > 0.1
    }
}
