use stratosort::ai::FileAnalysis;
use stratosort::core::smart_folders::SmartFolder;
use uuid::Uuid;
use chrono::Utc;

/// Creates sample file analyses for testing
pub fn create_sample_analyses() -> Vec<FileAnalysis> {
    vec![
        FileAnalysis {
            path: "/documents/contract_2024.pdf".to_string(),
            category: "Documents".to_string(),
            tags: vec!["contract".to_string(), "legal".to_string(), "2024".to_string()],
            summary: "Software development service agreement for 2024".to_string(),
            confidence: 0.95,
            extracted_text: Some("SOFTWARE DEVELOPMENT AGREEMENT\nThis agreement is between...".to_string()),
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "pages": 12,
                "created_date": "2024-01-15",
                "author": "Legal Department"
            }),
        },
        FileAnalysis {
            path: "/invoices/invoice_001_2024.pdf".to_string(),
            category: "Documents".to_string(),
            tags: vec!["invoice".to_string(), "financial".to_string(), "payment".to_string()],
            summary: "Invoice for Q1 2024 consulting services - $15,000".to_string(),
            confidence: 0.92,
            extracted_text: Some("INVOICE #001-2024\nBill To: Client Corp\nAmount: $15,000.00".to_string()),
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "amount": 15000.00,
                "currency": "USD",
                "due_date": "2024-02-15"
            }),
        },
        FileAnalysis {
            path: "/reports/monthly_report_march.xlsx".to_string(),
            category: "Documents".to_string(),
            tags: vec!["report".to_string(), "monthly".to_string(), "spreadsheet".to_string()],
            summary: "March 2024 financial and operational performance report".to_string(),
            confidence: 0.88,
            extracted_text: None,
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "sheets": 5,
                "charts": 12,
                "last_modified": "2024-03-31"
            }),
        },
        FileAnalysis {
            path: "/presentations/q1_review.pptx".to_string(),
            category: "Documents".to_string(),
            tags: vec!["presentation".to_string(), "quarterly".to_string(), "review".to_string()],
            summary: "Q1 2024 business review and performance metrics presentation".to_string(),
            confidence: 0.87,
            extracted_text: None,
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "slides": 24,
                "template": "corporate",
                "presentation_date": "2024-04-05"
            }),
        },
        FileAnalysis {
            path: "/photos/team_photo_2024.jpg".to_string(),
            category: "Images".to_string(),
            tags: vec!["photo".to_string(), "team".to_string(), "corporate".to_string()],
            summary: "Company team photo taken at annual conference 2024".to_string(),
            confidence: 0.82,
            extracted_text: None,
            detected_language: None,
            metadata: serde_json::json!({
                "width": 3840,
                "height": 2160,
                "camera": "Canon EOS R5",
                "taken_date": "2024-03-20"
            }),
        },
        FileAnalysis {
            path: "/videos/training_video.mp4".to_string(),
            category: "Videos".to_string(),
            tags: vec!["training".to_string(), "educational".to_string(), "onboarding".to_string()],
            summary: "Employee onboarding and training video - safety procedures".to_string(),
            confidence: 0.91,
            extracted_text: None,
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "duration": 1825, // seconds
                "resolution": "1920x1080",
                "codec": "H.264",
                "size_bytes": 245760000
            }),
        },
        FileAnalysis {
            path: "/audio/podcast_episode_05.mp3".to_string(),
            category: "Audio".to_string(),
            tags: vec!["podcast".to_string(), "interview".to_string(), "technology".to_string()],
            summary: "Tech Talk Podcast Episode 5: AI in Business - Interview with Industry Expert".to_string(),
            confidence: 0.85,
            extracted_text: None,
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "duration": 3600, // seconds
                "bitrate": 128,
                "artist": "Tech Talk Podcast",
                "album": "Season 1"
            }),
        },
        FileAnalysis {
            path: "/archives/project_backup_2024.zip".to_string(),
            category: "Archives".to_string(),
            tags: vec!["backup".to_string(), "project".to_string(), "archive".to_string()],
            summary: "Complete project backup archive containing source code and documentation".to_string(),
            confidence: 0.89,
            extracted_text: None,
            detected_language: None,
            metadata: serde_json::json!({
                "compressed_size": 52428800, // 50MB
                "uncompressed_size": 157286400, // 150MB
                "files_count": 1247,
                "compression_ratio": 0.33
            }),
        },
        FileAnalysis {
            path: "/receipts/office_supplies_receipt.png".to_string(),
            category: "Images".to_string(),
            tags: vec!["receipt".to_string(), "expense".to_string(), "office".to_string(), "supplies".to_string()],
            summary: "Receipt for office supplies purchase from OfficeMax - $127.45".to_string(),
            confidence: 0.94,
            extracted_text: Some("OFFICEMAX RECEIPT\nDate: 03/15/2024\nTotal: $127.45\nItems: Paper, pens, folders".to_string()),
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "width": 1080,
                "height": 1920,
                "amount": 127.45,
                "vendor": "OfficeMax",
                "date": "2024-03-15"
            }),
        },
        FileAnalysis {
            path: "/legal/privacy_policy_v2.docx".to_string(),
            category: "Documents".to_string(),
            tags: vec!["legal".to_string(), "privacy".to_string(), "policy".to_string(), "compliance".to_string()],
            summary: "Updated privacy policy document v2.0 - GDPR and CCPA compliance".to_string(),
            confidence: 0.96,
            extracted_text: Some("PRIVACY POLICY VERSION 2.0\nEffective Date: April 1, 2024\nThis policy describes...".to_string()),
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({
                "version": "2.0",
                "effective_date": "2024-04-01",
                "pages": 8,
                "compliance": ["GDPR", "CCPA"]
            }),
        }
    ]
}

