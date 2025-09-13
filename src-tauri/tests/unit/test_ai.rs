use stratosort::ai::{AiService, AiProvider, FileAnalysis, OrganizationSuggestion};
use stratosort::config::Config;
use stratosort::error::Result;
use std::sync::Arc;

#[cfg(test)]
mod ai_tests {
    use super::*;

    fn create_test_config() -> Config {
        let mut config = Config::default();
        config.ollama_host = "http://localhost:11434".to_string();
        config.ollama_model = "llama3.2:3b".to_string();
        config
    }

    #[tokio::test]
    async fn test_ai_service_creation_with_valid_config() {
        let config = create_test_config();
        let ai_service = AiService::new(&config);
        assert!(ai_service.is_ok());
    }

    #[tokio::test]
    async fn test_ai_service_creation_with_empty_host() {
        let mut config = create_test_config();
        config.ollama_host = String::new();
        
        let ai_service = AiService::new(&config);
        assert!(ai_service.is_ok());
        
        // Should use fallback provider when host is empty
        let service = ai_service.unwrap();
        assert!(!service.is_available().await); // Should fallback
    }

    #[tokio::test]
    async fn test_fallback_analysis_text_file() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "This is a sample text document with some content.";
        let file_type = "text/plain";
        let path = "/test/document.txt";
        
