use crate::ai::{AiProvider, AiService};
use crate::config::Config;

#[tokio::test]
async fn test_ai_service_initialization_with_ollama() {
    let config = create_test_config("ollama", "localhost:11434");

    // This will likely fail unless Ollama is running, which should fallback gracefully
    let ai_service = AiService::new(&config).await.unwrap();

    // Should either be Ollama (if running) or Fallback (if not)
    let status = ai_service.get_status().await;
    assert!(
        matches!(status.provider, AiProvider::Ollama)
            || matches!(status.provider, AiProvider::Fallback)
    );
}

#[tokio::test]
async fn test_ai_service_initialization_with_explicit_fallback() {
    let config = create_test_config("fallback", "");
    let ai_service = AiService::new(&config).await.unwrap();

    let status = ai_service.get_status().await;
    assert!(matches!(status.provider, AiProvider::Fallback));
    assert!(status.is_available);
}

#[tokio::test]
async fn test_ai_service_initialization_with_invalid_provider() {
    let config = create_test_config("unknown_provider", "localhost:11434");
    let ai_service = AiService::new(&config).await.unwrap();

    // Should default to Ollama behavior (and likely fallback to Fallback if Ollama not running)
    let status = ai_service.get_status().await;
    assert!(
        matches!(status.provider, AiProvider::Ollama)
            || matches!(status.provider, AiProvider::Fallback)
    );
}

#[tokio::test]
async fn test_ai_service_initialization_with_empty_host() {
    let config = create_test_config("ollama", "");
    let ai_service = AiService::new(&config).await.unwrap();

    // Should fallback when host is empty
    let status = ai_service.get_status().await;
    assert!(matches!(status.provider, AiProvider::Fallback));
}

#[tokio::test]
async fn test_update_config_provider_change() {
    let initial_config = create_test_config("fallback", "");
    let ai_service = AiService::new(&initial_config).await.unwrap();

    // Verify initial state
    let status = ai_service.get_status().await;
    assert!(matches!(status.provider, AiProvider::Fallback));

    // Update to attempt Ollama (will likely fallback if not running)
    let new_config = create_test_config("ollama", "localhost:11434");
    ai_service.update_config(&new_config).await.unwrap();

    // Should attempt to switch providers
    let new_status = ai_service.get_status().await;
    // Could be either Ollama or Fallback depending on if Ollama is running
    assert!(
        matches!(new_status.provider, AiProvider::Ollama)
            || matches!(new_status.provider, AiProvider::Fallback)
    );
}

#[tokio::test]
async fn test_update_config_host_change() {
    let initial_config = create_test_config("ollama", "localhost:11434");
    let ai_service = AiService::new(&initial_config).await.unwrap();

    // Change host
    let new_config = create_test_config("ollama", "localhost:11435"); // Different port
    ai_service.update_config(&new_config).await.unwrap();

    // Should attempt to reinitialize (likely will fail and fallback)
    let status = ai_service.get_status().await;
    // Likely will be fallback since port 11435 probably isn't running Ollama
    assert!(
        matches!(status.provider, AiProvider::Ollama)
            || matches!(status.provider, AiProvider::Fallback)
    );
}

#[tokio::test]
async fn test_update_config_explicit_fallback_to_ollama() {
    let initial_config = create_test_config("fallback", "");
    let ai_service = AiService::new(&initial_config).await.unwrap();

    // Verify fallback mode
    let status = ai_service.get_status().await;
    assert!(matches!(status.provider, AiProvider::Fallback));

    // Switch to Ollama
    let new_config = create_test_config("ollama", "localhost:11434");
    ai_service.update_config(&new_config).await.unwrap();

    let new_status = ai_service.get_status().await;
    // Should at least attempt to switch (may fallback if Ollama not available)
    assert!(
        matches!(new_status.provider, AiProvider::Ollama)
            || matches!(new_status.provider, AiProvider::Fallback)
    );
}