/// Creates sample smart folders for testing
pub fn create_sample_smart_folders() -> Vec<SmartFolder> {
    let now = Utc::now();
    
    vec![
        SmartFolder {
            id: Uuid::parse_str("01234567-89ab-cdef-0123-456789abcdef").unwrap(),
            name: "Financial Documents".to_string(),
            description: "Automatically organize invoices, receipts, and financial reports".to_string(),
            query: "category:Documents AND (tags:invoice OR tags:receipt OR tags:financial)".to_string(),
            auto_organize: true,
            target_path: "/organized/financial".to_string(),
            created_at: now - chrono::Duration::days(30),
            updated_at: now - chrono::Duration::days(1),
        },
        SmartFolder {
            id: Uuid::parse_str("fedcba98-7654-3210-fedc-ba9876543210").unwrap(),
            name: "Legal Documents".to_string(),
            description: "Contracts, agreements, and legal documentation".to_string(),
            query: "category:Documents AND (tags:contract OR tags:legal OR tags:agreement)".to_string(),
            auto_organize: true,
            target_path: "/organized/legal".to_string(),
            created_at: now - chrono::Duration::days(25),
            updated_at: now - chrono::Duration::days(5),
        },
        SmartFolder {
            id: Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap(),
            name: "Media Files".to_string(),
            description: "Photos, videos, and audio files".to_string(),
            query: "category:Images OR category:Videos OR category:Audio".to_string(),
            auto_organize: false,
            target_path: "/organized/media".to_string(),
            created_at: now - chrono::Duration::days(20),
            updated_at: now - chrono::Duration::days(10),
        },
        SmartFolder {
            id: Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap(),
            name: "Reports and Analytics".to_string(),
            description: "Business reports, analytics, and performance data".to_string(),
            query: "category:Documents AND (tags:report OR tags:analytics OR tags:metrics)".to_string(),
            auto_organize: true,
            target_path: "/organized/reports".to_string(),
            created_at: now - chrono::Duration::days(15),
            updated_at: now - chrono::Duration::days(2),
        },
        SmartFolder {
            id: Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap(),
            name: "Training Materials".to_string(),
            description: "Educational content, training videos, and onboarding materials".to_string(),
            query: "tags:training OR tags:educational OR tags:onboarding".to_string(),
            auto_organize: false,
            target_path: "/organized/training".to_string(),
            created_at: now - chrono::Duration::days(10),
            updated_at: now - chrono::Duration::days(3),
        }
    ]
}

