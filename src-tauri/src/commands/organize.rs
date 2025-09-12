use crate::{
    ai::OrganizationSuggestion,
    core::SmartFolder,
    error::Result,
    state::AppState,
    utils::security::{validate_and_sanitize_path_legacy as validate_and_sanitize_path, is_path_allowed},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct SmartFolderInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub rules: Vec<FolderRule>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub file_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SmartFolderResponse {
    pub id: String,
    pub name: String,
    pub path: String,
    pub query: String,
    pub created_at: i64,
    pub file_count: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FolderRule {
    pub rule_type: String,
    pub operator: String,
    pub value: String,
    pub case_sensitive: bool,
}

#[tauri::command]
pub async fn suggest_organization(
    files: Vec<String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<OrganizationSuggestion>> {
    if files.is_empty() {
        return Ok(vec![]);
    }
    
    // Combine Organizer (rules-based) with AI suggestions
    // First, try rules-based suggestions using Organizer
    let mut suggestions = state.organizer.organize_files(files.clone()).await.unwrap_or_default();
    
    // Then, get AI suggestions and merge
    let smart_folders = state.database.list_smart_folders().await.unwrap_or_default();
    match state.ai_service.suggest_organization(files, smart_folders).await {
        Ok(mut ai) => {
            suggestions.append(&mut ai);
        }
        Err(e) => {
            tracing::warn!("AI suggest_organization failed, using rules-only: {}", e);
        }
    }
    
    // Enhance suggestions with smart folder matches
    let enhanced = enhance_suggestions_with_smart_folders(suggestions, &state).await?;
    
    Ok(enhanced)
}

#[tauri::command]
pub async fn apply_organization(
    suggestions: Vec<OrganizationSuggestion>,
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<ApplyResult> {
    let mut successful = 0;
    let mut failed = 0;
    let mut errors = Vec::new();
    
    for suggestion in suggestions {
        // Validate and sanitize source path
        let sanitized_source = match validate_and_sanitize_path(&suggestion.source_path, &app) {
            Ok(path) => path,
            Err(e) => {
                failed += 1;
                errors.push(format!("Invalid source path '{}': {}", suggestion.source_path, e));
                continue;
            }
        };
        
        // Validate and sanitize target folder path
        let sanitized_target_dir = match validate_and_sanitize_path(&suggestion.target_folder, &app) {
            Ok(path) => path,
            Err(e) => {
                failed += 1;
                errors.push(format!("Invalid target folder '{}': {}", suggestion.target_folder, e));
                continue;
            }
        };
        
        // Ensure target directory exists
        if let Err(e) = tokio::fs::create_dir_all(&sanitized_target_dir).await {
            failed += 1;
            errors.push(format!("Failed to create {}: {}", sanitized_target_dir.display(), e));
            continue;
        }
        
        // Determine target path
        let file_name = sanitized_source.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| crate::error::AppError::InvalidInput { 
                message: "Source path has no valid file name".into() 
            })?;
        let target = sanitized_target_dir.join(file_name);
        
        // Final validation of the target path
        if !is_path_allowed(&target, &app)? {
            failed += 1;
            errors.push(format!("Target path not allowed: {}", target.display()));
            continue;
        }
        
        // Move the file
        match tokio::fs::rename(&sanitized_source, &target).await {
            Ok(_) => {
                successful += 1;
                
                // Record for undo
                state.undo_redo.record_move(
                    &suggestion.source_path,
                    &target.display().to_string()
                ).await?;
            }
            Err(e) => {
                // Try copy + delete as fallback
                if tokio::fs::copy(&sanitized_source, &target).await.is_ok() {
                    if tokio::fs::remove_file(&sanitized_source).await.is_ok() {
                        successful += 1;
                        state.undo_redo.record_move(
                            &suggestion.source_path,
                            &target.display().to_string()
                        ).await?;
                    } else {
                        failed += 1;
                        errors.push(format!("Failed to remove source: {}", e));
                    }
                } else {
                    failed += 1;
                    errors.push(format!("Failed to move {}: {}", sanitized_source.display(), e));
                }
            }
        }
    }
    
    Ok(ApplyResult {
        successful,
        failed,
        errors,
    })
}

#[tauri::command]
pub async fn create_smart_folder(
    name: String,
    path: String,
    query: String,
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<SmartFolderResponse> {
    // Validate and sanitize the folder path
    let sanitized_path = validate_and_sanitize_path(&path, &app)?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    
    // Convert simple query to basic rule
    let rules = vec![FolderRule {
        rule_type: "file_extension".to_string(),
        operator: "matches".to_string(),
        value: query.clone(),
        case_sensitive: false,
    }];
    
    let smart_folder = SmartFolder {
        id: id.clone(),
        name: name.clone(),
        path: sanitized_path.to_string_lossy().to_string(),
        rules: rules.clone(),
        icon: None,
        color: None,
        created_at: now,
        updated_at: now,
    };
    
    // Save to database
    state.smart_folders.create(smart_folder).await?;
    
    // Count matching files using sanitized path
    let file_count = count_matching_files(&sanitized_path.to_string_lossy().to_string(), &rules).await?;
    
    // Return simplified structure that matches frontend schema
    Ok(SmartFolderResponse {
        id,
        name,
        path: sanitized_path.to_string_lossy().to_string(),
        query,
        created_at: now,
        file_count: Some(file_count),
    })
}

#[tauri::command]
pub async fn get_smart_folders(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<SmartFolderInfo>> {
    let folders = state.smart_folders.get_all().await?;
    
    let mut infos = Vec::new();
    for folder in folders {
        let file_count = count_matching_files(&folder.path, &folder.rules).await?;
        
        infos.push(SmartFolderInfo {
            id: folder.id,
            name: folder.name,
            path: folder.path,
            rules: folder.rules,
            icon: folder.icon,
            color: folder.color,
            created_at: folder.created_at,
            updated_at: folder.updated_at,
            file_count,
        });
    }
    
    Ok(infos)
}

#[tauri::command]
pub async fn update_smart_folder(
    id: String,
    updates: SmartFolderUpdate,
    app: AppHandle,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<SmartFolderInfo> {
    let mut folder = state.smart_folders.get(&id).await?
        .ok_or_else(|| crate::error::AppError::InvalidInput {
            message: format!("Smart folder {} not found", id),
        })?;
    
    // Apply updates
    if let Some(name) = updates.name {
        folder.name = name;
    }
    if let Some(path) = updates.path {
        // Validate and sanitize the new path
        let sanitized_path = validate_and_sanitize_path(&path, &app)?;
        folder.path = sanitized_path.to_string_lossy().to_string();
    }
    if let Some(rules) = updates.rules {
        folder.rules = rules;
    }
    if let Some(icon) = updates.icon {
        folder.icon = Some(icon);
    }
    if let Some(color) = updates.color {
        folder.color = Some(color);
    }
    
    folder.updated_at = chrono::Utc::now().timestamp();
    
    // Save updates
    state.smart_folders.update(folder.clone()).await?;
    
    // Count files
    let file_count = count_matching_files(&folder.path, &folder.rules).await?;
    
    Ok(SmartFolderInfo {
        id: folder.id,
        name: folder.name,
        path: folder.path,
        rules: folder.rules,
        icon: folder.icon,
        color: folder.color,
        created_at: folder.created_at,
        updated_at: folder.updated_at,
        file_count,
    })
}

#[tauri::command]
pub async fn delete_smart_folder(
    id: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<()> {
    state.smart_folders.delete(&id).await?;
    Ok(())
}

#[tauri::command]
pub async fn match_to_folders(
    files: Vec<String>,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<FolderMatch>> {
    let folders = state.smart_folders.get_all().await?;
    let mut matches = Vec::new();
    
    for file_path in files {
        let file_name = PathBuf::from(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        
        for folder in &folders {
            if matches_rules(&file_name, &file_path, &folder.rules) {
                matches.push(FolderMatch {
                    file_path: file_path.clone(),
                    folder_id: folder.id.clone(),
                    folder_name: folder.name.clone(),
                    folder_path: folder.path.clone(),
                    confidence: calculate_confidence(&folder.rules),
                });
                break; // Only match to first folder
            }
        }
    }
    
    Ok(matches)
}

async fn enhance_suggestions_with_smart_folders(
    mut suggestions: Vec<OrganizationSuggestion>,
    state: &AppState,
) -> Result<Vec<OrganizationSuggestion>> {
    let folders = state.smart_folders.get_all().await?;
    
    for suggestion in &mut suggestions {
        let file_name = PathBuf::from(&suggestion.source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        
        for folder in &folders {
            if matches_rules(&file_name, &suggestion.source_path, &folder.rules) {
                suggestion.target_folder = folder.path.clone();
                suggestion.confidence = suggestion.confidence.max(0.8);
                break;
            }
        }
    }
    
    Ok(suggestions)
}

fn matches_rules(file_name: &str, file_path: &str, rules: &[FolderRule]) -> bool {
    for rule in rules {
        let matches = match rule.rule_type.as_str() {
            "extension" => {
                let path_buf = PathBuf::from(file_name);
                let ext = path_buf.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                match_value(ext, &rule.operator, &rule.value, rule.case_sensitive)
            }
            "pattern" => {
                match_value(file_name, &rule.operator, &rule.value, rule.case_sensitive)
            }
            "path" => {
                match_value(file_path, &rule.operator, &rule.value, rule.case_sensitive)
            }
            _ => false,
        };
        
        if !matches {
            return false; // All rules must match
        }
    }
    
    true
}

fn match_value(value: &str, operator: &str, pattern: &str, case_sensitive: bool) -> bool {
    let (val, pat) = if case_sensitive {
        (value.to_string(), pattern.to_string())
    } else {
        (value.to_lowercase(), pattern.to_lowercase())
    };
    
    match operator {
        "equals" => val == pat,
        "contains" => val.contains(&pat),
        "starts_with" => val.starts_with(&pat),
        "ends_with" => val.ends_with(&pat),
        _ => false,
    }
}

fn calculate_confidence(rules: &[FolderRule]) -> f32 {
    // More specific rules = higher confidence
    let base_confidence = 0.5;
    let rule_bonus = 0.1 * rules.len() as f32;
    (base_confidence + rule_bonus).min(1.0)
}

async fn count_matching_files(path: &str, _rules: &[FolderRule]) -> Result<usize> {
    use walkdir::WalkDir;
    use std::path::Path;
    
    let mut count = 0;
    let search_path = Path::new(path);
    
    // Additional security check - ensure the path doesn't contain dangerous patterns
    let path_str = search_path.to_string_lossy();
    if path_str.contains("..") || path_str.contains("~") || path_str.len() > 500 {
        return Err(crate::error::AppError::SecurityError {
            message: "Invalid path for file counting".to_string(),
        });
    }
    
    if !search_path.exists() {
        return Ok(0);
    }
    
    // Canonicalize path to prevent directory traversal
    let canonical_path = match search_path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Ok(0), // Path doesn't exist or can't be accessed
    };
    
    for entry in WalkDir::new(&canonical_path)
        .max_depth(3) // Limit depth to prevent excessive scanning
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() {
            count += 1;
            
            // Limit to prevent excessive processing
            if count > 10000 {
                break;
            }
        }
    }
    
    Ok(count)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyResult {
    pub successful: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SmartFolderUpdate {
    pub name: Option<String>,
    pub path: Option<String>,
    pub rules: Option<Vec<FolderRule>>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderMatch {
    pub file_path: String,
    pub folder_id: String,
    pub folder_name: String,
    pub folder_path: String,
    pub confidence: f32,
}