use crate::ai::{
    connection::{check_ollama_health, ConnectionPool},
    AiEngine, FileAnalysis, OrganizationSuggestion,
};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use ollama_rs::{
    generation::completion::request::GenerationRequest,
    generation::embeddings::request::GenerateEmbeddingsRequest, Ollama,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

/// Sanitizes input content for AI prompts to prevent injection attacks
/// Uses balanced filtering to protect against prompt injection while preserving legitimate content
pub(crate) fn sanitize_prompt_content(input: &str) -> Result<String> {
    // Check input length and return error if too large
    if input.len() > 2000 {
        return Err(AppError::InvalidInput {
            message: format!(
                "Input exceeds 2000 characters (got {}). Please truncate the input.",
                input.len()
            ),
        });
    }

    let mut result = input.to_string();

    // Remove null bytes and normalize line endings
    result = result
        .replace('\0', "")
        .replace(['\r'], "")
        .replace('\t', " ");

    // Remove potential prompt injection sequences (case-insensitive)
    // More targeted approach - only block clear injection attempts
    let injection_patterns = [
        // Direct instruction overrides
        ("ignore all previous instructions", "[FILTERED]"),
        ("ignore previous instructions", "[FILTERED]"),
        ("disregard all previous", "[FILTERED]"),
        ("forget everything above", "[FILTERED]"),
        ("start over with", "[FILTERED]"),
        ("reset your instructions", "[FILTERED]"),
        ("new instructions:", "[FILTERED]"),
        ("override your instructions", "[FILTERED]"),
        ("instead of following", "[FILTERED]"),
        // Role hijacking attempts - be more specific
        ("you are now a", "[FILTERED]"),
        ("act as a different", "[FILTERED]"),
        ("pretend to be", "[FILTERED]"),
        ("roleplay as", "[FILTERED]"),
        ("from now on you are", "[FILTERED]"),
        // System prompt attempts - only block obvious ones
        ("system:", "[FILTERED]"),
        ("assistant:", "[FILTERED]"),
        ("user:", "[FILTERED]"),
        ("human:", "[FILTERED]"),
        ("ai:", "[FILTERED]"),
        ("<|system|>", "[FILTERED]"),
        ("<|assistant|>", "[FILTERED]"),
        ("<|user|>", "[FILTERED]"),
        // Jailbreaking attempts
        ("jailbreak mode", "[FILTERED]"),
        ("break free from", "[FILTERED]"),
        ("bypass your safety", "[FILTERED]"),
        ("ignore safety guidelines", "[FILTERED]"),
        ("disable your filter", "[FILTERED]"),
        ("remove restrictions", "[FILTERED]"),
        ("unrestricted mode", "[FILTERED]"),
        ("developer mode", "[FILTERED]"),
        // Direct code execution attempts
        ("exec(", "[FILTERED]"),
        ("eval(", "[FILTERED]"),
        ("system(", "[FILTERED]"),
        ("<script>", "[FILTERED]"),
        ("javascript:", "[FILTERED]"),
        ("data:text/html", "[FILTERED]"),
        ("file://", "[FILTERED]"),
        // Common manipulation patterns
        ("please ignore everything", "[FILTERED]"),
        ("don't follow the", "[FILTERED]"),
        ("cancel all previous", "[FILTERED]"),
        ("void the previous", "[FILTERED]"),
        ("disregard your training", "[FILTERED]"),
    ];

    // Apply pattern replacements (case-insensitive)
    for (pattern, replacement) in injection_patterns {
        result = replace_case_insensitive(&result, pattern, replacement);
    }

    // More permissive character filtering - allow more legitimate characters
    result = result
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || c.is_whitespace()
                || ".,!?:;()[]{}\"'-_/\\@#$%&*+=<>~`^|".contains(*c)
                || c.is_ascii_punctuation()
        })
        .collect();

    // Remove excessive newlines but allow some formatting
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    // Final length check after sanitization - should not be needed since we checked at start
    if result.len() > 1800 {
        return Err(AppError::InvalidInput {
            message: "Input too large after sanitization".to_string(),
        });
    }

    Ok(result)
}

/// Extracts the first complete JSON object from a noisy LLM response.
///
/// Local vision/text models routinely wrap their JSON in ```` ```json ... ``` ````
/// fences or sandwich it between explanatory prose. Strict `serde_json::from_str`
/// fails on either. This walks the string, finds the first `{`, then scans
/// forward tracking brace depth (and skipping over string literals) until it
/// finds the matching `}`. Returns the substring if found.
pub(crate) fn extract_json_object(text: &str) -> Option<&str> {
    extract_balanced(text, b'{', b'}')
}