/// Creates sample embeddings data for testing semantic search
pub fn create_sample_embeddings() -> Vec<(String, Vec<f32>)> {
    vec![
        (
            "/documents/contract_2024.pdf".to_string(),
            vec![0.1, 0.2, 0.15, 0.8, 0.05, 0.9, 0.3, 0.7, 0.4, 0.6] // Contract-related embedding
        ),
        (
            "/documents/privacy_policy_v2.docx".to_string(),
            vec![0.12, 0.18, 0.16, 0.85, 0.04, 0.88, 0.31, 0.72, 0.38, 0.58] // Similar to contract
        ),
        (
            "/invoices/invoice_001_2024.pdf".to_string(),
            vec![0.9, 0.1, 0.8, 0.2, 0.7, 0.15, 0.6, 0.25, 0.5, 0.3] // Financial document embedding
        ),
        (
            "/receipts/office_supplies_receipt.png".to_string(),
            vec![0.85, 0.12, 0.75, 0.22, 0.68, 0.18, 0.58, 0.28, 0.48, 0.32] // Similar to invoice
        ),
        (
            "/reports/monthly_report_march.xlsx".to_string(),
            vec![0.3, 0.7, 0.4, 0.6, 0.2, 0.8, 0.1, 0.9, 0.35, 0.65] // Report-related embedding
        ),
        (
            "/presentations/q1_review.pptx".to_string(),
            vec![0.28, 0.72, 0.38, 0.62, 0.18, 0.82, 0.08, 0.92, 0.33, 0.67] // Similar to report
        ),
        (
            "/photos/team_photo_2024.jpg".to_string(),
            vec![0.5, 0.5, 0.6, 0.4, 0.55, 0.45, 0.52, 0.48, 0.58, 0.42] // Image embedding
        ),
        (
            "/videos/training_video.mp4".to_string(),
            vec![0.2, 0.4, 0.3, 0.7, 0.25, 0.75, 0.35, 0.65, 0.28, 0.72] // Video/training embedding
        ),
        (
            "/audio/podcast_episode_05.mp3".to_string(),
            vec![0.22, 0.42, 0.32, 0.68, 0.27, 0.73, 0.37, 0.63, 0.3, 0.7] // Similar to video
        ),
        (
            "/archives/project_backup_2024.zip".to_string(),
            vec![0.45, 0.35, 0.5, 0.5, 0.4, 0.6, 0.42, 0.58, 0.47, 0.53] // Archive embedding
        )
    ]
}

/// Creates sample operation history for testing
pub fn create_sample_operation_history() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": "op-001",
            "operation_type": "file_analysis",
            "description": "Analyzed 5 documents in /downloads",
            "timestamp": 1710000000, // March 10, 2024
            "success": true,
            "details": {
                "files_processed": 5,
                "duration_ms": 2340,
                "ai_provider": "ollama"
            }
        }),
        serde_json::json!({
            "id": "op-002", 
            "operation_type": "file_organization",
            "description": "Organized 12 files by type",
            "timestamp": 1710003600, // 1 hour later
            "success": true,
            "details": {
                "files_moved": 12,
                "folders_created": 3,
                "target_directory": "/organized"
            }
        }),
        serde_json::json!({
            "id": "op-003",
            "operation_type": "smart_folder_creation",
            "description": "Created smart folder: Financial Documents",
            "timestamp": 1710007200, // 2 hours later
            "success": true,
            "details": {
                "folder_name": "Financial Documents",
                "query": "category:Documents AND tags:financial",
                "auto_organize": true
            }
        }),
        serde_json::json!({
            "id": "op-004",
            "operation_type": "bulk_move",
            "description": "Moved 25 files to new locations",
            "timestamp": 1710010800, // 3 hours later
            "success": false,
            "details": {
                "attempted_files": 25,
                "successful_moves": 23,
                "failed_moves": 2,
                "error": "Permission denied for 2 files"
            }
        }),
        serde_json::json!({
            "id": "op-005",
            "operation_type": "ai_embedding_generation",
            "description": "Generated embeddings for 50 documents",
            "timestamp": 1710014400, // 4 hours later
            "success": true,
            "details": {
                "documents_processed": 50,
                "embedding_dimension": 768,
                "processing_time_ms": 45000
            }
        })
    ]
}

