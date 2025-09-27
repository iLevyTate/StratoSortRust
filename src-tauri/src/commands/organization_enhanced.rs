use crate::{
    ai::{FileAnalysis, ollama::OllamaClient},
    core::{AtomicFileOperation, ContentExtractor, FolderMatch, SemanticMatcher, smart_folders::SmartFolder},
    error::Result,
    services::NamingService,
    state::AppState,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tracing::{info, warn};

/// Options for organization operation
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrganizeOptions {
    pub confidence_threshold: f32,
    pub auto_rename: bool,
    pub use_smart_naming: bool,
    pub use_semantic_matching: bool,
    pub use_historical_patterns: bool,
    pub preserve_originals: bool,
    pub dry_run: bool,
}

impl Default for OrganizeOptions {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.7,
            auto_rename: true,
            use_smart_naming: true,
            use_semantic_matching: true,
            use_historical_patterns: true,
            preserve_originals: false,
            dry_run: false,
        }
    }
}

/// Result of an organization operation
#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizationResult {
    pub auto_organized: Vec<FileOrganization>,
    pub needs_review: Vec<ReviewItem>,
    pub errors: Vec<ErrorItem>,
    pub total_processed: usize,
    pub total_organized: usize,
    pub total_skipped: usize,
}

/// Represents a single file organization
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileOrganization {
    pub source_path: String,
    pub destination_path: String,
    pub suggested_name: String,
    pub confidence: f32,
    pub match_type: String,
    pub folder: SmartFolder,
}

/// Item that needs manual review
#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewItem {
    pub file_path: String,
    pub analysis: FileAnalysis,
    pub suggested_name: String,
    pub folder_suggestions: Vec<FolderMatch>,
}

/// Error item
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorItem {
    pub file_path: String,
    pub error: String,
}

