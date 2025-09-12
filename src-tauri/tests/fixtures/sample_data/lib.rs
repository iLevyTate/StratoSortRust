use std::path::Path;
use std::fs;
use serde::{Serialize, Deserialize};

/// Core library for file analysis and organization
pub mod file_analyzer;
pub mod organizer;
pub mod search;
pub mod utils;

/// Configuration for the file organization system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ai_model: String,
    pub max_file_size: u64,
    pub supported_formats: Vec<String>,
    pub output_directory: String,
    pub enable_ai: bool,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ai_model: "llama3.2:3b".to_string(),
            max_file_size: 100 * 1024 * 1024, // 100MB
            supported_formats: vec![
                "txt".to_string(), "pdf".to_string(), "doc".to_string(),
                "jpg".to_string(), "png".to_string(), "mp3".to_string(),
            ],
            output_directory: "./organized".to_string(),
            enable_ai: true,
            log_level: "info".to_string(),
        }
    }
}

/// Represents a file with metadata and analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzedFile {
    pub path: String,
    pub size: u64,
    pub file_type: FileType,
    pub mime_type: String,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub analysis: FileAnalysis,
}

/// File type categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    Document,
    Image,
    Audio,
    Video,
    Code,
    Archive,
    Data,
    Unknown,
}

/// Analysis results for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    pub category: String,
    pub tags: Vec<String>,
    pub description: String,
    pub confidence: f32,
    pub suggested_location: Option<String>,
    pub content_summary: Option<String>,
}

impl Default for FileAnalysis {
    fn default() -> Self {
        Self {
            category: "Unknown".to_string(),
            tags: Vec::new(),
            description: "No analysis available".to_string(),
            confidence: 0.0,
            suggested_location: None,
            content_summary: None,
        }
    }
}

/// Result of an organization operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationResult {
    pub files_processed: usize,
    pub files_moved: usize,
    pub files_skipped: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// Main library interface
pub struct FileOrganizer {
    config: Config,
}

