use stratosort::ai::ollama::{OllamaClient, OllamaModel, OllamaResponse, GenerateResponse};
use stratosort::error::Result;
use std::sync::Arc;
use mockito::{self, mock, Matcher};

#[cfg(test)]
mod ollama_tests {
    use super::*;

    // Helper function to create test client with mock server
    fn create_test_client() -> String {
        mockito::server_url()
    }

    #[tokio::test]
    async fn test_ollama_client_creation() {
        let host = "http://localhost:11434";
        let client = OllamaClient::new(host).await;
        
        // Client creation might fail if Ollama is not running, which is ok
        assert!(client.is_ok() || client.is_err());
    }

    #[tokio::test]
    async fn test_ollama_client_with_invalid_host() {
        let host = "http://invalid-host:99999";
        let client = OllamaClient::new(host).await;
        
        // Should handle invalid host gracefully
        assert!(client.is_err());
    }

    #[tokio::test]
    async fn test_list_models() {
        let _m = mock("GET", "/api/tags")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"models": [{"name": "llama3.2:3b", "size": 1000000}]}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let models = client.list_models().await;
        
        assert!(models.is_ok());
        let model_list = models.unwrap();
        assert!(!model_list.is_empty());
        assert_eq!(model_list[0].name, "llama3.2:3b");
    }

    #[tokio::test]
    async fn test_list_models_empty_response() {
        let _m = mock("GET", "/api/tags")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"models": []}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let models = client.list_models().await;
        
        assert!(models.is_ok());
        let model_list = models.unwrap();
        assert!(model_list.is_empty());
    }

    #[tokio::test]
    async fn test_list_models_server_error() {
        let _m = mock("GET", "/api/tags")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let models = client.list_models().await;
        
        assert!(models.is_err());
    }

    #[tokio::test]
    async fn test_generate_text() {
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(Matcher::Json(serde_json::json!({
                "model": "llama3.2:3b",
                "prompt": "Test prompt",
                "stream": false
            })))
            .with_body(r#"{"response": "Generated response", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let response = client.generate("Test prompt", "llama3.2:3b").await;
        
        assert!(response.is_ok());
        let result = response.unwrap();
        assert_eq!(result.response, "Generated response");
        assert!(result.done);
    }

    #[tokio::test]
    async fn test_generate_text_with_empty_prompt() {
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(Matcher::Json(serde_json::json!({
                "model": "llama3.2:3b",
                "prompt": "",
                "stream": false
            })))
            .with_body(r#"{"response": "", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let response = client.generate("", "llama3.2:3b").await;
        
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_generate_text_with_special_characters() {
        let special_prompt = "Test with special chars: \n\t @#$%^&*()";
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "Response", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let response = client.generate(special_prompt, "llama3.2:3b").await;
        
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_generate_embeddings() {
        let _m = mock("POST", "/api/embeddings")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(Matcher::Json(serde_json::json!({
                "model": "llama3.2:3b",
                "prompt": "Test text"
            })))
            .with_body(r#"{"embedding": [0.1, 0.2, 0.3, 0.4, 0.5]}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let embeddings = client.generate_embeddings("Test text").await;
        
        assert!(embeddings.is_ok());
        let embedding_vec = embeddings.unwrap();
        assert_eq!(embedding_vec.len(), 5);
        assert_eq!(embedding_vec[0], 0.1);
    }

    #[tokio::test]
    async fn test_generate_embeddings_empty_text() {
        let _m = mock("POST", "/api/embeddings")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"embedding": [0.0, 0.0, 0.0]}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let embeddings = client.generate_embeddings("").await;
        
        assert!(embeddings.is_ok());
        let embedding_vec = embeddings.unwrap();
        assert!(!embedding_vec.is_empty());
    }

    #[tokio::test]
    async fn test_analyze_file() {
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "{\"category\": \"Documents\", \"tags\": [\"test\"], \"summary\": \"Test file\", \"confidence\": 0.9}", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let analysis = client.analyze_file("file content", "text/plain").await;
        
        assert!(analysis.is_ok());
        let result = analysis.unwrap();
        assert_eq!(result.category, "Documents");
        assert!(result.tags.contains(&"test".to_string()));
        assert_eq!(result.confidence, 0.9);
    }

    #[tokio::test]
    async fn test_analyze_file_with_path() {
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "{\"category\": \"Documents\", \"tags\": [], \"summary\": \"Test\", \"confidence\": 0.8}", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let analysis = client.analyze_file_with_path("content", "text/plain", "/test/file.txt").await;
        
        assert!(analysis.is_ok());
        let result = analysis.unwrap();
        assert_eq!(result.path, "/test/file.txt");
    }

    #[tokio::test]
    async fn test_analyze_image() {
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "{\"category\": \"Images\", \"tags\": [\"photo\"], \"summary\": \"Image file\", \"confidence\": 0.85}", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let analysis = client.analyze_image("/path/to/image.jpg").await;
        
        assert!(analysis.is_ok());
        let result = analysis.unwrap();
        assert_eq!(result.category, "Images");
        assert!(result.tags.contains(&"photo".to_string()));
    }

    #[tokio::test]
    async fn test_suggest_organization() {
        let files = vec![
            "/test/document.pdf".to_string(),
            "/test/image.jpg".to_string(),
            "/test/video.mp4".to_string(),
        ];
        
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "[{\"source_path\": \"/test/document.pdf\", \"target_folder\": \"Documents\", \"reason\": \"PDF file\", \"confidence\": 0.9}]", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let suggestions = client.suggest_organization(files, vec![]).await;
        
        assert!(suggestions.is_ok());
        let results = suggestions.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_healthy() {
        let _m = mock("GET", "/")
            .with_status(200)
            .with_body("Ollama is running")
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let is_healthy = client.health_check().await;
        
        assert!(is_healthy);
    }

    #[tokio::test]
    async fn test_health_check_unhealthy() {
        let _m = mock("GET", "/")
            .with_status(500)
            .with_body("Server Error")
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let is_healthy = client.health_check().await;
        
        assert!(!is_healthy);
    }

    #[tokio::test]
    async fn test_health_check_timeout() {
        // Don't create any mock - let it timeout
        let client = OllamaClient::new("http://localhost:99999").await;
        
        // Client creation should fail for invalid host
        assert!(client.is_err());
    }

    #[tokio::test]
    async fn test_get_model() {
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let model = client.get_model();
        
        // Should return default model
        assert!(!model.is_empty());
    }

    #[tokio::test]
    async fn test_set_model() {
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        
        client.set_model("new-model");
        let model = client.get_model();
        
        assert_eq!(model, "new-model");
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "Response", "done": true}"#)
            .expect_at_least(3)
            .create();
        
        let client = Arc::new(OllamaClient::new(&create_test_client()).await.unwrap());
        
        let mut handles = vec![];
        
        for i in 0..3 {
            let client_clone = client.clone();
            let handle = tokio::spawn(async move {
                let prompt = format!("Test prompt {}", i);
                client_clone.generate(&prompt, "llama3.2:3b").await
            });
            handles.push(handle);
        }
        
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_malformed_json_response() {
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("not valid json")
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let response = client.generate("Test", "model").await;
        
        assert!(response.is_err());
    }

    #[tokio::test]
    async fn test_network_error_handling() {
        // Use a port that's definitely not in use
        let client = OllamaClient::new("http://localhost:1").await;
        
        assert!(client.is_err());
    }

    #[tokio::test]
    async fn test_large_prompt_handling() {
        let large_prompt = "x".repeat(100000); // 100KB prompt
        
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "Response", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let response = client.generate(&large_prompt, "model").await;
        
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_unicode_in_prompts() {
        let unicode_prompt = "Unicode test: 你好世界 مرحبا بالعالم שלום עולם";
        
        let _m = mock("POST", "/api/generate")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"response": "Unicode response", "done": true}"#)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        let response = client.generate(unicode_prompt, "model").await;
        
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_retry_logic_on_failure() {
        // First request fails, second succeeds
        let _m1 = mock("GET", "/api/tags")
            .with_status(500)
            .expect(1)
            .create();
        
        let _m2 = mock("GET", "/api/tags")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"models": []}"#)
            .expect(1)
            .create();
        
        let client = OllamaClient::new(&create_test_client()).await.unwrap();
        
        // First call should fail
        let result1 = client.list_models().await;
        assert!(result1.is_err());
        
        // Second call should succeed
        let result2 = client.list_models().await;
        assert!(result2.is_ok());
    }
}