/// Creates a comprehensive dataset for development and testing
pub struct SampleDataset {
    pub analyses: Vec<FileAnalysis>,
    pub smart_folders: Vec<SmartFolder>,
    pub embeddings: Vec<(String, Vec<f32>)>,
    pub operation_history: Vec<serde_json::Value>,
}

impl SampleDataset {
    pub fn new() -> Self {
        Self {
            analyses: create_sample_analyses(),
            smart_folders: create_sample_smart_folders(),
            embeddings: create_sample_embeddings(),
            operation_history: create_sample_operation_history(),
        }
    }

    /// Returns a subset of data for unit testing
    pub fn minimal() -> Self {
        let mut dataset = Self::new();
        
        // Keep only first 3 items of each type for minimal testing
        dataset.analyses.truncate(3);
        dataset.smart_folders.truncate(2);
        dataset.embeddings.truncate(3);
        dataset.operation_history.truncate(2);
        
        dataset
    }

    /// Returns a full dataset for integration testing
    pub fn full() -> Self {
        Self::new()
    }

    /// Returns data filtered by category
    pub fn by_category(category: &str) -> Vec<FileAnalysis> {
        Self::new()
            .analyses
            .into_iter()
            .filter(|analysis| analysis.category == category)
            .collect()
    }

    /// Returns data filtered by tags
    pub fn by_tags(required_tags: &[&str]) -> Vec<FileAnalysis> {
        Self::new()
            .analyses
            .into_iter()
            .filter(|analysis| {
                required_tags.iter().all(|&tag| {
                    analysis.tags.iter().any(|analysis_tag| analysis_tag.contains(tag))
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sample_analyses() {
        let analyses = create_sample_analyses();
        assert_eq!(analyses.len(), 10);
        
        // Test that all analyses have required fields
        for analysis in &analyses {
            assert!(!analysis.path.is_empty());
            assert!(!analysis.category.is_empty());
            assert!(!analysis.summary.is_empty());
            assert!(analysis.confidence > 0.0 && analysis.confidence <= 1.0);
        }
    }

    #[test]
    fn test_create_sample_smart_folders() {
        let folders = create_sample_smart_folders();
        assert_eq!(folders.len(), 5);
        
        // Test that all folders have unique IDs
        let mut ids = std::collections::HashSet::new();
        for folder in &folders {
            assert!(ids.insert(folder.id));
            assert!(!folder.name.is_empty());
            assert!(!folder.query.is_empty());
            assert!(!folder.target_path.is_empty());
        }
    }

    #[test]
    fn test_create_sample_embeddings() {
        let embeddings = create_sample_embeddings();
        assert_eq!(embeddings.len(), 10);
        
        // Test that all embeddings have consistent dimensions
        let expected_dim = 10;
        for (path, embedding) in &embeddings {
            assert!(!path.is_empty());
            assert_eq!(embedding.len(), expected_dim);
            
            // Test that embeddings are normalized-ish (values between 0 and 1)
            for &value in embedding {
                assert!(value >= 0.0 && value <= 1.0);
            }
        }
    }

    #[test]
    fn test_sample_dataset() {
        let dataset = SampleDataset::new();
        
        assert!(!dataset.analyses.is_empty());
        assert!(!dataset.smart_folders.is_empty());
        assert!(!dataset.embeddings.is_empty());
        assert!(!dataset.operation_history.is_empty());
        
        // Test minimal dataset
        let minimal = SampleDataset::minimal();
        assert!(minimal.analyses.len() <= 3);
        assert!(minimal.smart_folders.len() <= 2);
    }

    #[test]
    fn test_dataset_filtering() {
        let document_analyses = SampleDataset::by_category("Documents");
        assert!(!document_analyses.is_empty());
        
        for analysis in &document_analyses {
            assert_eq!(analysis.category, "Documents");
        }
        
        let contract_analyses = SampleDataset::by_tags(&["contract"]);
        assert!(!contract_analyses.is_empty());
        
        for analysis in &contract_analyses {
            assert!(analysis.tags.iter().any(|tag| tag.contains("contract")));
        }
    }
}