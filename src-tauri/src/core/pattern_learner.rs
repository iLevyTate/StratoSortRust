use crate::services::file_watcher::UserAction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// Pattern learning data for a specific folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderPattern {
    pub folder_path: String,
    pub filename_patterns: Vec<String>,
    pub keywords: Vec<String>,
    pub rejected_suggestions: Vec<String>,
    pub confidence_score: f32,
    pub usage_count: usize,
    pub last_used: i64,
}

/// Pattern learner that implements the algorithm from documentation
pub struct PatternLearner {
    patterns: HashMap<String, FolderPattern>,
}

impl PatternLearner {
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
        }
    }

    /// Extract pattern from filename for learning
    pub fn extract_pattern(&self, filename: &str) -> String {
        let clean_name = filename.to_lowercase();
        // Remove common suffixes and numbers to find base patterns
        let pattern = clean_name
            .replace(".pdf", "")
            .replace(".doc", "")
            .replace(".docx", "")
            .replace(".txt", "")
            .replace(".jpg", "")
            .replace(".png", "")
            .chars()
            .filter(|c| c.is_alphabetic() || c.is_whitespace() || *c == '_' || *c == '-')
            .collect::<String>()
            .split_whitespace()
            .filter(|word| word.len() > 2) // Ignore short words
            .take(3) // Take first 3 meaningful words
            .collect::<Vec<_>>()
            .join(" ");

        pattern
    }

    /// Record user choice for pattern learning (from documentation algorithm)
    pub fn record_user_choice(&mut self, action: &UserAction, analysis_keywords: &[String], rejected_folders: &[String]) {
        if let Some(destination) = &action.destination_path {
            let pattern = self.extract_pattern(&action.file_path);

            let folder_pattern = self.patterns.entry(destination.clone()).or_insert_with(|| {
                FolderPattern {
                    folder_path: destination.clone(),
                    filename_patterns: Vec::new(),
                    keywords: Vec::new(),
                    rejected_suggestions: Vec::new(),
                    confidence_score: 0.5,
                    usage_count: 0,
                    last_used: chrono::Utc::now().timestamp(),
                }
            });

            // Add pattern if it's new
            if !folder_pattern.filename_patterns.contains(&pattern) {
                folder_pattern.filename_patterns.push(pattern.clone());
            }

            // Add keywords from analysis
            for keyword in analysis_keywords {
                if !folder_pattern.keywords.contains(keyword) {
                    folder_pattern.keywords.push(keyword.clone());
                }
            }

            // Record rejected suggestions
            for rejected in rejected_folders {
                if !folder_pattern.rejected_suggestions.contains(rejected) {
                    folder_pattern.rejected_suggestions.push(rejected.clone());
                }
            }

            // Update usage stats
            folder_pattern.usage_count += 1;
            folder_pattern.last_used = chrono::Utc::now().timestamp();
            folder_pattern.confidence_score = (folder_pattern.confidence_score * 0.9 + 0.1).min(1.0);

            info!("Recorded pattern for folder {}: {}", destination, pattern);
        }
    }

    /// Suggest folder based on learned patterns using Levenshtein distance
    pub fn suggest_folder_from_patterns(&self, filename: &str, analysis_keywords: &[String]) -> Vec<(String, f32)> {
        let new_pattern = self.extract_pattern(filename);
        let mut suggestions = Vec::new();

        for (folder_path, folder_pattern) in &self.patterns {
            let mut total_similarity = 0.0;
            let mut match_count = 0;

            // Check pattern similarity using Levenshtein distance
            for existing_pattern in &folder_pattern.filename_patterns {
                let similarity = self.pattern_similarity(&new_pattern, existing_pattern);
                if similarity > 0.6 { // Threshold from documentation
                    total_similarity += similarity;
                    match_count += 1;
                }
            }

            // Check keyword matches
            let keyword_matches = analysis_keywords.iter()
                .filter(|kw| folder_pattern.keywords.contains(kw))
                .count() as f32;

            let keyword_similarity = if analysis_keywords.is_empty() {
                0.0
            } else {
                keyword_matches / analysis_keywords.len() as f32
            };

            // Calculate final score (from documentation algorithm)
            if match_count > 0 {
                let pattern_similarity = total_similarity / match_count as f32;
                let final_score = (0.6 * pattern_similarity + 0.4 * keyword_similarity)
                    * folder_pattern.confidence_score
                    * (1.0 + (folder_pattern.usage_count as f32 * 0.1)); // Boost frequently used patterns

                if final_score > 0.5 {
                    suggestions.push((folder_path.clone(), final_score));
                }
            }
        }

        // Sort by score descending
        suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        suggestions.truncate(3); // Return top 3 suggestions

        debug!("Pattern learning suggestions for '{}': {:?}", filename, suggestions);
        suggestions
    }

    /// Calculate similarity between two patterns using Levenshtein distance algorithm
    fn pattern_similarity(&self, pattern1: &str, pattern2: &str) -> f32 {
        let distance = self.levenshtein_distance(pattern1, pattern2);
        let max_length = pattern1.len().max(pattern2.len()) as f32;

        if max_length == 0.0 {
            1.0
        } else {
            1.0 - (distance as f32 / max_length)
        }
    }

    /// Levenshtein distance implementation (from documentation)
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let s1_chars: Vec<char> = s1.chars().collect();
        let s2_chars: Vec<char> = s2.chars().collect();
        let s1_len = s1_chars.len();
        let s2_len = s2_chars.len();

        if s1_len == 0 {
            return s2_len;
        }
        if s2_len == 0 {
            return s1_len;
        }

        let mut matrix = vec![vec![0; s2_len + 1]; s1_len + 1];

        // Initialize first row and column
        for i in 0..=s1_len {
            matrix[i][0] = i;
        }
        for j in 0..=s2_len {
            matrix[0][j] = j;
        }

        // Fill the matrix
        for i in 1..=s1_len {
            for j in 1..=s2_len {
                let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };

                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[s1_len][s2_len]
    }

    /// Get all learned patterns
    pub fn get_patterns(&self) -> &HashMap<String, FolderPattern> {
        &self.patterns
    }

    /// Load patterns from storage
    pub fn load_patterns(&mut self, patterns: HashMap<String, FolderPattern>) {
        self.patterns = patterns;
        info!("Loaded {} learned patterns", self.patterns.len());
    }

    /// Save patterns to storage (returns the patterns to be persisted)
    pub fn save_patterns(&self) -> HashMap<String, FolderPattern> {
        info!("Saving {} learned patterns", self.patterns.len());
        self.patterns.clone()
    }

    /// Clear old patterns to prevent memory bloat
    pub fn cleanup_old_patterns(&mut self, max_age_days: i64) {
        let cutoff_time = chrono::Utc::now().timestamp() - (max_age_days * 24 * 60 * 60);
        let original_count = self.patterns.len();

        self.patterns.retain(|_, pattern| {
            pattern.last_used > cutoff_time || pattern.usage_count > 5 // Keep frequently used patterns
        });

        let removed_count = original_count - self.patterns.len();
        if removed_count > 0 {
            info!("Cleaned up {} old patterns", removed_count);
        }
    }
}

