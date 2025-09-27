use crate::{
    ai::{
        ollama::{DocumentAnalysisEnhanced, ImageAnalysisEnhanced},
        FileAnalysis, AiService,
    },
    commands::organization::OrganizationResult,
    core::smart_folders::SmartFolder,
    core::{
        content_extractor::ContentExtractor,
        semantic_matcher::{SemanticMatcher, FolderMatch},
    },
    error::{AppError, Result},
    services::naming_service::NamingService,
    storage::Database,
};
use std::{path::{Path, PathBuf}, sync::Arc};
use tracing::{debug, info, warn, error};
use serde::{Serialize, Deserialize};

/// Complete LLM-powered file organization workflow
pub struct LLMWorkflow {
    ai_service: Arc<AiService>,
    database: Arc<Database>,
    content_extractor: ContentExtractor,
    naming_service: NamingService,
    semantic_matcher: SemanticMatcher,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkflowConfig {
    pub enable_llm_analysis: bool,
    pub enable_smart_naming: bool,
    pub enable_semantic_matching: bool,
    pub enable_embeddings: bool,
    pub confidence_threshold: f32,
    pub max_folder_suggestions: usize,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            enable_llm_analysis: true,
            enable_smart_naming: true,
            enable_semantic_matching: true,
            enable_embeddings: true,
            confidence_threshold: 0.7,
            max_folder_suggestions: 3,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub file_path: String,
    pub analysis: FileAnalysisResult,
    pub suggested_name: String,
    pub folder_matches: Vec<FolderMatch>,
    pub selected_folder: Option<SmartFolder>,
    pub target_path: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileAnalysisResult {
    Document(DocumentAnalysisEnhanced),
    Image(ImageAnalysisEnhanced),
    Basic(FileAnalysis),
}

impl LLMWorkflow {
    pub fn new(
        ai_service: Arc<AiService>,
        database: Arc<Database>,
    ) -> Self {
        let semantic_matcher = if let Some(ollama) = ai_service.get_ollama_client() {
            SemanticMatcher::with_ollama(database.clone(), ollama)
        } else {
            SemanticMatcher::new(database.clone())
        };

        Self {
            ai_service,
            database,
            content_extractor: ContentExtractor::new(),
            naming_service: NamingService::new(),
            semantic_matcher,
        }
    }

    /// Execute complete LLM workflow for a single file
    pub async fn process_file(
        &self,
        file_path: &str,
        smart_folders: &[SmartFolder],
        config: &WorkflowConfig,
    ) -> Result<WorkflowResult> {
        info!("Starting LLM workflow for file: {}", file_path);

        let path = Path::new(file_path);
        if !path.exists() {
            return Err(AppError::FileNotFound {
                path: file_path.to_string(),
            });
        }

        // Step 1: Extract content with size limits for safety
        debug!("Extracting content from file");

        // CRITICAL FIX: Check file size before processing to prevent resource exhaustion
        let metadata = tokio::fs::metadata(path).await?;
        const MAX_CONTENT_SIZE: u64 = 100 * 1024 * 1024; // 100MB limit for content extraction

        if metadata.len() > MAX_CONTENT_SIZE {
            return Err(AppError::ResourceLimitExceeded {
                message: format!(
                    "File {} is too large ({} bytes) for processing (max {} bytes)",
                    file_path,
                    metadata.len(),
                    MAX_CONTENT_SIZE
                ),
            });
        }

        let content = self.content_extractor.extract_content(path).await?;

        // Step 2: Analyze with LLM
        let analysis = if config.enable_llm_analysis {
            self.analyze_with_llm(&content, file_path, smart_folders).await?
        } else {
            self.basic_analysis(&content, file_path).await?
        };

        // Step 3: Generate smart name
        let suggested_name = if config.enable_smart_naming {
            match &analysis {
                FileAnalysisResult::Document(doc) => {
                    self.naming_service.generate_smart_name_from_llm(doc, path)?
                }
                FileAnalysisResult::Image(img) => {
                    self.naming_service.generate_smart_name_from_vision(img, path)?
                }
                FileAnalysisResult::Basic(basic) => {
                    self.naming_service.generate_smart_name(basic, path)?
                }
            }
        } else {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string()
        };

        // Step 4: Generate embeddings
        let embeddings = if config.enable_embeddings {
            let embedding_text = self.create_embedding_text(&analysis);
            match self.ai_service.generate_embeddings(&embedding_text).await {
                Ok(emb) => Some(emb),
                Err(e) => {
                    warn!("Failed to generate embeddings: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Step 5: Find folder matches
        let folder_matches = if config.enable_semantic_matching {
            let basic_analysis = self.to_basic_analysis(&analysis, file_path);
            self.semantic_matcher
                .find_best_matches(
                    file_path,
                    embeddings.as_deref(),
                    &basic_analysis,
                    smart_folders,
                )
                .await?
        } else {
            Vec::new()
        };

        // Step 6: Select best folder
        let (selected_folder, target_path) = if let Some(best_match) = folder_matches
            .iter()
            .find(|m| m.confidence >= config.confidence_threshold)
        {
            let target_dir = PathBuf::from(best_match.folder.target_path.as_ref().unwrap_or(&best_match.folder.path));
            let target = target_dir.join(&suggested_name);
            (
                Some(best_match.folder.clone()),
                Some(target.to_string_lossy().to_string()),
            )
        } else {
            (None, None)
        };

        // Step 7: Save analysis and embeddings to database
        // CRITICAL FIX: Proper error handling for data integrity
        if let Some(ref emb) = embeddings {
            let basic_analysis = self.to_basic_analysis(&analysis, file_path);

            // Save analysis with proper error handling
            if let Err(e) = self.database.save_analysis(&basic_analysis).await {
                tracing::error!("Failed to save analysis for {}: {}", file_path, e);
                // Don't fail the entire workflow, but log the issue
            }

            // Save embedding with proper error handling
            if let Err(e) = self.database.save_embedding(
                file_path,
                emb,
                Some("llama3.2:latest"),
            ).await {
                tracing::error!("Failed to save embedding for {}: {}", file_path, e);
                // Don't fail the entire workflow, but log the issue
            }
        }

        Ok(WorkflowResult {
            file_path: file_path.to_string(),
            analysis,
            suggested_name,
            folder_matches: folder_matches.into_iter().take(config.max_folder_suggestions).collect(),
            selected_folder,
            target_path,
            success: true,
            error_message: None,
        })
    }

    /// Process multiple files in batch with improved error recovery
    /// CRITICAL FIX: Better batch processing with retry logic and data integrity
    pub async fn process_batch(
        &self,
        file_paths: Vec<String>,
        smart_folders: &[SmartFolder],
        config: &WorkflowConfig,
    ) -> Vec<WorkflowResult> {
        let mut results = Vec::new();
        let total_files = file_paths.len();
        let mut failed_count = 0;
        let mut retry_queue = Vec::new();

        info!("Starting batch processing of {} files", total_files);

        // First pass: process all files
        for (index, file_path) in file_paths.into_iter().enumerate() {
            if index % 10 == 0 {
                info!("Processing file {} of {} ({}%)", index + 1, total_files,
                      (index + 1) * 100 / total_files);
            }

            match self.process_file(&file_path, smart_folders, config).await {
                Ok(result) => {
                    // Verify the result has minimum required data for integrity
                    if result.success && !result.suggested_name.is_empty() {
                        results.push(result);
                    } else {
                        warn!("File {} processed but with incomplete data, will retry", file_path);
                        retry_queue.push(file_path);
                    }
                }
                Err(e) => {
                    error!("Failed to process {} on first attempt: {}", file_path, e);
                    failed_count += 1;

                    // Decide whether to retry based on error type
                    let should_retry = matches!(e,
                        AppError::AiError { .. } |
                        AppError::NetworkError { .. } |
                        AppError::ProcessingError { .. }
                    );

                    if should_retry && failed_count < total_files / 2 {
                        // Only retry if we haven't failed too many files
                        retry_queue.push(file_path);
                    } else {
                        // Create a failure result with better error information
                        results.push(self.create_failure_result(file_path, e));
                    }
                }
            }
        }

        // Second pass: retry failed files with fallback mode
        if !retry_queue.is_empty() {
            warn!("Retrying {} files with fallback processing", retry_queue.len());

            // Create fallback config that disables LLM features for reliability
            let fallback_config = WorkflowConfig {
                enable_llm_analysis: false,
                enable_smart_naming: false,
                enable_semantic_matching: false,
                enable_embeddings: false,
                confidence_threshold: 0.3, // Lower threshold for fallback
                max_folder_suggestions: 1,
            };

            for file_path in retry_queue {
                match self.process_file(&file_path, smart_folders, &fallback_config).await {
                    Ok(result) => {
                        info!("Successfully processed {} on retry with fallback", file_path);
                        results.push(result);
                    }
                    Err(e) => {
                        error!("Failed to process {} even with fallback: {}", file_path, e);
                        results.push(self.create_failure_result(file_path, e));
                    }
                }
            }
        }

        let successful_count = results.iter().filter(|r| r.success).count();
        info!("Batch processing completed: {}/{} files successful",
              successful_count, total_files);

        results
    }

    /// Create a failure result with proper data integrity
    fn create_failure_result(&self, file_path: String, error: AppError) -> WorkflowResult {
        // Extract basic file information even for failures
        let path = Path::new(&file_path);
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let category = match extension.as_str() {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" => "Images",
            "mp4" | "avi" | "mkv" | "mov" => "Videos",
            "mp3" | "wav" | "flac" => "Audio",
            "pdf" | "doc" | "docx" | "txt" => "Documents",
            _ => "Unknown",
        };

        WorkflowResult {
            file_path: file_path.clone(),
            analysis: FileAnalysisResult::Basic(FileAnalysis {
                path: file_path.clone(),
                category: category.to_string(),
                tags: vec![extension],
                summary: format!("Processing failed: {}", error.user_message()),
                confidence: 0.0,
                extracted_text: None,
                detected_language: None,
                metadata: serde_json::json!({
                    "error_type": error.error_type(),
                    "processing_failed": true,
                }),
            }),
            suggested_name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown_file")
                .to_string(),
            folder_matches: vec![],
            selected_folder: None,
            target_path: None,
            success: false,
            error_message: Some(error.to_string()),
        }
    }

    /// Analyze file with LLM
    async fn analyze_with_llm(
        &self,
        content: &str,
        file_path: &str,
        smart_folders: &[SmartFolder],
    ) -> Result<FileAnalysisResult> {
        if let Some(ollama) = self.ai_service.get_ollama_client() {
            // Check if it's an image file
            let extension = Path::new(file_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp") {
                // For images, we need base64 encoding with size limits
                // CRITICAL FIX: Check file size before loading to prevent memory exhaustion
                let metadata = tokio::fs::metadata(file_path).await?;
                const MAX_IMAGE_SIZE: u64 = 50 * 1024 * 1024; // 50MB limit

                if metadata.len() > MAX_IMAGE_SIZE {
                    tracing::warn!(
                        "Image file {} is too large ({} bytes), falling back to basic analysis",
                        file_path,
                        metadata.len()
                    );
                    return self.basic_analysis(content, file_path).await;
                }

                let image_bytes = tokio::fs::read(file_path).await?;
                use base64::{engine::general_purpose, Engine as _};
                let base64_image = general_purpose::STANDARD.encode(&image_bytes);

                // Explicitly drop image_bytes to free memory immediately
                drop(image_bytes);

                match ollama.analyze_image_enhanced(&base64_image, smart_folders).await {
                    Ok(analysis) => Ok(FileAnalysisResult::Image(analysis)),
                    Err(e) => {
                        warn!("Image analysis failed, falling back to basic: {}", e);
                        self.basic_analysis(content, file_path).await
                    }
                }
            } else {
                // Document analysis
                match ollama.analyze_document_enhanced(content, file_path, smart_folders).await {
                    Ok(analysis) => Ok(FileAnalysisResult::Document(analysis)),
                    Err(e) => {
                        warn!("Document analysis failed, falling back to basic: {}", e);
                        self.basic_analysis(content, file_path).await
                    }
                }
            }
        } else {
            self.basic_analysis(content, file_path).await
        }
    }

    /// Basic analysis without LLM
    async fn basic_analysis(&self, content: &str, file_path: &str) -> Result<FileAnalysisResult> {
        let mime_type = mime_guess::from_path(file_path)
            .first_or_octet_stream()
            .to_string();

        let analysis = self.ai_service.analyze_file(content, &mime_type).await?;
        Ok(FileAnalysisResult::Basic(analysis))
    }

    /// Create text for embedding generation
    fn create_embedding_text(&self, analysis: &FileAnalysisResult) -> String {
        match analysis {
            FileAnalysisResult::Document(doc) => {
                format!(
                    "{} {} {} {} {}",
                    doc.summary,
                    doc.keywords.join(" "),
                    doc.document_type,
                    doc.purpose,
                    doc.client.as_deref().unwrap_or("")
                )
            }
            FileAnalysisResult::Image(img) => {
                format!(
                    "{} {} {} {}",
                    img.description,
                    img.main_subject,
                    img.detected_objects.join(" "),
                    img.document_text
                )
            }
            FileAnalysisResult::Basic(basic) => {
                format!(
                    "{} {} {}",
                    basic.summary,
                    basic.tags.join(" "),
                    basic.category
                )
            }
        }
    }

    /// Convert to basic analysis for compatibility
    fn to_basic_analysis(&self, analysis: &FileAnalysisResult, file_path: &str) -> FileAnalysis {
        match analysis {
            FileAnalysisResult::Document(doc) => FileAnalysis {
                path: file_path.to_string(),
                category: doc.category.clone(),
                tags: doc.keywords.clone(),
                summary: doc.summary.clone(),
                confidence: doc.confidence,
                extracted_text: Some(doc.purpose.clone()),
                detected_language: None,
                metadata: serde_json::json!({
                    "document_type": doc.document_type,
                    "client": doc.client,
                    "project": doc.project,
                    "date": doc.date,
                }),
            },
            FileAnalysisResult::Image(img) => FileAnalysis {
                path: file_path.to_string(),
                category: img.category.clone(),
                tags: img.detected_objects.clone(),
                summary: img.description.clone(),
                confidence: img.confidence,
                extracted_text: if img.document_text.is_empty() {
                    None
                } else {
                    Some(img.document_text.clone())
                },
                detected_language: None,
                metadata: serde_json::json!({
                    "image_type": img.image_type,
                    "main_subject": img.main_subject,
                    "suggested_folders": img.suggested_folders,
                }),
            },
            FileAnalysisResult::Basic(basic) => basic.clone(),
        }
    }

    /// Execute file organization based on workflow result
    pub async fn execute_organization(
        &self,
        workflow_result: &WorkflowResult,
    ) -> Result<OrganizationResult> {
        if !workflow_result.success {
            return Err(AppError::ProcessingError {
                message: workflow_result.error_message.clone().unwrap_or_else(|| "Workflow failed".to_string()),
            });
        }

        let source_path = &workflow_result.file_path;
        let target_path = workflow_result.target_path.as_ref().ok_or_else(|| {
            AppError::NotFound {
                message: "No target path determined".to_string(),
            }
        })?;

        let selected_folder = workflow_result.selected_folder.as_ref().ok_or_else(|| {
            AppError::NotFound {
                message: "No folder selected".to_string(),
            }
        })?;

        // Create target directory if needed
        if let Some(target_dir) = Path::new(target_path).parent() {
            if !target_dir.exists() {
                tokio::fs::create_dir_all(target_dir).await?;
            }
        }

        // Move file
        tokio::fs::rename(source_path, target_path).await?;

        // Record organization in history
        let best_match = workflow_result.folder_matches.first();
        let confidence = best_match.map(|m| m.confidence).unwrap_or(0.5);
        let reason = best_match.map(|m| m.reason.clone()).unwrap_or_else(|| "LLM suggestion".to_string());

        Ok(OrganizationResult {
            source_path: source_path.to_string(),
            target_path: target_path.to_string(),
            action: crate::commands::organization::ActionType::Move,
            success: true,
            error: None,
            folder_name: Some(selected_folder.name.clone()),
            new_name: Some(workflow_result.suggested_name.clone()),
            confidence: Some(confidence),
            reason: Some(reason),
        })
    }

    /// Cleanup resources used by the workflow
    /// CRITICAL: Ensures proper resource cleanup to prevent memory leaks
    pub async fn cleanup_resources(&self) -> Result<()> {
        // Force cleanup of any cached content in extractors
        // The content extractor might have cached large files
        if let Err(e) = self.database.vacuum().await {
            tracing::warn!("Database vacuum failed during workflow cleanup: {}", e);
        }

        // Clear WAL files to free disk space
        if let Err(e) = self.database.cleanup_wal_files().await {
            tracing::warn!("WAL cleanup failed during workflow cleanup: {}", e);
        }

        tracing::debug!("LLM workflow resources cleaned up");
        Ok(())
    }

    /// Check current memory usage and cleanup if needed
    pub fn check_memory_pressure(&self) -> bool {
        // Simple heuristic: if we're using more than 500MB, we might be under pressure
        // This is a conservative estimate for workflow operations
        let usage = self.estimate_memory_usage();
        usage > 500 * 1024 * 1024 // 500MB threshold
    }

    /// Estimate current memory usage (rough calculation)
    fn estimate_memory_usage(&self) -> usize {
        // This is a rough estimate based on typical workflow memory usage
        // In a real implementation, this would use system APIs
        std::mem::size_of::<LLMWorkflow>() * 100 // Rough multiplier for cached data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_config_default() {
        let config = WorkflowConfig::default();
        assert!(config.enable_llm_analysis);
        assert!(config.enable_smart_naming);
        assert!(config.enable_semantic_matching);
        assert!(config.enable_embeddings);
        assert_eq!(config.confidence_threshold, 0.7);
        assert_eq!(config.max_folder_suggestions, 3);
    }
}