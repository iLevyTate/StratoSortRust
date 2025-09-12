use tempfile::TempDir;
use stratosort::{
    commands::organization::{
        OrganizationRule, RuleType, RuleCondition, ConditionOperator, 
        RuleAction, ActionType, SmartFolder
    },
    storage::Database,
};

#[tokio::test]
async fn test_smart_folder_creation_and_organization() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path().to_path_buf();

    // Initialize the database with the temp directory
    let db = Database::new_test(&base_path.join("test.db")).await?;

    // Test creating a smart folder for 3D printing files
    let print_3d_folder = SmartFolder {
        id: "3d_print_folder".to_string(),
        name: "3D Print".to_string(),
        description: Some("Smart folder for 3D printing files".to_string()),
        rules: vec![
            OrganizationRule {
                id: "3d_rule_1".to_string(),
                rule_type: RuleType::FileExtension,
                condition: RuleCondition {
                    field: "extension".to_string(),
                    operator: ConditionOperator::Equals,
                    value: "stl".to_string(),
                    case_sensitive: Some(false),
                },
                action: RuleAction {
                    action_type: ActionType::Move,
                    target_folder: "stl_files".to_string(),
                    rename_pattern: None,
                },
                priority: 1,
                enabled: true,
            },
        ],
        target_path: "/organized/3d_print".to_string(),
        enabled: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Test that we can serialize and work with the smart folder
    println!("Created smart folder: {:?}", print_3d_folder.name);
    println!("Rules count: {}", print_3d_folder.rules.len());

    // Test rule validation
    let rule = &print_3d_folder.rules[0];
    assert_eq!(rule.rule_type, RuleType::FileExtension);
    assert_eq!(rule.condition.operator, ConditionOperator::Equals);
    assert_eq!(rule.condition.value, "stl");
    assert_eq!(rule.action.action_type, ActionType::Move);
    assert_eq!(rule.action.target_folder, "stl_files");

    Ok(())
}

#[tokio::test]
async fn test_document_smart_folder() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path().to_path_buf();
    let db = Database::new_test(&base_path.join("test.db")).await?;

    let documents_folder = SmartFolder {
        id: "documents_folder".to_string(),
        name: "Documents".to_string(),
        description: Some("Smart folder for document files".to_string()),
        rules: vec![
            OrganizationRule {
                id: "doc_rule_1".to_string(),
                rule_type: RuleType::FileExtension,
                condition: RuleCondition {
                    field: "extension".to_string(),
                    operator: ConditionOperator::Equals,
                    value: "pdf".to_string(),
                    case_sensitive: Some(false),
                },
                action: RuleAction {
                    action_type: ActionType::Move,
                    target_folder: "pdf_files".to_string(),
                    rename_pattern: None,
                },
                priority: 1,
                enabled: true,
            },
            OrganizationRule {
                id: "doc_rule_2".to_string(),
                rule_type: RuleType::FileExtension,
                condition: RuleCondition {
                    field: "extension".to_string(),
                    operator: ConditionOperator::Equals,
                    value: "docx".to_string(),
                    case_sensitive: Some(false),
                },
                action: RuleAction {
                    action_type: ActionType::Move,
                    target_folder: "word_docs".to_string(),
                    rename_pattern: None,
                },
                priority: 2,
                enabled: true,
            },
        ],
        target_path: "/organized/documents".to_string(),
        enabled: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    println!("Created documents folder with {} rules", documents_folder.rules.len());
    assert_eq!(documents_folder.rules.len(), 2);

    Ok(())
}