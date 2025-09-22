use std::collections::HashMap;
use tracing::debug;

/// Provides intelligent fallback mechanisms when AI services are unavailable
pub struct IntelligentFallbacks;

impl IntelligentFallbacks {
    /// Get intelligent category based on filename patterns - EXACT LOGIC from original codebase
    pub fn get_intelligent_category(file_name: &str, extension: &str) -> String {
        let lower_file_name = file_name.to_lowercase();
        let mut category_scores: HashMap<String, f32> = HashMap::new();

        // Pattern-based scoring - EXACT MATCH from original codebase
        let patterns: HashMap<&str, Vec<&str>> = [
            ("Financial", vec!["invoice", "receipt", "payment", "billing", "expense",
                             "budget", "finance", "tax", "cost", "price", "salary",
                             "payroll", "revenue", "profit", "loss", "accounting",
                             "transaction", "purchase", "sale", "refund", "credit"]),
            ("Legal", vec!["contract", "agreement", "terms", "legal", "clause",
                          "license", "patent", "copyright", "trademark", "nda",
                          "compliance", "regulation", "policy", "statute", "law",
                          "litigation", "court", "attorney", "lawyer", "deed"]),
            ("Project", vec!["project", "proposal", "plan", "roadmap", "timeline",
                            "milestone", "deliverable", "scope", "requirement", "specification",
                            "sprint", "backlog", "task", "ticket", "issue",
                            "schedule", "gantt", "agile", "scrum", "kanban"]),
            ("Personal", vec!["personal", "private", "diary", "journal", "note",
                             "reminder", "todo", "list", "memo", "draft",
                             "letter", "email", "message", "correspondence"]),
            ("Media", vec!["photo", "video", "audio", "image", "media",
                          "picture", "film", "movie", "music", "song",
                          "podcast", "recording", "stream", "clip", "footage",
                          "gallery", "album", "playlist", "screenshot", "capture"]),
            ("Documents", vec!["document", "report", "summary", "analysis", "paper",
                              "article", "essay", "thesis", "dissertation", "research",
                              "whitepaper", "manual", "guide", "tutorial", "documentation",
                              "presentation", "slide", "deck", "powerpoint", "pdf"]),
            ("Technical", vec!["code", "script", "program", "software", "api",
                              "database", "server", "cloud", "network", "system",
                              "configuration", "setup", "installation", "deployment",
                              "bug", "error", "debug", "test", "log"]),
            ("Marketing", vec!["marketing", "campaign", "advertisement", "promotion", "brand",
                              "seo", "social", "content", "blog", "newsletter",
                              "press", "release", "announcement", "launch", "product"]),
        ].iter().cloned().collect();

        // Score each category based on keyword matches
        for (category, keywords) in patterns.iter() {
            let mut score = 0.0;
            for keyword in keywords {
                if lower_file_name.contains(keyword) {
                    // Longer matches = higher score (EXACT LOGIC)
                    score += keyword.len() as f32;

                    // Exact word matches get bonus points
                    if lower_file_name.split(|c: char| !c.is_alphanumeric())
                        .any(|word| word == *keyword) {
                        score += 5.0;
                    }
                }
            }

            // Extension-based bonuses (exact from original)
            match (category.as_ref(), extension) {
                ("Financial", ext) if ["pdf", "xlsx", "xls", "csv"].contains(&ext) => score += 10.0,
                ("Legal", ext) if ["pdf", "doc", "docx"].contains(&ext) => score += 10.0,
                ("Media", ext) if ["jpg", "jpeg", "png", "gif", "mp4", "avi", "mp3"].contains(&ext) => score += 15.0,
                ("Technical", ext) if ["rs", "py", "js", "ts", "cpp", "java", "go"].contains(&ext) => score += 20.0,
                _ => {}
            }

            if score > 0.0 {
                category_scores.insert(category.to_string(), score);
            }
        }

        // Return highest scoring category
        if let Some((category, _)) = category_scores.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)) {
            debug!("Intelligent category for '{}': {} (scores: {:?})",
                   file_name, category, category_scores);
            category.clone()
        } else {
            // Default fallback
            "Documents".to_string()
        }
    }

    /// Get intelligent keywords from filename - EXACT LOGIC from original codebase
    pub fn get_intelligent_keywords(file_name: &str, extension: &str) -> Vec<String> {
        let mut keywords = Vec::new();
        let lower_file_name = file_name.to_lowercase();

        // Extract contextual keywords (exact from original)
        const KEYWORD_PATTERNS: &[(&str, &str)] = &[
            ("report", "report"),
            ("summary", "summary"),
            ("analysis", "analysis"),
            ("proposal", "proposal"),
            ("presentation", "presentation"),
            ("invoice", "invoice"),
            ("contract", "contract"),
            ("agreement", "agreement"),
            ("project", "project"),
            ("meeting", "meeting"),
            ("notes", "notes"),
            ("draft", "draft"),
            ("final", "final"),
            ("review", "review"),
            ("budget", "budget"),
        ];

        for (pattern, keyword) in KEYWORD_PATTERNS {
            if lower_file_name.contains(pattern) {
                keywords.push(keyword.to_string());
            }
        }

        // Add year if present
        if let Some(year) = Self::extract_year(&lower_file_name) {
            keywords.push(year);
        }

        // Add extension as keyword
        if !extension.is_empty() {
            keywords.push(extension.replace('.', ""));
        }

        // Extract meaningful words from filename
        let words: Vec<&str> = lower_file_name
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3)
            .filter(|w| !Self::is_common_word(w))
            .take(5)
            .collect();

        for word in words {
            if !keywords.contains(&word.to_string()) {
                keywords.push(word.to_string());
            }
        }

        // Return max 7 keywords as per original
        keywords.truncate(7);
        keywords
    }

    /// Safe suggested name generation when AI unavailable (exact logic)
    pub fn safe_suggested_name(file_name: &str, extension: &str) -> String {
        file_name
            .replace(extension, "")
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect::<String>()
            .replace("__", "_")
            .trim_matches('_')
            .to_string()
    }

    /// Calculate confidence score (exact logic from original)
    pub fn calculate_confidence(
        base_confidence: f32,
        content_length: usize,
        is_ocr: bool,
        folder_match_score: f32,
    ) -> f32 {
        let mut confidence = base_confidence;

        // Adjust based on extraction quality
        if content_length < 100 {
            confidence *= 0.8;
        }

        if is_ocr {
            confidence *= 0.9;
        }

        // Adjust based on folder match quality
        if folder_match_score > 0.8 {
            confidence *= 1.1;
        } else if folder_match_score < 0.5 {
            confidence *= 0.9;
        }

        // Normalize to 0-100 range
        confidence.min(100.0).max(0.0)
    }

    fn extract_year(text: &str) -> Option<String> {
        // Look for 4-digit years between 1900-2099
        for word in text.split(|c: char| !c.is_numeric()) {
            if word.len() == 4 {
                if let Ok(year) = word.parse::<u32>() {
                    if year >= 1900 && year <= 2099 {
                        return Some(word.to_string());
                    }
                }
            }
        }
        None
    }

    fn is_common_word(word: &str) -> bool {
        const COMMON_WORDS: &[&str] = &[
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
            "of", "with", "by", "from", "as", "is", "was", "are", "were", "been",
            "have", "has", "had", "do", "does", "did", "will", "would", "could", "should",
            "file", "document", "copy", "new", "old", "temp", "tmp", "test", "draft"
        ];
        COMMON_WORDS.contains(&word.to_lowercase().as_str())
    }
}