/// Enhanced organization command with AI and smart features
#[tauri::command]
pub async fn organize_files_with_ai(
    files: Vec<String>,
    options: OrganizeOptions,
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
) -> Result<OrganizationResult> {
    info!(
        "Starting enhanced organization for {} files with confidence threshold {}",
        files.len(),
        options.confidence_threshold
    );

    let mut result = OrganizationResult {
        auto_organized: Vec::new(),
        needs_review: Vec::new(),
        errors: Vec::new(),
        total_processed: files.len(),
        total_organized: 0,
        total_skipped: 0,
    };

    // Initialize services
    let naming_service = NamingService::new();
    let semantic_matcher = SemanticMatcher::new(state.database.clone());

    // Initialize Ollama client for enhanced analysis
    let ollama_host = state.config.read().ollama_host.clone();
    let ollama_client = match OllamaClient::new(&ollama_host).await {
        Ok(client) => {
            info!("Ollama client initialized for enhanced organization");
            Some(Arc::new(client))
        }
        Err(e) => {
            warn!("Ollama client not available for enhanced organization: {}", e);
            None
        }
    };

    // Initialize content extractor with LLM
    let content_extractor = ContentExtractor::new_with_llm().await;

    // Get all smart folders
    let smart_folders = state.smart_folders.get_all().await?;

    // Create operation ID for progress tracking
    let operation_id = state.start_operation(
        crate::state::OperationType::Organization,
        format!("Organizing {} files with AI", files.len()),
    );

    // Phase 1: Analyze all files
    for (index, file_path) in files.iter().enumerate() {
        // Update progress
        state.update_progress(
            operation_id,
            (index as f32 / files.len() as f32) * 0.5, // First 50% for analysis
            format!("Analyzing file {} of {}", index + 1, files.len()),
        );

        // Check for cancellation
        if state.is_operation_cancelled(operation_id) {
            info!("Organization cancelled by user");
            break;
        }

        match analyze_and_match_file(
            file_path,
            &smart_folders,
            &options,
            &state,
            &naming_service,
            &semantic_matcher,
            ollama_client.as_ref(),
            &content_extractor,
        )
        .await
        {
            Ok((analysis, suggested_name, matches)) => {
                if let Some(best_match) = matches.first() {
                    if best_match.confidence >= options.confidence_threshold {
                        // Auto-organize with high confidence
                        let destination_path = build_destination_path(
                            &best_match.folder.path,
                            &suggested_name,
                        );

                        result.auto_organized.push(FileOrganization {
                            source_path: file_path.clone(),
                            destination_path: destination_path.to_string_lossy().to_string(),
                            suggested_name: suggested_name.clone(),
                            confidence: best_match.confidence,
                            match_type: format!("{:?}", best_match.match_type),
                            folder: best_match.folder.clone(),
                        });
                    } else {
                        // Needs manual review
                        result.needs_review.push(ReviewItem {
                            file_path: file_path.clone(),
                            analysis,
                            suggested_name,
                            folder_suggestions: matches.into_iter().take(3).collect(),
                        });
                    }
                } else {
                    // No matches found
                    result.needs_review.push(ReviewItem {
                        file_path: file_path.clone(),
                        analysis,
                        suggested_name,
                        folder_suggestions: vec![],
                    });
                }
            }
            Err(e) => {
                result.errors.push(ErrorItem {
                    file_path: file_path.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // Phase 2: Execute organization operations
    if !options.dry_run && !result.auto_organized.is_empty() {
        info!("Executing {} file organizations", result.auto_organized.len());

        // Create atomic transaction
        let mut atomic_op = AtomicFileOperation::new()?;

        for (index, org) in result.auto_organized.iter().enumerate() {
            // Update progress
            state.update_progress(
                operation_id,
                0.5 + (index as f32 / result.auto_organized.len() as f32) * 0.5, // Second 50% for execution
                format!("Moving file {} of {}", index + 1, result.auto_organized.len()),
            );

            let source = Path::new(&org.source_path);
            let destination = Path::new(&org.destination_path);

            // Add operation to transaction
            if options.preserve_originals {
                atomic_op.add_copy(source, destination)?;
            } else {
                atomic_op.add_move(source, destination)?;
            }
        }

        // Execute all operations atomically
        match atomic_op.execute().await {
            Ok(_) => {
                info!("Successfully organized {} files", result.auto_organized.len());
                result.total_organized = result.auto_organized.len();

                // Record in undo/redo manager
                for org in &result.auto_organized {
                    state.undo_redo.record_move(
                        &org.source_path,
                        &org.destination_path,
                    ).await?;
                }

                // Emit success event
                let _ = app.emit(
                    "organization:completed",
                    serde_json::json!({
                        "organized": result.total_organized,
                        "skipped": result.needs_review.len(),
                        "errors": result.errors.len(),
                    }),
                );
            }
            Err(e) => {
                error!("Failed to execute organization: {}", e);
                // Transaction automatically rolled back on error
                return Err(e);
            }
        }
    } else if options.dry_run {
        info!("Dry run completed - no files were moved");
    }

    result.total_skipped = result.needs_review.len();

    // Complete operation
    state.complete_operation(operation_id);

    Ok(result)
}

/// Analyze a file and find matching folders
#[allow(clippy::too_many_arguments)]
async fn analyze_and_match_file(
    file_path: &str,
    smart_folders: &[SmartFolder],
    options: &OrganizeOptions,
    state: &Arc<AppState>,
    naming_service: &NamingService,
    semantic_matcher: &SemanticMatcher,
    ollama_client: Option<&Arc<OllamaClient>>,
    content_extractor: &ContentExtractor,
) -> Result<(FileAnalysis, String, Vec<FolderMatch>)> {
    // Analyze file with AI
    let mut analysis = state.file_analyzer.analyze_file(file_path).await?;

    // Enhanced LLM analysis if available
    let (enhanced_analysis, llm_suggestions) = if let Some(client) = ollama_client {
        // Extract content for deeper analysis
        let content = content_extractor.extract_content_with_options(
            Path::new(file_path),
            true // Use LLM enhancement
        ).await.unwrap_or_default();

        // Get enhanced document analysis
        let doc_analysis = match client.analyze_document_enhanced(&content, file_path, smart_folders).await {
            Ok(analysis) => Some(analysis),
            Err(e) => {
                warn!("Failed to get enhanced document analysis: {}", e);
                None
            }
        };

        // Get creative folder suggestions
        let folder_suggestions = match client.suggest_folders_creative(&analysis, smart_folders).await {
            Ok(suggestions) => suggestions,
            Err(e) => {
                warn!("Failed to get creative folder suggestions: {}", e);
                vec![]
            }
        };

        // Get contextual suggestions
        let contextual_suggestions = if !folder_suggestions.is_empty() {
            match client.get_contextual_suggestions(&analysis, smart_folders).await {
                Ok(suggestions) => suggestions,
                Err(e) => {
                    warn!("Failed to get contextual suggestions: {}", e);
                    vec![]
                }
            }
        } else {
            vec![]
        };

        // Merge suggestions
        let mut all_suggestions = folder_suggestions;
        all_suggestions.extend(contextual_suggestions);

        (doc_analysis, all_suggestions)
    } else {
        (None, vec![])
    };

    // Generate smart name based on enhanced analysis if available
    let suggested_name = if options.use_smart_naming {
        if let Some(ref enhanced) = enhanced_analysis {
            // Use LLM-suggested name or generate from enhanced analysis
            if !enhanced.suggested_name.is_empty() {
                format!(
                    "{}.{}",
                    enhanced.suggested_name,
                    Path::new(file_path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                )
            } else {
                naming_service.generate_smart_name_from_llm(enhanced, Path::new(file_path))?
            }
        } else {
            naming_service.generate_smart_name(&analysis, Path::new(file_path))?
        }
    } else {
        Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string()
    };

    // Update analysis with enhanced data if available
    if let Some(enhanced) = enhanced_analysis {
        analysis.category = enhanced.category;
        analysis.tags = enhanced.keywords;
        analysis.summary = format!("{} - {}", enhanced.purpose, enhanced.summary);
        if let Some(date) = enhanced.date {
            analysis.metadata["date"] = serde_json::Value::String(date);
        }
        if let Some(client) = enhanced.client {
            analysis.metadata["client"] = serde_json::Value::String(client);
        }
        if let Some(project) = enhanced.project {
            analysis.metadata["project"] = serde_json::Value::String(project);
        }
    }

    // Find matching folders using multiple strategies
    let mut all_matches = Vec::new();

    // 0. Add LLM suggestions with high priority
    for suggestion in llm_suggestions {
        if let Some(folder) = smart_folders.iter().find(|f| f.name == suggestion.folder_name) {
            all_matches.push(FolderMatch {
                folder: folder.clone(),
                confidence: suggestion.confidence,
                match_type: crate::core::MatchType::Semantic,
                reason: format!("LLM: {}", suggestion.reasoning),
                similarity_details: Some(crate::core::semantic_matcher::SimilarityDetails {
                    cosine_similarity: suggestion.confidence,
                    euclidean_distance: 1.0 - suggestion.confidence,
                    matching_keywords: vec!["llm_analysis".to_string()],
                    category_match: true,
                }),
            });
        }
    }

    // 1. Semantic matching with embeddings
    if options.use_semantic_matching {
        // Get or generate embedding for the file
        let embedding = match state.database.get_embedding(file_path).await {
            Ok(Some(emb)) => Some(emb),
            Ok(None) => {
                // Generate embedding
                match state.ai_service.generate_embeddings(&format!(
                    "{} {}",
                    analysis.summary,
                    analysis.tags.join(" ")
                )).await {
                    Ok(emb) => {
                        // Save for future use
                        let embedding_model = state.config.read().ollama_embedding_model.clone();
                        let _ = state.database.save_embedding(
                            file_path,
                            &emb,
                            Some(&embedding_model),
                        ).await;
                        Some(emb)
                    }
                    Err(e) => {
                        warn!("Failed to generate embedding: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get embedding: {}", e);
                None
            }
        };

        if let Some(emb) = embedding {
            let semantic_matches = semantic_matcher
                .find_similar_folders(&emb, smart_folders, 0.6)
                .await?;
            all_matches.extend(semantic_matches);
        }
    }

    // 2. Rule-based matching
    for folder in smart_folders {
        if matches_folder_rules(&analysis, folder) {
            all_matches.push(FolderMatch {
                folder: folder.clone(),
                confidence: 0.8,
                match_type: crate::core::MatchType::RuleBased,
                reason: "Matches folder rules".to_string(),
                similarity_details: None,
            });
        }
    }

    // 3. Historical pattern matching
    if options.use_historical_patterns {
        let historical_matches = semantic_matcher
            .find_best_matches(
                file_path,
                None,
                &analysis,
                smart_folders,
            )
            .await?;
        all_matches.extend(historical_matches);
    }

    // Combine and deduplicate matches
    let final_matches = combine_and_rank_matches(all_matches);

    Ok((analysis, suggested_name, final_matches))
}

/// Check if analysis matches folder rules
fn matches_folder_rules(analysis: &FileAnalysis, folder: &SmartFolder) -> bool {
    // Check category match
    if folder.name.to_lowercase().contains(&analysis.category.to_lowercase()) {
        return true;
    }

    // Check tag matches
    for tag in &analysis.tags {
        if folder.name.to_lowercase().contains(&tag.to_lowercase()) {
            return true;
        }
    }

    // Note: smart_folders::SmartFolder doesn't have a description field
    // Could potentially use folder.path or other fields for matching in the future

    false
}

/// Build destination path from folder and filename
fn build_destination_path(folder_path: &str, filename: &str) -> PathBuf {
    let mut path = PathBuf::from(folder_path);
    path.push(filename);

    // Ensure unique name if file exists
    let mut unique_path = path.clone();
    let mut counter = 1;

    while unique_path.exists() {
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let new_name = if extension.is_empty() {
            format!("{}_{}", stem, counter)
        } else {
            format!("{}_{}.{}", stem, counter, extension)
        };

        unique_path = path.parent()
            .map(|p| p.join(&new_name))
            .unwrap_or_else(|| PathBuf::from(new_name));

        counter += 1;
    }

    unique_path
}

/// Combine and rank matches from different sources
fn combine_and_rank_matches(matches: Vec<FolderMatch>) -> Vec<FolderMatch> {
    use std::collections::HashMap;

    let mut combined: HashMap<String, FolderMatch> = HashMap::new();

    for match_item in matches {
        let folder_id = &match_item.folder.id;

        combined
            .entry(folder_id.clone())
            .and_modify(|existing| {
                // Average confidence scores
                existing.confidence = (existing.confidence + match_item.confidence) / 2.0;
                existing.match_type = crate::core::MatchType::Hybrid;
                existing.reason = format!("{} + {}", existing.reason, match_item.reason);
            })
            .or_insert(match_item);
    }

    let mut result: Vec<FolderMatch> = combined.into_values().collect();
    result.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

    result
}

/// Batch organization preview
#[tauri::command]
pub async fn preview_organization(
    files: Vec<String>,
    options: OrganizeOptions,
    state: State<'_, Arc<AppState>>,
) -> Result<OrganizationResult> {
    let mut preview_options = options;
    preview_options.dry_run = true;

    let app_handle = state.handle.clone();
    organize_files_with_ai(files, preview_options, state, app_handle).await
}

use tracing::error;