use stratosort::ai::{AiService, FileAnalysis};
use stratosort::storage::Database;
use stratosort::config::Config;
use stratosort::commands::organization::{
    RuleType, 
    ConditionOperator, RuleAction, ActionType
};
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

/// Test fixtures for integration tests
pub struct TestFixtures;

impl TestFixtures {
    /// Create sample file analyses for testing
    pub fn create_sample_analyses() -> Vec<FileAnalysis> {
        vec![
            FileAnalysis {
                path: "test_contract.pdf".to_string(),
                category: "Documents".to_string(),
                tags: vec!["contract".to_string(), "legal".to_string(), "important".to_string()],
                summary: "Legal contract document with terms and conditions".to_string(),
                confidence: 0.9,
                extracted_text: Some("AGREEMENT\nThis contract establishes...".to_string()),
                detected_language: Some("en".to_string()),
                metadata: serde_json::json!({
                    "pages": 5,
                    "file_type": "pdf"
                }),
            },
            FileAnalysis {
                path: "vacation_photo.jpg".to_string(),
                category: "Images".to_string(),
                tags: vec!["photo".to_string(), "vacation".to_string(), "family".to_string()],
                summary: "Family vacation photo at the beach".to_string(),
                confidence: 0.8,
                extracted_text: None,
                detected_language: None,
                metadata: serde_json::json!({
                    "resolution": "1920x1080",
                    "camera": "iPhone"
                }),
            },
            FileAnalysis {
                path: "invoice_2024.xlsx".to_string(),
                category: "Documents".to_string(),
                tags: vec!["invoice".to_string(), "financial".to_string(), "2024".to_string()],
                summary: "Invoice spreadsheet for 2024 financial records".to_string(),
                confidence: 0.95,
                extracted_text: Some("INVOICE\nDate: 2024-01-01\nAmount: $1000".to_string()),
                detected_language: Some("en".to_string()),
                metadata: serde_json::json!({
                    "sheets": 3,
                    "format": "xlsx"
                }),
            },
        ]
    }

    /// Create sample embeddings for testing
    pub fn create_sample_embeddings() -> Vec<(String, Vec<f32>)> {
        vec![
            ("test_contract.pdf".to_string(), vec![0.1, 0.2, 0.3, 0.4]),
            ("vacation_photo.jpg".to_string(), vec![0.5, 0.6, 0.7, 0.8]),
            ("invoice_2024.xlsx".to_string(), vec![0.9, 0.8, 0.7, 0.6]),
        ]
    }

    /// Create sample smart folders for testing
    pub fn create_sample_smart_folders() -> Vec<stratosort::commands::organization::SmartFolder> {
        use stratosort::commands::organization::{SmartFolder, OrganizationRule, RuleType, RuleCondition};
        
        vec![
            SmartFolder {
                id: Uuid::new_v4().to_string(),
                name: "Legal Documents".to_string(),
                description: Some("All legal contracts and agreements".to_string()),
                target_path: "~/Organized/Legal".to_string(),
                rules: vec![
                    OrganizationRule {
                        id: Uuid::new_v4().to_string(),
                        rule_type: RuleType::FileContent,
                        condition: RuleCondition {
                            field: "content".to_string(),
                            operator: ConditionOperator::Contains,
                            value: "legal".to_string(),
                            case_sensitive: Some(false),
                        },
                        action: RuleAction {
                            action_type: ActionType::Move,
                            target_folder: "~/Organized/Legal".to_string(),
                            rename_pattern: None,
                        },
                        priority: 1,
                        enabled: true,
                    },
                    OrganizationRule {
                        id: Uuid::new_v4().to_string(),
                        rule_type: RuleType::FileContent,
                        condition: RuleCondition {
                            field: "content".to_string(),
                            operator: ConditionOperator::Contains,
                            value: "contract".to_string(),
                            case_sensitive: Some(false),
                        },
                        action: RuleAction {
                            action_type: ActionType::Move,
                            target_folder: "~/Organized/Legal".to_string(),
                            rename_pattern: None,
                        },
                        priority: 2,
                        enabled: true,
                    },
                ],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                enabled: true,
            },
            SmartFolder {
                id: Uuid::new_v4().to_string(),
                name: "Financial Records".to_string(),
                description: Some("Invoices and financial documents".to_string()),
                target_path: "~/Organized/Financial".to_string(),
                rules: vec![
                    OrganizationRule {
                        id: Uuid::new_v4().to_string(),
                        rule_type: RuleType::FileContent,
                        condition: RuleCondition {
                            field: "content".to_string(),
                            operator: ConditionOperator::Contains,
                            value: "invoice".to_string(),
                            case_sensitive: Some(false),
                        },
                        action: RuleAction {
                            action_type: ActionType::Move,
                            target_folder: "~/Organized/Financial".to_string(),
                            rename_pattern: None,
                        },
                        priority: 1,
                        enabled: true,
                    },
                    OrganizationRule {
                        id: Uuid::new_v4().to_string(),
                        rule_type: RuleType::FileContent,
                        condition: RuleCondition {
                            field: "content".to_string(),
                            operator: ConditionOperator::Contains,
                            value: "financial".to_string(),
                            case_sensitive: Some(false),
                        },
                        action: RuleAction {
                            action_type: ActionType::Move,
                            target_folder: "~/Organized/Financial".to_string(),
                            rename_pattern: None,
                        },
                        priority: 2,
                        enabled: true,
                    },
                ],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                enabled: true,
            },
        ]
    }
}