/// Same as `extract_json_object` but for JSON arrays. Used for endpoints that
/// return a `[ ... ]` list (e.g. organization suggestions).
pub(crate) fn extract_json_array(text: &str) -> Option<&str> {
    extract_balanced(text, b'[', b']')
}

fn extract_balanced(text: &str, open: u8, close: u8) -> Option<&str> {
    let bytes = text.as_bytes();
    let start = bytes.iter().position(|&b| b == open)?;

    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, &b) in bytes[start..].iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        if b == b'"' {
            in_string = true;
        } else if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(&text[start..=start + i]);
            }
        }
    }
    None
}

/// Tolerantly parse a vision model's response into the fields the dispatcher
/// expects. Vision models (especially llava variants on local hardware) routinely:
///   * wrap output in ```` ```json ... ``` ```` fences,
///   * prefix the JSON with "Here is the analysis:" or similar prose,
///   * emit only a subset of fields, or
///   * return free-form prose with no JSON at all when confused.
///
/// Strict `serde_json::from_str` fails on every one of those and used to take
/// down the whole vision pipeline. This helper:
///   1. Tries to extract the first balanced JSON object via `extract_json_object`.
///   2. Deserializes it into `VisionAnalysisResponse` (whose fields are
///      `#[serde(default)]`, so missing keys don't kill the parse).
///   3. If steps 1 or 2 fail, synthesizes a low-confidence "Images" result
///      using the raw response as a summary so the file is still recorded.
///   4. Clamps `confidence` to [0.0, 1.0] and forces a non-empty `category`.
///
/// Returns `(category, confidence, VisionAnalysisResponse)` — `category` and
/// `confidence` are pre-normalized; the rest is in the struct.
pub(crate) fn parse_vision_response(
    response: &str,
) -> (String, f32, VisionAnalysisResponse) {
    let analysis = extract_json_object(response)
        .and_then(|json| serde_json::from_str::<VisionAnalysisResponse>(json).ok())
        .unwrap_or_else(|| {
            warn!(
                "Vision response was not valid JSON, synthesizing fallback. Response: {}",
                response.chars().take(500).collect::<String>()
            );
            VisionAnalysisResponse {
                category: "Images".to_string(),
                tags: vec!["image".to_string()],
                summary: response.chars().take(500).collect(),
                confidence: 0.4,
                ..Default::default()
            }
        });

    let confidence = analysis.confidence.clamp(0.0, 1.0);
    let category = if analysis.category.is_empty() {
        "Images".to_string()
    } else {
        analysis.category.clone()
    };

    (category, confidence, analysis)
}

/// Case-insensitive string replacement
fn replace_case_insensitive(text: &str, pattern: &str, replacement: &str) -> String {
    let pattern_lower = pattern.to_lowercase();
    let text_lower = text.to_lowercase();

    let mut result = String::new();
    let mut last_end = 0;

    while let Some(start) = text_lower[last_end..].find(&pattern_lower) {
        let absolute_start = last_end + start;
        let absolute_end = absolute_start + pattern.len();

        result.push_str(&text[last_end..absolute_start]);
        result.push_str(replacement);
        last_end = absolute_end;
    }

    result.push_str(&text[last_end..]);
    result
}

pub struct OllamaClient {
    client: Ollama,
    text_model: String,
    vision_model: String,
    embedding_model: String,
    max_retries: u32,
    initial_retry_delay: Duration,
    connection_health: Arc<RwLock<ConnectionHealth>>,
    connection_pool: ConnectionPool,
}

#[derive(Debug, Clone)]
struct ConnectionHealth {
    is_healthy: bool,
    last_check: std::time::Instant,
    consecutive_failures: u32,
    available_models: Vec<String>,
}

impl Default for ConnectionHealth {
    fn default() -> Self {
        Self {
            is_healthy: false,
            last_check: std::time::Instant::now(),
            consecutive_failures: 0,
            available_models: Vec::new(),
        }
    }
}

/// Default model names used when the caller does not provide one
pub const DEFAULT_TEXT_MODEL: &str = "llama3.2:3b";
pub const DEFAULT_VISION_MODEL: &str = "llava:7b";
pub const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text";

impl OllamaClient {
    /// Creates a new OllamaClient with default model names.
    /// Prefer `new_with_models` so user-configured models are honored.
    pub async fn new(host: &str) -> Result<Self> {
        Self::new_with_models(
            host,
            DEFAULT_TEXT_MODEL,
            DEFAULT_VISION_MODEL,
            DEFAULT_EMBEDDING_MODEL,
        )
        .await
    }

