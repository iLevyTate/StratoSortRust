use crate::{
    core::smart_folders::SmartFolder,
    error::{AppError, Result},
    storage::Database,
    ai::ollama::DocumentAnalysisEnhanced,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

/// Type of match found
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchType {
    Semantic,      // Based on embedding similarity
    RuleBased,     // Based on folder rules
    Pattern,       // Based on pattern recognition
    Historical,    // Based on historical patterns
    Hybrid,        // Combination of multiple methods
}

/// Represents a folder match with confidence score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderMatch {
    pub folder: SmartFolder,
    pub confidence: f32,
    pub match_type: MatchType,
    pub reason: String,
    pub similarity_details: Option<SimilarityDetails>,
}

/// Details about similarity calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityDetails {
    pub cosine_similarity: f32,
    pub euclidean_distance: f32,
    pub matching_keywords: Vec<String>,
    pub category_match: bool,
}

/// Service for semantic matching using embeddings and LLM
pub struct SemanticMatcher {
    database: Arc<Database>,
    ollama_client: Option<Arc<crate::ai::ollama::OllamaClient>>,
}

impl SemanticMatcher {
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            ollama_client: None,
        }
    }

    /// Create with Ollama client for enhanced matching
    pub fn with_ollama(database: Arc<Database>, ollama_client: Arc<crate::ai::ollama::OllamaClient>) -> Self {
        Self {
            database,
            ollama_client: Some(ollama_client),
        }
    }

    /// Find semantically similar folders based on embeddings
    pub async fn find_similar_folders(
        &self,
        file_embedding: &[f32],
        folders: &[SmartFolder],
        threshold: f32,
    ) -> Result<Vec<FolderMatch>> {
        let mut matches = Vec::new();

        for folder in folders {
            // Get folder's representative embedding
            match self.get_folder_embedding(&folder.id).await {
                Ok(folder_embedding) => {
                    // Calculate similarity
                    let similarity = self.cosine_similarity(file_embedding, &folder_embedding);

                    if similarity >= threshold {
                        let distance = self.euclidean_distance(file_embedding, &folder_embedding);

                        matches.push(FolderMatch {
                            folder: folder.clone(),
                            confidence: similarity,
                            match_type: MatchType::Semantic,
                            reason: format!("Semantic similarity: {:.2}%", similarity * 100.0),
                            similarity_details: Some(SimilarityDetails {
                                cosine_similarity: similarity,
                                euclidean_distance: distance,
                                matching_keywords: vec![],
                                category_match: false,
                            }),
                        });

                        debug!(
                            "Found semantic match: {} with similarity {:.3}",
                            folder.name, similarity
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to get embedding for folder {}: {}", folder.id, e);
                }
            }
        }

        // Sort by confidence
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        Ok(matches)
    }

    /// Find folders using hybrid matching (semantic + LLM + rules + patterns)
    pub async fn find_best_matches(
        &self,
        file_path: &str,
        file_embedding: Option<&[f32]>,
        file_analysis: &crate::ai::FileAnalysis,
        folders: &[SmartFolder],
    ) -> Result<Vec<FolderMatch>> {
        let mut all_matches = Vec::new();

        // 1. LLM-based creative matching if available
        if let Some(ref ollama) = self.ollama_client {
            match ollama.suggest_folders_creative(file_analysis, folders).await {
                Ok(suggestions) => {
                    for suggestion in suggestions {
                        if let Some(folder) = folders.iter().find(|f| f.name == suggestion.folder_name) {
                            all_matches.push(FolderMatch {
                                folder: folder.clone(),
                                confidence: suggestion.confidence,
                                match_type: MatchType::Semantic,
                                reason: suggestion.reasoning,
                                similarity_details: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    warn!("LLM folder suggestion failed: {}", e);
                }
            }
        }

        // 2. Semantic matching if embedding is available
        if let Some(embedding) = file_embedding {
            let semantic_matches = self.find_similar_folders(embedding, folders, 0.6).await?;
            all_matches.extend(semantic_matches);
        }

        // 3. Category-based matching
        for folder in folders {
            if let Some(match_result) = self.match_by_category(file_analysis, folder).await? {
                all_matches.push(match_result);
            }
        }

        // 4. Keyword-based matching
        for folder in folders {
            if let Some(match_result) = self.match_by_keywords(file_analysis, folder).await? {
                all_matches.push(match_result);
            }
        }

        // 5. Historical pattern matching
        if let Ok(historical_matches) = self.find_historical_patterns(file_path).await {
            all_matches.extend(historical_matches);
        }

        // Combine and deduplicate matches
        let combined = self.combine_matches(all_matches);

        Ok(combined)
    }

    /// Enhanced matching with LLM analysis
    pub async fn find_matches_with_llm_analysis(
        &self,
        enhanced_analysis: &DocumentAnalysisEnhanced,
        folders: &[SmartFolder],
    ) -> Result<Vec<FolderMatch>> {
        let mut matches = Vec::new();

        // Find folder by category name from LLM
        if let Some(folder) = folders.iter().find(|f| f.name == enhanced_analysis.category) {
            matches.push(FolderMatch {
                folder: folder.clone(),
                confidence: enhanced_analysis.confidence,
                match_type: MatchType::Semantic,
                reason: format!("LLM categorized as: {}", enhanced_analysis.category),
                similarity_details: None,
            });
        }

        // Match by document type
        for folder in folders {
            let folder_name_lower = folder.name.to_lowercase();
            let doc_type_lower = enhanced_analysis.document_type.to_lowercase();

            if folder_name_lower.contains(&doc_type_lower) || doc_type_lower.contains(&folder_name_lower) {
                matches.push(FolderMatch {
                    folder: folder.clone(),
                    confidence: 0.75,
                    match_type: MatchType::Pattern,
                    reason: format!("Document type match: {}", enhanced_analysis.document_type),
                    similarity_details: None,
                });
            }
        }

        // Match by client or project
        if let Some(ref client) = enhanced_analysis.client {
            for folder in folders {
                if folder.name.to_lowercase().contains(&client.to_lowercase()) {
                    matches.push(FolderMatch {
                        folder: folder.clone(),
                        confidence: 0.85,
                        match_type: MatchType::Pattern,
                        reason: format!("Client match: {}", client),
                        similarity_details: None,
                    });
                }
            }
        }

        if let Some(ref project) = enhanced_analysis.project {
            for folder in folders {
                if folder.name.to_lowercase().contains(&project.to_lowercase()) {
                    matches.push(FolderMatch {
                        folder: folder.clone(),
                        confidence: 0.85,
                        match_type: MatchType::Pattern,
                        reason: format!("Project match: {}", project),
                        similarity_details: None,
                    });
                }
            }
        }

        Ok(self.combine_matches(matches))
    }

    /// Match folders based on category
    async fn match_by_category(
        &self,
        analysis: &crate::ai::FileAnalysis,
        folder: &SmartFolder,
    ) -> Result<Option<FolderMatch>> {
        // Check if folder name or description contains the category
        let folder_text = format!("{} {}",
            folder.name.to_lowercase(),
            folder.description.as_ref().unwrap_or(&String::new()).to_lowercase()
        );

        let category_lower = analysis.category.to_lowercase();

        if folder_text.contains(&category_lower) || category_lower.contains(&folder.name.to_lowercase()) {
            return Ok(Some(FolderMatch {
                folder: folder.clone(),
                confidence: 0.75,
                match_type: MatchType::Pattern,
                reason: format!("Category match: {}", analysis.category),
                similarity_details: Some(SimilarityDetails {
                    cosine_similarity: 0.0,
                    euclidean_distance: 0.0,
                    matching_keywords: vec![],
                    category_match: true,
                }),
            }));
        }

        Ok(None)
    }

    /// Match folders based on keywords/tags
    async fn match_by_keywords(
        &self,
        analysis: &crate::ai::FileAnalysis,
        folder: &SmartFolder,
    ) -> Result<Option<FolderMatch>> {
        let folder_text = format!("{} {}",
            folder.name.to_lowercase(),
            folder.description.as_ref().unwrap_or(&String::new()).to_lowercase()
        );

        let mut matching_keywords = Vec::new();

        for tag in &analysis.tags {
            if folder_text.contains(&tag.to_lowercase()) {
                matching_keywords.push(tag.clone());
            }
        }

        if !matching_keywords.is_empty() {
            let confidence = (matching_keywords.len() as f32 / analysis.tags.len() as f32).min(0.9);

            return Ok(Some(FolderMatch {
                folder: folder.clone(),
                confidence,
                match_type: MatchType::Pattern,
                reason: format!("Keywords match: {}", matching_keywords.join(", ")),
                similarity_details: Some(SimilarityDetails {
                    cosine_similarity: 0.0,
                    euclidean_distance: 0.0,
                    matching_keywords,
                    category_match: false,
                }),
            }));
        }

        Ok(None)
    }

    /// Find historical patterns for a file
    async fn find_historical_patterns(&self, file_path: &str) -> Result<Vec<FolderMatch>> {
        // Query database for similar files that were previously organized
        let query = r#"
            SELECT DISTINCT
                target_folder,
                COUNT(*) as occurrence_count,
                AVG(confidence) as avg_confidence
            FROM file_organization_history
            WHERE
                source_path LIKE ? OR
                file_type = (SELECT file_type FROM file_analysis WHERE path = ? LIMIT 1) OR
                category = (SELECT category FROM file_analysis WHERE path = ? LIMIT 1)
            GROUP BY target_folder
            ORDER BY occurrence_count DESC, avg_confidence DESC
            LIMIT 5
        "#;

        // Get file pattern (e.g., same directory, similar name)
        let pattern = std::path::Path::new(file_path)
            .parent()
            .and_then(|p| p.to_str())
            .map(|p| format!("{}%", p))
            .unwrap_or_else(|| "%".to_string());

        let rows = sqlx::query(query)
            .bind(&pattern)
            .bind(file_path)
            .bind(file_path)
            .fetch_all(self.database.pool())
            .await
            .map_err(|e| AppError::DatabaseError {
                message: format!("Failed to query historical patterns: {}", e),
            })?;

        let mut matches = Vec::new();

        for row in rows {
            if let (Ok(target_folder), Ok(count), Ok(confidence)) = (
                sqlx::Row::try_get::<String, _>(&row, "target_folder"),
                sqlx::Row::try_get::<i32, _>(&row, "occurrence_count"),
                sqlx::Row::try_get::<f32, _>(&row, "avg_confidence"),
            ) {
                // Load the smart folder
                if let Ok(Some(folder)) = self.database.get_smart_folder(&target_folder).await {
                    matches.push(FolderMatch {
                        folder,
                        confidence: (confidence * 0.8).min(0.95), // Scale down historical confidence
                        match_type: MatchType::Historical,
                        reason: format!("Historical pattern: {} similar files organized here", count),
                        similarity_details: None,
                    });
                }
            }
        }

        Ok(matches)
    }

    /// Get or compute folder embedding
    async fn get_folder_embedding(&self, folder_id: &str) -> Result<Vec<f32>> {
        // Try to get cached embedding first
        if let Ok(Some(embedding)) = self.database.get_embedding(folder_id).await {
            return Ok(embedding);
        }

        // Compute embedding from folder's representative files
        let embedding = self.compute_folder_embedding(folder_id).await?;

        // Cache for future use - using save_embedding with folder prefix
        let folder_path = format!("folder:{}", folder_id);
        let _ = self.database.save_embedding(&folder_path, &embedding, Some("folder_embedding")).await;

        Ok(embedding)
    }

    /// Compute folder embedding from its files
    async fn compute_folder_embedding(&self, folder_id: &str) -> Result<Vec<f32>> {
        // Get embeddings of files in this folder
        let query = r#"
            SELECT embedding
            FROM file_embeddings
            WHERE file_path IN (
                SELECT path FROM files WHERE folder_id = ?
            )
            LIMIT 20
        "#;

        let rows = sqlx::query(query)
            .bind(folder_id)
            .fetch_all(self.database.pool())
            .await?;

        if rows.is_empty() {
            return Err(AppError::NotFound {
                message: format!("No embeddings found for folder {}", folder_id),
            });
        }

        // Average the embeddings to create folder representation
        let mut avg_embedding = vec![0.0; 768]; // Assuming 768-dim embeddings
        let count = rows.len() as f32;

        for row in rows {
            if let Ok(embedding_bytes) = sqlx::Row::try_get::<Vec<u8>, _>(&row, "embedding") {
                let embedding = self.bytes_to_embedding(&embedding_bytes)?;
                for (i, val) in embedding.iter().enumerate() {
                    if i < avg_embedding.len() {
                        avg_embedding[i] += val / count;
                    }
                }
            }
        }

        // Normalize the averaged embedding
        self.normalize_embedding(&mut avg_embedding);

        Ok(avg_embedding)
    }

    /// Calculate cosine similarity between two embeddings
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }

        (dot_product / (magnitude_a * magnitude_b)).max(0.0).min(1.0)
    }

    /// Calculate Euclidean distance between two embeddings
    fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::MAX;
        }

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Normalize an embedding vector
    fn normalize_embedding(&self, embedding: &mut [f32]) {
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for val in embedding.iter_mut() {
                *val /= magnitude;
            }
        }
    }

    /// Convert bytes to embedding vector
    fn bytes_to_embedding(&self, bytes: &[u8]) -> Result<Vec<f32>> {
        if bytes.len() % 4 != 0 {
            return Err(AppError::ProcessingError {
                message: "Invalid embedding byte length".to_string(),
            });
        }

        let mut embedding = Vec::with_capacity(bytes.len() / 4);
        for chunk in bytes.chunks(4) {
            let float_bytes: [u8; 4] = chunk.try_into().map_err(|_| AppError::ProcessingError {
                message: "Failed to convert bytes to float".to_string(),
            })?;
            embedding.push(f32::from_le_bytes(float_bytes));
        }

        Ok(embedding)
    }

    /// Combine and deduplicate matches from different sources
    fn combine_matches(&self, matches: Vec<FolderMatch>) -> Vec<FolderMatch> {
        use std::collections::HashMap;

        let mut combined: HashMap<String, FolderMatch> = HashMap::new();

        for match_item in matches {
            let folder_id = &match_item.folder.id;

            combined
                .entry(folder_id.clone())
                .and_modify(|existing| {
                    // Combine confidence scores with weighted average
                    let weight = match match_item.match_type {
                        MatchType::Semantic => 1.0,
                        MatchType::Historical => 0.9,
                        MatchType::Pattern => 0.8,
                        MatchType::RuleBased => 0.7,
                        MatchType::Hybrid => 1.0,
                    };

                    let existing_weight = match existing.match_type {
                        MatchType::Semantic => 1.0,
                        MatchType::Historical => 0.9,
                        MatchType::Pattern => 0.8,
                        MatchType::RuleBased => 0.7,
                        MatchType::Hybrid => 1.0,
                    };

                    let total_weight = weight + existing_weight;
                    existing.confidence = (existing.confidence * existing_weight + match_item.confidence * weight) / total_weight;
                    existing.match_type = MatchType::Hybrid;
                    existing.reason = format!("{} + {}", existing.reason, match_item.reason);
                })
                .or_insert(match_item);
        }

        let mut result: Vec<FolderMatch> = combined.into_values().collect();
        result.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        result
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let matcher = SemanticMatcher::new(Arc::new(Database::new_mock()));

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(matcher.cosine_similarity(&a, &b), 1.0);

        let c = vec![0.0, 1.0, 0.0];
        assert_eq!(matcher.cosine_similarity(&a, &c), 0.0);
    }

    #[test]
    fn test_euclidean_distance() {
        let matcher = SemanticMatcher::new(Arc::new(Database::new_mock()));

        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(matcher.euclidean_distance(&a, &b), 0.0);

        let c = vec![4.0, 6.0, 3.0];
        let distance = matcher.euclidean_distance(&a, &c);
        assert!(distance > 0.0);
    }
}
*/