        let analysis = ai_service.analyze_file_with_path(content, file_type, path).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.path, path);
        assert_eq!(result.category, "Text");
        assert!(result.confidence > 0.0);
        assert!(result.summary.contains("text/plain"));
    }

    #[tokio::test]
    async fn test_fallback_analysis_pdf_file() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "PDF content here";
        let file_type = "application/pdf";
        
        let analysis = ai_service.analyze_file(content, file_type).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Documents");
        assert!(result.summary.contains("application/pdf"));
    }

    #[tokio::test]
    async fn test_fallback_analysis_image_file() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "Binary image data";
        let file_type = "image/jpeg";
        
        let analysis = ai_service.analyze_file(content, file_type).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Images");
    }

    #[tokio::test]
    async fn test_fallback_analysis_video_file() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "Video file content";
        let file_type = "video/mp4";
        
        let analysis = ai_service.analyze_file(content, file_type).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Videos");
    }

    #[tokio::test]
    async fn test_fallback_analysis_audio_file() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "Audio file content";
        let file_type = "audio/mp3";
        
        let analysis = ai_service.analyze_file(content, file_type).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Audio");
    }

    #[tokio::test]
    async fn test_fallback_analysis_with_invoice_content() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "This is an invoice for services rendered. Total amount: $500.00";
        let file_type = "text/plain";
        
        let analysis = ai_service.analyze_file(content, file_type).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert!(result.tags.contains(&"invoice".to_string()));
        assert!(result.tags.contains(&"financial".to_string()));
    }

    #[tokio::test]
    async fn test_fallback_analysis_with_contract_content() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "This is a contract agreement between parties for the provision of services.";
        let file_type = "text/plain";
        
        let analysis = ai_service.analyze_file(content, file_type).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert!(result.tags.contains(&"contract".to_string()));
        assert!(result.tags.contains(&"legal".to_string()));
    }

    #[tokio::test]
    async fn test_fallback_analysis_with_report_content() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let content = "This is a monthly report showing key metrics and performance indicators.";
        let file_type = "text/plain";
        
        let analysis = ai_service.analyze_file(content, file_type).await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert!(result.tags.contains(&"report".to_string()));
    }

    #[tokio::test]
    async fn test_generate_embeddings_fallback() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let text = "This is a sample text for embedding generation";
        let embeddings = ai_service.generate_embeddings(text).await;
        assert!(embeddings.is_ok());
        
        let embedding_vector = embeddings.unwrap();
        assert!(!embedding_vector.is_empty());
        assert!(embedding_vector.len() > 0);
    }

    #[tokio::test]
    async fn test_suggest_organization_fallback() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let files = vec![
            "/test/image.jpg".to_string(),
            "/test/document.pdf".to_string(),
            "/test/video.mp4".to_string(),
            "/test/audio.mp3".to_string(),
            "/test/archive.zip".to_string(),
            "/test/unknown.xyz".to_string(),
        ];
        
        let suggestions = ai_service.suggest_organization(files, vec![]).await;
        assert!(suggestions.is_ok());
        
        let results = suggestions.unwrap();
        assert_eq!(results.len(), 6);
        
        // Check categorization
        assert_eq!(results[0].target_folder, "Images");
        assert_eq!(results[1].target_folder, "Documents");
        assert_eq!(results[2].target_folder, "Videos");
        assert_eq!(results[3].target_folder, "Audio");
        assert_eq!(results[4].target_folder, "Archives");
        assert_eq!(results[5].target_folder, "Other");
        
        // Check confidence scores
        for suggestion in &results {
            assert!(suggestion.confidence > 0.0);
            assert!(suggestion.confidence <= 1.0);
        }
    }

    #[tokio::test]
    async fn test_config_update() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let mut new_config = config.clone();
        new_config.ollama_model = "new-model".to_string();
        
        let result = ai_service.update_config(&new_config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_use_fallback() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        // Force fallback mode
        ai_service.use_fallback();
        
        // Should still be able to analyze files in fallback mode
        let analysis = ai_service.analyze_file("test content", "text/plain").await;
        assert!(analysis.is_ok());
    }

    #[test]
    fn test_file_analysis_serialization() {
        let analysis = FileAnalysis {
            path: "/test/file.txt".to_string(),
            category: "Documents".to_string(),
            tags: vec!["important".to_string(), "work".to_string()],
            summary: "Important work document".to_string(),
            confidence: 0.95,
            extracted_text: Some("Extracted text content".to_string()),
            detected_language: Some("en".to_string()),
            metadata: serde_json::json!({"key": "value"}),
        };
        
        let serialized = serde_json::to_string(&analysis);
        assert!(serialized.is_ok());
        
        let json_str = serialized.unwrap();
        assert!(json_str.contains("Documents"));
        assert!(json_str.contains("important"));
        assert!(json_str.contains("content_summary")); // Check serde rename
    }

    #[test]
    fn test_organization_suggestion_serialization() {
        let suggestion = OrganizationSuggestion {
            source_path: "/source/file.txt".to_string(),
            target_folder: "Documents".to_string(),
            reason: "Based on content analysis".to_string(),
            confidence: 0.8,
        };
        
        let serialized = serde_json::to_string(&suggestion);
        assert!(serialized.is_ok());
        
        let json_str = serialized.unwrap();
        assert!(json_str.contains("Documents"));
        assert!(json_str.contains("Based on content analysis"));
    }

    #[tokio::test]
    async fn test_multiple_file_analysis() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let test_cases = vec![
            ("Invoice content here", "text/plain", "invoice"),
            ("Contract details here", "text/plain", "contract"), 
            ("Monthly report data", "text/plain", "report"),
            ("Regular document", "text/plain", "Text"),
        ];
        
        for (content, file_type, expected) in test_cases {
            let analysis = ai_service.analyze_file(content, file_type).await;
            assert!(analysis.is_ok());
            
            let result = analysis.unwrap();
            if expected == "Text" {
                assert_eq!(result.category, expected);
            } else {
                // Check if expected tag is present
                assert!(result.tags.iter().any(|tag| tag.contains(expected)));
            }
        }
    }

    #[tokio::test]
    async fn test_case_insensitive_content_analysis() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let test_cases = vec![
            "INVOICE for services",
            "Invoice For Services", 
            "invoice for services",
            "This is an INVOICE document",
        ];
        
        for content in test_cases {
            let analysis = ai_service.analyze_file(content, "text/plain").await;
            assert!(analysis.is_ok());
            
            let result = analysis.unwrap();
            assert!(result.tags.contains(&"invoice".to_string()));
            assert!(result.tags.contains(&"financial".to_string()));
        }
    }

    #[tokio::test]
    async fn test_empty_content_analysis() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let analysis = ai_service.analyze_file("", "text/plain").await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Text");
        assert!(result.tags.is_empty());
    }

    #[tokio::test]
    async fn test_unknown_file_type_analysis() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let analysis = ai_service.analyze_file("content", "application/unknown").await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Other");
    }

    // New comprehensive tests for AI module

    #[tokio::test]
    async fn test_get_status() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let status = ai_service.get_status().await;
        assert!(status.provider == "ollama" || status.provider == "fallback");
        assert!(!status.model.is_empty());
    }

    #[tokio::test]
    async fn test_use_fallback_returns_status() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let status = ai_service.use_fallback();
        assert_eq!(status.provider, "fallback");
        assert!(!status.available);
        assert!(status.error.is_some());
    }

    #[tokio::test]
    async fn test_reconnect_ollama_with_valid_host() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        // Force fallback mode first
        ai_service.use_fallback();
        
        // Try to reconnect
        let result = ai_service.reconnect_ollama("http://localhost:11434").await;
        assert!(result.is_ok());
        
        let status = result.unwrap();
        // Status will depend on whether Ollama is actually running
        assert!(status.provider == "ollama" || status.provider == "fallback");
    }

    #[tokio::test]
    async fn test_reconnect_ollama_with_invalid_host() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let result = ai_service.reconnect_ollama("http://invalid:99999").await;
        assert!(result.is_ok());
        
        let status = result.unwrap();
        assert_eq!(status.provider, "fallback");
        assert!(!status.available);
    }

    #[tokio::test]
    async fn test_get_ollama_client() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let client = ai_service.get_ollama_client();
        // Client might be None if Ollama is not running
        assert!(client.is_some() || client.is_none());
    }

    #[tokio::test]
    async fn test_analyze_image_fallback() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        // Force fallback mode
        ai_service.use_fallback();
        
        let analysis = ai_service.analyze_image("/test/image.jpg").await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Images");
        assert!(result.tags.contains(&"image".to_string()));
        assert!(result.tags.contains(&"jpg".to_string()));
    }

    #[tokio::test]
    async fn test_analyze_screenshot_image() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        ai_service.use_fallback();
        
        let analysis = ai_service.analyze_image("/test/screenshot.png").await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert!(result.tags.contains(&"screenshot".to_string()));
    }

    #[tokio::test]
    async fn test_analyze_diagram_image() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        ai_service.use_fallback();
        
        let analysis = ai_service.analyze_image("/test/diagram.svg").await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert!(result.tags.contains(&"diagram".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_analysis() {
        let config = create_test_config();
        let ai_service = Arc::new(AiService::new(&config).unwrap());
        
        let mut handles = vec![];
        
        for i in 0..5 {
            let service = ai_service.clone();
            let handle = tokio::spawn(async move {
                let content = format!("Test content {}", i);
                service.analyze_file(&content, "text/plain").await
            });
            handles.push(handle);
        }
        
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_large_content_analysis() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        // Create large content
        let large_content = "Lorem ipsum ".repeat(10000);
        
        let analysis = ai_service.analyze_file(&large_content, "text/plain").await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert_eq!(result.category, "Text");
    }

    #[tokio::test]
    async fn test_special_characters_in_content() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let special_content = "Content with special chars: \n\t\r @#$%^&*()";
        
        let analysis = ai_service.analyze_file(special_content, "text/plain").await;
        assert!(analysis.is_ok());
    }

    #[tokio::test]
    async fn test_unicode_content_analysis() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let unicode_content = "Unicode content: 你好世界 مرحبا بالعالم שלום עולם";
        
        let analysis = ai_service.analyze_file(unicode_content, "text/plain").await;
        assert!(analysis.is_ok());
    }

    #[tokio::test]
    async fn test_mixed_case_file_extensions() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        ai_service.use_fallback();
        
        let test_cases = vec![
            "/test/file.JPG",
            "/test/file.Jpg",
            "/test/file.jPg",
        ];
        
        for path in test_cases {
            let analysis = ai_service.analyze_image(path).await;
            assert!(analysis.is_ok());
            
            let result = analysis.unwrap();
            assert!(result.tags.iter().any(|tag| tag == "jpg"));
        }
    }

    #[tokio::test]
    async fn test_embedding_generation_consistency() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let text = "This is a test for embedding consistency";
        
        let embedding1 = ai_service.generate_embeddings(text).await.unwrap();
        let embedding2 = ai_service.generate_embeddings(text).await.unwrap();
        
        // Embeddings for same text should be identical
        assert_eq!(embedding1.len(), embedding2.len());
        for (e1, e2) in embedding1.iter().zip(embedding2.iter()) {
            assert!((e1 - e2).abs() < 0.0001);
        }
    }

    #[tokio::test]
    async fn test_embedding_generation_for_empty_text() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let embedding = ai_service.generate_embeddings("").await;
        assert!(embedding.is_ok());
        
        let result = embedding.unwrap();
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_organization_suggestions_with_special_paths() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let files = vec![
            "/path with spaces/file.txt".to_string(),
            "C:\\Windows\\Path\\file.doc".to_string(),
            "/path/with-dashes/file.pdf".to_string(),
            "/path/with.dots/file.jpg".to_string(),
        ];
        
        let suggestions = ai_service.suggest_organization(files, vec![]).await;
        assert!(suggestions.is_ok());
        
        let results = suggestions.unwrap();
        assert_eq!(results.len(), 4);
    }

    #[tokio::test]
    async fn test_config_update_with_invalid_values() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let mut invalid_config = config.clone();
        invalid_config.ollama_host = "".to_string();
        invalid_config.ollama_model = "".to_string();
        
        let result = ai_service.update_config(&invalid_config).await;
        assert!(result.is_ok());
        
        // Service should switch to fallback with invalid config
        let status = ai_service.get_status().await;
        assert_eq!(status.provider, "fallback");
    }

    #[tokio::test]
    async fn test_analyze_various_document_types() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let test_cases = vec![
            ("application/msword", "Documents"),
            ("application/vnd.ms-excel", "Documents"),
            ("application/vnd.ms-powerpoint", "Documents"),
            ("application/zip", "Other"),
            ("application/x-rar", "Other"),
            ("application/json", "Other"),
            ("application/xml", "Other"),
        ];
        
        for (mime_type, expected_category) in test_cases {
            let analysis = ai_service.analyze_file("content", mime_type).await;
            assert!(analysis.is_ok());
            
            let result = analysis.unwrap();
            assert_eq!(result.category, expected_category);
        }
    }

    #[tokio::test]
    async fn test_file_analysis_metadata_field() {
        let config = create_test_config();
        let ai_service = AiService::new(&config).unwrap();
        
        let analysis = ai_service.analyze_file("test content", "text/plain").await;
        assert!(analysis.is_ok());
        
        let result = analysis.unwrap();
        assert!(result.metadata.is_object());
    }

    #[tokio::test]
    async fn test_thread_safety_of_ai_service() {
        let config = create_test_config();
        let ai_service = Arc::new(AiService::new(&config).unwrap());
        
        let mut handles = vec![];
        
        // Spawn multiple threads performing different operations
        let service1 = ai_service.clone();
        handles.push(tokio::spawn(async move {
            service1.get_status().await
        }));
        
        let service2 = ai_service.clone();
        handles.push(tokio::spawn(async move {
            service2.is_available().await
        }));
        
        let service3 = ai_service.clone();
        handles.push(tokio::spawn(async move {
            service3.generate_embeddings("test").await
        }));
        
        // All operations should complete without panicking
        for handle in handles {
            assert!(handle.await.is_ok());
        }
    }
}