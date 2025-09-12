use crate::{error::Result, state::AppState};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmartFolder {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rules: Vec<OrganizationRule>,
    pub target_path: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrganizationRule {
    pub id: String,
    pub rule_type: RuleType,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RuleType {
    FileExtension,
    FileSize,
    FileName,
    FileContent,
    CreationDate,
    ModificationDate,
    MimeType,
    Path,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuleCondition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: String,
    pub case_sensitive: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ConditionOperator {
    Equals,
    Contains,
    StartsWith,
    EndsWith,
    GreaterThan,
    LessThan,
    Regex,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuleAction {
    pub action_type: ActionType,
    pub target_folder: String,
    pub rename_pattern: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ActionType {
    Move,
    Copy,
    Rename,
    Tag,
}

#[tauri::command]
pub async fn create_smart_folder(
    name: String,
    description: Option<String>,
    target_path: String,
    rules: Vec<OrganizationRule>,
    state: State<'_, std::sync::Arc<AppState>>,
    app: AppHandle,
) -> Result<SmartFolder> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let smart_folder = SmartFolder {
        id: id.clone(),
        name: name.clone(),
        description,
        rules,
        target_path,
        created_at: now,
        updated_at: now,
        enabled: true,
    };

    // Save to database
    match state.database.save_smart_folder(&smart_folder).await {
        Ok(_) => {
            // Emit success notification
            let _ = app.emit(
                "notification",
                serde_json::json!({
                    "type": "success",
                    "title": "Smart Folder Created",
                    "message": format!("Smart folder '{}' has been created successfully", name),
                    "timestamp": chrono::Utc::now().timestamp(),
                }),
            );

            tracing::info!("Created smart folder: {} ({})", name, id);
            Ok(smart_folder)
        }
        Err(e) => {
            tracing::error!("Failed to create smart folder '{}': {}", name, e);

            let _ = app.emit("notification", serde_json::json!({
                "type": "error",
                "title": "Failed to Create Smart Folder",
                "message": format!("Could not create smart folder '{}': {}", name, e.user_message()),
                "timestamp": chrono::Utc::now().timestamp(),
            }));

            Err(e)
        }
    }
}

#[tauri::command]
pub async fn update_smart_folder(
    id: String,
    name: Option<String>,
    description: Option<String>,
    target_path: Option<String>,
    rules: Option<Vec<OrganizationRule>>,
    enabled: Option<bool>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<SmartFolder> {
    let mut smart_folder = state.database.get_smart_folder(&id).await?.ok_or_else(|| {
        crate::error::AppError::NotFound {
            message: format!("Smart folder not found: {}", id),
        }
    })?;

    // Update fields
    if let Some(n) = name {
        smart_folder.name = n;
    }
    if let Some(d) = description {
        smart_folder.description = Some(d);
    }
    if let Some(tp) = target_path {
        smart_folder.target_path = tp;
    }
    if let Some(r) = rules {
        smart_folder.rules = r;
    }
    if let Some(e) = enabled {
        smart_folder.enabled = e;
    }

    smart_folder.updated_at = chrono::Utc::now();

    // Save to database
    state.database.save_smart_folder(&smart_folder).await?;

    Ok(smart_folder)
}

#[tauri::command]
pub async fn delete_smart_folder(
    id: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<bool> {
    state.database.delete_smart_folder(&id).await?;
    Ok(true)
}

#[tauri::command]
pub async fn list_smart_folders(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<SmartFolder>> {
    state.database.list_smart_folders().await
}

#[tauri::command]
pub async fn get_smart_folder(
    id: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Option<SmartFolder>> {
    state.database.get_smart_folder(&id).await
}

#[tauri::command]
pub async fn apply_smart_folder_rules(
    folder_id: String,
    file_paths: Vec<String>,
    dry_run: bool,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<OrganizationPreview>> {
    let smart_folder = state
        .database
        .get_smart_folder(&folder_id)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound {
            message: format!("Smart folder not found: {}", folder_id),
        })?;

    if !smart_folder.enabled {
        return Ok(vec![]);
    }

    let mut previews = Vec::new();

    for file_path in file_paths {
        let file_info = match tokio::fs::metadata(&file_path).await {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        // Check if any rules match this file
        for rule in &smart_folder.rules {
            if !rule.enabled {
                continue;
            }

            let matches = match rule.rule_type {
                RuleType::FileExtension => {
                    let ext = std::path::Path::new(&file_path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    evaluate_condition(&rule.condition, ext)
                }
                RuleType::FileName => {
                    let name = std::path::Path::new(&file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    evaluate_condition(&rule.condition, name)
                }
                RuleType::FileSize => {
                    evaluate_condition(&rule.condition, &file_info.len().to_string())
                }
                RuleType::MimeType => {
                    let mime = mime_guess::from_path(&file_path)
                        .first_or_octet_stream()
                        .to_string();
                    evaluate_condition(&rule.condition, &mime)
                }
                RuleType::FileContent => {
                    // For file content matching, read a portion of the file
                    if let Ok(content) = tokio::fs::read_to_string(&file_path).await {
                        evaluate_condition(&rule.condition, &content)
                    } else {
                        false
                    }
                }
                RuleType::CreationDate => {
                    // Get file creation time
                    if let Ok(created) = file_info.created() {
                        if let Ok(datetime) = created.duration_since(std::time::UNIX_EPOCH) {
                            let timestamp = datetime.as_secs().to_string();
                            evaluate_condition(&rule.condition, &timestamp)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                RuleType::ModificationDate => {
                    // Get file modification time
                    if let Ok(modified) = file_info.modified() {
                        if let Ok(datetime) = modified.duration_since(std::time::UNIX_EPOCH) {
                            let timestamp = datetime.as_secs().to_string();
                            evaluate_condition(&rule.condition, &timestamp)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                RuleType::Path => {
                    // Match against the full file path
                    evaluate_condition(&rule.condition, &file_path)
                }
            };

            if matches {
                let target_path = std::path::Path::new(&smart_folder.target_path)
                    .join(&rule.action.target_folder);

                let preview = OrganizationPreview {
                    source_path: file_path.clone(),
                    target_path: target_path.display().to_string(),
                    action: rule.action.action_type.clone(),
                    rule_id: rule.id.clone(),
                    confidence: 1.0, // Rule-based, so high confidence
                };

                previews.push(preview);

                // If not dry run, perform the action
                if !dry_run {
                    match rule.action.action_type {
                        ActionType::Move => {
                            if let Some(parent) = target_path.parent() {
                                let _ = tokio::fs::create_dir_all(parent).await;
                            }
                            let _ = tokio::fs::rename(&file_path, &target_path).await;
                        }
                        ActionType::Copy => {
                            if let Some(parent) = target_path.parent() {
                                let _ = tokio::fs::create_dir_all(parent).await;
                            }
                            let _ = tokio::fs::copy(&file_path, &target_path).await;
                        }
                        ActionType::Rename => {
                            // For rename, construct new filename based on pattern
                            let new_name = rule.action.target_folder.clone(); // Target folder is used as rename pattern
                            let parent = std::path::Path::new(&file_path)
                                .parent()
                                .unwrap_or(std::path::Path::new("."));
                            let new_path = parent.join(&new_name);
                            let _ = tokio::fs::rename(&file_path, &new_path).await;
                        }
                        ActionType::Tag => {
                            // For tagging, store metadata in database
                            // This would typically involve adding tags to the file's metadata
                            // For now, we'll just log it as the database operation would need more context
                            tracing::debug!(
                                "Tagged file {} with folder {}",
                                file_path,
                                rule.action.target_folder
                            );
                        }
                    }
                }

                break; // Only apply first matching rule
            }
        }
    }

    Ok(previews)
}

#[tauri::command]
pub async fn auto_organize_directory(
    directory_path: String,
    use_ai: bool,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<OrganizationPreview>> {
    // Start progress tracking for organization
    let operation_id = state.start_operation(
        crate::state::OperationType::Organization,
        format!("Auto-organizing directory: {}", directory_path),
    );

    // Phase 1: Scan directory for files (10% progress)
    state.update_progress(
        operation_id,
        0.1,
        "Scanning directory for files".to_string(),
    );

    let mut file_paths = Vec::new();
    let mut entries = tokio::fs::read_dir(&directory_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        // Check for cancellation
        if let Some(op) = state.active_operations.get(&operation_id) {
            if op.cancellation_token.is_cancelled() {
                state.complete_operation(operation_id);
                return Err(crate::error::AppError::Cancelled);
            }
        }

        if entry.file_type().await?.is_file() {
            file_paths.push(entry.path().display().to_string());
        }
    }

    state.update_progress(
        operation_id,
        0.2,
        format!("Found {} files to organize", file_paths.len()),
    );

    let mut all_previews = Vec::new();

    // Phase 2: AI analysis if requested (20-60% progress)
    let ai_analyses = if use_ai {
        state.update_progress(
            operation_id,
            0.3,
            "Starting AI analysis of files".to_string(),
        );

        let mut analyses = std::collections::HashMap::new();
        let total_files = file_paths.len();

        for (index, file_path) in file_paths.iter().enumerate() {
            // Check for cancellation
            if let Some(op) = state.active_operations.get(&operation_id) {
                if op.cancellation_token.is_cancelled() {
                    state.complete_operation(operation_id);
                    return Err(crate::error::AppError::Cancelled);
                }
            }

            // Analyze files that don't already have analysis results
            if state
                .database
                .get_analysis(file_path)
                .await
                .unwrap_or(None)
                .is_none()
            {
                if let Ok(content) = std::fs::read_to_string(file_path) {
                    if let Ok(analysis) = state.ai_service.analyze_file(&content, "").await {
                        analyses.insert(file_path.clone(), analysis.category.clone());
                        // Store analysis result for future use
                        let _ = state.database.save_analysis(&analysis).await;
                    }
                }
            }

            // Update progress for AI analysis (30-60% range)
            if index % 10 == 0 || index == total_files - 1 {
                let ai_progress = 0.3 + (0.3 * index as f32 / total_files as f32);
                state.update_progress(
                    operation_id,
                    ai_progress,
                    format!("Analyzed {} of {} files", index + 1, total_files),
                );
            }
        }
        analyses
    } else {
        std::collections::HashMap::new()
    };

    // Phase 3: Apply smart folder rules (60-90% progress)
    state.update_progress(operation_id, 0.6, "Applying smart folder rules".to_string());

    let smart_folders = state.database.list_smart_folders().await?;
    for (index, folder) in smart_folders.iter().enumerate() {
        // Check for cancellation
        if let Some(op) = state.active_operations.get(&operation_id) {
            if op.cancellation_token.is_cancelled() {
                state.complete_operation(operation_id);
                return Err(crate::error::AppError::Cancelled);
            }
        }

        if folder.enabled {
            let previews = apply_smart_folder_rules_enhanced(
                folder.id.clone(),
                file_paths.clone(),
                true, // dry run
                &ai_analyses,
                state.clone(),
            )
            .await?;
            all_previews.extend(previews);
        }

        // Update progress for rule application
        let rule_progress = 0.6 + (0.3 * index as f32 / smart_folders.len() as f32);
        state.update_progress(
            operation_id,
            rule_progress,
            format!("Applied rule: {}", folder.name),
        );
    }

    // Phase 4: AI fallback suggestions if needed (90-95% progress)
    if use_ai && all_previews.is_empty() {
        state.update_progress(
            operation_id,
            0.9,
            "Generating AI organization suggestions".to_string(),
        );

        let smart_folders = state
            .database
            .list_smart_folders()
            .await
            .unwrap_or_default();
        let suggestions = state
            .ai_service
            .suggest_organization(file_paths, smart_folders)
            .await?;
        for suggestion in suggestions {
            all_previews.push(OrganizationPreview {
                source_path: suggestion.source_path,
                target_path: format!("{}/{}", directory_path, suggestion.target_folder),
                action: ActionType::Move,
                rule_id: "ai-suggestion".to_string(),
                confidence: suggestion.confidence,
            });
        }
    }

    // Complete the operation
    state.update_progress(
        operation_id,
        1.0,
        format!(
            "Organization complete: {} suggestions generated",
            all_previews.len()
        ),
    );
    state.complete_operation(operation_id);

    Ok(all_previews)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizationPreview {
    pub source_path: String,
    pub target_path: String,
    pub action: ActionType,
    pub rule_id: String,
    pub confidence: f32,
}

#[tauri::command]
pub async fn suggest_file_organization(
    paths: Vec<String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<crate::ai::OrganizationSuggestion>> {
    if paths.is_empty() {
        return Ok(vec![]);
    }

    if paths.len() > 1000 {
        return Err(crate::error::AppError::SecurityError {
            message: "Too many files for organization suggestion (max 1000)".to_string(),
        });
    }

    let smart_folders = state
        .database
        .list_smart_folders()
        .await
        .unwrap_or_default();
    state
        .ai_service
        .suggest_organization(paths, smart_folders)
        .await
}

#[tauri::command]
pub async fn apply_organization(
    operations: Vec<OrganizationOperation>,
    state: State<'_, std::sync::Arc<AppState>>,
    app: AppHandle,
) -> Result<Vec<OrganizationResult>> {
    if operations.is_empty() {
        return Err(crate::error::AppError::InvalidInput {
            message: "No operations provided".to_string(),
        });
    }

    if operations.len() > 500 {
        return Err(crate::error::AppError::SecurityError {
            message: "Too many operations requested (max 500)".to_string(),
        });
    }

    let mut results = Vec::new();
    let operation_id = state.start_operation(
        crate::state::OperationType::BulkOperation,
        format!("Organizing {} files", operations.len()),
    );

    for (index, op) in operations.iter().enumerate() {
        let progress = index as f32 / operations.len() as f32;
        state.update_progress(
            operation_id,
            progress,
            format!(
                "Organizing: {}",
                std::path::Path::new(&op.source_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
        );

        let result = match perform_organization_operation(op).await {
            Ok(_) => {
                // Record for undo
                if let Err(e) = state
                    .undo_redo
                    .record_move(&op.source_path, &op.target_path)
                    .await
                {
                    tracing::warn!("Failed to record undo operation: {}", e);
                }

                // Emit success event for this operation
                let _ = app.emit(
                    "organization-success",
                    serde_json::json!({
                        "source_path": op.source_path,
                        "target_path": op.target_path,
                        "action": op.action,
                        "timestamp": chrono::Utc::now().timestamp(),
                    }),
                );

                OrganizationResult {
                    source_path: op.source_path.clone(),
                    target_path: op.target_path.clone(),
                    action: op.action.clone(),
                    success: true,
                    error: None,
                }
            }
            Err(e) => {
                tracing::error!(
                    "Organization operation failed for {}: {}",
                    op.source_path,
                    e
                );

                // Emit error event for this operation
                let _ = app.emit(
                    "organization-failed",
                    serde_json::json!({
                        "source_path": op.source_path,
                        "target_path": op.target_path,
                        "action": op.action,
                        "error": e.to_string(),
                        "error_type": e.error_type(),
                        "recoverable": e.is_recoverable(),
                        "timestamp": chrono::Utc::now().timestamp(),
                    }),
                );

                OrganizationResult {
                    source_path: op.source_path.clone(),
                    target_path: op.target_path.clone(),
                    action: op.action.clone(),
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        };

        results.push(result);
    }

    state.complete_operation(operation_id);

    // Emit completion summary
    let success_count = results.iter().filter(|r| r.success).count();
    let total_count = results.len();

    if success_count == total_count {
        let _ = app.emit(
            "notification",
            serde_json::json!({
                "type": "success",
                "title": "Organization Complete",
                "message": format!("Successfully organized {} files", success_count),
                "timestamp": chrono::Utc::now().timestamp(),
            }),
        );
    } else {
        let _ = app.emit("notification", serde_json::json!({
            "type": "warning",
            "title": "Organization Partially Complete",
            "message": format!("Organized {} of {} files. {} failed.", success_count, total_count, total_count - success_count),
            "timestamp": chrono::Utc::now().timestamp(),
        }));
    }

    Ok(results)
}

#[tauri::command]
pub async fn get_smart_folders(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<SmartFolder>> {
    state.database.list_smart_folders().await
}

#[tauri::command]
pub async fn match_to_folders(
    paths: Vec<String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<FolderMatch>> {
    let smart_folders = state.database.list_smart_folders().await?;
    let mut matches = Vec::new();

    for path in paths {
        let mut best_match: Option<FolderMatch> = None;
        let mut best_confidence = 0.0;

        for folder in &smart_folders {
            if !folder.enabled {
                continue;
            }

            let confidence = calculate_folder_match_confidence(&path, folder).await;
            if confidence > best_confidence && confidence > 0.5 {
                best_confidence = confidence;
                best_match = Some(FolderMatch {
                    file_path: path.clone(),
                    folder_id: folder.id.clone(),
                    folder_name: folder.name.clone(),
                    confidence,
                    suggested_action: ActionType::Move,
                });
            }
        }

        if let Some(m) = best_match {
            matches.push(m);
        }
    }

    Ok(matches)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizationOperation {
    pub source_path: String,
    pub target_path: String,
    pub action: ActionType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizationResult {
    pub source_path: String,
    pub target_path: String,
    pub action: ActionType,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderMatch {
    pub file_path: String,
    pub folder_id: String,
    pub folder_name: String,
    pub confidence: f32,
    pub suggested_action: ActionType,
}

async fn perform_organization_operation(op: &OrganizationOperation) -> Result<()> {
    let source = std::path::Path::new(&op.source_path);
    let target = std::path::Path::new(&op.target_path);

    if !source.exists() {
        return Err(crate::error::AppError::FileNotFound {
            path: op.source_path.clone(),
        });
    }

    // Create target directory if needed
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    match op.action {
        ActionType::Move => {
            tokio::fs::rename(source, target).await?;
        }
        ActionType::Copy => {
            if source.is_dir() {
                copy_dir_all(source, target).await?;
            } else {
                tokio::fs::copy(source, target).await?;
            }
        }
        ActionType::Rename => {
            tokio::fs::rename(source, target).await?;
        }
        _ => {
            return Err(crate::error::AppError::InvalidInput {
                message: format!("Unsupported action: {:?}", op.action),
            });
        }
    }

    Ok(())
}

async fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    tokio::fs::create_dir_all(dst).await?;

    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            Box::pin(copy_dir_all(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }

    Ok(())
}

pub async fn calculate_folder_match_confidence(file_path: &str, folder: &SmartFolder) -> f32 {
    let path = std::path::Path::new(file_path);
    let mut total_score = 0.0;
    let mut rule_count = 0;

    // Try to get AI analysis for enhanced matching
    // Note: For now, we'll extract category info from file extension/path pattern
    // A future enhancement can integrate with AI service analysis results
    let ai_category: Option<String> = infer_file_category(file_path);

    for rule in &folder.rules {
        if !rule.enabled {
            continue;
        }

        rule_count += 1;

        let matches = match rule.rule_type {
            RuleType::FileExtension => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                evaluate_condition(&rule.condition, ext)
            }
            RuleType::FileName => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                evaluate_condition(&rule.condition, name)
            }
            RuleType::MimeType => {
                let mime = mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string();
                evaluate_condition(&rule.condition, &mime)
            }
            RuleType::FileSize => {
                // Get file size for comparison
                if let Ok(metadata) = std::fs::metadata(file_path) {
                    let size_str = metadata.len().to_string();
                    evaluate_condition(&rule.condition, &size_str)
                } else {
                    false
                }
            }
            RuleType::FileContent => {
                // Read file content for matching (first 10KB for performance)
                if let Ok(content) = std::fs::read_to_string(file_path) {
                    let preview = if content.len() > 10240 {
                        &content[..10240]
                    } else {
                        &content
                    };
                    evaluate_condition(&rule.condition, preview)
                } else {
                    false
                }
            }
            RuleType::CreationDate => {
                // Get creation date for comparison
                if let Ok(metadata) = std::fs::metadata(file_path) {
                    if let Ok(created) = metadata.created() {
                        if let Ok(duration) = created.duration_since(std::time::UNIX_EPOCH) {
                            let timestamp_str = duration.as_secs().to_string();
                            evaluate_condition(&rule.condition, &timestamp_str)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            RuleType::ModificationDate => {
                // Get modification date for comparison
                if let Ok(metadata) = std::fs::metadata(file_path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                            let timestamp_str = duration.as_secs().to_string();
                            evaluate_condition(&rule.condition, &timestamp_str)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            RuleType::Path => {
                // Match against the full file path
                evaluate_condition(&rule.condition, file_path)
            }
        };

        if matches {
            total_score += 1.0;
        }
    }

    // Enhanced confidence calculation
    let base_confidence = if rule_count > 0 {
        total_score / rule_count as f32
    } else {
        0.0
    };

    // Boost confidence if AI analysis matches folder purpose/name

    if let Some(category) = ai_category {
        let folder_name_lower = folder.name.to_lowercase();
        let category_lower = category.to_lowercase();

        // If AI category matches folder name/type, boost confidence
        if folder_name_lower.contains(&category_lower)
            || category_lower.contains(&folder_name_lower)
        {
            (base_confidence + 0.3).min(1.0) // Boost by 30% but cap at 100%
        } else {
            base_confidence
        }
    } else {
        base_confidence
    }
}

fn evaluate_condition(condition: &RuleCondition, value: &str) -> bool {
    let case_sensitive = condition.case_sensitive.unwrap_or(false);

    // Prepare values for comparison based on case sensitivity
    let (compare_value, compare_condition) = if case_sensitive {
        (value.to_string(), condition.value.clone())
    } else {
        (value.to_lowercase(), condition.value.to_lowercase())
    };

    match condition.operator {
        ConditionOperator::Equals => compare_value == compare_condition,
        ConditionOperator::Contains => compare_value.contains(&compare_condition),
        ConditionOperator::StartsWith => compare_value.starts_with(&compare_condition),
        ConditionOperator::EndsWith => compare_value.ends_with(&compare_condition),
        ConditionOperator::GreaterThan => {
            if let (Ok(val), Ok(cond)) = (value.parse::<f64>(), condition.value.parse::<f64>()) {
                val > cond
            } else {
                false
            }
        }
        ConditionOperator::LessThan => {
            if let (Ok(val), Ok(cond)) = (value.parse::<f64>(), condition.value.parse::<f64>()) {
                val < cond
            } else {
                false
            }
        }
        ConditionOperator::Regex => {
            let regex_flags = if case_sensitive {
                &condition.value
            } else {
                // For case-insensitive regex, we need to add the 'i' flag if not already present
                &format!("(?i){}", condition.value)
            };

            if let Ok(regex) = regex::Regex::new(regex_flags) {
                regex.is_match(value)
            } else {
                false
            }
        }
    }
}

/// Enhanced smart folder rules application with AI analysis integration
async fn apply_smart_folder_rules_enhanced(
    folder_id: String,
    file_paths: Vec<String>,
    dry_run: bool,
    ai_analyses: &std::collections::HashMap<String, String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<OrganizationPreview>> {
    let folder = state.database.get_smart_folder(&folder_id).await?.ok_or(
        crate::error::AppError::DatabaseError {
            message: "Smart folder not found".to_string(),
        },
    )?;

    let mut previews = Vec::new();

    for file_path in file_paths {
        // Calculate confidence using both rules and AI analysis
        let confidence =
            calculate_folder_match_confidence_with_ai(&file_path, &folder, ai_analyses).await;

        if confidence > 0.5 {
            let target_path = std::path::Path::new(&folder.target_path)
                .join(
                    std::path::Path::new(&file_path)
                        .file_name()
                        .unwrap_or_default(),
                )
                .display()
                .to_string();

            previews.push(OrganizationPreview {
                source_path: file_path,
                target_path,
                action: ActionType::Move,
                rule_id: folder_id.clone(),
                confidence,
            });
        }
    }

    if !dry_run {
        // Actually perform the moves (implementation would be here)
        // For now, just return the previews
    }

    Ok(previews)
}

/// Calculate folder match confidence enhanced with AI analysis
async fn calculate_folder_match_confidence_with_ai(
    file_path: &str,
    folder: &SmartFolder,
    ai_analyses: &std::collections::HashMap<String, String>,
) -> f32 {
    // Start with base confidence from rules
    let base_confidence = calculate_folder_match_confidence(file_path, folder).await;

    // Enhance with AI analysis if available
    if let Some(ai_category) = ai_analyses.get(file_path) {
        let folder_name_lower = folder.name.to_lowercase();
        let category_lower = ai_category.to_lowercase();

        // AI category matches folder name/purpose
        if folder_name_lower.contains(&category_lower)
            || category_lower.contains(&folder_name_lower)
            || folder
                .description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&category_lower))
                .unwrap_or(false)
        {
            (base_confidence + 0.4).min(1.0) // Boost by 40% but cap at 100%
        } else {
            base_confidence
        }
    } else {
        base_confidence
    }
}

/// Infer file category from extension and path patterns for AI-enhanced matching
fn infer_file_category(file_path: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    match extension.to_lowercase().as_str() {
        // Document files
        "pdf" | "doc" | "docx" | "docm" | "dot" | "dotx" | "dotm" | "txt" | "rtf" | "odt"
        | "ott" | "pages" => Some("Documents".to_string()),

        // Image files
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "tiff" => {
            Some("Images".to_string())
        }

        // Video files
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" => {
            Some("Videos".to_string())
        }

        // Audio files
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" => Some("Audio".to_string()),

        // Code files
        "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "h" | "hpp" | "go" | "php" => {
            Some("Code".to_string())
        }

        // Archive files
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" => Some("Archives".to_string()),

        // Spreadsheet files
        "xls" | "xlsx" | "xlsm" | "xlsb" | "xlt" | "xltx" | "xltm" | "csv" | "tsv" | "ods"
        | "ots" | "numbers" => Some("Spreadsheets".to_string()),

        // Presentation files
        "ppt" | "pptx" | "pptm" | "ppsx" | "pps" | "pot" | "potx" | "potm" | "odp" | "key" => {
            Some("Presentations".to_string())
        }

        // 3D Print files
        "stl" | "obj" | "3mf" | "amf" | "ply" | "x3d" | "dae" | "blend" | "fbx" | "3ds" | "max"
        | "c4d" | "ma" | "mb" | "skp" | "dwg" | "dxf" | "step" | "stp" | "iges" | "igs"
        | "brep" | "gcode" | "g" | "ngc" | "cnc" | "prusa" | "chitubox" | "lgs" | "pws" | "sl1"
        | "ctb" | "cbddlp" | "photon" | "pmsq" | "zip3d" => Some("3D Print Files".to_string()),

        _ => {
            // Try to infer from filename patterns
            let filename_lower = filename.to_lowercase();

            if filename_lower.contains("screenshot") || filename_lower.contains("screen") {
                Some("Screenshots".to_string())
            } else if filename_lower.contains("download") {
                Some("Downloads".to_string())
            } else if filename_lower.contains("backup") {
                Some("Backups".to_string())
            } else if filename_lower.contains("temp") || filename_lower.contains("tmp") {
                Some("Temporary".to_string())
            } else if filename_lower.contains("print")
                || filename_lower.contains("model")
                || filename_lower.contains("miniature")
                || filename_lower.contains("figurine")
                || filename_lower.contains("prototype")
            {
                Some("3D Print Files".to_string())
            } else {
                None
            }
        }
    }
}

// New validation and testing structures
#[derive(Debug, Serialize, Deserialize)]
pub struct RuleValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleTestResult {
    pub file_path: String,
    pub matches: bool,
    pub match_value: Option<String>,
    pub error: Option<String>,
    pub execution_time_ms: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleTestSummary {
    pub rule_id: String,
    pub total_files: usize,
    pub matched_files: usize,
    pub failed_files: usize,
    pub average_execution_time_ms: f64,
    pub results: Vec<RuleTestResult>,
}

/// Validate a rule for correctness and provide suggestions
#[tauri::command]
pub async fn validate_rule(
    rule: OrganizationRule,
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<RuleValidationResult> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut suggestions = Vec::new();

    // Validate rule structure
    if rule.id.trim().is_empty() {
        errors.push("Rule ID cannot be empty".to_string());
    }

    if rule.condition.field.trim().is_empty() {
        errors.push("Rule condition field cannot be empty".to_string());
    }

    if rule.condition.value.trim().is_empty() {
        warnings.push("Rule condition value is empty - this may not match anything".to_string());
    }

    if rule.action.target_folder.trim().is_empty() {
        errors.push("Target folder cannot be empty".to_string());
    }

    // Validate rule type and condition compatibility
    match rule.rule_type {
        RuleType::FileSize => {
            if !matches!(
                rule.condition.operator,
                ConditionOperator::GreaterThan
                    | ConditionOperator::LessThan
                    | ConditionOperator::Equals
            ) {
                warnings.push("File size rules typically work best with GreaterThan, LessThan, or Equals operators".to_string());
            }

            if rule.condition.value.parse::<u64>().is_err() {
                errors.push(
                    "File size condition value must be a valid number (in bytes)".to_string(),
                );
            } else {
                suggestions.push("Consider using human-readable units in your UI (KB, MB, GB) and convert to bytes".to_string());
            }
        }
        RuleType::CreationDate | RuleType::ModificationDate => {
            if !matches!(
                rule.condition.operator,
                ConditionOperator::GreaterThan
                    | ConditionOperator::LessThan
                    | ConditionOperator::Equals
            ) {
                warnings.push("Date rules typically work best with GreaterThan, LessThan, or Equals operators".to_string());
            }

            // Try to parse as timestamp
            if rule.condition.value.parse::<u64>().is_err() {
                // Try to parse as date string
                if chrono::DateTime::parse_from_rfc3339(&rule.condition.value).is_err() {
                    errors.push(
                        "Date condition value must be a valid timestamp or ISO 8601 date string"
                            .to_string(),
                    );
                }
            }
        }
        RuleType::FileExtension => {
            if rule.condition.value.starts_with('.') {
                suggestions.push("File extension should not include the dot (.) - it will be added automatically".to_string());
            }

            if rule.condition.value.contains(' ') {
                warnings.push("File extensions typically don't contain spaces".to_string());
            }
        }
        RuleType::FileName | RuleType::Path => {
            if matches!(
                rule.condition.operator,
                ConditionOperator::GreaterThan | ConditionOperator::LessThan
            ) {
                warnings.push(
                    "Numeric comparison operators are unusual for file name or path rules"
                        .to_string(),
                );
            }

            // Check regex patterns if using regex operator
            if matches!(rule.condition.operator, ConditionOperator::Regex) {
                if let Err(e) = regex::Regex::new(&rule.condition.value) {
                    errors.push(format!("Invalid regex pattern: {}", e));
                } else {
                    suggestions.push(
                        "Test your regex pattern with sample data to ensure it works as expected"
                            .to_string(),
                    );
                }
            }
        }
        RuleType::MimeType => {
            // Basic MIME type validation
            if !rule.condition.value.contains('/') {
                warnings.push(
                    "MIME types typically contain a '/' character (e.g., 'image/jpeg')".to_string(),
                );
            }
        }
        RuleType::FileContent => {
            if matches!(
                rule.condition.operator,
                ConditionOperator::GreaterThan | ConditionOperator::LessThan
            ) {
                warnings.push(
                    "Numeric comparison operators are unusual for file content rules".to_string(),
                );
            }

            // Check regex patterns if using regex operator
            if matches!(rule.condition.operator, ConditionOperator::Regex) {
                if let Err(e) = regex::Regex::new(&rule.condition.value) {
                    errors.push(format!("Invalid regex pattern: {}", e));
                } else {
                    suggestions.push(
                        "Test your regex pattern with sample data to ensure it works as expected"
                            .to_string(),
                    );
                }
            }

            suggestions.push("File content matching can be slow for large files. Consider using other rule types when possible".to_string());
        }
    }

    // Validate priority
    if rule.priority < 0 {
        warnings.push(
            "Negative priority values are unusual. Lower numbers = higher priority".to_string(),
        );
    }

    if rule.priority > 1000 {
        warnings.push("Very high priority values (>1000) are unusual".to_string());
    }

    // Case sensitivity suggestions
    if rule.condition.case_sensitive.is_none() {
        suggestions.push(
            "Consider specifying case_sensitive explicitly for predictable behavior".to_string(),
        );
    }

    // Action validation
    match rule.action.action_type {
        ActionType::Rename => {
            if rule.action.rename_pattern.is_none()
                || rule
                    .action
                    .rename_pattern
                    .as_ref()
                    .map_or(true, |p| p.trim().is_empty())
            {
                errors.push("Rename action requires a rename pattern".to_string());
            }
        }
        ActionType::Tag => {
            suggestions.push("Tag action stores metadata but doesn't move files. Ensure this matches your intent".to_string());
        }
        _ => {}
    }

    let valid = errors.is_empty();

    if valid {
        suggestions.push("Rule validation passed! Consider testing with sample files".to_string());
    }

    Ok(RuleValidationResult {
        valid,
        errors,
        warnings,
        suggestions,
    })
}

/// Test a rule against a set of sample files
#[tauri::command]
pub async fn test_rule_against_files(
    rule: OrganizationRule,
    file_paths: Vec<String>,
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<RuleTestSummary> {
    if file_paths.len() > 100 {
        return Err(crate::error::AppError::InvalidInput {
            message: "Too many files for rule testing (max 100)".to_string(),
        });
    }

    let mut results = Vec::new();
    let mut total_execution_time = 0.0;
    let mut matched_files = 0;
    let mut failed_files = 0;

    for file_path in &file_paths {
        let start_time = std::time::Instant::now();

        let test_result = match test_rule_against_single_file(&rule, file_path).await {
            Ok((matches, match_value)) => {
                if matches {
                    matched_files += 1;
                }

                RuleTestResult {
                    file_path: file_path.clone(),
                    matches,
                    match_value: Some(match_value),
                    error: None,
                    execution_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                }
            }
            Err(e) => {
                failed_files += 1;
                RuleTestResult {
                    file_path: file_path.clone(),
                    matches: false,
                    match_value: None,
                    error: Some(e.to_string()),
                    execution_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                }
            }
        };

        total_execution_time += test_result.execution_time_ms;
        results.push(test_result);
    }

    let average_execution_time = if file_paths.is_empty() {
        0.0
    } else {
        total_execution_time / file_paths.len() as f64
    };

    Ok(RuleTestSummary {
        rule_id: rule.id.clone(),
        total_files: file_paths.len(),
        matched_files,
        failed_files,
        average_execution_time_ms: average_execution_time,
        results,
    })
}

/// Helper function to test a rule against a single file
async fn test_rule_against_single_file(
    rule: &OrganizationRule,
    file_path: &str,
) -> Result<(bool, String)> {
    if !std::path::Path::new(file_path).exists() {
        return Err(crate::error::AppError::FileNotFound {
            path: file_path.to_string(),
        });
    }

    let path = std::path::Path::new(file_path);

    let (matches, match_value) = match rule.rule_type {
        RuleType::FileExtension => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            (evaluate_condition(&rule.condition, ext), ext.to_string())
        }
        RuleType::FileName => {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            (evaluate_condition(&rule.condition, name), name.to_string())
        }
        RuleType::Path => (
            evaluate_condition(&rule.condition, file_path),
            file_path.to_string(),
        ),
        RuleType::MimeType => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            (evaluate_condition(&rule.condition, &mime), mime)
        }
        RuleType::FileSize => {
            let metadata = tokio::fs::metadata(file_path).await?;
            let size_str = metadata.len().to_string();
            (evaluate_condition(&rule.condition, &size_str), size_str)
        }
        RuleType::FileContent => {
            let content = tokio::fs::read_to_string(file_path).await.map_err(|_| {
                crate::error::AppError::InvalidInput {
                    message: format!("Could not read file content: {}", file_path),
                }
            })?;

            // Limit content size for testing
            let preview = if content.len() > 10240 {
                &content[..10240]
            } else {
                &content
            };

            (
                evaluate_condition(&rule.condition, preview),
                format!("Content preview ({} chars)", preview.len()),
            )
        }
        RuleType::CreationDate => {
            let metadata = tokio::fs::metadata(file_path).await?;
            let created = metadata
                .created()
                .map_err(|_| crate::error::AppError::InvalidInput {
                    message: "Could not get file creation time".to_string(),
                })?;

            let duration = created.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
                crate::error::AppError::InvalidInput {
                    message: "Invalid file creation time".to_string(),
                }
            })?;

            let timestamp_str = duration.as_secs().to_string();
            (
                evaluate_condition(&rule.condition, &timestamp_str),
                timestamp_str,
            )
        }
        RuleType::ModificationDate => {
            let metadata = tokio::fs::metadata(file_path).await?;
            let modified =
                metadata
                    .modified()
                    .map_err(|_| crate::error::AppError::InvalidInput {
                        message: "Could not get file modification time".to_string(),
                    })?;

            let duration = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| crate::error::AppError::InvalidInput {
                    message: "Invalid file modification time".to_string(),
                })?;

            let timestamp_str = duration.as_secs().to_string();
            (
                evaluate_condition(&rule.condition, &timestamp_str),
                timestamp_str,
            )
        }
    };

    Ok((matches, match_value))
}
