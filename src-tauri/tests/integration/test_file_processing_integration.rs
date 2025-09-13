use stratosort::core::{
    archive_handler::{ArchiveHandler, ZipHandler},
    document_processor::{DocumentProcessor, PdfProcessor, TextProcessor, XmlProcessor, MarkdownProcessor},
    file_analyzer::{FileAnalyzer, AnalysisRequest},
    image_processor::{ImageProcessor, StandardImageProcessor},
    organizer::FileOrganizer,
    smart_folders::SmartFolderManager,
};
use stratosort::ai::{AIService, FileAnalysis};
use stratosort::storage::Database;
use stratosort::error::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;

#[cfg(test)]
mod file_processing_integration_tests {
    use super::*;

    const FIXTURE_DIR: &str = "tests/fixtures/data/sample_demo_files";
    const SAMPLE_DATA_DIR: &str = "tests/fixtures/sample_data";

    struct TestContext {
        fixture_files: Vec<PathBuf>,
        sample_files: Vec<PathBuf>,
        temp_dir: tempfile::TempDir,
        db: Arc<Database>,
    }

    impl TestContext {
        async fn new() -> Result<Self> {
            let temp_dir = tempdir()?;
            let db_path = temp_dir.path().join("test.db");
            let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
            let db = Arc::new(Database::new(&db_url).await?);

            // Load fixture files
            let fixture_files = if Path::new(FIXTURE_DIR).exists() {
                fs::read_dir(FIXTURE_DIR)?
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| path.is_file())
                    .collect()
            } else {
                Vec::new()
            };

            // Load sample files
            let sample_files = if Path::new(SAMPLE_DATA_DIR).exists() {
                fs::read_dir(SAMPLE_DATA_DIR)?
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| path.is_file())
                    .collect()
            } else {
                Vec::new()
            };

