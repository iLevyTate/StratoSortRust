use stratosort::core::{
    archive_handler::{ArchiveHandler, ZipHandler},
    document_processor::{DocumentProcessor, PdfProcessor, TextProcessor},
    file_analyzer::{FileAnalyzer, AnalysisRequest},
    image_processor::{ImageProcessor, StandardImageProcessor},
};
use stratosort::storage::Database;
use stratosort::error::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

#[cfg(test)]
mod file_processing_performance_tests {
    use super::*;

    const FIXTURE_DIR: &str = "tests/fixtures/data/sample_demo_files";
    const SAMPLE_DATA_DIR: &str = "tests/fixtures/sample_data";
    
    // Performance thresholds
    const MAX_SMALL_FILE_TIME_MS: u128 = 100;  // 100ms for files < 1MB
    const MAX_MEDIUM_FILE_TIME_MS: u128 = 500; // 500ms for files 1-10MB
    const MAX_LARGE_FILE_TIME_MS: u128 = 2000; // 2s for files > 10MB
    const MAX_BATCH_TIME_PER_FILE_MS: u128 = 50; // 50ms per file in batch processing

    struct PerformanceMetrics {
        file_path: PathBuf,
        file_size: u64,
        processing_time: Duration,
        operation: String,
        success: bool,
    }

    impl PerformanceMetrics {
        fn throughput_mbps(&self) -> f64 {
            if self.processing_time.as_secs_f64() > 0.0 {
                (self.file_size as f64 / 1_048_576.0) / self.processing_time.as_secs_f64()
            } else {
                0.0
            }
        }
    }

    async fn measure_operation<F, Fut>(
        file_path: &Path,
        operation_name: &str,
        operation: F,
    ) -> PerformanceMetrics
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let file_size = fs::metadata(file_path)
            .map(|m| m.len())
            .unwrap_or(0);
        
        let start = Instant::now();
        let success = operation().await.is_ok();
        let processing_time = start.elapsed();
        
