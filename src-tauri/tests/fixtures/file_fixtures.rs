use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

/// Creates a temporary directory with a realistic file structure for testing
pub struct TestFileStructure {
    pub temp_dir: TempDir,
    pub root_path: PathBuf,
    pub file_paths: Vec<PathBuf>,
}

impl TestFileStructure {
    /// Creates a comprehensive test file structure
    pub fn new() -> std::io::Result<Self> {
        let temp_dir = tempdir()?;
        let root_path = temp_dir.path().to_path_buf();
        let mut file_paths = Vec::new();

        // Create directory structure
        let directories = vec![
            "documents/contracts",
            "documents/invoices", 
            "documents/reports",
            "documents/presentations",
            "downloads/misc",
            "photos/2024/events",
            "photos/2024/personal",
            "videos/training",
            "videos/marketing",
            "audio/podcasts",
            "audio/music",
            "archives/backups",
            "receipts/2024",
            "legal/policies",
            "projects/web_app",
            "projects/mobile_app",
        ];

        for dir in directories {
            fs::create_dir_all(root_path.join(dir))?;
        }

        // Create document files
        let documents = vec![
            ("documents/contracts/service_agreement_2024.pdf", include_str!("../data/sample_contract.txt")),
            ("documents/contracts/nda_template.docx", "NON-DISCLOSURE AGREEMENT\n\nThis Non-Disclosure Agreement (\"Agreement\") is entered into by and between [Company Name] and [Recipient Name]..."),
            ("documents/contracts/freelance_contract.pdf", "FREELANCE SERVICES AGREEMENT\n\nProject: Website Redesign\nDuration: 3 months\nPayment: $15,000..."),
            
            ("documents/invoices/invoice_2024_001.pdf", "INVOICE #2024-001\n\nBill To: Acme Corporation\nDate: March 15, 2024\nAmount Due: $5,250.00\n\nServices:\n- Web Development: 35 hours @ $150/hr"),
            ("documents/invoices/invoice_2024_002.xlsx", "Invoice,Date,Client,Amount,Status\n2024-002,2024-03-22,TechCorp Inc,$8750.00,Paid"),
            ("documents/invoices/expense_report_march.pdf", "EXPENSE REPORT - March 2024\n\nTravel: $450.00\nMeals: $127.50\nSupplies: $89.25\nTotal: $666.75"),
            
            ("documents/reports/monthly_metrics_q1.xlsx", "Month,Revenue,Expenses,Profit\nJanuary,$45000,$32000,$13000\nFebruary,$52000,$35000,$17000\nMarch,$48000,$31000,$17000"),
            ("documents/reports/annual_review_2023.pdf", "ANNUAL BUSINESS REVIEW 2023\n\nExecutive Summary:\nFY2023 was marked by significant growth across all business segments..."),
            ("documents/reports/customer_satisfaction.docx", "CUSTOMER SATISFACTION SURVEY RESULTS\n\nQ1 2024 Results:\n- Overall Satisfaction: 4.2/5\n- Response Rate: 78%\n- Net Promoter Score: 42"),
            
            ("documents/presentations/q1_board_meeting.pptx", "Q1 2024 BOARD PRESENTATION\n\nSlide 1: Quarterly Highlights\nSlide 2: Financial Performance\nSlide 3: Market Position\nSlide 4: Strategic Initiatives"),
            ("documents/presentations/product_launch.pptx", "NEW PRODUCT LAUNCH PRESENTATION\n\nProduct: AI-Powered Analytics Platform\nLaunch Date: Q2 2024\nTarget Market: Enterprise Customers"),
            
            ("downloads/misc/software_manual.pdf", "SOFTWARE USER MANUAL\n\nVersion 2.1\nLast Updated: March 2024\n\nTable of Contents:\n1. Installation\n2. Configuration\n3. User Guide"),
            ("downloads/misc/meeting_notes_march_28.txt", "MEETING NOTES - March 28, 2024\n\nAttendees: John, Sarah, Mike\nTopic: Project Timeline Review\n\nAction Items:\n- Finalize requirements (John)\n- Update design mockups (Sarah)"),
            ("downloads/misc/price_quote_website.pdf", "WEBSITE DEVELOPMENT QUOTE\n\nClient: Local Business\nProject: E-commerce Website\nEstimate: $12,000 - $18,000\nTimeline: 8-12 weeks"),
        ];

        for (path, content) in documents {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        // Create image files (with placeholder content)
        let images = vec![
            ("photos/2024/events/company_picnic.jpg", Self::create_fake_jpeg_content()),
            ("photos/2024/events/team_building.png", Self::create_fake_png_content()),
            ("photos/2024/events/conference_booth.jpg", Self::create_fake_jpeg_content()),
            ("photos/2024/personal/vacation_beach.jpg", Self::create_fake_jpeg_content()),
            ("photos/2024/personal/family_dinner.png", Self::create_fake_png_content()),
            ("receipts/2024/office_supplies_march.jpg", Self::create_fake_receipt_content()),
            ("receipts/2024/restaurant_lunch_team.png", Self::create_fake_receipt_content()),
        ];

        for (path, content) in images {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        // Create video files (with placeholder content)
        let videos = vec![
            ("videos/training/onboarding_session_1.mp4", Self::create_fake_mp4_content()),
            ("videos/training/safety_procedures.avi", Self::create_fake_avi_content()),
            ("videos/marketing/product_demo.mp4", Self::create_fake_mp4_content()),
            ("videos/marketing/customer_testimonials.mov", Self::create_fake_mov_content()),
        ];

        for (path, content) in videos {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        // Create audio files (with placeholder content)
        let audio = vec![
            ("audio/podcasts/tech_talk_ep_15.mp3", Self::create_fake_mp3_content()),
            ("audio/podcasts/business_insights_march.wav", Self::create_fake_wav_content()),
            ("audio/music/focus_playlist.mp3", Self::create_fake_mp3_content()),
        ];

        for (path, content) in audio {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        // Create archive files
        let archives = vec![
            ("archives/backups/project_backup_q1_2024.zip", Self::create_fake_zip_content()),
            ("archives/backups/database_backup_march.tar.gz", Self::create_fake_tar_content()),
        ];

        for (path, content) in archives {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        // Create legal documents
        let legal_docs = vec![
            ("legal/policies/privacy_policy_v3.docx", "PRIVACY POLICY VERSION 3.0\n\nEffective Date: April 1, 2024\n\nWe respect your privacy and are committed to protecting your personal data..."),
            ("legal/policies/terms_of_service.pdf", "TERMS OF SERVICE\n\nLast Updated: March 1, 2024\n\nBy using our service, you agree to these terms..."),
            ("legal/policies/employee_handbook.pdf", "EMPLOYEE HANDBOOK 2024\n\nWelcome to our company! This handbook contains important information about your employment..."),
        ];

        for (path, content) in legal_docs {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        // Create project files
        let project_files = vec![
            ("projects/web_app/README.md", "# Web Application Project\n\n## Overview\nThis is a modern web application built with React and Node.js.\n\n## Setup\n1. Install dependencies: `npm install`\n2. Start development server: `npm start`"),
            ("projects/web_app/package.json", r#"{"name": "web-app", "version": "1.0.0", "dependencies": {"react": "^18.0.0"}}"#),
            ("projects/mobile_app/README.md", "# Mobile Application\n\n## Platform\n- iOS and Android\n- Built with React Native\n\n## Features\n- User authentication\n- Data synchronization\n- Offline support"),
            ("projects/mobile_app/config.json", r#"{"name": "MobileApp", "version": "2.1.0", "platform": "react-native"}"#),
        ];

        for (path, content) in project_files {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        Ok(TestFileStructure {
            temp_dir,
            root_path,
            file_paths,
        })
    }

    /// Creates a minimal test structure with just a few files
    pub fn minimal() -> std::io::Result<Self> {
        let temp_dir = tempdir()?;
        let root_path = temp_dir.path().to_path_buf();
        let mut file_paths = Vec::new();

        let minimal_files = vec![
            ("document.txt", "This is a test document with some content for analysis."),
            ("image.jpg", &String::from_utf8_lossy(&Self::create_fake_jpeg_content())),
            ("data.json", r#"{"name": "test", "value": 123, "active": true}"#),
        ];

        for (path, content) in minimal_files {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        Ok(TestFileStructure {
            temp_dir,
            root_path, 
            file_paths,
        })
    }

    /// Creates files with specific characteristics for testing edge cases
    pub fn edge_cases() -> std::io::Result<Self> {
        let temp_dir = tempdir()?;
        let root_path = temp_dir.path().to_path_buf();
        let mut file_paths = Vec::new();

        let edge_case_files = vec![
            // Empty file
            ("empty.txt", ""),
            
            // Very small file
            ("tiny.txt", "x"),
            
            // Large text file
            ("large.txt", &"Lorem ipsum dolor sit amet. ".repeat(10000)),
            
            // File with special characters in name
            ("file with spaces & symbols.txt", "Content with special filename"),
            
            // Unicode content
            ("unicode.txt", "Hello 世界 🌍 Привет мир 你好世界"),
            
            // Different line endings
            ("windows_endings.txt", "Line 1\r\nLine 2\r\nLine 3\r\n"),
            ("unix_endings.txt", "Line 1\nLine 2\nLine 3\n"),
            
            // Binary-like content in text file
            ("mixed_content.txt", "Text content\x00\x01\x02Binary data mixed in"),
            
            // Very long filename
            (&format!("very_long_filename_{}.txt", "a".repeat(100)), "Long filename content"),
            
            // Different encodings (simulated)
            ("latin1.txt", "Café naïve résumé"),
        ];

        for (path, content) in edge_case_files {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        Ok(TestFileStructure {
            temp_dir,
            root_path,
            file_paths,
        })
    }

    /// Get all document files from the structure
    pub fn get_documents(&self) -> Vec<&Path> {
        self.file_paths
            .iter()
            .filter(|path| {
                let path_str = path.to_string_lossy().to_lowercase();
                path_str.contains("documents/") || 
                path_str.contains("legal/") || 
                path_str.contains("projects/")
            })
            .map(|p| p.as_path())
            .collect()
    }

    /// Get all media files (images, videos, audio)
    pub fn get_media_files(&self) -> Vec<&Path> {
        self.file_paths
            .iter()
            .filter(|path| {
                let path_str = path.to_string_lossy().to_lowercase();
                path_str.contains("photos/") || 
                path_str.contains("videos/") || 
                path_str.contains("audio/")
            })
            .map(|p| p.as_path())
            .collect()
    }

    /// Get files by extension
    pub fn get_files_by_extension(&self, extension: &str) -> Vec<&Path> {
        self.file_paths
            .iter()
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case(extension))
                    .unwrap_or(false)
            })
            .map(|p| p.as_path())
            .collect()
    }

    /// Get files containing specific text in filename
    pub fn get_files_containing(&self, text: &str) -> Vec<&Path> {
        self.file_paths
            .iter()
            .filter(|path| {
                path.to_string_lossy()
                    .to_lowercase()
                    .contains(&text.to_lowercase())
            })
            .map(|p| p.as_path())
            .collect()
    }

    // Helper methods to create fake binary content for different file types
    
    fn create_fake_jpeg_content() -> Vec<u8> {
        let mut content = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        content.extend_from_slice(b"Fake JPEG content for testing purposes");
        content.extend_from_slice(&[0xFF, 0xD9]); // JPEG end marker
        content
    }

    fn create_fake_png_content() -> Vec<u8> {
        let mut content = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG signature
        content.extend_from_slice(b"Fake PNG content for testing");
        content
    }

    fn create_fake_mp4_content() -> Vec<u8> {
        let mut content = vec![0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70]; // MP4 header
        content.extend_from_slice(b"Fake MP4 video content for testing purposes");
        content
    }

    fn create_fake_avi_content() -> Vec<u8> {
        let mut content = vec![0x52, 0x49, 0x46, 0x46]; // RIFF header
        content.extend_from_slice(b"Fake AVI video content");
        content
    }

    fn create_fake_mov_content() -> Vec<u8> {
        let mut content = vec![0x00, 0x00, 0x00, 0x14, 0x66, 0x74, 0x79, 0x70]; // QuickTime header
        content.extend_from_slice(b"Fake QuickTime MOV content");
        content
    }

    fn create_fake_mp3_content() -> Vec<u8> {
        let mut content = vec![0xFF, 0xFB, 0x90, 0x00]; // MP3 header
        content.extend_from_slice(b"Fake MP3 audio content for testing");
        content
    }

    fn create_fake_wav_content() -> Vec<u8> {
        let mut content = vec![0x52, 0x49, 0x46, 0x46]; // RIFF header
        content.extend_from_slice(&[0x24, 0x00, 0x00, 0x00]); // File size
        content.extend_from_slice(b"WAVE");
        content.extend_from_slice(b"Fake WAV audio content");
        content
    }

    fn create_fake_zip_content() -> Vec<u8> {
        let mut content = vec![0x50, 0x4B, 0x03, 0x04]; // ZIP local file header
        content.extend_from_slice(b"Fake ZIP archive content for testing");
        content
    }

    fn create_fake_tar_content() -> Vec<u8> {
        let mut content = vec![0x1F, 0x8B]; // GZIP header
        content.extend_from_slice(b"Fake TAR.GZ archive content for testing purposes");
        content
    }

    fn create_fake_receipt_content() -> Vec<u8> {
        // Create a fake receipt image with some OCR-readable text
        let mut content = Self::create_fake_jpeg_content();
        content.extend_from_slice(b"\nRECEIPT\nStore: TestMart\nDate: 03/15/2024\nTotal: $42.99");
        content
    }
}

/// Utility functions for creating specific test scenarios
pub struct TestScenarios;

impl TestScenarios {
    /// Creates a scenario with duplicate filenames in different directories
    pub fn duplicate_names() -> std::io::Result<TestFileStructure> {
        let temp_dir = tempdir()?;
        let root_path = temp_dir.path().to_path_buf();
        let mut file_paths = Vec::new();

        fs::create_dir_all(root_path.join("folder1"))?;
        fs::create_dir_all(root_path.join("folder2"))?;
        fs::create_dir_all(root_path.join("folder3"))?;

        let duplicate_files = vec![
            ("folder1/document.txt", "Content from folder 1"),
            ("folder2/document.txt", "Content from folder 2"),
            ("folder3/document.txt", "Content from folder 3"),
            ("folder1/report.pdf", "Report from folder 1"),
            ("folder2/report.pdf", "Report from folder 2"),
        ];

        for (path, content) in duplicate_files {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            file_paths.push(file_path);
        }

        Ok(TestFileStructure {
            temp_dir,
            root_path,
            file_paths,
        })
    }

    /// Creates a scenario with nested directory structure
    pub fn deep_nesting() -> std::io::Result<TestFileStructure> {
        let temp_dir = tempdir()?;
        let root_path = temp_dir.path().to_path_buf();
        let mut file_paths = Vec::new();

        let nested_structure = vec![
            "level1/level2/level3/level4/deep_file.txt",
            "level1/level2/another_file.txt",
            "level1/file_at_level1.txt",
            "deeply/nested/folder/structure/with/many/levels/final_file.txt",
        ];

        for path in nested_structure {
            let file_path = root_path.join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&file_path, format!("Content at path: {}", path))?;
            file_paths.push(file_path);
        }

        Ok(TestFileStructure {
            temp_dir,
            root_path,
            file_paths,
        })
    }

    /// Creates a scenario with mixed file permissions (on Unix systems)
    #[cfg(unix)]
    pub fn mixed_permissions() -> std::io::Result<TestFileStructure> {
        use std::os::unix::fs::PermissionsExt;
        
        let temp_dir = tempdir()?;
        let root_path = temp_dir.path().to_path_buf();
        let mut file_paths = Vec::new();

        let permission_tests = vec![
            ("readable.txt", "Readable file", 0o644),
            ("readonly.txt", "Read-only file", 0o444),
            ("executable.sh", "#!/bin/bash\necho 'test'", 0o755),
        ];

        for (path, content, mode) in permission_tests {
            let file_path = root_path.join(path);
            fs::write(&file_path, content)?;
            
            let mut permissions = fs::metadata(&file_path)?.permissions();
            permissions.set_mode(mode);
            fs::set_permissions(&file_path, permissions)?;
            
            file_paths.push(file_path);
        }

        Ok(TestFileStructure {
            temp_dir,
            root_path,
            file_paths,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_comprehensive_structure() {
        let structure = TestFileStructure::new().unwrap();
        
        assert!(!structure.file_paths.is_empty());
        assert!(structure.file_paths.len() > 20); // Should have many test files
        
        // Verify some key directories exist
        assert!(structure.root_path.join("documents").exists());
        assert!(structure.root_path.join("photos").exists());
        assert!(structure.root_path.join("videos").exists());
        
        // Verify some files exist
        let contract_exists = structure.file_paths.iter()
            .any(|path| path.to_string_lossy().contains("contract"));
        assert!(contract_exists);
    }

    #[test]
    fn test_minimal_structure() {
        let structure = TestFileStructure::minimal().unwrap();
        
        assert_eq!(structure.file_paths.len(), 3);
        
        // Verify all files exist
        for file_path in &structure.file_paths {
            assert!(file_path.exists());
        }
    }

    #[test]
    fn test_edge_cases_structure() {
        let structure = TestFileStructure::edge_cases().unwrap();
        
        // Should have various edge case files
        assert!(!structure.file_paths.is_empty());
        
        // Check for empty file
        let empty_file_exists = structure.file_paths.iter()
            .any(|path| path.file_name()
                .and_then(|name| name.to_str())
                .map_or(false, |name| name == "empty.txt"));
        assert!(empty_file_exists);
        
        // Check for unicode content file
        let unicode_file_exists = structure.file_paths.iter()
            .any(|path| path.file_name()
                .and_then(|name| name.to_str())
                .map_or(false, |name| name == "unicode.txt"));
        assert!(unicode_file_exists);
    }

    #[test]
    fn test_file_filtering_methods() {
        let structure = TestFileStructure::new().unwrap();
        
        // Test getting documents
        let documents = structure.get_documents();
        assert!(!documents.is_empty());
        
        // Test getting media files
        let media = structure.get_media_files();
        assert!(!media.is_empty());
        
        // Test getting files by extension
        let pdf_files = structure.get_files_by_extension("pdf");
        assert!(!pdf_files.is_empty());
        
        // Test getting files containing text
        let contract_files = structure.get_files_containing("contract");
        assert!(!contract_files.is_empty());
    }

    #[test]
    fn test_duplicate_names_scenario() {
        let structure = TestScenarios::duplicate_names().unwrap();
        
        // Should have multiple files with same names
        let document_files = structure.get_files_containing("document.txt");
        assert_eq!(document_files.len(), 3);
        
        let report_files = structure.get_files_containing("report.pdf");
        assert_eq!(report_files.len(), 2);
    }

    #[test]
    fn test_deep_nesting_scenario() {
        let structure = TestScenarios::deep_nesting().unwrap();
        
        // Should have files at various nesting levels
        assert!(!structure.file_paths.is_empty());
        
        // Check for deeply nested file
        let deep_file_exists = structure.file_paths.iter()
            .any(|path| path.to_string_lossy().contains("level4"));
        assert!(deep_file_exists);
    }

    #[test]
    fn test_fake_binary_content() {
        let jpeg_content = TestFileStructure::create_fake_jpeg_content();
        assert!(jpeg_content.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0])); // JPEG header
        
        let png_content = TestFileStructure::create_fake_png_content();
        assert!(png_content.starts_with(&[0x89, 0x50, 0x4E, 0x47])); // PNG signature
        
        let mp4_content = TestFileStructure::create_fake_mp4_content();
        assert!(mp4_content.starts_with(&[0x00, 0x00, 0x00, 0x20])); // MP4 header start
    }
}