use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};
use rand::{Rng, thread_rng};
use rand::seq::SliceRandom;

/// Comprehensive test data generator for creating realistic file scenarios
pub struct TestDataGenerator {
    temp_dir: TempDir,
    files_created: Vec<PathBuf>,
}

impl TestDataGenerator {
    pub fn new() -> Self {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        Self {
            temp_dir,
            files_created: Vec::new(),
        }
    }

    pub fn base_path(&self) -> &Path {
        self.temp_dir.path()
    }

    pub fn created_files(&self) -> &[PathBuf] {
        &self.files_created
    }

    /// Create a comprehensive test file structure
    pub fn create_comprehensive_test_structure(&mut self) -> Result<(), std::io::Error> {
        let base = self.base_path();
        
        // Create directory structure
        let directories = vec![
            "documents",
            "documents/work",
            "documents/personal",
            "images",
            "images/photos",
            "images/graphics", 
            "code",
            "code/rust",
            "code/python",
            "archives",
            "temp",
            "empty_dir",
            "special_chars_αβγ",
            "deeply/nested/directory/structure/here",
        ];
        
        for dir in directories {
            fs::create_dir_all(base.join(dir))?;
        }
        
        // Create various file types
        self.create_document_files()?;
        self.create_image_files()?;
        self.create_code_files()?;
        self.create_archive_files()?;
        self.create_special_files()?;
        self.create_large_files()?;
        self.create_unicode_files()?;
        
        Ok(())
    }