        PerformanceMetrics {
            file_path: file_path.to_path_buf(),
            file_size,
            processing_time,
            operation: operation_name.to_string(),
            success,
        }
    }

    #[tokio::test]
    async fn test_file_analyzer_performance() {
        let fixture_files = get_fixture_files();
        if fixture_files.is_empty() {
            println!("Skipping test: no fixture files found");
            return;
        }

        let analyzer = FileAnalyzer::new().unwrap();
        let mut metrics = Vec::new();

        for file_path in fixture_files.iter().take(20) {
            let metric = measure_operation(
                file_path,
                "file_analysis",
                || async {
                    let request = AnalysisRequest {
                        path: file_path.to_string_lossy().to_string(),
                        force_reanalyze: false,
                        extract_metadata: true,
                    };
                    analyzer.analyze_file(&request).await.map(|_| ())
                },
            ).await;
            
            metrics.push(metric);
        }

        // Analyze performance
        for metric in &metrics {
            let max_time = match metric.file_size {
                0..=1_048_576 => MAX_SMALL_FILE_TIME_MS,
                1_048_577..=10_485_760 => MAX_MEDIUM_FILE_TIME_MS,
                _ => MAX_LARGE_FILE_TIME_MS,
            };
            
            assert!(
                metric.processing_time.as_millis() <= max_time,
                "File analysis took too long for {}: {}ms (max: {}ms, size: {} bytes)",
                metric.file_path.display(),
                metric.processing_time.as_millis(),
                max_time,
                metric.file_size
            );
        }

        // Print summary
        let avg_time: u128 = metrics.iter()
            .map(|m| m.processing_time.as_millis())
            .sum::<u128>() / metrics.len() as u128;
        
        println!("File Analyzer Performance Summary:");
        println!("  Files processed: {}", metrics.len());
        println!("  Average time: {}ms", avg_time);
        println!("  Success rate: {:.1}%", 
            metrics.iter().filter(|m| m.success).count() as f64 / metrics.len() as f64 * 100.0);
    }

    #[tokio::test]
    async fn test_image_processor_performance() {
        let fixture_files = get_fixture_files();
        let image_files: Vec<_> = fixture_files.iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| matches!(e, "png" | "jpg" | "jpeg" | "gif" | "bmp"))
                    .unwrap_or(false)
            })
            .collect();

        if image_files.is_empty() {
            println!("Skipping test: no image fixtures found");
            return;
        }

        let processor = StandardImageProcessor::new();
        let mut metrics = Vec::new();

        for image_path in image_files.iter().take(10) {
            let metric = measure_operation(
                image_path,
                "image_processing",
                || async {
                    processor.process(image_path).await.map(|_| ())
                },
            ).await;
            
            metrics.push(metric);
        }

        // Verify performance
        for metric in &metrics {
            assert!(
                metric.processing_time.as_millis() <= MAX_MEDIUM_FILE_TIME_MS,
                "Image processing took too long for {}: {}ms",
                metric.file_path.display(),
                metric.processing_time.as_millis()
            );
        }

        // Print performance summary
        if !metrics.is_empty() {
            let avg_throughput: f64 = metrics.iter()
                .map(|m| m.throughput_mbps())
                .sum::<f64>() / metrics.len() as f64;
            
            println!("Image Processor Performance Summary:");
            println!("  Images processed: {}", metrics.len());
            println!("  Average throughput: {:.2} MB/s", avg_throughput);
        }
    }

    #[tokio::test]
    async fn test_concurrent_file_processing_performance() {
        let fixture_files = get_fixture_files();
        if fixture_files.len() < 5 {
            println!("Skipping test: insufficient fixture files");
            return;
        }

        let concurrent_limit = 10;
        let semaphore = Arc::new(Semaphore::new(concurrent_limit));
        let mut tasks = JoinSet::new();

        let start = Instant::now();

        for file_path in fixture_files.iter().take(20) {
            let sem = semaphore.clone();
            let path = file_path.clone();
            
            tasks.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let analyzer = FileAnalyzer::new().unwrap();
                let request = AnalysisRequest {
                    path: path.to_string_lossy().to_string(),
                    force_reanalyze: false,
                    extract_metadata: true,
                };
                analyzer.analyze_file(&request).await
            });
        }

        let mut success_count = 0;
        let mut failure_count = 0;
        
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(_)) => success_count += 1,
                _ => failure_count += 1,
            }
        }

        let total_time = start.elapsed();
        let total_files = success_count + failure_count;
        let avg_time_per_file = if total_files > 0 {
            total_time.as_millis() / total_files as u128
        } else {
            0
        };

        println!("Concurrent Processing Performance:");
        println!("  Files processed: {}", total_files);
        println!("  Successful: {}", success_count);
        println!("  Failed: {}", failure_count);
        println!("  Total time: {}ms", total_time.as_millis());
        println!("  Average time per file: {}ms", avg_time_per_file);
        println!("  Concurrency level: {}", concurrent_limit);

        // Verify performance meets expectations
        assert!(
            avg_time_per_file <= MAX_BATCH_TIME_PER_FILE_MS * 2, // Allow 2x for concurrent overhead
            "Concurrent processing too slow: {}ms per file (max: {}ms)",
            avg_time_per_file,
            MAX_BATCH_TIME_PER_FILE_MS * 2
        );
    }

    #[tokio::test]
    async fn test_document_processor_performance() {
        let fixture_files = get_fixture_files();
        let text_files: Vec<_> = fixture_files.iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| matches!(e, "txt" | "md" | "xml" | "json" | "csv"))
                    .unwrap_or(false)
            })
            .collect();

        if text_files.is_empty() {
            println!("Skipping test: no text fixtures found");
            return;
        }

        let processor = TextProcessor;
        let mut metrics = Vec::new();

        for text_path in text_files.iter().take(15) {
            let metric = measure_operation(
                text_path,
                "text_processing",
                || async {
                    processor.process(text_path).await.map(|_| ())
                },
            ).await;
            
            metrics.push(metric);
        }

        // Analyze performance
        for metric in &metrics {
            assert!(
                metric.processing_time.as_millis() <= MAX_SMALL_FILE_TIME_MS,
                "Text processing took too long for {}: {}ms",
                metric.file_path.display(),
                metric.processing_time.as_millis()
            );
        }

        // Calculate statistics
        if !metrics.is_empty() {
            let min_time = metrics.iter().map(|m| m.processing_time.as_millis()).min().unwrap();
            let max_time = metrics.iter().map(|m| m.processing_time.as_millis()).max().unwrap();
            let avg_time = metrics.iter()
                .map(|m| m.processing_time.as_millis())
                .sum::<u128>() / metrics.len() as u128;
            
            println!("Document Processor Performance:");
            println!("  Documents processed: {}", metrics.len());
            println!("  Min time: {}ms", min_time);
            println!("  Max time: {}ms", max_time);
            println!("  Avg time: {}ms", avg_time);
        }
    }

    #[tokio::test]
    async fn test_archive_handler_performance() {
        let fixture_files = get_fixture_files();
        let archive_files: Vec<_> = fixture_files.iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e == "zip")
                    .unwrap_or(false)
            })
            .collect();

        if archive_files.is_empty() {
            println!("Skipping test: no archive fixtures found");
            return;
        }

        let handler = ZipHandler;
        
        for archive_path in archive_files {
            let start = Instant::now();
            let result = handler.list_contents(archive_path).await;
            let list_time = start.elapsed();
            
            if result.is_ok() {
                let info = result.unwrap();
                println!("Archive {} - Files: {}, List time: {}ms",
                    archive_path.display(),
                    info.total_files,
                    list_time.as_millis());
                
                // Performance check
                assert!(
                    list_time.as_millis() <= MAX_SMALL_FILE_TIME_MS,
                    "Archive listing took too long: {}ms",
                    list_time.as_millis()
                );
            }
        }
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        let fixture_files = get_fixture_files();
        if fixture_files.is_empty() {
            println!("Skipping test: no fixture files found");
            return;
        }

        // Process multiple large files to test memory efficiency
        let large_files: Vec<_> = fixture_files.iter()
            .filter(|p| {
                fs::metadata(p)
                    .map(|m| m.len() > 100_000) // Files > 100KB
                    .unwrap_or(false)
            })
            .take(5)
            .collect();

        if large_files.is_empty() {
            println!("No large files found for memory test");
            return;
        }

        let analyzer = FileAnalyzer::new().unwrap();
        
        for file_path in large_files {
            let request = AnalysisRequest {
                path: file_path.to_string_lossy().to_string(),
                force_reanalyze: false,
                extract_metadata: true,
            };
            
            // Process file (memory should be freed after each iteration)
            let _ = analyzer.analyze_file(&request).await;
        }
        
        // If we get here without OOM, the test passes
        println!("Memory efficiency test completed successfully");
    }

    #[tokio::test]
    async fn test_database_operations_performance() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("perf_test.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let db = Arc::new(Database::new(&db_url).await.unwrap());

        let fixture_files = get_fixture_files();
        if fixture_files.len() < 5 {
            println!("Skipping test: insufficient fixture files");
            return;
        }

        // Test batch insertions
        let start = Instant::now();
        let mut file_ids = Vec::new();
        
        for file_path in fixture_files.iter().take(20) {
            let file_id = db.add_file(
                &file_path.to_string_lossy(),
                "test_type",
                fs::metadata(file_path).map(|m| m.len()).unwrap_or(0) as i64,
            ).await.unwrap();
            file_ids.push(file_id);
        }
        
        let insert_time = start.elapsed();
        let avg_insert_time = insert_time.as_millis() / file_ids.len() as u128;
        
        println!("Database Performance:");
        println!("  Files inserted: {}", file_ids.len());
        println!("  Total insert time: {}ms", insert_time.as_millis());
        println!("  Average insert time: {}ms", avg_insert_time);
        
        // Test batch queries
        let start = Instant::now();
        for file_id in &file_ids {
            let _ = db.get_file_by_id(file_id).await;
        }
        let query_time = start.elapsed();
        let avg_query_time = query_time.as_millis() / file_ids.len() as u128;
        
        println!("  Total query time: {}ms", query_time.as_millis());
        println!("  Average query time: {}ms", avg_query_time);
        
        // Performance assertions
        assert!(avg_insert_time <= 10, "Database inserts too slow: {}ms", avg_insert_time);
        assert!(avg_query_time <= 5, "Database queries too slow: {}ms", avg_query_time);
    }

    #[tokio::test]
    async fn test_scalability_with_file_count() {
        let fixture_files = get_fixture_files();
        if fixture_files.len() < 10 {
            println!("Skipping test: insufficient fixture files for scalability test");
            return;
        }

        let test_sizes = vec![5, 10, 20];
        let mut results = Vec::new();

        for size in test_sizes {
            let files_to_process: Vec<_> = fixture_files.iter()
                .cycle()
                .take(size)
                .collect();
            
            let start = Instant::now();
            let analyzer = FileAnalyzer::new().unwrap();
            
            for file_path in files_to_process {
                let request = AnalysisRequest {
                    path: file_path.to_string_lossy().to_string(),
                    force_reanalyze: false,
                    extract_metadata: false, // Faster for scalability test
                };
                let _ = analyzer.analyze_file(&request).await;
            }
            
            let total_time = start.elapsed();
            let avg_time = total_time.as_millis() / size as u128;
            
            results.push((size, total_time, avg_time));
        }

        println!("Scalability Test Results:");
        for (size, total, avg) in &results {
            println!("  {} files: {}ms total, {}ms avg", size, total.as_millis(), avg);
        }

        // Check that performance scales reasonably (not exponentially)
        if results.len() >= 2 {
            let ratio = results[1].2 as f64 / results[0].2 as f64;
            assert!(
                ratio < 1.5,
                "Performance degradation too high when scaling: {:.2}x slower",
                ratio
            );
        }
    }

    // Helper function to get fixture files
    fn get_fixture_files() -> Vec<PathBuf> {
        let mut files = Vec::new();
        
        if Path::new(FIXTURE_DIR).exists() {
            files.extend(
                fs::read_dir(FIXTURE_DIR)
                    .unwrap()
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| path.is_file())
            );
        }
        
        if Path::new(SAMPLE_DATA_DIR).exists() {
            files.extend(
                fs::read_dir(SAMPLE_DATA_DIR)
                    .unwrap()
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| path.is_file())
            );
        }
        
        files
    }
}