    /// Creates a new OllamaClient with explicit model names.
    /// Empty model names fall back to the defaults so a partially-populated
    /// `Config` cannot silently break inference.
    pub async fn new_with_models(
        host: &str,
        text_model: &str,
        vision_model: &str,
        embedding_model: &str,
    ) -> Result<Self> {
        // Validate input
        if host.is_empty() {
            return Err(AppError::InvalidInput {
                message: "Ollama host cannot be empty".to_string(),
            });
        }

        // Parse and validate the host URL
        let parsed_host = if host.starts_with("http://") || host.starts_with("https://") {
            host.to_string()
        } else {
            format!("http://{}", host)
        };

        // Validate URL format
        let url = match url::Url::parse(&parsed_host) {
            Ok(url) => url,
            Err(e) => {
                return Err(AppError::InvalidInput {
                    message: format!("Invalid Ollama host URL '{}': {}", host, e),
                });
            }
        };

        let hostname = match url.host_str() {
            Some(h) if !h.is_empty() => h,
            _ => {
                return Err(AppError::InvalidInput {
                    message: format!("Invalid hostname in URL: {}", parsed_host),
                });
            }
        };
        let port = url.port().unwrap_or(11434);

        // Check if Ollama server is running before creating client
        debug!("Checking Ollama availability at {}:{}", hostname, port);

        let is_reachable = check_ollama_health(hostname, port).await?;

        if !is_reachable {
            warn!("Ollama server is not reachable at {}:{}", hostname, port);
            return Err(AppError::AiError {
                message: format!("Ollama server is not running or unreachable at {}:{}. Please ensure Ollama is installed and running.", hostname, port),
            });
        }

        info!("Ollama server is reachable at {}:{}", hostname, port);

        // Create Ollama client - simplified approach
        let client = Ollama::new(hostname.to_string(), port);

        // Validate the client by making a test request
        let test_result = timeout(Duration::from_secs(3), client.list_local_models()).await;

        match test_result {
            Ok(Ok(models)) => {
                info!(
                    "Successfully connected to Ollama. Found {} models",
                    models.len()
                );
            }
            Ok(Err(e)) => {
                error!("Ollama client created but API call failed: {}", e);
                return Err(AppError::AiError {
                    message: format!(
                        "Ollama is running but API is not responding correctly: {}",
                        e
                    ),
                });
            }
            Err(_) => {
                warn!("Ollama health check timed out");
                return Err(AppError::AiError {
                    message: "Ollama server timed out. It may be starting up or under heavy load."
                        .to_string(),
                });
            }
        }

        let resolved_text_model = if text_model.is_empty() {
            DEFAULT_TEXT_MODEL.to_string()
        } else {
            text_model.to_string()
        };
        let resolved_vision_model = if vision_model.is_empty() {
            DEFAULT_VISION_MODEL.to_string()
        } else {
            vision_model.to_string()
        };
        let resolved_embedding_model = if embedding_model.is_empty() {
            DEFAULT_EMBEDDING_MODEL.to_string()
        } else {
            embedding_model.to_string()
        };

        info!(
            "OllamaClient models: text={} vision={} embedding={}",
            resolved_text_model, resolved_vision_model, resolved_embedding_model
        );

        let ollama_client = Self {
            client,
            text_model: resolved_text_model,
            vision_model: resolved_vision_model,
            embedding_model: resolved_embedding_model,
            max_retries: 3,
            initial_retry_delay: Duration::from_millis(100),
            connection_health: Arc::new(RwLock::new(ConnectionHealth {
                is_healthy: true,
                last_check: std::time::Instant::now(),
                consecutive_failures: 0,
                available_models: Vec::new(),
            })),
            connection_pool: ConnectionPool::new(5), // Allow 5 concurrent connections
        };

        // Perform initial health check and model discovery
        let _ = ollama_client.check_and_update_health().await;

        Ok(ollama_client)
    }

    pub async fn health_check(&self) -> Result<()> {
        self.check_and_update_health().await
    }

    /// Check connection health and update internal state
    async fn check_and_update_health(&self) -> Result<()> {
        let mut health = self.connection_health.write().await;

        // Rate limit health checks
        if health.last_check.elapsed() < Duration::from_secs(5) && health.is_healthy {
            return Ok(());
        }

        match timeout(Duration::from_secs(3), self.client.list_local_models()).await {
            Ok(Ok(models)) => {
                health.is_healthy = true;
                health.consecutive_failures = 0;
                health.last_check = std::time::Instant::now();
                health.available_models = models.iter().map(|m| m.name.clone()).collect();

                debug!(
                    "Ollama is healthy with {} models available",
                    health.available_models.len()
                );
                Ok(())
            }
            Ok(Err(e)) => {
                health.is_healthy = false;
                health.consecutive_failures += 1;
                health.last_check = std::time::Instant::now();

                error!(
                    "Ollama error (failures: {}): {}",
                    health.consecutive_failures, e
                );
                Err(AppError::AiError {
                    message: format!("Ollama is not responding: {}", e),
                })
            }
            Err(_) => {
                health.is_healthy = false;
                health.consecutive_failures += 1;
                health.last_check = std::time::Instant::now();

                warn!(
                    "Ollama health check timed out (failures: {})",
                    health.consecutive_failures
                );
                Err(AppError::AiError {
                    message: "Ollama connection timed out".to_string(),
                })
            }
        }
    }