/// Fallback confidence calculation for when AI is not used
impl IntelligentFallbacks {
    pub fn calculate_fallback_confidence(file_name: &str, category: &str, keywords: &[String]) -> f32 {
        let mut confidence = 0.5; // Base confidence for fallback

        // Increase confidence if filename contains category
        if file_name.to_lowercase().contains(&category.to_lowercase()) {
            confidence += 0.2;
        }

        // Increase confidence based on keyword matches
        let keyword_bonus = (keywords.len() as f32 * 0.05).min(0.25);
        confidence += keyword_bonus;

        // Cap at 0.75 for fallback (never as confident as AI)
        confidence.min(0.75)
    }
}

/// Intelligent folder matching without AI
pub struct FolderMatcher;

impl FolderMatcher {
    /// Match file to folder based on patterns
    pub fn match_to_folder(
        file_name: &str,
        extension: &str,
        available_folders: &[String],
    ) -> Option<(String, f32)> {
        let lower_file_name = file_name.to_lowercase();
        let category = IntelligentFallbacks::get_intelligent_category(file_name, extension);

        // Try exact category match first
        for folder in available_folders {
            if folder.to_lowercase() == category.to_lowercase() {
                return Some((folder.clone(), 0.9));
            }
        }

        // Try partial matches
        for folder in available_folders {
            let folder_lower = folder.to_lowercase();

            // Check if filename contains folder name
            if lower_file_name.contains(&folder_lower) || folder_lower.contains(&lower_file_name) {
                return Some((folder.clone(), 0.7));
            }

            // Check category similarity
            if category.to_lowercase().contains(&folder_lower) || folder_lower.contains(&category.to_lowercase()) {
                return Some((folder.clone(), 0.6));
            }
        }

        // Default to first available folder with low confidence
        available_folders.first().map(|f| (f.clone(), 0.3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intelligent_category() {
        assert_eq!(
            IntelligentFallbacks::get_intelligent_category("invoice_12345.pdf", "pdf"),
            "Financial"
        );

        assert_eq!(
            IntelligentFallbacks::get_intelligent_category("contract_service_agreement.doc", "doc"),
            "Legal"
        );

        assert_eq!(
            IntelligentFallbacks::get_intelligent_category("main.rs", "rs"),
            "Technical"
        );
    }

    #[test]
    fn test_intelligent_keywords() {
        let keywords = IntelligentFallbacks::get_intelligent_keywords("Q3_financial_report_2024.pdf", "pdf");
        assert!(keywords.contains(&"report".to_string()));
        assert!(keywords.contains(&"pdf".to_string()));
        assert!(keywords.len() > 0);
    }

    #[test]
    fn test_folder_matching() {
        let folders = vec![
            "Financial".to_string(),
            "Legal".to_string(),
            "Projects".to_string(),
        ];

        let (folder, confidence) = FolderMatcher::match_to_folder(
            "invoice_acme_2024.pdf",
            "pdf",
            &folders
        ).unwrap();

        assert_eq!(folder, "Financial");
        assert!(confidence > 0.5);
    }
}