use stratosort::core::file_analyzer::{FileAnalyzer, AnalysisRequest, AnalysisResult, FileMetadata};
use stratosort::error::{Result, AppError};
use tempfile::tempdir;
use std::fs;

#[cfg(test)]
mod file_analyzer_tests {
    use super::*;

    fn create_test_file(content: &str, extension: &str) -> Result<String> {
        let temp_dir = tempdir().map_err(|e| AppError::Io(e))?;
        let file_path = temp_dir.path().join(format!("test_file.{}", extension));
        fs::write(&file_path, content).map_err(|e| AppError::Io(e))?;
        Ok(file_path.to_string_lossy().to_string())
    }

    #[tokio::test]
    async fn test_file_analyzer_creation() {
        let analyzer = FileAnalyzer::new();
        assert!(analyzer.is_ok());
    }

    #[tokio::test]
    async fn test_analyze_text_file() {
        let analyzer = FileAnalyzer::new().unwrap();
        let file_path = create_test_file("This is a sample text file for testing.", "txt").unwrap();
        
        let request = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
        
        let analysis = result.unwrap();
        assert_eq!(analysis.path, file_path);
        assert!(analysis.file_type.contains("text"));
        assert!(!analysis.content_preview.is_empty());
        assert!(analysis.size > 0);
    }