    /// Execute a request with retry logic and exponential backoff
    async fn execute_with_retry<F, T>(&self, operation_name: &str, mut operation: F) -> Result<T>
    where
        F: FnMut() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>,
    {
        let mut retry_count = 0;
        let mut delay = self.initial_retry_delay;

        loop {
            // Acquire connection permit from pool
            let permit = match self.connection_pool.acquire().await {
                Ok(permit) => permit,
                Err(e) => {
                    // Circuit breaker is open or pool is exhausted
                    if retry_count < self.max_retries {
                        retry_count += 1;
                        warn!(
                            "{} connection pool error (attempt {}/{}): {}",
                            operation_name, retry_count, self.max_retries, e
                        );
                        sleep(delay).await;
                        continue;
                    }
                    return Err(e);
                }
            };

            // Check health before attempting operation
            if retry_count == 0 {
                let health = self.connection_health.read().await;
                if !health.is_healthy && health.last_check.elapsed() > Duration::from_secs(30) {
                    drop(health);
                    let _ = self.check_and_update_health().await;
                }
            }

            match operation().await {
                Ok(result) => {
                    // Mark request as successful
                    permit.success().await;

                    // Reset consecutive failures on success
                    let mut health = self.connection_health.write().await;
                    health.consecutive_failures = 0;
                    health.is_healthy = true;
                    return Ok(result);
                }
                Err(e) if retry_count < self.max_retries => {
                    // Mark request as failed
                    permit.failure().await;

                    retry_count += 1;
                    warn!(
                        "{} failed (attempt {}/{}): {}. Retrying in {:?}",
                        operation_name, retry_count, self.max_retries, e, delay
                    );

                    sleep(delay).await;

                    // Exponential backoff with simple jitter (no rand dependency needed)
                    let jitter_ms = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        % 100) as u64;
                    delay = std::cmp::min(
                        delay * 2 + Duration::from_millis(jitter_ms),
                        Duration::from_secs(5),
                    );
                }
                Err(e) => {
                    // Mark request as failed
                    permit.failure().await;

                    error!(
                        "{} failed after {} retries: {}",
                        operation_name, self.max_retries, e
                    );

                    // Update health status
                    let mut health = self.connection_health.write().await;
                    health.consecutive_failures += 1;
                    health.is_healthy = false;

                    return Err(e);
                }
            }
        }
    }

    /// Verify if a specific model is available
    pub async fn is_model_available(&self, model_name: &str) -> bool {
        let health = self.connection_health.read().await;
        health
            .available_models
            .iter()
            .any(|m| m.starts_with(model_name))
    }

    /// Get connection pool statistics
    pub async fn get_connection_stats(&self) -> crate::ai::connection::ConnectionPoolStats {
        self.connection_pool.get_stats().await
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        let models = self
            .client
            .list_local_models()
            .await
            .map_err(|e| AppError::AiError {
                message: format!("Failed to list models: {}", e),
            })?;

        Ok(models.into_iter().map(|m| m.name).collect())
    }

    pub async fn list_models_detailed(&self) -> Result<Vec<ModelInfo>> {
        let models = self
            .client
            .list_local_models()
            .await
            .map_err(|e| AppError::AiError {
                message: format!("Failed to list models: {}", e),
            })?;

        Ok(models
            .into_iter()
            .map(|m| ModelInfo {
                name: m.name,
                size: m.size,
                modified_at: m.modified_at,
            })
            .collect())
    }

    pub async fn pull_model(&self, model_name: &str) -> Result<()> {
        info!("Pulling model: {}", model_name);

        // Model pulling can take a long time, use extended timeout
        timeout(
            Duration::from_secs(300), // 5 minute timeout for model pulling
            self.client.pull_model(model_name.to_string(), false),
        )
        .await
        .map_err(|_| AppError::AiError {
            message: format!("Model pull timed out after 5 minutes: {}", model_name),
        })?
        .map_err(|e| AppError::AiError {
            message: format!("Failed to pull model {}: {}", model_name, e),
        })?;

        info!("Successfully pulled model: {}", model_name);
        Ok(())
    }

    pub async fn generate_embeddings(&self, text: &str) -> Result<Vec<f32>> {
        // Limit input size to prevent memory issues
        if text.len() > 8192 {
            return Err(AppError::InvalidInput {
                message: "Text too large for embedding generation (max 8KB)".to_string(),
            });
        }

        let text_clone = text.to_string();
        let model = self.embedding_model.clone();
        let client = self.client.clone();

        self.execute_with_retry("generate_embeddings", move || {
            let request = GenerateEmbeddingsRequest::new(model.clone(), text_clone.clone().into());
            let client = client.clone();

            Box::pin(async move {
                let response =
                    timeout(Duration::from_secs(30), client.generate_embeddings(request))
                        .await
                        .map_err(|_| AppError::AiError {
                            message: "Embedding generation timed out after 30 seconds".to_string(),
                        })?
                        .map_err(|e| AppError::AiError {
                            message: format!("Failed to generate embeddings: {}", e),
                        })?;

                response
                    .embeddings
                    .into_iter()
                    .next()
                    .ok_or_else(|| AppError::AiError {
                        message: "Ollama returned empty embeddings response".to_string(),
                    })
            })
        })
        .await
    }

    async fn generate_completion(&self, prompt: &str, model: &str) -> Result<String> {
        let prompt_clone = prompt.to_string();
        let model_clone = model.to_string();
        let client = self.client.clone();

        self.execute_with_retry("generate_completion", move || {
            let request = GenerationRequest::new(model_clone.clone(), prompt_clone.clone());
            let client = client.clone();

            Box::pin(async move {
                let response = timeout(Duration::from_secs(30), client.generate(request))
                    .await
                    .map_err(|_| AppError::AiError {
                        message: "Generation timed out".to_string(),
                    })?
                    .map_err(|e| AppError::AiError {
                        message: format!("Generation failed: {}", e),
                    })?;

                Ok(response.response)
            })
        })
        .await
    }

    /// Analyzes file content using Ollama's text model
    pub async fn analyze_file(&self, content: &str, file_type: &str) -> Result<FileAnalysis> {
        // Create a comprehensive analysis prompt
        let prompt = format!(
            r#"You are a file organization assistant. Analyze this file content thoroughly and provide a structured JSON response.

IMPORTANT: Base your analysis on the actual file content, not just the file type.

Categories to choose from:
- Documents: Text documents, PDFs, Word docs, notes, reports
- Code: Source code, scripts, configuration files, development files
- Data: Spreadsheets, CSVs, databases, JSON data files
- Presentations: PowerPoint, slides, keynotes
- Spreadsheets: Excel files, calculation sheets, financial data
- Images: Photos, graphics, screenshots, diagrams
- Videos: Movie files, recordings, clips
- Audio: Music, podcasts, recordings
- Archives: Compressed files, backups, zip files
- 3D Print Files: STL, OBJ, 3MF, GCODE, CAD files
- Other: Files that don't fit above categories

File type hint: {}
Content excerpt (first 10000 characters):
---
{}
---

Analyze the content above and respond with ONLY this JSON structure (no additional text):
{{
    "path": "",
    "category": "<one category from the list above that best matches the content>",
    "tags": ["<tag1>", "<tag2>", "<tag3>", "up to 10 relevant descriptive tags based on actual content"],
    "summary": "<detailed 1-2 sentence description of what this file contains and its purpose>",
    "confidence": <0.0 to 1.0 based on how certain you are about the categorization>,
    "metadata": {{}}
}}"#,
            file_type,
            &content.chars().take(10000).collect::<String>()
        );

        // Get completion from text model
        let response = self.generate_completion(&prompt, &self.text_model).await?;

        // Try to parse the JSON response. Strip prose/markdown fences first;
        // fall back to the raw response if no `{...}` block is found.
        let json_candidate = extract_json_object(&response).unwrap_or(response.as_str());
        match serde_json::from_str::<FileAnalysis>(json_candidate) {
            Ok(mut analysis) => {
                analysis.confidence = analysis.confidence.clamp(0.0, 1.0);
                Ok(analysis)
            }
            Err(e) => {
                // If JSON parsing fails, create a basic analysis
                warn!(
                    "Failed to parse Ollama response as JSON: {} - Response: {}",
                    e, response
                );

                // Try to extract useful information from the response anyway
                let category = if file_type.contains("text") || file_type.contains("document") {
                    "Documents"
                } else if file_type.contains("code") || file_type.contains("script") {
                    "Code"
                } else if file_type.contains("data")
                    || file_type.contains("json")
                    || file_type.contains("csv")
                {
                    "Data"
                } else {
                    "Other"
                };

                Ok(FileAnalysis {
                    path: String::new(),
                    category: category.to_string(),
                    tags: vec![file_type.to_string()],
                    summary: response.chars().take(200).collect(),
                    confidence: 0.5,
                    extracted_text: None,
                    detected_language: None,
                    metadata: serde_json::Value::Object(serde_json::Map::new()),
                })
            }
        }
    }

    /// Analyzes an image file using Ollama's vision model (llava)
    pub async fn analyze_image(&self, image_path: &str) -> Result<FileAnalysis> {
        // Validate image path
        let path = Path::new(image_path);
        if !path.exists() {
            return Err(AppError::FileNotFound {
                path: image_path.to_string(),
            });
        }

        // Check if it's actually an image file
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .unwrap_or_default();

        if !matches!(
            extension.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
        ) {
            return Err(AppError::InvalidInput {
                message: format!("Unsupported image format: {}", extension),
            });
        }

        // Check file size before reading to prevent memory exhaustion
        let metadata = tokio::fs::metadata(image_path)
            .await
            .map_err(AppError::Io)?;

        const MAX_IMAGE_SIZE: u64 = 50 * 1024 * 1024; // 50MB limit
        if metadata.len() > MAX_IMAGE_SIZE {
            return Err(AppError::InvalidInput {
                message: format!(
                    "Image file too large: {:.2}MB (max: 50MB)",
                    metadata.len() as f64 / (1024.0 * 1024.0)
                ),
            });
        }

        // Read and encode image to base64
        let image_bytes = tokio::fs::read(image_path).await.map_err(AppError::Io)?;

        // Additional check after reading - base64 encoding increases size by ~33%
        const MAX_PROCESSED_SIZE: usize = 66 * 1024 * 1024; // 66MB for base64
        if image_bytes.len() > MAX_PROCESSED_SIZE {
            return Err(AppError::InvalidInput {
                message: format!(
                    "Image processing size too large: {:.2}MB (max after encoding: 66MB)",
                    image_bytes.len() as f64 / (1024.0 * 1024.0)
                ),
            });
        }

        let base64_image = general_purpose::STANDARD.encode(&image_bytes);

        // Create vision prompt
        let prompt = format!(
            r#"Analyze this image and provide a JSON response with the following structure:
{{
    "category": "Images",
    "tags": ["array", "of", "relevant", "tags", "describing", "image", "content"],
    "summary": "detailed description of what you see in the image",
    "confidence": 0.0 to 1.0,
    "detected_objects": ["list", "of", "objects", "or", "subjects", "in", "image"],
    "scene_type": "indoor/outdoor/portrait/landscape/document/screenshot/etc",
    "colors": ["dominant", "colors", "in", "image"],
    "text_detected": "any text visible in the image or empty string if none"
}}

Analyze this {} image and describe what you see. Focus on:
- Main subjects or objects
- Scene setting and context  
- Any text or writing visible
- Overall composition and style
- Relevant tags for organization

Respond ONLY with valid JSON, no explanations."#,
            extension
        );

        // Call vision model with image
        let response = self
            .generate_vision_completion(&prompt, &base64_image)
            .await?;

        let (category, confidence, analysis) = parse_vision_response(&response);

        // Convert to FileAnalysis
        Ok(FileAnalysis {
            path: image_path.to_string(),
            category,
            tags: analysis.tags,
            summary: analysis.summary,
            confidence,
            extracted_text: if analysis.text_detected.is_empty() {
                None
            } else {
                Some(analysis.text_detected)
            },
            detected_language: None,
            metadata: serde_json::json!({
                "detected_objects": analysis.detected_objects,
                "scene_type": analysis.scene_type,
                "colors": analysis.colors,
                "image_format": extension
            }),
        })
    }

    /// Generate completion using vision model with image
    async fn generate_vision_completion(&self, prompt: &str, base64_image: &str) -> Result<String> {
        // Implement retry logic manually since with_retries doesn't exist
        let mut last_error = None;

        for attempt in 0..3 {
            if attempt > 0 {
                // Exponential backoff between retries
                tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
            }

            // Acquire connection permit
            let permit = match self.connection_pool.acquire().await {
                Ok(p) => p,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            let request = serde_json::json!({
                "model": self.vision_model,
                "prompt": prompt,
                "images": [base64_image],
                "stream": false,
                "options": {
                    "temperature": 0.1,
                    "top_p": 0.9,
                    "num_predict": 512
                }
            });

            // Make HTTP request directly since ollama-rs doesn't support vision yet.
            // Previously this used `format!("http://{}/api/generate", self.client.uri())`,
            // which produced "http://http://localhost:11434/..." when uri() already
            // contained a scheme — vision requests then failed silently.
            let base = self.client.uri();
            let base = base.trim_end_matches('/');
            let url = if base.starts_with("http://") || base.starts_with("https://") {
                format!("{}/api/generate", base)
            } else {
                format!("http://{}/api/generate", base)
            };
            let client = reqwest::Client::new();
            let response_result = timeout(
                Duration::from_secs(60), // Vision analysis can take longer
                client.post(&url).json(&request).send(),
            )
            .await;

            let response = match response_result {
                Ok(Ok(resp)) => resp,
                Ok(Err(e)) => {
                    permit.failure().await;
                    last_error = Some(AppError::NetworkError {
                        message: format!("Vision request failed: {}", e),
                    });
                    continue;
                }
                Err(_) => {
                    permit.failure().await;
                    last_error = Some(AppError::Timeout {
                        message: "Vision analysis timed out".to_string(),
                    });
                    continue;
                }
            };

            if !response.status().is_success() {
                permit.failure().await;
                last_error = Some(AppError::AiError {
                    message: format!(
                        "Vision model request failed with status: {}",
                        response.status()
                    ),
                });
                continue;
            }

            let response_text = match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    permit.failure().await;
                    last_error = Some(AppError::NetworkError {
                        message: format!("Failed to read vision response: {}", e),
                    });
                    continue;
                }
            };

            // Parse the response (Ollama returns JSON with "response" field)
            let json_response: serde_json::Value = match serde_json::from_str(&response_text) {
                Ok(json) => json,
                Err(e) => {
                    permit.failure().await;
                    last_error = Some(AppError::ParseError {
                        message: format!("Invalid JSON from vision model: {}", e),
                    });
                    continue;
                }
            };

            let result = json_response
                .get("response")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| AppError::ParseError {
                    message: "No response field in vision model output".to_string(),
                });

            match result {
                Ok(text) => {
                    permit.success().await;
                    return Ok(text);
                }
                Err(e) => {
                    permit.failure().await;
                    last_error = Some(e);
                }
            }
        }

        // If we get here, all retries failed
        Err(last_error.unwrap_or_else(|| AppError::AiError {
            message: "Failed to generate vision completion after retries".to_string(),
        }))
    }
}