#[tokio::test]
async fn test_use_fallback_method() {
    let config = create_test_config("ollama", "localhost:11434");
    let ai_service = AiService::new(&config).await.unwrap();

    // Force switch to fallback
    let status = ai_service.use_fallback();

    assert!(matches!(status.provider, AiProvider::Fallback));
    assert!(status.is_available);
    assert!(!status.ollama_connected);
    assert!(status.models_available.contains(&"fallback".to_string()));

    // Verify the service actually switched
    let current_status = ai_service.get_status().await;
    assert!(matches!(current_status.provider, AiProvider::Fallback));
}

#[tokio::test]
async fn test_analyze_file_basic() {
    let config = create_test_config("fallback", "");
    let ai_service = AiService::new(&config).await.unwrap();

    let content = "This is a test document with some content.";
    let result = ai_service
        .analyze_file(content, "text/plain")
        .await
        .unwrap();

    assert!(!result.category.is_empty());
    assert!(!result.summary.is_empty());
    assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
    assert!(result.path.is_empty()); // Should be empty when not using analyze_file_with_path
}

#[tokio::test]
async fn test_analyze_file_with_path() {
    let config = create_test_config("fallback", "");
    let ai_service = AiService::new(&config).await.unwrap();

    let content = "Invoice #12345 for consulting services.";
    let path = "/documents/invoice_12345.pdf";

    let result = ai_service
        .analyze_file_with_path(content, "application/pdf", path)
        .await
        .unwrap();

    assert_eq!(result.path, path);
    assert!(!result.category.is_empty());
    assert!(!result.summary.is_empty());
    // Should detect invoice-related content
    assert!(
        result.tags.iter().any(|tag| tag.contains("invoice"))
            || result.summary.to_lowercase().contains("invoice")
    );
}

#[tokio::test]
async fn test_concurrent_analysis() {
    let config = create_test_config("fallback", "");
    let ai_service = std::sync::Arc::new(AiService::new(&config).await.unwrap());

    // Test concurrent file analysis
    let mut handles = vec![];

    for i in 0..5 {
        let service = ai_service.clone();
        let content = format!("Test document content {}", i);

        let handle =
            tokio::spawn(async move { service.analyze_file(&content, "text/plain").await });

        handles.push(handle);
    }

    // Wait for all analyses to complete
    let results = futures::future::join_all(handles).await;

    // All should succeed
    for result in results {
        let analysis = result.unwrap().unwrap();
        assert!(!analysis.category.is_empty());
        assert!(!analysis.summary.is_empty());
    }
}

#[tokio::test]
async fn test_is_available() {
    let config = create_test_config("fallback", "");
    let ai_service = AiService::new(&config).await.unwrap();

    // Fallback should always be available
    assert!(ai_service.is_available().await);
}

#[tokio::test]
async fn test_get_ollama_client() {
    let config = create_test_config("fallback", "");
    let ai_service = AiService::new(&config).await.unwrap();

    // Should be None for fallback mode
    assert!(ai_service.get_ollama_client().is_none());
}

#[tokio::test]
async fn test_reconnect_ollama() {
    let config = create_test_config("fallback", "");
    let ai_service = AiService::new(&config).await.unwrap();

    // Try to connect to a likely non-existent Ollama instance
    let result = ai_service.reconnect_ollama("localhost:11435").await;

    // Should not panic, but likely will fail to connect
    assert!(result.is_ok());

    let status = result.unwrap();
    // Will likely have an error since port 11435 probably doesn't have Ollama
    assert!(status.last_error.is_some() || status.ollama_connected);
}

#[tokio::test]
async fn test_generate_embeddings_fallback() {
    let config = create_test_config("fallback", "");
    let ai_service = AiService::new(&config).await.unwrap();

    let text = "This is test text for embedding generation.";
    let result = ai_service.generate_embeddings(text).await.unwrap();

    // Should return some kind of embedding vector
    assert!(!result.is_empty());
    assert!(result.len() > 10); // Should have reasonable dimensionality

    // All values should be finite
    for value in result {
        assert!(value.is_finite());
    }
}

// Helper function
fn create_test_config(provider: &str, ollama_host: &str) -> Config {
    Config {
        ai_provider: provider.to_string(),
        ollama_host: ollama_host.to_string(),
        ..Default::default()
    }
}
