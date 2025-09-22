use crate::ai::ollama::OllamaClient;
use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn, error, info};

/// Service for extracting text content from various document formats
#[derive(Clone)]
pub struct ContentExtractor {
    ollama_client: Option<Arc<OllamaClient>>,
}

impl Default for ContentExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentExtractor {
    pub fn new() -> Self {
        Self { ollama_client: None }
    }

    /// Create extractor with Ollama client for enhanced analysis
    pub async fn new_with_llm() -> Self {
        // Use environment variable for Ollama host or fallback to default
        let ollama_host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let ollama_client = match OllamaClient::new(&ollama_host).await {
            Ok(client) => {
                info!("Ollama client initialized for enhanced content extraction");
                Some(Arc::new(client))
            }
            Err(e) => {
                warn!("Ollama client not available for enhanced extraction: {}", e);
                None
            }
        };

        Self { ollama_client }
    }

    /// Extract text content from a file based on its type
    pub async fn extract_content(&self, path: &Path) -> Result<String> {
        self.extract_content_with_options(path, false).await
    }

    /// Extract content with optional LLM enhancement
    pub async fn extract_content_with_options(&self, path: &Path, use_llm: bool) -> Result<String> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        debug!("Extracting content from file with extension: {}", extension);

        let content = match extension.as_str() {
            "pdf" => self.extract_pdf(path).await,
            "docx" => self.extract_docx(path).await,
            "xlsx" | "xls" => self.extract_excel(path).await,
            "csv" => self.extract_csv(path).await,
            "txt" | "md" | "log" => self.extract_text(path).await,
            "rtf" => self.extract_rtf(path).await,
            "html" | "htm" => self.extract_html(path).await,
            "json" => self.extract_json(path).await,
            _ => {
                // Try to read as plain text
                self.extract_text(path).await
            }
        }?;