impl FileOrganizer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(Config::default())
    }

    /// Analyze a single file
    pub fn analyze_file(&self, file_path: &str) -> Result<AnalyzedFile, Box<dyn std::error::Error>> {
        let path = Path::new(file_path);
        
        if !path.exists() {
            return Err(format!("File does not exist: {}", file_path).into());
        }

        let metadata = fs::metadata(path)?;
        let file_size = metadata.len();
        
        if file_size > self.config.max_file_size {
            return Err(format!("File too large: {} bytes (max: {})", 
                              file_size, self.config.max_file_size).into());
        }

        let file_type = self.determine_file_type(path);
        let mime_type = self.detect_mime_type(path);
        
        let analysis = if self.config.enable_ai {
            self.ai_analyze(path)?
        } else {
            self.basic_analyze(path)?
        };

        Ok(AnalyzedFile {
            path: file_path.to_string(),
            size: file_size,
            file_type,
            mime_type,
            created: None, // Would be populated from metadata
            modified: None, // Would be populated from metadata
            analysis,
        })
    }

    /// Organize files in a directory
    pub fn organize_directory(&self, dir_path: &str) -> Result<OrganizationResult, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();
        let mut result = OrganizationResult {
            files_processed: 0,
            files_moved: 0,
            files_skipped: 0,
            errors: Vec::new(),
            duration_ms: 0,
        };

        let path = Path::new(dir_path);
        if !path.exists() || !path.is_dir() {
            return Err(format!("Directory does not exist: {}", dir_path).into());
        }

        let entries = fs::read_dir(path)?;
        
        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();
            
            if entry_path.is_file() {
                result.files_processed += 1;
                
                match self.analyze_file(&entry_path.to_string_lossy()) {
                    Ok(analyzed_file) => {
                        if let Some(target_dir) = &analyzed_file.analysis.suggested_location {
                            // Would move file here
                            result.files_moved += 1;
                            println!("Would move {} to {}", analyzed_file.path, target_dir);
                        } else {
                            result.files_skipped += 1;
                        }
                    }
                    Err(e) => {
                        result.errors.push(format!("Error analyzing {}: {}", 
                                                  entry_path.display(), e));
                        result.files_skipped += 1;
                    }
                }
            }
        }

        result.duration_ms = start_time.elapsed().as_millis() as u64;
        Ok(result)
    }

    /// Search for files by content
    pub fn search_files(&self, _query: &str, _directory: &str) -> Result<Vec<AnalyzedFile>, Box<dyn std::error::Error>> {
        // Would implement semantic search here
        Ok(Vec::new())
    }

    // Private helper methods
    
    fn determine_file_type(&self, path: &Path) -> FileType {
        if let Some(extension) = path.extension() {
            match extension.to_string_lossy().to_lowercase().as_str() {
                "txt" | "md" | "pdf" | "doc" | "docx" => FileType::Document,
                "jpg" | "jpeg" | "png" | "gif" | "bmp" => FileType::Image,
                "mp3" | "wav" | "flac" | "aac" => FileType::Audio,
                "mp4" | "avi" | "mkv" | "mov" => FileType::Video,
                "rs" | "py" | "js" | "c" | "cpp" | "h" => FileType::Code,
                "zip" | "tar" | "gz" | "rar" | "7z" => FileType::Archive,
                "json" | "xml" | "csv" | "sql" => FileType::Data,
                _ => FileType::Unknown,
            }
        } else {
            FileType::Unknown
        }
    }

    fn detect_mime_type(&self, path: &Path) -> String {
        // Simplified MIME type detection
        if let Some(extension) = path.extension() {
            match extension.to_string_lossy().to_lowercase().as_str() {
                "txt" => "text/plain",
                "pdf" => "application/pdf",
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "mp3" => "audio/mpeg",
                "mp4" => "video/mp4",
                "json" => "application/json",
                "xml" => "application/xml",
                _ => "application/octet-stream",
            }
        } else {
            "application/octet-stream"
        }.to_string()
    }

    fn ai_analyze(&self, path: &Path) -> Result<FileAnalysis, Box<dyn std::error::Error>> {
        // Would integrate with AI service here
        let mut analysis = FileAnalysis::default();
        
        analysis.category = match self.determine_file_type(path) {
            FileType::Document => "Document",
            FileType::Image => "Image", 
            FileType::Audio => "Audio",
            FileType::Video => "Video",
            FileType::Code => "Code",
            FileType::Archive => "Archive",
            FileType::Data => "Data",
            FileType::Unknown => "Unknown",
        }.to_string();
        
        analysis.confidence = 0.85;
        analysis.tags = vec!["ai-analyzed".to_string()];
        analysis.description = format!("AI-analyzed {} file", analysis.category.to_lowercase());
        
        // Suggest organization folder
        analysis.suggested_location = match analysis.category.as_str() {
            "Document" => Some("Documents".to_string()),
            "Image" => Some("Images".to_string()),
            "Audio" => Some("Music".to_string()),
            "Video" => Some("Videos".to_string()),
            "Code" => Some("Code".to_string()),
            "Archive" => Some("Archives".to_string()),
            "Data" => Some("Data".to_string()),
            _ => None,
        };

        Ok(analysis)
    }

    fn basic_analyze(&self, path: &Path) -> Result<FileAnalysis, Box<dyn std::error::Error>> {
        let mut analysis = FileAnalysis::default();
        
        let file_type = self.determine_file_type(path);
        analysis.category = format!("{:?}", file_type);
        analysis.confidence = 0.6;
        analysis.tags = vec!["basic-analysis".to_string()];
        analysis.description = format!("Basic analysis of {:?} file", file_type);
        
        Ok(analysis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.ai_model, "llama3.2:3b");
        assert_eq!(config.max_file_size, 100 * 1024 * 1024);
        assert!(config.enable_ai);
    }

    #[test]
    fn test_file_type_detection() {
        let organizer = FileOrganizer::with_default_config();
        
        let test_cases = vec![
            ("test.txt", FileType::Document),
            ("image.jpg", FileType::Image),
            ("song.mp3", FileType::Audio),
            ("code.rs", FileType::Code),
            ("unknown", FileType::Unknown),
        ];

        for (filename, expected_type) in test_cases {
            let path = Path::new(filename);
            let detected_type = organizer.determine_file_type(&path);
            assert_eq!(format!("{:?}", detected_type), format!("{:?}", expected_type));
        }
    }

    #[test]
    fn test_analyze_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "This is a test file").unwrap();
        }
        
        let organizer = FileOrganizer::with_default_config();
        let result = organizer.analyze_file(&file_path.to_string_lossy());
        
        assert!(result.is_ok());
        let analyzed = result.unwrap();
        assert_eq!(analyzed.path, file_path.to_string_lossy());
        assert!(analyzed.size > 0);
    }

    #[test]
    fn test_organize_directory() {
        let temp_dir = tempdir().unwrap();
        
        // Create test files
        let files = vec!["doc.txt", "image.jpg", "song.mp3"];
        for filename in files {
            let file_path = temp_dir.path().join(filename);
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "Test content").unwrap();
        }
        
        let organizer = FileOrganizer::with_default_config();
        let result = organizer.organize_directory(&temp_dir.path().to_string_lossy());
        
        assert!(result.is_ok());
        let org_result = result.unwrap();
        assert_eq!(org_result.files_processed, 3);
    }
}