    /// Create various document files
    fn create_document_files(&mut self) -> Result<(), std::io::Error> {
        let documents = vec![
            ("documents/readme.txt", "This is a simple text file with basic content."),
            ("documents/work/project_plan.txt", include_str!("sample_data/project_plan.txt")),
            ("documents/work/meeting_notes.md", include_str!("sample_data/meeting_notes.md")),
            ("documents/personal/diary.txt", include_str!("sample_data/diary.txt")),
            ("documents/config.json", r#"{"theme": "dark", "language": "en", "version": "1.0"}"#),
            ("documents/data.xml", include_str!("sample_data/sample.xml")),
            ("documents/spreadsheet.csv", "Name,Age,City\nJohn,25,New York\nJane,30,San Francisco\nBob,22,Chicago"),
        ];
        
        for (path, content) in documents {
            let file_path = self.base_path().join(path);
            fs::write(&file_path, content)?;
            self.files_created.push(file_path);
        }
        
        Ok(())
    }

    /// Create mock image files (not actual images, just files with image extensions)
    fn create_image_files(&mut self) -> Result<(), std::io::Error> {
        let image_files = vec![
            ("images/photo1.jpg", "JPEG_MOCK_DATA"),
            ("images/photo2.png", "PNG_MOCK_DATA"),
            ("images/graphics/logo.svg", r#"<svg><circle cx="50" cy="50" r="40"/></svg>"#),
            ("images/graphics/icon.ico", "ICO_MOCK_DATA"),
            ("images/animation.gif", "GIF_MOCK_DATA"),
            ("images/high_quality.tiff", "TIFF_MOCK_DATA"),
        ];
        
        for (path, content) in image_files {
            let file_path = self.base_path().join(path);
            fs::write(&file_path, content)?;
            self.files_created.push(file_path);
        }
        
        Ok(())
    }

    /// Create code files
    fn create_code_files(&mut self) -> Result<(), std::io::Error> {
        let code_files = vec![
            ("code/rust/main.rs", include_str!("sample_data/main.rs")),
            ("code/rust/lib.rs", include_str!("sample_data/lib.rs")),
            ("code/python/script.py", include_str!("sample_data/script.py")),
            ("code/python/utils.py", include_str!("sample_data/utils.py")),
            ("code/javascript.js", "function hello() { console.log('Hello, World!'); }"),
            ("code/styles.css", "body { font-family: Arial; margin: 0; padding: 20px; }"),
            ("code/index.html", "<!DOCTYPE html><html><head><title>Test</title></head><body><h1>Test Page</h1></body></html>"),
        ];
        
        for (path, content) in code_files {
            let file_path = self.base_path().join(path);
            fs::write(&file_path, content)?;
            self.files_created.push(file_path);
        }
        
        Ok(())
    }

    /// Create mock archive files
    fn create_archive_files(&mut self) -> Result<(), std::io::Error> {
        let archive_files = vec![
            ("archives/backup.zip", "ZIP_MOCK_DATA"),
            ("archives/old_files.tar", "TAR_MOCK_DATA"),
            ("archives/compressed.gz", "GZIP_MOCK_DATA"),
            ("archives/data.7z", "7Z_MOCK_DATA"),
        ];
        
        for (path, content) in archive_files {
            let file_path = self.base_path().join(path);
            fs::write(&file_path, content)?;
            self.files_created.push(file_path);
        }
        
        Ok(())
    }

    /// Create files with special characteristics
    fn create_special_files(&mut self) -> Result<(), std::io::Error> {
        // Empty file
        let empty_file = self.base_path().join("empty_file.txt");
        fs::write(&empty_file, "")?;
        self.files_created.push(empty_file);
        
        // File with only whitespace
        let whitespace_file = self.base_path().join("whitespace.txt");
        fs::write(&whitespace_file, "   \n\t\r\n   ")?;
        self.files_created.push(whitespace_file);
        
        // File with special characters in name
        let special_name_file = self.base_path().join("file with spaces & symbols!@#.txt");
        fs::write(&special_name_file, "File with special characters in name")?;
        self.files_created.push(special_name_file);
        
        // Hidden file (Unix-style)
        let hidden_file = self.base_path().join(".hidden_file");
        fs::write(&hidden_file, "This is a hidden file")?;
        self.files_created.push(hidden_file);
        
        // File with no extension
        let no_ext_file = self.base_path().join("no_extension");
        fs::write(&no_ext_file, "This file has no extension")?;
        self.files_created.push(no_ext_file);
        
        Ok(())
    }

    /// Create large files for testing performance
    fn create_large_files(&mut self) -> Result<(), std::io::Error> {
        // Small large file (1KB)
        let small_large = self.base_path().join("large_1kb.txt");
        let content_1kb = "a".repeat(1024);
        fs::write(&small_large, content_1kb)?;
        self.files_created.push(small_large);
        
        // Medium large file (1MB)
        let medium_large = self.base_path().join("large_1mb.txt");
        let content_1mb = "b".repeat(1024 * 1024);
        fs::write(&medium_large, content_1mb)?;
        self.files_created.push(medium_large);
        
        // Large file (10MB) - only create if we have space
        if let Ok(metadata) = fs::metadata(self.base_path()) {
            // Only create if we're in a test environment with enough space
            if self.base_path().to_string_lossy().contains("tmp") {
                let large_file = self.base_path().join("large_10mb.txt");
                let content_10mb = "c".repeat(10 * 1024 * 1024);
                fs::write(&large_file, content_10mb)?;
                self.files_created.push(large_file);
            }
        }
        
        Ok(())
    }

    /// Create files with various Unicode characters
    fn create_unicode_files(&mut self) -> Result<(), std::io::Error> {
        let unicode_files = vec![
            ("unicode_basic.txt", "Basic unicode: café, naïve, résumé"),
            ("unicode_emoji.txt", "Emoji test: 🚀 🔥 💻 ⚡ 🎯 🔒 ✅ ❌ 🌟 💡"),
            ("unicode_japanese.txt", "Japanese: こんにちは世界 (Hello World)"),
            ("unicode_chinese.txt", "Chinese: 你好世界 (Hello World)"),
            ("unicode_arabic.txt", "Arabic: مرحبا بالعالم (Hello World)"),
            ("unicode_russian.txt", "Russian: Привет мир (Hello World)"),
            ("unicode_mixed.txt", "Mixed: Hello 🌍, 世界, мир, عالم!"),
            ("special_chars_αβγ/unicode_greek.txt", "Greek letters: αβγδε ΑΒΓ∆Ε"),
        ];
        
        for (path, content) in unicode_files {
            let file_path = self.base_path().join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&file_path, content)?;
            self.files_created.push(file_path);
        }
        
        Ok(())
    }

    /// Generate random test data
    pub fn generate_random_files(&mut self, count: usize, size_range: (usize, usize)) -> Result<(), std::io::Error> {
        let mut rng = thread_rng();
        
        let prefixes = vec!["random", "test", "data", "file", "sample", "demo"];
        let extensions = vec!["txt", "dat", "log", "tmp", "bak"];
        
        for i in 0..count {
            let prefix = prefixes.choose(&mut rng).unwrap();
            let extension = extensions.choose(&mut rng).unwrap();
            let filename = format!("{}_{}.{}", prefix, i, extension);
            
            let size = rng.gen_range(size_range.0..=size_range.1);
            let content = generate_random_text(size);
            
            let file_path = self.base_path().join("temp").join(filename);
            fs::write(&file_path, content)?;
            self.files_created.push(file_path);
        }
        
        Ok(())
    }

    /// Create files that should trigger security warnings
    pub fn create_security_test_files(&mut self) -> Result<(), std::io::Error> {
        let security_files = vec![
            ("suspicious.exe", "FAKE_EXECUTABLE_DATA"),
            ("malware.bat", "@echo off\necho This is a test batch file"),
            ("script.sh", "#!/bin/bash\necho 'Test shell script'"),
            ("config.ini", "[database]\nhost=localhost\nuser=admin\npassword=secret123"),
            (".env", "API_KEY=fake_key_12345\nDATABASE_URL=postgres://localhost"),
            ("private_key.pem", "-----BEGIN PRIVATE KEY-----\nFAKE_KEY_DATA\n-----END PRIVATE KEY-----"),
        ];
        
        for (filename, content) in security_files {
            let file_path = self.base_path().join("temp").join(filename);
            fs::write(&file_path, content)?;
            self.files_created.push(file_path);
        }
        
        Ok(())
    }

    /// Create files for testing specific edge cases
    pub fn create_edge_case_files(&mut self) -> Result<(), std::io::Error> {
        // Very long filename (approaching filesystem limits)
        let long_name = "very_".repeat(50) + "long_filename.txt";
        let long_name_file = self.base_path().join(&long_name[..std::cmp::min(long_name.len(), 255)]);
        fs::write(&long_name_file, "File with very long name")?;
        self.files_created.push(long_name_file);
        
        // File with only numbers as name
        let number_file = self.base_path().join("12345");
        fs::write(&number_file, "Filename is only numbers")?;
        self.files_created.push(number_file);
        
        // File with multiple dots
        let multi_dot_file = self.base_path().join("file.with.many.dots.txt");
        fs::write(&multi_dot_file, "File with multiple dots")?;
        self.files_created.push(multi_dot_file);
        
        // File starting with dot but not hidden
        let dot_start_file = self.base_path().join(".not_hidden.txt");
        fs::write(&dot_start_file, "File starting with dot")?;
        self.files_created.push(dot_start_file);
        
        // File with trailing spaces in name (if filesystem allows)
        let trailing_space_file = self.base_path().join("file_with_trailing_space ");
        let _ = fs::write(&trailing_space_file, "File with trailing space in name");
        // Don't add to files_created as it might not be created successfully
        
        Ok(())
    }

    /// Get files by extension
    pub fn files_by_extension(&self, extension: &str) -> Vec<&PathBuf> {
        self.files_created.iter()
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case(extension))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get files containing specific content
    pub fn files_containing_content(&self, search_term: &str) -> Vec<&PathBuf> {
        self.files_created.iter()
            .filter(|path| {
                if let Ok(content) = fs::read_to_string(path) {
                    content.contains(search_term)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Get files larger than specified size
    pub fn files_larger_than(&self, size_bytes: u64) -> Vec<&PathBuf> {
        self.files_created.iter()
            .filter(|path| {
                fs::metadata(path)
                    .map(|metadata| metadata.len() > size_bytes)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Clean up all created files
    pub fn cleanup(self) {
        // TempDir automatically cleans up when dropped
        drop(self);
    }
}

/// Generate random text content
fn generate_random_text(size: usize) -> String {
    let mut rng = thread_rng();
    let words = vec![
        "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit",
        "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore",
        "magna", "aliqua", "enim", "ad", "minim", "veniam", "quis", "nostrud",
        "exercitation", "ullamco", "laboris", "nisi", "aliquip", "ex", "ea", "commodo",
        "consequat", "duis", "aute", "irure", "in", "reprehenderit", "voluptate",
        "velit", "esse", "cillum", "fugiat", "nulla", "pariatur", "excepteur", "sint",
        "occaecat", "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia",
        "deserunt", "mollit", "anim", "id", "est", "laborum", "test", "data", "random",
        "content", "file", "example", "sample", "demo", "mock", "fake", "placeholder"
    ];
    
    let mut content = String::new();
    let mut current_size = 0;
    
    while current_size < size {
        let word = words.choose(&mut rng).unwrap();
        let addition = if rng.gen_bool(0.1) { // 10% chance of newline
            format!("{}\n", word)
        } else {
            format!("{} ", word)
        };
        
        if current_size + addition.len() > size {
            // Add partial content to reach exactly the desired size
            let remaining = size - current_size;
            content.push_str(&addition[..remaining]);
            break;
        }
        
        content.push_str(&addition);
        current_size += addition.len();
    }
    
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_generator_creation() {
        let mut generator = TestDataGenerator::new();
        assert!(generator.base_path().exists());
        assert_eq!(generator.created_files().len(), 0);
    }

    #[test]
    fn test_comprehensive_structure_creation() {
        let mut generator = TestDataGenerator::new();
        generator.create_comprehensive_test_structure().unwrap();
        
        assert!(generator.created_files().len() > 0);
        assert!(generator.base_path().join("documents").exists());
        assert!(generator.base_path().join("images").exists());
        assert!(generator.base_path().join("code").exists());
    }

    #[test]
    fn test_random_file_generation() {
        let mut generator = TestDataGenerator::new();
        generator.generate_random_files(5, (100, 200)).unwrap();
        
        assert_eq!(generator.created_files().len(), 5);
        
        for file_path in generator.created_files() {
            let content = fs::read_to_string(file_path).unwrap();
            assert!(content.len() >= 100 && content.len() <= 200);
        }
    }

    #[test]
    fn test_file_filtering() {
        let mut generator = TestDataGenerator::new();
        generator.create_comprehensive_test_structure().unwrap();
        
        let txt_files = generator.files_by_extension("txt");
        let large_files = generator.files_larger_than(1000);
        
        assert!(txt_files.len() > 0);
        println!("Found {} .txt files", txt_files.len());
        println!("Found {} large files", large_files.len());
    }
}