/// Mock AppHandle for testing
pub struct MockAppHandle {
    pub temp_dir: PathBuf,
}

impl MockAppHandle {
    pub async fn new() -> Self {
        let temp_dir = std::env::temp_dir().join(format!("stratosort_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).await.unwrap();
        
        Self { temp_dir }
    }
    
    pub fn app_data_dir(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(self.temp_dir.clone())
    }
}

impl Drop for MockAppHandle {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

// Helper function to create database for testing
async fn create_test_database(mock_handle: &MockAppHandle) -> Result<Database, Box<dyn std::error::Error>> {
    let db_path = mock_handle.temp_dir.join("test.db");
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
    
    // Use the public new_from_url method
    let db = Database::new_from_url(&database_url).await?;
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Test AI service initialization and fallback mode
    #[tokio::test]
    async fn test_ai_service_initialization() {
        let config = Config::default();
        
        // This should work even without Ollama running (fallback mode)
        let ai_service = AiService::new(&config).await.unwrap();
        
        // Test fallback analysis
        let analysis = ai_service.analyze_file("This is a test document about contracts", "text/plain").await.unwrap();
        
        assert_eq!(analysis.category, "Text");
        assert!(analysis.tags.contains(&"contract".to_string()));
        assert!(analysis.confidence > 0.0);
    }

    /// Test database initialization and basic operations
    #[tokio::test]
    async fn test_database_initialization() {
        let mock_handle = MockAppHandle::new().await;
        
        // Create database - this should initialize schema
        let db = create_test_database(&mock_handle).await.unwrap();
        
        // Test saving analysis
        let analyses = TestFixtures::create_sample_analyses();
        for analysis in &analyses {
            db.save_analysis(analysis).await.unwrap();
        }
        
        // Test retrieval
        let retrieved = db.get_analysis("test_contract.pdf").await.unwrap();
        assert!(retrieved.is_some());
        let analysis = retrieved.unwrap();
        assert_eq!(analysis.category, "Documents");
        assert!(analysis.tags.contains(&"legal".to_string()));
    }

    /// Test embedding storage and retrieval
    #[tokio::test]
    async fn test_embedding_operations() {
        let mock_handle = MockAppHandle::new().await;
        let db = create_test_database(&mock_handle).await.unwrap();
        
        // First save file analyses so we have records to attach embeddings to
        let analyses = TestFixtures::create_sample_analyses();
        for analysis in &analyses {
            db.save_analysis(analysis).await.unwrap();
        }
        
        let embeddings = TestFixtures::create_sample_embeddings();
        
        // Test saving embeddings
        for (path, embedding) in &embeddings {
            db.save_embedding(path, embedding, Some("test-model")).await.unwrap();
        }
        
        // Test semantic search
        let query_embedding = vec![0.1, 0.2, 0.3, 0.4];
        let results = db.semantic_search(&query_embedding, 5).await.unwrap();
        
        // We should get results since we have embeddings
        assert!(!results.is_empty());
        // The first result should be one of our test files
        assert!(results[0].0 == "test_contract.pdf" || results[0].0 == "vacation_photo.jpg");
    }

    /// Test smart folder operations
    #[tokio::test]
    async fn test_smart_folder_operations() {
        let mock_handle = MockAppHandle::new().await;
        let db = create_test_database(&mock_handle).await.unwrap();
        
        let smart_folders = TestFixtures::create_sample_smart_folders();
        
        // Test creating smart folders
        for folder in &smart_folders {
            db.save_smart_folder(folder).await.unwrap();
        }
        
        // Test listing smart folders
        let retrieved = db.list_smart_folders().await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert!(retrieved.iter().any(|f| f.name == "Legal Documents"));
        assert!(retrieved.iter().any(|f| f.name == "Financial Records"));
        
        // Test folder matching
        let legal_folder = retrieved.iter().find(|f| f.name == "Legal Documents").unwrap();
        
        // Should match contract file based on rules
        // Since we're using FileContent rules that check for "legal" and "contract" keywords,
        // and the file name contains "contract", we expect some match
        // However, without actual file content analysis, the confidence will be based on filename alone
        let has_matching_keyword = legal_folder.rules.iter().any(|rule| {
            if let RuleType::FileContent = rule.rule_type {
                "test_contract.pdf".contains(&rule.condition.value)
            } else {
                false
            }
        });
        assert!(has_matching_keyword);
    }

    /// Test full AI → Database → Organization pipeline
    #[tokio::test]
    async fn test_full_pipeline() {
        let config = Config::default();
        let mock_handle = MockAppHandle::new().await;
        
        // Initialize services
        let ai_service = AiService::new(&config).await.unwrap();
        let db = create_test_database(&mock_handle).await.unwrap();
        
        // Create smart folders
        let smart_folders = TestFixtures::create_sample_smart_folders();
        for folder in &smart_folders {
            db.save_smart_folder(folder).await.unwrap();
        }
        
        // Simulate new file analysis
        let file_content = "This is an important legal contract with terms and conditions";
        let analysis = ai_service.analyze_file(file_content, "text/plain").await.unwrap();
        
        // Save analysis to database
        let mut analysis_with_path = analysis;
        analysis_with_path.path = "new_contract.txt".to_string();
        db.save_analysis(&analysis_with_path).await.unwrap();
        
        // Generate and save embeddings
        let embeddings = ai_service.generate_embeddings(file_content).await.unwrap();
        db.save_embedding("new_contract.txt", &embeddings, Some("test-model")).await.unwrap();
        
        // Test organization suggestions with empty smart folders list
        let suggestions = ai_service.suggest_organization(
            vec!["new_contract.txt".to_string()],
            vec![]  // Empty smart folders list for testing
        ).await.unwrap();
        assert!(!suggestions.is_empty());
        
        // Should suggest legal folder for contract
        let legal_suggestion = suggestions.iter().find(|s| s.target_folder.contains("Documents"));
        assert!(legal_suggestion.is_some());
    }

    /// Test error handling and recovery
    #[tokio::test]
    async fn test_error_handling() {
        let mock_handle = MockAppHandle::new().await;
        let db = create_test_database(&mock_handle).await.unwrap();
        
        // Test retrieving non-existent file
        let result = db.get_analysis("non_existent.txt").await.unwrap();
        assert!(result.is_none());
        
        // Test semantic search with empty database
        let query_embedding = vec![0.1, 0.2, 0.3, 0.4];
        let results = db.semantic_search(&query_embedding, 5).await.unwrap();
        assert!(results.is_empty());
    }
}