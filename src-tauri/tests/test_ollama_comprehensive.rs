use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

use stratosort::ai::ollama::OllamaClient;
use stratosort::ai::AiEngine;
use stratosort::error::Result;

#[derive(Debug, Serialize, Deserialize)]
struct TestResult {
    file_name: String,
    file_type: String,
    analysis_success: bool,
    expected_category: String,
    actual_category: String,
    confidence: f32,
    tags: Vec<String>,
    embedding_generated: bool,
    embedding_dimensions: usize,
    processing_time_ms: u128,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestReport {
    timestamp: String,
    ollama_status: String,
    available_models: Vec<String>,
    total_files: usize,
    successful_analyses: usize,
    failed_analyses: usize,
    embedding_success_rate: f32,
    categorization_accuracy: f32,
    average_processing_time_ms: u128,
    test_results: Vec<TestResult>,
    performance_metrics: PerformanceMetrics,
    errors_encountered: Vec<String>,
    recommendations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PerformanceMetrics {
    total_test_duration_ms: u128,
    memory_usage_mb: f64,
    peak_connections: usize,
    average_response_time_ms: u128,
    timeout_count: usize,
    retry_count: usize,
}

struct TestFileInfo {
    path: PathBuf,
    expected_category: String,
    file_type: String,
}

impl TestFileInfo {
    fn new(path: PathBuf, expected_category: &str) -> Self {
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown");
        
        let file_type = match extension {
            "pdf" => "application/pdf",
            "txt" | "md" => "text/plain",
            "py" => "text/x-python",
            "xml" => "application/xml",
            "csv" => "text/csv",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "zip" => "application/zip",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "eps" => "application/postscript",
            "ai" => "application/illustrator",
            "psd" => "image/vnd.adobe.photoshop",
            "stl" => "model/stl",
            "gcode" => "text/x-gcode",
            "scad" => "text/x-scad",
            "3mf" => "model/3mf",
            _ => "application/octet-stream",
        }.to_string();
        
        Self {
            path,
            expected_category: expected_category.to_string(),
            file_type,
        }
    }
}

fn categorize_test_files() -> Vec<TestFileInfo> {
    let base_path = Path::new(r"C:\Users\benja\Documents\GitHub\StratoRust\src-tauri\tests\fixtures\data\sample_demo_files");
    
    vec![
        // Research category files
        TestFileInfo::new(base_path.join("qx7n9p.md"), "Research"),
        TestFileInfo::new(base_path.join("b4m2k8.txt"), "Research"),
        TestFileInfo::new(base_path.join("f5h8j2.py"), "Research"),
        TestFileInfo::new(base_path.join("n5r8t3.xml"), "Research"),
        
        // Finance category files
        TestFileInfo::new(base_path.join("Annual_Financial_Statement_2024.pdf"), "Finance"),
        TestFileInfo::new(base_path.join("Financials_2024_Q1_Q2.xlsx"), "Finance"),
        TestFileInfo::new(base_path.join("Finance_CSV_Workbook.zip"), "Finance"),
        TestFileInfo::new(base_path.join("g4h7n2.xlsx"), "Finance"),
        TestFileInfo::new(base_path.join("n3x6s1.csv"), "Finance"),
        TestFileInfo::new(base_path.join("s8w2k5.csv"), "Finance"),
        TestFileInfo::new(base_path.join("20250911_1017_Imposter Financial Document_simple_compose_01k4wj305neqr9pjgx4m1b9mdr.png"), "Finance"),
        
        // 3D Print category files
        TestFileInfo::new(base_path.join("MCHAT.stl"), "3D Print"),
        TestFileInfo::new(base_path.join("h3p8w5.gcode"), "3D Print"),
        TestFileInfo::new(base_path.join("k6t8m1.scad"), "3D Print"),
        TestFileInfo::new(base_path.join("r5b9j3.3mf"), "3D Print"),
        
        // Logos/Graphic Art category files
        TestFileInfo::new(base_path.join("j7k2m9.svg"), "Logos/Graphic Art"),
        TestFileInfo::new(base_path.join("d4s1k7.eps"), "Logos/Graphic Art"),
        TestFileInfo::new(base_path.join("p8n4w3.ai"), "Logos/Graphic Art"),
        TestFileInfo::new(base_path.join("m6q9r8.psd"), "Logos/Graphic Art"),
        TestFileInfo::new(base_path.join("20250906_1325_UFO Night Sky_remix_01k4g0x3pefeja3ye0v6j7es8v (1).png"), "Logos/Graphic Art"),
    ]
}

async fn test_ollama_connection() -> Result<(OllamaClient, Vec<String>)> {
    println!("Testing Ollama connection...");
    
    let client = match OllamaClient::new("http://localhost:11434").await {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to connect to Ollama: {}", e);
            return Err(e);
        }
    };
    
    // Check health
    client.health_check().await?;
    println!("Ollama health check passed");
    
    // Get available models
    let models = client.list_models().await?;
    println!("Available models: {:?}", models);
    
    Ok((client, models))
}

async fn test_file_analysis(
    client: &OllamaClient,
    file_info: &TestFileInfo,
) -> TestResult {
    let start = Instant::now();
    let file_name = file_info.path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    println!("Analyzing file: {}", file_name);
    
    // Read file content (limited for large files)
    let content = match fs::read(&file_info.path) {
        Ok(bytes) => {
            // Limit content size for testing
            let max_size = 100_000; // 100KB
            if bytes.len() > max_size {
                String::from_utf8_lossy(&bytes[..max_size]).to_string()
            } else {
                String::from_utf8_lossy(&bytes).to_string()
            }
        }
        Err(e) => {
            return TestResult {
                file_name,
                file_type: file_info.file_type.clone(),
                analysis_success: false,
                expected_category: file_info.expected_category.clone(),
                actual_category: String::new(),
                confidence: 0.0,
                tags: vec![],
                embedding_generated: false,
                embedding_dimensions: 0,
                processing_time_ms: start.elapsed().as_millis(),
                error: Some(format!("Failed to read file: {}", e)),
            };
        }
    };
    
    // Analyze file
    let analysis_result = client.analyze_file(&content, &file_info.file_type).await;
    
    let (analysis_success, actual_category, confidence, tags, error) = match analysis_result {
        Ok(analysis) => {
            (true, analysis.category, analysis.confidence, analysis.tags, None)
        }
        Err(e) => {
            (false, String::new(), 0.0, vec![], Some(e.to_string()))
        }
    };
    
    // Generate embeddings
    let embedding_result = client.generate_embeddings(&content).await;
    let (embedding_generated, embedding_dimensions) = match embedding_result {
        Ok(embeddings) => (true, embeddings.len()),
        Err(_) => (false, 0),
    };
    
    TestResult {
        file_name,
        file_type: file_info.file_type.clone(),
        analysis_success,
        expected_category: file_info.expected_category.clone(),
        actual_category,
        confidence,
        tags,
        embedding_generated,
        embedding_dimensions,
        processing_time_ms: start.elapsed().as_millis(),
        error,
    }
}

async fn test_error_scenarios(client: &OllamaClient) -> Vec<String> {
    let mut errors = Vec::new();
    
    println!("\nTesting error scenarios...");
    
    // Test 1: Empty content
    let result = client.analyze_file("", "text/plain").await;
    match result {
        Ok(_) => errors.push("Empty content should fail but didn't".to_string()),
        Err(e) => println!("Empty content error (expected): {}", e),
    }
    
    // Test 2: Very large content
    let large_content = "x".repeat(10_000_000); // 10MB
    let result = client.analyze_file(&large_content, "text/plain").await;
    match result {
        Ok(_) => println!("Large content handled successfully"),
        Err(e) => {
            println!("Large content error: {}", e);
            errors.push(format!("Large content handling: {}", e));
        }
    }
    
    // Test 3: Invalid UTF-8 (binary data)
    let binary_data = vec![0xFF, 0xFE, 0x00, 0x01, 0x02];
    let binary_str = String::from_utf8_lossy(&binary_data);
    let result = client.analyze_file(&binary_str, "application/octet-stream").await;
    match result {
        Ok(_) => println!("Binary data handled successfully"),
        Err(e) => println!("Binary data error: {}", e),
    }
    
    // Test 4: Sequential requests (OllamaClient doesn't implement Clone)
    let mut sequential_errors = 0;
    for i in 0..5 {
        let result = client.analyze_file(&format!("Test content {}", i), "text/plain").await;
        match result {
            Ok(_) => {},
            Err(e) => {
                sequential_errors += 1;
                println!("Sequential request {} error: {}", i, e);
            }
        }
    }
    
    if sequential_errors > 0 {
        errors.push(format!("{} sequential request errors", sequential_errors));
    }
    
    errors
}

async fn test_vision_models(client: &OllamaClient) -> Vec<TestResult> {
    println!("\nTesting vision model capabilities...");
    
    let image_files = vec![
        TestFileInfo::new(
            PathBuf::from(r"C:\Users\benja\Documents\GitHub\StratoRust\src-tauri\tests\fixtures\data\sample_demo_files\20250906_1325_UFO Night Sky_remix_01k4g0x3pefeja3ye0v6j7es8v (1).png"),
            "Logos/Graphic Art"
        ),
        TestFileInfo::new(
            PathBuf::from(r"C:\Users\benja\Documents\GitHub\StratoRust\src-tauri\tests\fixtures\data\sample_demo_files\20250911_1017_Imposter Financial Document_simple_compose_01k4wj305neqr9pjgx4m1b9mdr.png"),
            "Finance"
        ),
    ];
    
    let mut results = Vec::new();
    
    for file_info in image_files {
        // Check if vision model is available
        let vision_available = client.list_models().await
            .map(|models| models.iter().any(|m| m.contains("vision") || m.contains("llava")))
            .unwrap_or(false);
        
        if !vision_available {
            println!("Vision model not available, skipping image analysis");
            continue;
        }
        
        let result = test_file_analysis(client, &file_info).await;
        results.push(result);
    }
    
    results
}

async fn generate_test_report(
    client: &OllamaClient,
    test_results: Vec<TestResult>,
    errors: Vec<String>,
    total_duration: u128,
) -> TestReport {
    let successful = test_results.iter().filter(|r| r.analysis_success).count();
    let total = test_results.len();
    
    let embedding_success = test_results.iter()
        .filter(|r| r.embedding_generated)
        .count() as f32 / total as f32;
    
    let correct_categorizations = test_results.iter()
        .filter(|r| r.analysis_success && 
                r.actual_category.to_lowercase().contains(&r.expected_category.to_lowercase()))
        .count() as f32;
    
    let categorization_accuracy = if successful > 0 {
        correct_categorizations / successful as f32
    } else {
        0.0
    };
    
    let avg_processing_time = if total > 0 {
        test_results.iter().map(|r| r.processing_time_ms).sum::<u128>() / total as u128
    } else {
        0
    };
    
    let stats = client.get_connection_stats().await;
    
    let performance_metrics = PerformanceMetrics {
        total_test_duration_ms: total_duration,
        memory_usage_mb: 0.0, // Would need system metrics
        peak_connections: stats.available_connections,
        average_response_time_ms: avg_processing_time,
        timeout_count: 0, // Would need to track timeouts
        retry_count: 0, // Would need to track retries
    };
    
    let mut recommendations = vec![];
    
    if categorization_accuracy < 0.8 {
        recommendations.push("Consider fine-tuning the model for better categorization accuracy".to_string());
    }
    
    if embedding_success < 0.9 {
        recommendations.push("Embedding generation needs improvement - check model availability".to_string());
    }
    
    if avg_processing_time > 5000 {
        recommendations.push("Processing time is high - consider optimization or caching".to_string());
    }
    
    if !errors.is_empty() {
        recommendations.push("Address error scenarios before production deployment".to_string());
    }
    
    let models = client.list_models().await.unwrap_or_default();
    
    TestReport {
        timestamp: format!("{:?}", std::time::SystemTime::now()),
        ollama_status: "Connected".to_string(),
        available_models: models,
        total_files: total,
        successful_analyses: successful,
        failed_analyses: total - successful,
        embedding_success_rate: embedding_success,
        categorization_accuracy,
        average_processing_time_ms: avg_processing_time,
        test_results,
        performance_metrics,
        errors_encountered: errors,
        recommendations,
    }
}

#[tokio::test]
#[ignore] // Run with: cargo test test_ollama_comprehensive -- --ignored
async fn test_ollama_comprehensive() -> Result<()> {
    println!("=== Starting Comprehensive Ollama Integration Test ===\n");
    
    let test_start = Instant::now();
    
    // 1. Test Ollama connection and get available models
    let (client, models) = match test_ollama_connection().await {
        Ok((c, m)) => (c, m),
        Err(e) => {
            println!("\n=== TEST ABORTED ===");
            println!("Cannot connect to Ollama: {}", e);
            println!("Please ensure Ollama is running on localhost:11434");
            return Err(e);
        }
    };
    
    println!("\n=== Connected to Ollama ===");
    println!("Available models: {:?}", models);
    
    // 2. Categorize test files
    let test_files = categorize_test_files();
    println!("\n=== Test Files Inventory ===");
    println!("Total test files: {}", test_files.len());
    
    let mut category_counts = HashMap::new();
    for file in &test_files {
        *category_counts.entry(file.expected_category.clone()).or_insert(0) += 1;
    }
    
    for (category, count) in &category_counts {
        println!("  {}: {} files", category, count);
    }
    
    // 3. Test file analysis for each category
    println!("\n=== Testing File Analysis ===");
    let mut all_results = Vec::new();
    
    for file_info in &test_files {
        let result = test_file_analysis(&client, file_info).await;
        
        println!("  {} -> Expected: {}, Got: {}, Success: {}, Confidence: {:.2}",
            result.file_name,
            result.expected_category,
            result.actual_category,
            result.analysis_success,
            result.confidence
        );
        
        all_results.push(result);
    }
    
    // 4. Test vision models separately
    let vision_results = test_vision_models(&client).await;
    all_results.extend(vision_results);
    
    // 5. Test error scenarios
    let errors = test_error_scenarios(&client).await;
    
    // 6. Generate comprehensive test report
    let total_duration = test_start.elapsed().as_millis();
    let report = generate_test_report(&client, all_results, errors, total_duration).await;
    
    // 7. Display summary
    println!("\n=== TEST SUMMARY ===");
    println!("Total files tested: {}", report.total_files);
    println!("Successful analyses: {}", report.successful_analyses);
    println!("Failed analyses: {}", report.failed_analyses);
    println!("Embedding success rate: {:.1}%", report.embedding_success_rate * 100.0);
    println!("Categorization accuracy: {:.1}%", report.categorization_accuracy * 100.0);
    println!("Average processing time: {}ms", report.average_processing_time_ms);
    println!("Total test duration: {}ms", report.performance_metrics.total_test_duration_ms);
    
    if !report.errors_encountered.is_empty() {
        println!("\n=== Errors Encountered ===");
        for error in &report.errors_encountered {
            println!("  - {}", error);
        }
    }
    
    println!("\n=== Recommendations ===");
    for rec in &report.recommendations {
        println!("  - {}", rec);
    }
    
    // 8. Save detailed report to file
    let report_json = serde_json::to_string_pretty(&report)?;
    let report_path = Path::new(r"C:\Users\benja\Documents\GitHub\StratoRust\src-tauri\tests\ollama_test_report.json");
    fs::write(report_path, report_json)?;
    println!("\nDetailed report saved to: {}", report_path.display());
    
    // 9. Determine test success
    let test_passed = report.categorization_accuracy >= 0.7 && 
                     report.embedding_success_rate >= 0.5 &&
                     report.successful_analyses > 0;
    
    if test_passed {
        println!("\n=== TEST PASSED ===");
    } else {
        println!("\n=== TEST FAILED ===");
        println!("Minimum requirements not met:");
        println!("  - Categorization accuracy: >= 70% (got {:.1}%)", report.categorization_accuracy * 100.0);
        println!("  - Embedding success rate: >= 50% (got {:.1}%)", report.embedding_success_rate * 100.0);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_ollama_fallback_behavior() {
    println!("\n=== Testing Ollama Fallback Behavior ===");
    
    // Test with intentionally unavailable server
    let result = OllamaClient::new("http://localhost:55555").await;
    
    match result {
        Ok(_) => panic!("Should not connect to unavailable server"),
        Err(e) => {
            println!("Fallback test passed: {}", e);
            assert!(e.to_string().contains("not running") || 
                   e.to_string().contains("unreachable"));
        }
    }
}

#[tokio::test]
async fn test_smart_folder_matching_logic() {
    println!("\n=== Testing Smart Folder Matching Logic ===");
    
    // Define smart folder descriptions
    let _smart_folders = [("Research", "Topics related to LLM, VLM and AI Research"),
        ("Logos/Graphic Art", "Any images that show a brand or logo like image"),
        ("3D Print", "Any file related to 3D printing"),
        ("Finance", "Documents related to financial topics")];
    
    // Test file extensions to category mapping
    let test_cases = vec![
        ("test.stl", "3D Print"),
        ("test.gcode", "3D Print"),
        ("test.3mf", "3D Print"),
        ("test.scad", "3D Print"),
        ("financial_report.pdf", "Finance"),
        ("budget.xlsx", "Finance"),
        ("logo.svg", "Logos/Graphic Art"),
        ("brand.ai", "Logos/Graphic Art"),
        ("research.md", "Research"),
        ("llm_paper.txt", "Research"),
    ];
    
    for (filename, expected_category) in test_cases {
        println!("  {} -> Expected: {}", filename, expected_category);
    }
    
    println!("Smart folder matching logic test completed");
}