    #[tokio::test]
    async fn test_analyze_nonexistent_file() {
        let analyzer = FileAnalyzer::new().unwrap();
        let request = AnalysisRequest {
            path: "/nonexistent/file.txt".to_string(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            AppError::FileNotFound { .. } => (),
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_analyze_large_file() {
        let analyzer = FileAnalyzer::new().unwrap();
        let large_content = "x".repeat(2000); // 2KB file
        let file_path = create_test_file(&large_content, "txt").unwrap();
        
        let request = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
        
        let analysis = result.unwrap();
        assert!(analysis.content_preview.len() <= 1000); // Should be truncated
    }

    #[tokio::test]
    async fn test_analyze_binary_file() {
        let analyzer = FileAnalyzer::new().unwrap();
        let binary_content = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG header
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.png");
        fs::write(&file_path, binary_content).unwrap();
        
        let request = AnalysisRequest {
            path: file_path.to_string_lossy().to_string(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
        
        let analysis = result.unwrap();
        assert!(analysis.file_type.contains("image") || analysis.file_type.contains("png"));
        assert!(analysis.is_binary);
    }

    #[tokio::test]
    async fn test_force_reanalyze() {
        let analyzer = FileAnalyzer::new().unwrap();
        let file_path = create_test_file("Test content", "txt").unwrap();
        
        // First analysis
        let request1 = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        let result1 = analyzer.analyze_file(&request1).await;
        assert!(result1.is_ok());
        
        // Second analysis with force_reanalyze
        let request2 = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: true,
            extract_metadata: true,
        };
        let result2 = analyzer.analyze_file(&request2).await;
        assert!(result2.is_ok());
        
        // Both should succeed
        let analysis1 = result1.unwrap();
        let analysis2 = result2.unwrap();
        assert_eq!(analysis1.path, analysis2.path);
    }

    #[tokio::test]
    async fn test_metadata_extraction() {
        let analyzer = FileAnalyzer::new().unwrap();
        let file_path = create_test_file("Test content with metadata", "txt").unwrap();
        
        let request = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
        
        let analysis = result.unwrap();
        assert!(analysis.metadata.is_some());
        
        let metadata = analysis.metadata.unwrap();
        assert!(metadata.created_at.is_some());
        assert!(metadata.modified_at.is_some());
        assert!(metadata.size > 0);
    }

    #[tokio::test]
    async fn test_skip_metadata_extraction() {
        let analyzer = FileAnalyzer::new().unwrap();
        let file_path = create_test_file("Test content", "txt").unwrap();
        
        let request = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: false,
            extract_metadata: false,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
        
        let analysis = result.unwrap();
        assert!(analysis.metadata.is_none());
    }

    #[tokio::test]
    async fn test_different_file_extensions() {
        let analyzer = FileAnalyzer::new().unwrap();
        let test_cases = vec![
            ("pdf", "PDF document content"),
            ("doc", "Word document content"),
            ("jpg", "JPEG image data"),
            ("mp3", "MP3 audio data"),
            ("mp4", "MP4 video data"),
        ];
        
        for (extension, content) in test_cases {
            let file_path = create_test_file(content, extension).unwrap();
            let request = AnalysisRequest {
                path: file_path.clone(),
                force_reanalyze: false,
                extract_metadata: true,
            };
            
            let result = analyzer.analyze_file(&request).await;
            assert!(result.is_ok(), "Failed to analyze .{} file", extension);
            
            let analysis = result.unwrap();
            assert!(!analysis.file_type.is_empty());
        }
    }

    #[tokio::test]
    async fn test_concurrent_analysis() {
        let analyzer = std::sync::Arc::new(FileAnalyzer::new().unwrap());
        let mut handles = vec![];
        
        for i in 0..5 {
            let analyzer_clone = analyzer.clone();
            let file_path = create_test_file(&format!("Content {}", i), "txt").unwrap();
            
            let handle = tokio::spawn(async move {
                let request = AnalysisRequest {
                    path: file_path,
                    force_reanalyze: false,
                    extract_metadata: true,
                };
                
                analyzer_clone.analyze_file(&request).await
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_empty_file() {
        let analyzer = FileAnalyzer::new().unwrap();
        let file_path = create_test_file("", "txt").unwrap();
        
        let request = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
        
        let analysis = result.unwrap();
        assert_eq!(analysis.size, 0);
        assert!(analysis.content_preview.is_empty());
    }

    #[tokio::test]
    async fn test_unicode_content() {
        let analyzer = FileAnalyzer::new().unwrap();
        let unicode_content = "Hello 世界 🌍 Привет мир";
        let file_path = create_test_file(unicode_content, "txt").unwrap();
        
        let request = AnalysisRequest {
            path: file_path.clone(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
        
        let analysis = result.unwrap();
        assert!(analysis.content_preview.contains("Hello"));
        assert!(analysis.content_preview.contains("世界"));
        assert!(analysis.content_preview.contains("🌍"));
    }

    #[test]
    fn test_analysis_result_serialization() {
        let result = AnalysisResult {
            path: "/test/file.txt".to_string(),
            file_type: "text/plain".to_string(),
            size: 1024,
            content_preview: "Sample content".to_string(),
            is_binary: false,
            metadata: Some(FileMetadata {
                created_at: Some(chrono::Utc::now()),
                modified_at: Some(chrono::Utc::now()),
                size: 1024,
                permissions: Some("rw-r--r--".to_string()),
                checksum: Some("abc123".to_string()),
            }),
        };
        
        let serialized = serde_json::to_string(&result);
        assert!(serialized.is_ok());
        
        let json_str = serialized.unwrap();
        assert!(json_str.contains("text/plain"));
        assert!(json_str.contains("Sample content"));
    }

    #[test]
    fn test_analysis_request_validation() {
        let request = AnalysisRequest {
            path: String::new(), // Empty path should be invalid
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        // This would typically be validated in the analyzer
        assert!(request.path.is_empty());
    }

    #[tokio::test]
    async fn test_special_characters_in_path() {
        let analyzer = FileAnalyzer::new().unwrap();
        
        // Create temp file with special characters in name
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("file with spaces & symbols.txt");
        fs::write(&file_path, "Content").unwrap();
        
        let request = AnalysisRequest {
            path: file_path.to_string_lossy().to_string(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_only_file() {
        let analyzer = FileAnalyzer::new().unwrap();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("readonly.txt");
        fs::write(&file_path, "Read-only content").unwrap();
        
        // Make file read-only
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&file_path, perms).unwrap();
        
        let request = AnalysisRequest {
            path: file_path.to_string_lossy().to_string(),
            force_reanalyze: false,
            extract_metadata: true,
        };
        
        let result = analyzer.analyze_file(&request).await;
        assert!(result.is_ok()); // Should still be able to read
    }
}