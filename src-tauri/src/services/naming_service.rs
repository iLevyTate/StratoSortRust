use crate::{ai::{FileAnalysis, ollama::{DocumentAnalysisEnhanced, ImageAnalysisEnhanced}}, error::Result};
use chrono::{DateTime, Local};
use regex::Regex;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Configuration for smart naming
#[derive(Debug, Clone)]
pub struct NamingConfig {
    pub date_format: String,
    pub separator: String,
    pub max_length: usize,
    pub include_date: bool,
    pub include_category: bool,
    pub include_keywords: bool,
    pub keyword_count: usize,
    pub case_style: CaseStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaseStyle {
    Lower,
    Upper,
    Title,
    Camel,
    Snake,
    Kebab,
    Pascal,
}

impl Default for NamingConfig {
    fn default() -> Self {
        Self {
            date_format: "%Y-%m-%d".to_string(),
            separator: "_".to_string(),
            max_length: 100,
            include_date: true,
            include_category: true,
            include_keywords: true,
            keyword_count: 2,
            case_style: CaseStyle::Snake,
        }
    }
}

/// Service for generating intelligent file names based on content analysis
pub struct NamingService {
    config: NamingConfig,
    date_pattern: Regex,
}

impl Default for NamingService {
    fn default() -> Self {
        Self::with_config(NamingConfig::default())
    }
}

impl NamingService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: NamingConfig) -> Self {
        Self {
            config,
            // Common date patterns in documents
            // SAFETY: Create regex with proper error handling
            date_pattern: Regex::new(r"(?i)(\d{1,2})[/-](\d{1,2})[/-](\d{4})|(\d{4})[/-](\d{1,2})[/-](\d{1,2})|(\w{3,})\s+(\d{1,2}),?\s+(\d{4})|(\d{1,2})\s+(\w{3,})\s+(\d{4})")
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to compile complex date regex: {}", e);
                    // Fallback to a simple date pattern if the complex one fails
                    Regex::new(r"\d{4}-\d{2}-\d{2}")
                        .unwrap_or_else(|e2| {
                            tracing::error!("Failed to compile fallback date regex: {}", e2);
                            // Ultimate fallback: match nothing (safe pattern that always compiles)
                            // Use a pattern that matches nothing (word boundary followed by non-word boundary)
                            Regex::new("\\b\\B").unwrap_or_else(|_| {
                                // This should never fail, but if it does, create minimal regex
                                Regex::new("^$").expect("Empty regex should always compile")
                            })
                        })
                }),
        }
    }

    /// Generate a smart name based on enhanced LLM analysis - exact naming logic from original codebase
    pub fn generate_smart_name_from_llm(&self, analysis: &DocumentAnalysisEnhanced, original_path: &Path) -> Result<String> {
        let mut components = Vec::new();

        // 1. Date prefix (YYYY-MM-DD format) - EXACT MATCH
        if let Some(ref date_str) = analysis.date {
            components.push(date_str.clone());
        } else if let Some(date) = self.extract_date_from_path(original_path) {
            components.push(self.format_date(&date));
        }

        // 2. Document type or category - EXACT MATCH
        if !analysis.document_type.is_empty() && analysis.document_type != "general" {
            components.push(self.capitalize_first(&analysis.document_type));
        } else if !analysis.category.is_empty() {
            components.push(analysis.category.clone());
        }

        // 3. Client/Project identifier - EXACT MATCH
        if let Some(ref client) = analysis.client {
            components.push(client.replace(' ', ""));
        } else if let Some(ref project) = analysis.project {
            components.push(project.replace(' ', ""));
        }

        // 4. Top keywords (max 2) - EXACT MATCH
        if !analysis.keywords.is_empty() {
            let filtered_keywords: Vec<String> = analysis.keywords
                .iter()
                .filter(|k| k.len() > 3 && !self.is_common_word(k))
                .take(2)
                .map(|k| self.capitalize_first(k))
                .collect();
            components.extend(filtered_keywords);
        }

        // 5. Build final name - EXACT MATCH
        let final_name = components
            .into_iter()
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join("_")
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect::<String>()
            .replace("__", "_")
            .chars()
            .take(100)
            .collect::<String>();

        // Add original extension
        let extension = original_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if !extension.is_empty() {
            Ok(format!("{}.{}", final_name, extension))
        } else {
            Ok(final_name)
        }
    }

    /// Generate a smart name for images based on vision analysis
    pub fn generate_smart_name_from_vision(&self, analysis: &ImageAnalysisEnhanced, original_path: &Path) -> Result<String> {
        let mut components = Vec::new();

        // 1. Add date if detected in image or from metadata
        if self.config.include_date {
            // Extract date from document text if present
            if let Some(date) = self.extract_date_from_text(&analysis.document_text)
                .or_else(|| self.extract_date_from_path(original_path)) {
                components.push(self.format_date(&date));
            }
        }

        // 2. Add image type
        if !analysis.image_type.is_empty() && analysis.image_type != "other" {
            components.push(self.apply_case_style(&analysis.image_type));
        }

        // 3. Add main subject
        if !analysis.main_subject.is_empty() {
            components.push(self.apply_case_style(&self.clean_component(&analysis.main_subject)));
        }

        // 4. Use suggested name if available
        if components.is_empty() && !analysis.suggested_name.is_empty() {
            components = analysis.suggested_name
                .split('_')
                .map(|s| self.apply_case_style(s))
                .collect();
        }

        self.finalize_name(components, original_path)
    }

    /// Generate a smart name based on file analysis
    pub fn generate_smart_name(&self, analysis: &FileAnalysis, original_path: &Path) -> Result<String> {
        let mut components = Vec::new();

        // 1. Add date component
        if self.config.include_date {
            if let Some(date) = self.extract_date_from_analysis(analysis)
                .or_else(|| self.extract_date_from_path(original_path)) {
                let formatted_date = self.format_date(&date);
                components.push(formatted_date);
            }
        }

        // 2. Add category
        if self.config.include_category && !analysis.category.is_empty() {
            let category = self.clean_component(&analysis.category);
            components.push(self.apply_case_style(&category));
        }

        // 3. Add relevant keywords
        if self.config.include_keywords && !analysis.tags.is_empty() {
            let keywords = self.select_best_keywords(analysis);
            for keyword in keywords.iter().take(self.config.keyword_count) {
                let cleaned = self.clean_component(keyword);
                if !cleaned.is_empty() {
                    components.push(self.apply_case_style(&cleaned));
                }
            }
        }

        // 4. If no components generated, use summary or fallback
        if components.is_empty() {
            if !analysis.summary.is_empty() {
                // Extract key words from summary
                let summary_words = self.extract_keywords_from_text(&analysis.summary, 3);
                for word in summary_words {
                    components.push(self.apply_case_style(&word));
                }
            } else {
                // Fallback to timestamp
                components.push(format!("file_{}", chrono::Utc::now().timestamp()));
            }
        }

        self.finalize_name(components, original_path)
    }

    /// Finalize the name by joining components and adding extension
    fn finalize_name(&self, components: Vec<String>, original_path: &Path) -> Result<String> {
        // Join components
        let mut name = components.join(&self.config.separator);

        // Ensure valid length
        if name.len() > self.config.max_length {
            name = self.truncate_intelligently(&name, self.config.max_length);
        }

        // Add original extension
        let extension = original_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if !extension.is_empty() {
            name = format!("{}.{}", name, extension);
        }

        debug!("Generated smart name: {} -> {}", original_path.display(), name);
        Ok(name)
    }

    /// Generate a unique name if file already exists
    pub fn generate_unique_name(&self, path: &Path) -> PathBuf {
        let mut unique_path = path.to_path_buf();
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

    /// Extract date from analysis results
    fn extract_date_from_analysis(&self, analysis: &FileAnalysis) -> Option<DateTime<Local>> {
        // Look for date in tags
        for tag in &analysis.tags {
            if let Some(date) = self.parse_date_string(tag) {
                return Some(date);
            }
        }

        // Look for date in summary
        if let Some(captures) = self.date_pattern.find(&analysis.summary) {
            if let Some(date) = self.parse_date_string(captures.as_str()) {
                return Some(date);
            }
        }

        None
    }

    /// Extract date from file path or metadata
    fn extract_date_from_path(&self, path: &Path) -> Option<DateTime<Local>> {
        // Try to extract date from filename
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(captures) = self.date_pattern.find(filename) {
                if let Some(date) = self.parse_date_string(captures.as_str()) {
                    return Some(date);
                }
            }
        }

        // Try to use file modification time
        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                return Some(DateTime::from(modified));
            }
        }

        None
    }

    /// Parse various date string formats
    fn parse_date_string(&self, date_str: &str) -> Option<DateTime<Local>> {
        // Try various date formats
        let formats = [
            "%Y-%m-%d",
            "%d-%m-%Y",
            "%m/%d/%Y",
            "%Y/%m/%d",
            "%d/%m/%Y",
            "%B %d, %Y",
            "%b %d, %Y",
            "%d %B %Y",
            "%d %b %Y",
        ];

        for format in &formats {
            if let Ok(date) = DateTime::parse_from_str(&format!("{} 00:00:00 +0000", date_str), &format!("{} %H:%M:%S %z", format)) {
                return Some(date.with_timezone(&Local));
            }
        }

        None
    }

    /// Format date according to config
    fn format_date(&self, date: &DateTime<Local>) -> String {
        date.format(&self.config.date_format).to_string()
    }

    /// Select the most relevant keywords
    fn select_best_keywords(&self, analysis: &FileAnalysis) -> Vec<String> {
        let mut keywords = analysis.tags.clone();

        // Sort by relevance (could be enhanced with scoring)
        keywords.sort_by(|a, b| {
            // Prefer shorter, more specific keywords
            let a_score = (20 - a.len().min(20)) + if a.chars().all(|c| c.is_alphanumeric()) { 5 } else { 0 };
            let b_score = (20 - b.len().min(20)) + if b.chars().all(|c| c.is_alphanumeric()) { 5 } else { 0 };
            b_score.cmp(&a_score)
        });

        // Remove duplicates and very common words
        let stop_words = ["the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for"];
        keywords.retain(|k| {
            let lower = k.to_lowercase();
            !stop_words.contains(&lower.as_str()) && k.len() > 2
        });

        keywords
    }

    /// Extract date from text using pattern matching
    fn extract_date_from_text(&self, text: &str) -> Option<DateTime<Local>> {
        if let Some(captures) = self.date_pattern.find(text) {
            if let Some(date) = self.parse_date_string(captures.as_str()) {
                return Some(date);
            }
        }
        None
    }

    /// Extract keywords from text
    fn extract_keywords_from_text(&self, text: &str, count: usize) -> Vec<String> {
        let words: Vec<String> = text
            .split_whitespace()
            .filter_map(|w| {
                let cleaned = w.trim_matches(|c: char| !c.is_alphanumeric());
                if cleaned.len() > 3 && cleaned.chars().any(|c| c.is_alphabetic()) {
                    Some(cleaned.to_lowercase())
                } else {
                    None
                }
            })
            .collect();

        // Get unique words, preserving order
        let mut seen = std::collections::HashSet::new();
        let mut unique = Vec::new();
        for word in words {
            if seen.insert(word.clone()) {
                unique.push(word);
            }
        }

        unique.into_iter().take(count).collect()
    }

    /// Clean a component for use in filename
    fn clean_component(&self, component: &str) -> String {
        component
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    }

    /// Check if word is common (should be filtered)
    fn is_common_word(&self, word: &str) -> bool {
        const COMMON_WORDS: &[&str] = &[
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
            "of", "with", "by", "from", "as", "is", "was", "are", "were", "been",
            "have", "has", "had", "do", "does", "did", "will", "would", "could", "should"
        ];
        COMMON_WORDS.contains(&word.to_lowercase().as_str())
    }

    /// Capitalize first letter of word
    fn capitalize_first(&self, text: &str) -> String {
        let mut chars = text.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Apply case style transformation
    fn apply_case_style(&self, text: &str) -> String {
        match self.config.case_style {
            CaseStyle::Lower => text.to_lowercase(),
            CaseStyle::Upper => text.to_uppercase(),
            CaseStyle::Title => self.to_title_case(text),
            CaseStyle::Camel => self.to_camel_case(text),
            CaseStyle::Snake => self.to_snake_case(text),
            CaseStyle::Kebab => text.to_lowercase().replace('_', "-"),
            CaseStyle::Pascal => self.to_pascal_case(text),
        }
    }

    fn to_title_case(&self, text: &str) -> String {
        text.split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                }
            })
            .collect::<Vec<_>>()
            .join("_")
    }

    fn to_camel_case(&self, text: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = false;

        for ch in text.chars() {
            if ch == '_' || ch == '-' || ch.is_whitespace() {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(ch.to_ascii_lowercase());
            }
        }

        result
    }

    fn to_snake_case(&self, text: &str) -> String {
        text.to_lowercase()
            .replace(|c: char| c.is_whitespace() || c == '-', "_")
    }

    fn to_pascal_case(&self, text: &str) -> String {
        let camel = self.to_camel_case(text);
        let mut chars = camel.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Truncate intelligently at word boundaries
    fn truncate_intelligently(&self, text: &str, max_length: usize) -> String {
        if text.len() <= max_length {
            return text.to_string();
        }

        // Try to truncate at a separator
        if let Some(pos) = text[..max_length].rfind(&self.config.separator) {
            return text[..pos].to_string();
        }

        // Otherwise truncate and add ellipsis
        format!("{}...", &text[..max_length - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_name_generation() {
        let service = NamingService::new();
        let analysis = FileAnalysis {
            path: "test.pdf".to_string(),
            category: "Invoice".to_string(),
            tags: vec!["payment".to_string(), "2024".to_string(), "acme".to_string()],
            summary: "Invoice from ACME Corp for services rendered in January 2024".to_string(),
            confidence: 0.9,
            metadata: serde_json::Value::Null,
            extracted_text: Some("Invoice from ACME Corp".to_string()),
            detected_language: Some("English".to_string()),
        };

        let name = service.generate_smart_name(&analysis, Path::new("test.pdf"))
            .expect("Failed to generate smart name in test");
        assert!(name.contains("invoice"));
        assert!(name.ends_with(".pdf"));
    }

    #[test]
    fn test_unique_name_generation() {
        let service = NamingService::new();
        let path = Path::new("test.txt");

        // Since test.txt doesn't exist in test environment, it should return the same path
        let unique = service.generate_unique_name(path);
        assert_eq!(path, unique); // Path doesn't exist, so no modification needed
    }
}