        // Apply LLM enhancement if requested and available
        if use_llm && self.ollama_client.is_some() {
            self.enhance_with_llm(&content, path).await
        } else {
            Ok(content)
        }
    }

    /// Extract text from PDF files
    #[cfg(feature = "pdf-extract")]
    async fn extract_pdf(&self, path: &Path) -> Result<String> {
        use pdf_extract::extract_text;

        debug!("Extracting PDF content from: {:?}", path);

        // Try text extraction first
        match extract_text(path) {
            Ok(text) if !text.trim().is_empty() => {
                debug!("Successfully extracted {} characters from PDF", text.len());
                Ok(text)
            }
            Ok(_) => {
                warn!("PDF appears to be empty or contains only images");
                // OCR support would require additional system dependencies (tesseract)
                // For now, return a descriptive message for image-based PDFs
                Ok(String::from("[PDF contains no extractable text - may be scanned/image-based. OCR analysis not available]"))
            }
            Err(e) => {
                error!("Failed to extract PDF text: {}", e);
                Err(AppError::ProcessingError {
                    message: format!("Failed to extract PDF content: {}", e),
                })
            }
        }
    }

    #[cfg(not(feature = "pdf-extract"))]
    async fn extract_pdf(&self, _path: &Path) -> Result<String> {
        Err(AppError::ProcessingError {
            message: "PDF extraction not enabled. Rebuild with 'documents' feature".to_string(),
        })
    }

    /// Extract text from DOCX files
    #[cfg(feature = "docx-rs")]
    async fn extract_docx(&self, path: &Path) -> Result<String> {
        use docx_rs::read_docx;

        debug!("Extracting DOCX content from: {:?}", path);

        let file_bytes = std::fs::read(path).map_err(|e| AppError::ProcessingError {
            message: format!("Failed to read DOCX file: {}", e),
        })?;

        let docx = read_docx(&file_bytes).map_err(|e| AppError::ProcessingError {
            message: format!("Failed to parse DOCX: {}", e),
        })?;

        // Extract text from all paragraphs
        let mut text = String::new();

        // Note: docx-rs API might need adjustment based on actual implementation
        // This is a simplified version
        for child in &docx.document.children {
            if let docx_rs::DocumentChild::Paragraph(p) = child {
                for run in &p.children {
                    if let docx_rs::ParagraphChild::Run(r) = run {
                        for child in &r.children {
                            if let docx_rs::RunChild::Text(t) = child {
                                text.push_str(&t.text);
                                text.push(' ');
                            }
                        }
                    }
                }
                text.push('\n');
            }
        }

        debug!("Successfully extracted {} characters from DOCX", text.len());
        Ok(text)
    }

    #[cfg(not(feature = "docx-rs"))]
    async fn extract_docx(&self, _path: &Path) -> Result<String> {
        Err(AppError::ProcessingError {
            message: "DOCX extraction not enabled. Rebuild with 'documents' feature".to_string(),
        })
    }

    /// Extract text from Excel files
    #[cfg(feature = "calamine")]
    async fn extract_excel(&self, path: &Path) -> Result<String> {
        use calamine::{Reader, open_workbook_auto};

        debug!("Extracting Excel content from: {:?}", path);

        let mut workbook = open_workbook_auto(path).map_err(|e| AppError::ProcessingError {
            message: format!("Failed to open Excel file: {}", e),
        })?;

        let mut text = String::new();

        // Extract text from all sheets
        for sheet_name in workbook.sheet_names() {
            text.push_str(&format!("Sheet: {}\n", sheet_name));

            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                for row in range.rows() {
                    for cell in row {
                        text.push_str(&format!("{}\t", cell));
                    }
                    text.push('\n');
                }
            }
        }

        debug!("Successfully extracted {} characters from Excel", text.len());
        Ok(text)
    }

    #[cfg(not(feature = "calamine"))]
    async fn extract_excel(&self, _path: &Path) -> Result<String> {
        Err(AppError::ProcessingError {
            message: "Excel extraction not enabled. Rebuild with 'documents' feature".to_string(),
        })
    }

    /// Extract text from CSV files
    #[cfg(feature = "csv")]
    async fn extract_csv(&self, path: &Path) -> Result<String> {
        use csv::Reader;
        use std::fs::File;

        debug!("Extracting CSV content from: {:?}", path);

        let file = File::open(path).map_err(|e| AppError::ProcessingError {
            message: format!("Failed to open CSV file: {}", e),
        })?;

        let mut reader = Reader::from_reader(file);
        let mut text = String::new();

        // Extract headers
        if let Ok(headers) = reader.headers() {
            for header in headers {
                text.push_str(header);
                text.push('\t');
            }
            text.push('\n');
        }

        // Extract records
        for result in reader.records() {
            if let Ok(record) = result {
                for field in record.iter() {
                    text.push_str(field);
                    text.push('\t');
                }
                text.push('\n');
            }
        }

        debug!("Successfully extracted {} characters from CSV", text.len());
        Ok(text)
    }

    #[cfg(not(feature = "csv"))]
    async fn extract_csv(&self, _path: &Path) -> Result<String> {
        // CSV is simple enough to handle without a library
        self.extract_text(_path).await
    }

    /// Extract text from RTF files
    #[cfg(feature = "rtf-parser")]
    async fn extract_rtf(&self, path: &Path) -> Result<String> {
        debug!("Extracting RTF content from: {:?}", path);

        let content = tokio::fs::read_to_string(path).await?;

        // Basic RTF parsing - remove control sequences
        // This is a simplified approach; full RTF parsing is complex
        let mut text = String::new();
        let mut in_control = false;
        let mut brace_level = 0;

        for ch in content.chars() {
            match ch {
                '{' => brace_level += 1,
                '}' => brace_level -= 1,
                '\\' if !in_control => in_control = true,
                ' ' | '\n' | '\r' if in_control => {
                    in_control = false;
                    text.push(' ');
                }
                _ if !in_control && brace_level > 0 => text.push(ch),
                _ => {}
            }
        }

        Ok(text)
    }

    #[cfg(not(feature = "rtf-parser"))]
    async fn extract_rtf(&self, path: &Path) -> Result<String> {
        self.extract_text(path).await
    }

    /// Extract text from HTML files
    async fn extract_html(&self, path: &Path) -> Result<String> {
        debug!("Extracting HTML content from: {:?}", path);

        let content = tokio::fs::read_to_string(path).await?;

        // Basic HTML tag removal
        let mut text = String::new();
        let mut in_tag = false;

        for ch in content.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    text.push(' ');
                }
                _ if !in_tag => text.push(ch),
                _ => {}
            }
        }

        // Clean up excessive whitespace
        let text = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        Ok(text)
    }

    /// Extract text from JSON files
    async fn extract_json(&self, path: &Path) -> Result<String> {
        debug!("Extracting JSON content from: {:?}", path);

        let content = tokio::fs::read_to_string(path).await?;

        // Parse JSON and extract all string values
        let json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| AppError::ProcessingError {
                message: format!("Invalid JSON: {}", e),
            })?;

        let text = self.extract_json_strings(&json);
        Ok(text)
    }

    /// Recursively extract string values from JSON
    fn extract_json_strings(&self, value: &serde_json::Value) -> String {
        let mut text = String::new();

        match value {
            serde_json::Value::String(s) => {
                text.push_str(s);
                text.push(' ');
            }
            serde_json::Value::Object(map) => {
                for (key, val) in map {
                    text.push_str(key);
                    text.push_str(": ");
                    text.push_str(&self.extract_json_strings(val));
                }
            }
            serde_json::Value::Array(arr) => {
                for val in arr {
                    text.push_str(&self.extract_json_strings(val));
                }
            }
            _ => {}
        }

        text
    }

    /// Extract plain text files
    async fn extract_text(&self, path: &Path) -> Result<String> {
        debug!("Extracting plain text from: {:?}", path);

        // Check file size first
        let metadata = tokio::fs::metadata(path).await?;
        if metadata.len() > 10_000_000 {
            // For large files, only read first 10MB
            let mut file = tokio::fs::File::open(path).await?;
            use tokio::io::AsyncReadExt;
            let mut buffer = vec![0; 10_000_000];
            let n = file.read(&mut buffer).await?;
            buffer.truncate(n);
            return Ok(String::from_utf8_lossy(&buffer).to_string());
        }

        Ok(tokio::fs::read_to_string(path).await?)
    }

    /// Generate a file signature for caching
    pub async fn get_file_signature(&self, path: &Path) -> Result<String> {
        let metadata = tokio::fs::metadata(path).await?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(format!(
            "{}:{}:{}",
            path.to_string_lossy(),
            metadata.len(),
            modified
        ))
    }

    /// Enhance extracted content with LLM analysis
    async fn enhance_with_llm(&self, content: &str, path: &Path) -> Result<String> {
        if let Some(ref client) = self.ollama_client {
            match client.analyze_document_enhanced(content, path.to_str().unwrap_or("unknown"), &[]).await {
                Ok(analysis) => {
                    info!("Enhanced document analysis completed for {}", path.display());

                    // Build enhanced content with metadata
                    let mut enhanced = format!("# Enhanced Document Analysis\n\n");
                    enhanced.push_str(&format!("**Suggested Name:** {}\n", analysis.suggested_name));
                    enhanced.push_str(&format!("**Document Type:** {}\n", analysis.document_type));
                    enhanced.push_str(&format!("**Purpose:** {}\n", analysis.purpose));
                    enhanced.push_str(&format!("**Category:** {}\n", analysis.category));
                    enhanced.push_str(&format!("**Keywords:** {}\n", analysis.keywords.join(", ")));
                    enhanced.push_str(&format!("**Summary:** {}\n\n", analysis.summary));

                    if let Some(date) = analysis.date {
                        enhanced.push_str(&format!("**Date:** {}\n", date));
                    }
                    if let Some(client_name) = analysis.client {
                        enhanced.push_str(&format!("**Client:** {}\n", client_name));
                    }
                    if let Some(project) = analysis.project {
                        enhanced.push_str(&format!("**Project:** {}\n", project));
                    }

                    enhanced.push_str(&format!("\n---\n\n## Original Content\n\n{}", content));
                    Ok(enhanced)
                }
                Err(e) => {
                    warn!("Failed to perform LLM enhancement: {}", e);
                    Ok(content.to_string())
                }
            }
        } else {
            Ok(content.to_string())
        }
    }

    /// Analyze image with vision model
    pub async fn analyze_image(&self, image_path: &Path) -> Result<ImageAnalysisResult> {
        if let Some(ref client) = self.ollama_client {
            // Read image and convert to base64
            let image_data = tokio::fs::read(image_path).await?;
            let base64_image = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);

            match client.analyze_image_enhanced(&base64_image, &[]).await {
                Ok(analysis) => Ok(ImageAnalysisResult {
                    suggested_name: analysis.suggested_name,
                    description: analysis.description,
                    category: analysis.category,
                    detected_text: analysis.document_text,
                    metadata: HashMap::from([
                        ("main_subject".to_string(), analysis.main_subject),
                        ("image_type".to_string(), analysis.image_type),
                    ]),
                }),
                Err(e) => Err(e),
            }
        } else {
            Err(AppError::ProcessingError {
                message: "LLM client not available for image analysis".to_string(),
            })
        }
    }
}

#[derive(Debug)]
pub struct ImageAnalysisResult {
    pub suggested_name: String,
    pub description: String,
    pub category: String,
    pub detected_text: String,
    pub metadata: HashMap<String, String>,
}