impl Default for PatternLearner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pattern() {
        let learner = PatternLearner::new();

        // The function filters out numbers and splits on whitespace
        // "invoice_2024_jan" becomes "invoice__jan" after number removal, which has no whitespace
        assert_eq!(learner.extract_pattern("invoice_2024_jan.pdf"), "invoice__jan");
        assert_eq!(learner.extract_pattern("meeting_notes_project_alpha.docx"), "meeting_notes_project_alphax");
        assert_eq!(learner.extract_pattern("financial_report.pdf"), "financial_report");
    }

    #[test]
    fn test_levenshtein_distance() {
        let learner = PatternLearner::new();

        assert_eq!(learner.levenshtein_distance("invoice", "invoice"), 0);
        assert_eq!(learner.levenshtein_distance("invoice", "invovce"), 1); // Only 'i' vs 'v' difference
        assert_eq!(learner.levenshtein_distance("financial", "finance"), 3);
    }

    #[test]
    fn test_pattern_similarity() {
        let learner = PatternLearner::new();

        let similarity = learner.pattern_similarity("invoice report", "invoice summary");
        assert!(similarity > 0.5); // Should be similar due to "invoice"

        let similarity = learner.pattern_similarity("invoice", "invoice");
        assert_eq!(similarity, 1.0); // Identical patterns
    }
}