#[async_trait]
impl AiEngine for OllamaClient {
    async fn analyze_file(&self, content: &str, file_type: &str) -> Result<FileAnalysis> {
        // Sanitize inputs to prevent prompt injection
        let sanitized_content = sanitize_prompt_content(content)?;
        let sanitized_file_type = sanitize_prompt_content(file_type)?;

        let prompt = format!(
            r#"Analyze this file content and provide a JSON response with the following structure:
{{
    "category": "string (Documents/Images/Code/Data/Media/Archives/Other)",
    "tags": ["array", "of", "relevant", "tags"],
    "summary": "brief description of the file content",
    "confidence": 0.0 to 1.0
}}

File type: {}
Content preview:
{}

Respond ONLY with valid JSON, no explanations."#,
            sanitized_file_type, sanitized_content
        );

        let response = self.generate_completion(&prompt, &self.text_model).await?;
        // Reference vision model name to keep parity/config discoverable
        let _vision_model_name = &self.vision_model;

        // Sanitize response to prevent XSS or injection through AI output
        let sanitized_response = sanitize_prompt_content(&response)?;

        // Parse JSON response tolerantly — strip prose/fences then deserialize.
        let json_candidate = extract_json_object(&sanitized_response)
            .unwrap_or(sanitized_response.as_str());
        let analysis: AnalysisResponse = serde_json::from_str(json_candidate).map_err(|e| {
            error!(
                "Failed to parse AI response: {} - Response: {}",
                e,
                sanitized_response.chars().take(300).collect::<String>()
            );
            AppError::ParseError {
                message: "Invalid AI response format".to_string(),
            }
        })?;

        // Validate analysis content
        if analysis.category.len() > 100
            || analysis.content_summary.len() > 500
            || analysis.tags.iter().any(|tag| tag.len() > 50)
            || analysis.tags.len() > 20
        {
            return Err(AppError::ParseError {
                message: "AI response contains potentially malicious content".to_string(),
            });
        }

        Ok(FileAnalysis {
            path: "".to_string(), // Will be set by the caller
            category: analysis.category,
            tags: analysis.tags,
            summary: analysis.content_summary,
            confidence: analysis.confidence,
            extracted_text: None,
            detected_language: None,
            metadata: serde_json::json!({}),
        })
    }