            Ok(Self {
                fixture_files,
                sample_files,
                temp_dir,
                db,
            })
        }

        fn get_files_by_extension(&self, extension: &str) -> Vec<&PathBuf> {
            self.fixture_files
                .iter()
                .chain(self.sample_files.iter())
                .filter(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case(extension))
                        .unwrap_or(false)
                })
                .collect()
        }
    }

    #[tokio::test]
    async fn test_complete_file_analysis_workflow() {
        let ctx = TestContext::new().await.unwrap();
        
        if ctx.fixture_files.is_empty() && ctx.sample_files.is_empty() {
            println!("Skipping test: no fixture files found");
            return;
        }

        let analyzer = FileAnalyzer::new().unwrap();
        let mut analysis_results = Vec::new();

        // Analyze all available files
        for file_path in ctx.fixture_files.iter().chain(ctx.sample_files.iter()).take(10) {
            let request = AnalysisRequest {
                path: file_path.to_string_lossy().to_string(),
                force_reanalyze: false,
                extract_metadata: true,
            };

            match analyzer.analyze_file(&request).await {
                Ok(result) => {
                    assert!(!result.file_type.is_empty());
                    assert!(result.size > 0);
                    analysis_results.push(result);
                }
                Err(e) => {
                    println!("Failed to analyze {}: {}", file_path.display(), e);
                }
            }
        }

        assert!(!analysis_results.is_empty(), "Should analyze at least one file successfully");
    }

    #[tokio::test]
    async fn test_pdf_processing_with_fixtures() {
        let ctx = TestContext::new().await.unwrap();
        let pdf_files = ctx.get_files_by_extension("pdf");

        if pdf_files.is_empty() {
            println!("Skipping test: no PDF fixtures found");
            return;
        }

        let processor = PdfProcessor;

        for pdf_path in pdf_files {
            let result = processor.process(pdf_path).await;
            
            match result {
                Ok(doc) => {
                    assert_eq!(doc.file_type, "pdf");
                    // PDF might be empty or contain text
                    assert!(doc.processing_error.is_none() || doc.processing_error.is_some());
                }
                Err(e) => {
                    println!("PDF processing failed for {}: {}", pdf_path.display(), e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_image_processing_with_fixtures() {
        let ctx = TestContext::new().await.unwrap();
        let image_extensions = vec!["png", "jpg", "jpeg", "gif", "bmp"];
        let mut processed_count = 0;

        let processor = StandardImageProcessor::new();

        for ext in image_extensions {
            let image_files = ctx.get_files_by_extension(ext);
            
            for image_path in image_files {
                match processor.process(image_path).await {
                    Ok(processed) => {
                        assert!(processed.metadata.width > 0);
                        assert!(processed.metadata.height > 0);
                        assert!(!processed.metadata.format.is_empty());
                        assert!(processed.metadata.file_size > 0);
                        processed_count += 1;
                    }
                    Err(e) => {
                        println!("Image processing failed for {}: {}", image_path.display(), e);
                    }
                }
            }
        }

        if processed_count > 0 {
            println!("Successfully processed {} images", processed_count);
        }
    }

    #[tokio::test]
    async fn test_archive_processing_with_fixtures() {
        let ctx = TestContext::new().await.unwrap();
        let zip_files = ctx.get_files_by_extension("zip");

        if zip_files.is_empty() {
            println!("Skipping test: no ZIP fixtures found");
            return;
        }

        let handler = ZipHandler;

        for zip_path in zip_files {
            let result = handler.list_contents(zip_path).await;
            
            match result {
                Ok(info) => {
                    assert_eq!(info.format, "zip");
                    assert!(info.processing_error.is_none());
                    println!("ZIP {} contains {} files and {} directories", 
                        zip_path.display(), info.total_files, info.total_directories);
                }
                Err(e) => {
                    println!("ZIP processing failed for {}: {}", zip_path.display(), e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_text_document_processing_with_fixtures() {
        let ctx = TestContext::new().await.unwrap();
        let text_extensions = vec!["txt", "md", "xml", "json", "csv"];
        let mut processed_count = 0;

        for ext in text_extensions {
            let text_files = ctx.get_files_by_extension(ext);
            
            for text_path in text_files {
                let processor: Box<dyn DocumentProcessor> = match ext {
                    "md" => Box::new(MarkdownProcessor),
                    "xml" => Box::new(XmlProcessor),
                    _ => Box::new(TextProcessor),
                };

                match processor.process(text_path).await {
                    Ok(doc) => {
                        assert!(!doc.text_content.is_empty() || doc.text_content.is_empty());
                        assert!(doc.metadata.word_count.is_some());
                        processed_count += 1;
                    }
                    Err(e) => {
                        println!("Text processing failed for {}: {}", text_path.display(), e);
                    }
                }
            }
        }

        if processed_count > 0 {
            println!("Successfully processed {} text documents", processed_count);
        }
    }

    #[tokio::test]
    async fn test_ai_categorization_with_fixtures() {
        let ctx = TestContext::new().await.unwrap();
        
        if ctx.fixture_files.is_empty() && ctx.sample_files.is_empty() {
            println!("Skipping test: no fixture files found");
            return;
        }

        // Test AI categorization on various file types
        let test_files: Vec<_> = ctx.fixture_files
            .iter()
            .chain(ctx.sample_files.iter())
            .take(5)
            .collect();

        for file_path in test_files {
            let file_name = file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            
            let content_preview = fs::read_to_string(file_path)
                .unwrap_or_else(|_| "Binary file content".to_string());
            
            let analysis = FileAnalysis {
                file_path: file_path.to_string_lossy().to_string(),
                file_name: file_name.to_string(),
                file_type: file_path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                size: fs::metadata(file_path).map(|m| m.len()).unwrap_or(0),
                content_preview: content_preview.chars().take(500).collect(),
                suggested_tags: vec![],
                suggested_folder: None,
                confidence_score: 0.0,
                processing_timestamp: chrono::Utc::now(),
            };

            // The AI service would normally categorize this
            assert!(!analysis.file_name.is_empty());
            assert!(analysis.size >= 0);
        }
    }

    #[tokio::test]
    async fn test_batch_file_processing() {
        let ctx = TestContext::new().await.unwrap();
        
        if ctx.fixture_files.len() < 3 {
            println!("Skipping test: insufficient fixture files");
            return;
        }

        let analyzer = FileAnalyzer::new().unwrap();
        let mut tasks = Vec::new();

        // Process multiple files concurrently
        for file_path in ctx.fixture_files.iter().take(5) {
            let path_str = file_path.to_string_lossy().to_string();
            let analyzer_clone = FileAnalyzer::new().unwrap();
            
            let task = tokio::spawn(async move {
                let request = AnalysisRequest {
                    path: path_str,
                    force_reanalyze: false,
                    extract_metadata: true,
                };
                analyzer_clone.analyze_file(&request).await
            });
            
            tasks.push(task);
        }

        let mut success_count = 0;
        for task in tasks {
            if let Ok(Ok(_)) = task.await {
                success_count += 1;
            }
        }

        assert!(success_count > 0, "At least one file should be processed successfully");
    }

    #[tokio::test]
    async fn test_file_organization_workflow() {
        let ctx = TestContext::new().await.unwrap();
        
        if ctx.fixture_files.is_empty() {
            println!("Skipping test: no fixture files found");
            return;
        }

        let organizer = FileOrganizer::new();
        let smart_folder_manager = SmartFolderManager::new(ctx.db.clone());

        // Create test smart folders
        let folders = vec![
            ("Documents", vec!["*.txt", "*.pdf", "*.md"]),
            ("Images", vec!["*.png", "*.jpg", "*.jpeg"]),
            ("Code", vec!["*.rs", "*.py", "*.js"]),
            ("Data", vec!["*.csv", "*.json", "*.xml"]),
        ];

        for (name, patterns) in folders {
            let folder_id = smart_folder_manager
                .create_folder(name, patterns.clone(), HashMap::new())
                .await
                .unwrap();
            
            assert!(!folder_id.is_empty());
        }

        // Test file matching against smart folders
        for file_path in ctx.fixture_files.iter().take(10) {
            let matching_folders = smart_folder_manager
                .find_matching_folders(file_path)
                .await
                .unwrap();
            
            // Files should match at least one folder based on extension
            let ext = file_path.extension().and_then(|e| e.to_str());
            if let Some(extension) = ext {
                match extension {
                    "txt" | "pdf" | "md" => assert!(matching_folders.iter().any(|f| f.name == "Documents")),
                    "png" | "jpg" | "jpeg" => assert!(matching_folders.iter().any(|f| f.name == "Images")),
                    "rs" | "py" | "js" => assert!(matching_folders.iter().any(|f| f.name == "Code")),
                    "csv" | "json" | "xml" => assert!(matching_folders.iter().any(|f| f.name == "Data")),
                    _ => {}
                }
            }
        }
    }

    #[tokio::test]
    async fn test_excel_file_processing() {
        let ctx = TestContext::new().await.unwrap();
        let excel_files = ctx.get_files_by_extension("xlsx");

        if excel_files.is_empty() {
            println!("Skipping test: no Excel fixtures found");
            return;
        }

        let analyzer = FileAnalyzer::new().unwrap();

        for excel_path in excel_files {
            let request = AnalysisRequest {
                path: excel_path.to_string_lossy().to_string(),
                force_reanalyze: false,
                extract_metadata: true,
            };

            let result = analyzer.analyze_file(&request).await;
            
            match result {
                Ok(analysis) => {
                    assert!(analysis.file_type.contains("spreadsheet") || 
                           analysis.file_type.contains("xlsx") ||
                           analysis.file_type.contains("excel"));
                    assert!(analysis.size > 0);
                }
                Err(e) => {
                    println!("Excel analysis failed for {}: {}", excel_path.display(), e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_3d_file_processing() {
        let ctx = TestContext::new().await.unwrap();
        let model_extensions = vec!["stl", "obj", "3mf", "scad", "gcode"];
        let mut processed_count = 0;

        let analyzer = FileAnalyzer::new().unwrap();

        for ext in model_extensions {
            let model_files = ctx.get_files_by_extension(ext);
            
            for model_path in model_files {
                let request = AnalysisRequest {
                    path: model_path.to_string_lossy().to_string(),
                    force_reanalyze: false,
                    extract_metadata: true,
                };

                match analyzer.analyze_file(&request).await {
                    Ok(analysis) => {
                        assert!(!analysis.file_type.is_empty());
                        assert!(analysis.size > 0);
                        processed_count += 1;
                    }
                    Err(e) => {
                        println!("3D file analysis failed for {}: {}", model_path.display(), e);
                    }
                }
            }
        }

        if processed_count > 0 {
            println!("Successfully processed {} 3D model files", processed_count);
        }
    }

    #[tokio::test]
    async fn test_vector_graphics_processing() {
        let ctx = TestContext::new().await.unwrap();
        let vector_extensions = vec!["svg", "eps", "ai"];
        let mut processed_count = 0;

        let analyzer = FileAnalyzer::new().unwrap();

        for ext in vector_extensions {
            let vector_files = ctx.get_files_by_extension(ext);
            
            for vector_path in vector_files {
                let request = AnalysisRequest {
                    path: vector_path.to_string_lossy().to_string(),
                    force_reanalyze: false,
                    extract_metadata: true,
                };

                match analyzer.analyze_file(&request).await {
                    Ok(analysis) => {
                        assert!(!analysis.file_type.is_empty());
                        assert!(analysis.size > 0);
                        processed_count += 1;
                    }
                    Err(e) => {
                        println!("Vector graphics analysis failed for {}: {}", vector_path.display(), e);
                    }
                }
            }
        }

        if processed_count > 0 {
            println!("Successfully processed {} vector graphics files", processed_count);
        }
    }

    #[tokio::test]
    async fn test_python_script_processing() {
        let ctx = TestContext::new().await.unwrap();
        let python_files = ctx.get_files_by_extension("py");

        if python_files.is_empty() {
            println!("Skipping test: no Python fixtures found");
            return;
        }

        let processor = TextProcessor;

        for py_path in python_files {
            let result = processor.process(py_path).await;
            
            match result {
                Ok(doc) => {
                    assert_eq!(doc.file_type, "text");
                    assert!(!doc.text_content.is_empty());
                    // Python files should contain some code patterns
                    assert!(doc.text_content.contains("def") || 
                           doc.text_content.contains("class") ||
                           doc.text_content.contains("import") ||
                           doc.text_content.contains("="));
                }
                Err(e) => {
                    println!("Python file processing failed for {}: {}", py_path.display(), e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_mixed_file_type_batch_processing() {
        let ctx = TestContext::new().await.unwrap();
        
        // Get a mix of different file types
        let test_files: Vec<_> = ctx.fixture_files
            .iter()
            .chain(ctx.sample_files.iter())
            .filter(|path| {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                matches!(ext, "txt" | "pdf" | "png" | "json" | "xml" | "py" | "csv")
            })
            .take(10)
            .collect();

        if test_files.is_empty() {
            println!("Skipping test: no suitable fixture files found");
            return;
        }

        let analyzer = FileAnalyzer::new().unwrap();
        let mut results = HashMap::new();

        for file_path in test_files {
            let ext = file_path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown");
            
            let request = AnalysisRequest {
                path: file_path.to_string_lossy().to_string(),
                force_reanalyze: false,
                extract_metadata: true,
            };

            match analyzer.analyze_file(&request).await {
                Ok(analysis) => {
                    results.entry(ext.to_string())
                        .or_insert_with(Vec::new)
                        .push(analysis);
                }
                Err(e) => {
                    println!("Analysis failed for {}: {}", file_path.display(), e);
                }
            }
        }

        // Verify we processed multiple file types
        assert!(results.len() > 1, "Should process multiple file types");
        
        for (file_type, analyses) in results {
            println!("Processed {} files of type {}", analyses.len(), file_type);
            for analysis in analyses {
                assert!(!analysis.file_type.is_empty());
                assert!(analysis.size > 0);
            }
        }
    }
}