    async fn generate_embeddings(&self, text: &str) -> Result<Vec<f32>> {
        // Call the struct's own generate_embeddings method
        OllamaClient::generate_embeddings(self, text).await
    }

    async fn suggest_organization(
        &self,
        files: Vec<String>,
        smart_folders: Vec<crate::commands::organization::SmartFolder>,
    ) -> Result<Vec<OrganizationSuggestion>> {
        let files_list = files
            .iter()
            .take(20) // Limit to prevent prompt overflow
            .map(|f| {
                let filename = std::path::Path::new(f)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(f);
                let extension = std::path::Path::new(f)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                format!("- File: {} (type: {})", filename, extension)
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Build available smart folders context for the LLM with more detail
        let smart_folders_context = smart_folders
            .iter()
            .filter(|folder| folder.enabled)
            .map(|folder| {
                let rules_summary = folder
                    .rules
                    .iter()
                    .filter(|r| r.enabled)
                    .map(|r| format!("{:?}", r.rule_type))
                    .collect::<Vec<_>>()
                    .join(", ");

                format!(
                    "* Folder: '{}'\n  Description: {}\n  Rules: [{}]\n  Target Path: {}",
                    folder.name,
                    folder
                        .description
                        .as_deref()
                        .unwrap_or("General purpose folder"),
                    if rules_summary.is_empty() {
                        "No specific rules"
                    } else {
                        &rules_summary
                    },
                    folder.target_path
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            r#"You are a file organization assistant. Analyze these files and match them to the most appropriate smart folders.

AVAILABLE SMART FOLDERS:
{}

FILES TO ORGANIZE:
{}

INSTRUCTIONS:
1. Read each file name and extension carefully
2. Match files to folders based on:
   - Folder name and description
   - File type and extension
   - Folder rules (if any)
3. If a file clearly matches a folder's purpose, use high confidence (0.7-1.0)
4. If unsure but there's a reasonable match, use medium confidence (0.4-0.6)
5. If no good match exists, use low confidence (0.1-0.3) for the best available option

Respond with ONLY a JSON array (no additional text):
[
  {{
    "source_path": "<exact file path from the list>",
    "target_folder": "<exact folder name from available folders>",
    "reason": "<clear explanation why this file belongs in this folder>",
    "confidence": <0.0 to 1.0>
  }}
]"#,
            smart_folders_context, files_list
        );

        let response = self.generate_completion(&prompt, &self.text_model).await?;

        let json_candidate = extract_json_array(&response).unwrap_or(response.as_str());
        let suggestions: Vec<SuggestionResponse> = serde_json::from_str(json_candidate)
            .map_err(|e| {
                error!(
                    "Failed to parse suggestions: {} - Response: {}",
                    e,
                    response.chars().take(300).collect::<String>()
                );
                AppError::ParseError {
                    message: "Invalid suggestion format".to_string(),
                }
            })?;

        Ok(suggestions
            .into_iter()
            .map(|s| OrganizationSuggestion {
                source_path: s.source_path,
                target_folder: s.target_folder,
                reason: s.reason,
                confidence: s.confidence,
            })
            .collect())
    }
}

/// Setup Ollama on first run
pub async fn setup_ollama() -> Result<()> {
    let client = OllamaClient::new("http://localhost:11434").await?;

    // Check if Ollama is running
    if client.health_check().await.is_err() {
        warn!("Ollama is not running. Please start Ollama to use AI features.");
        return Ok(());
    }

    // Check for required models
    let model_names = client.list_models().await?;

    let required_models = vec![
        DEFAULT_TEXT_MODEL,
        DEFAULT_VISION_MODEL,
        DEFAULT_EMBEDDING_MODEL,
    ];

    for model in required_models {
        if !model_names.iter().any(|m| m.starts_with(model)) {
            info!("Model {} not found, consider installing it", model);
        }
    }

    info!("Ollama setup complete");
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub size: u64,
    pub modified_at: String,
}

#[derive(Debug, Deserialize)]
struct AnalysisResponse {
    category: String,
    tags: Vec<String>,
    #[serde(alias = "summary", alias = "content_summary")]
    content_summary: String,
    confidence: f32,
}

#[derive(Debug, Deserialize)]
struct SuggestionResponse {
    source_path: String,
    target_folder: String,
    reason: String,
    confidence: f32,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct VisionAnalysisResponse {
    #[serde(default)]
    pub(crate) category: String,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default, alias = "description", alias = "content_summary")]
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) confidence: f32,
    #[serde(default)]
    pub(crate) detected_objects: Vec<String>,
    #[serde(default)]
    pub(crate) scene_type: String,
    #[serde(default)]
    pub(crate) colors: Vec<String>,
    #[serde(default, alias = "text", alias = "ocr_text")]
    pub(crate) text_detected: String,
}
