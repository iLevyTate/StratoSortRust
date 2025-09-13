use stratosort::core::archive_handler::{ArchiveHandler, ArchiveInfo, ZipHandler, TarHandler, RarHandler};
use stratosort::error::{AppError, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};
use zip::{write::FileOptions, ZipWriter};
use tar::{Builder, Header};
use flate2::write::GzEncoder;
use flate2::Compression;

#[cfg(test)]
mod archive_handler_tests {
    use super::*;

    // Helper function to create test directory structure
    fn create_test_directory_structure(root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        // Create subdirectories
        let docs_dir = root.join("documents");
        let images_dir = root.join("images");
        let nested_dir = root.join("documents").join("nested");
        
        fs::create_dir_all(&docs_dir)?;
        fs::create_dir_all(&images_dir)?;
        fs::create_dir_all(&nested_dir)?;
        
        // Create test files with various content
        let file1 = docs_dir.join("report.txt");
        fs::write(&file1, "Annual Report 2024\n".repeat(100))?;
        files.push(file1);
        
        let file2 = docs_dir.join("readme.md");
        fs::write(&file2, "# Project Documentation\n\n## Overview\n".repeat(50))?;
        files.push(file2);
        
        let file3 = images_dir.join("logo.png");
        // Create a minimal PNG header
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        fs::write(&file3, png_header)?;
        files.push(file3);
        
        let file4 = nested_dir.join("config.json");
        fs::write(&file4, r#"{"version": "1.0", "settings": {"theme": "dark"}}"#)?;
        files.push(file4);
        
        Ok(files)
    }

    // Helper function to create a ZIP archive for testing
    fn create_test_zip_archive(files: &[PathBuf]) -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let archive_path = temp_dir.path().join("test_archive.zip");
        
        let file = File::create(&archive_path)?;
        let mut zip = ZipWriter::new(file);
        
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);
        
        for file_path in files {
            let file_name = file_path.file_name()
                .ok_or_else(|| AppError::ProcessingError {
                    message: "Invalid file name".to_string(),
                })?
                .to_string_lossy();
            
            zip.start_file(file_name.to_string(), options)?;
            let content = fs::read(file_path)?;
            zip.write_all(&content)?;
        }
        
        zip.finish()?;
        Ok(archive_path)
    }

    // Helper function to create a TAR.GZ archive for testing
    fn create_test_tar_gz_archive(files: &[PathBuf]) -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let archive_path = temp_dir.path().join("test_archive.tar.gz");
        
        let tar_gz = File::create(&archive_path)?;
        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = Builder::new(enc);
        
        for file_path in files {
            let file_name = file_path.file_name()
                .ok_or_else(|| AppError::ProcessingError {
                    message: "Invalid file name".to_string(),
                })?;
            
            let mut file = File::open(file_path)?;
            tar.append_file(file_name, &mut file)?;
        }
        
        tar.finish()?;
        Ok(archive_path)
    }

    #[tokio::test]
    async fn test_zip_handler_list_contents() {
        let temp_dir = tempdir().unwrap();
        let files = create_test_directory_structure(temp_dir.path()).unwrap();
        let archive_path = create_test_zip_archive(&files).unwrap();
        
        let handler = ZipHandler;
        let result = handler.list_contents(&archive_path).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        assert_eq!(info.format, "zip");
        assert_eq!(info.total_files, files.len() as u32);
        assert!(info.uncompressed_size > 0);
        assert!(info.processing_error.is_none());
        
        // Verify all files are listed
        let entry_paths: Vec<String> = info.entries.iter().map(|e| e.path.clone()).collect();
        for file in &files {
            let file_name = file.file_name().unwrap().to_string_lossy().to_string();
            assert!(entry_paths.contains(&file_name));
        }
    }

    #[tokio::test]
    async fn test_zip_handler_extract_to() {
        let temp_dir = tempdir().unwrap();
        let files = create_test_directory_structure(temp_dir.path()).unwrap();
        let archive_path = create_test_zip_archive(&files).unwrap();
        
        let extract_dir = tempdir().unwrap();
        let handler = ZipHandler;
        
        let result = handler.extract_to(&archive_path, extract_dir.path()).await;
        assert!(result.is_ok());
        
        let extracted_files = result.unwrap();
        assert_eq!(extracted_files.len(), files.len());
        
        // Verify extracted files exist and have correct content
        for extracted in &extracted_files {
            assert!(extracted.exists());
            let content = fs::read(extracted).unwrap();
            assert!(content.len() > 0);
        }
    }

    #[tokio::test]
    async fn test_zip_handler_with_fixture_file() {
        let fixture_path = PathBuf::from("tests/fixtures/data/sample_demo_files/Finance_CSV_Workbook.zip");
        
        // Skip test if fixture doesn't exist
        if !fixture_path.exists() {
            println!("Skipping test: fixture file not found");
            return;
        }
        
        let handler = ZipHandler;
        let result = handler.list_contents(&fixture_path).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        assert_eq!(info.format, "zip");
        assert!(info.total_files > 0 || info.total_directories > 0);
        assert!(info.processing_error.is_none());
    }

    #[tokio::test]
    async fn test_tar_handler_list_contents() {
        let temp_dir = tempdir().unwrap();
        let files = create_test_directory_structure(temp_dir.path()).unwrap();
        let archive_path = create_test_tar_gz_archive(&files).unwrap();
        
        let handler = TarHandler;
        let result = handler.list_contents(&archive_path).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        assert!(info.format.contains("tar"));
        assert_eq!(info.total_files, files.len() as u32);
        assert!(info.uncompressed_size > 0);
        assert!(info.processing_error.is_none());
    }

    #[tokio::test]
    async fn test_tar_handler_extract_to() {
        let temp_dir = tempdir().unwrap();
        let files = create_test_directory_structure(temp_dir.path()).unwrap();
        let archive_path = create_test_tar_gz_archive(&files).unwrap();
        
        let extract_dir = tempdir().unwrap();
        let handler = TarHandler;
        
        let result = handler.extract_to(&archive_path, extract_dir.path()).await;
        assert!(result.is_ok());
        
        let extracted_files = result.unwrap();
        assert_eq!(extracted_files.len(), files.len());
        
        // Verify extracted files exist
        for extracted in &extracted_files {
            assert!(extracted.exists());
        }
    }

    #[tokio::test]
    async fn test_handler_with_corrupted_archive() {
        let temp_dir = tempdir().unwrap();
        let corrupted_path = temp_dir.path().join("corrupted.zip");
        
        // Create a corrupted ZIP file
        fs::write(&corrupted_path, b"This is not a valid ZIP file").unwrap();
        
        let handler = ZipHandler;
        let result = handler.list_contents(&corrupted_path).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::ProcessingError { message } => {
                assert!(message.contains("ZIP") || message.contains("archive"));
            }
            _ => panic!("Expected ProcessingError"),
        }
    }

    #[tokio::test]
    async fn test_handler_with_empty_archive() {
        let temp_dir = tempdir().unwrap();
        let archive_path = temp_dir.path().join("empty.zip");
        
        // Create an empty ZIP file
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.finish().unwrap();
        
        let handler = ZipHandler;
        let result = handler.list_contents(&archive_path).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        assert_eq!(info.total_files, 0);
        assert_eq!(info.total_directories, 0);
        assert_eq!(info.entries.len(), 0);
    }

    #[tokio::test]
    async fn test_handler_with_nested_directories() {
        let temp_dir = tempdir().unwrap();
        let archive_path = temp_dir.path().join("nested.zip");
        
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        
        // Create nested directory structure in ZIP
        let options = FileOptions::default();
        
        zip.add_directory("folder1/", options)?;
        zip.add_directory("folder1/subfolder/", options)?;
        zip.start_file("folder1/file1.txt", options)?;
        zip.write_all(b"Content 1")?;
        zip.start_file("folder1/subfolder/file2.txt", options)?;
        zip.write_all(b"Content 2")?;
        
        zip.finish()?;
        
        let handler = ZipHandler;
        let result = handler.list_contents(&archive_path).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        assert!(info.total_files >= 2);
        assert!(info.total_directories >= 2);
        
        // Check for nested structure in entries
        let has_nested = info.entries.iter().any(|e| e.path.contains('/'));
        assert!(has_nested);
    }

    #[tokio::test]
    async fn test_handler_extract_preserves_structure() {
        let temp_dir = tempdir().unwrap();
        let archive_path = temp_dir.path().join("structured.zip");
        
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        
        let options = FileOptions::default();
        
        // Create a structured archive
        zip.add_directory("docs/", options)?;
        zip.add_directory("images/", options)?;
        zip.start_file("docs/readme.txt", options)?;
        zip.write_all(b"Documentation")?;
        zip.start_file("images/logo.png", options)?;
        zip.write_all(&[0x89, 0x50, 0x4E, 0x47])?;
        
        zip.finish()?;
        
        let extract_dir = tempdir().unwrap();
        let handler = ZipHandler;
        
        let result = handler.extract_to(&archive_path, extract_dir.path()).await;
        assert!(result.is_ok());
        
        // Verify directory structure is preserved
        assert!(extract_dir.path().join("docs").exists());
        assert!(extract_dir.path().join("images").exists());
        assert!(extract_dir.path().join("docs/readme.txt").exists());
        assert!(extract_dir.path().join("images/logo.png").exists());
    }

    #[tokio::test]
    async fn test_handler_supported_extensions() {
        let zip_handler = ZipHandler;
        let zip_extensions = zip_handler.supported_extensions();
        assert!(zip_extensions.contains(&"zip"));
        
        let tar_handler = TarHandler;
        let tar_extensions = tar_handler.supported_extensions();
        assert!(tar_extensions.contains(&"tar"));
        assert!(tar_extensions.contains(&"gz"));
        assert!(tar_extensions.contains(&"tgz"));
        
        let rar_handler = RarHandler;
        let rar_extensions = rar_handler.supported_extensions();
        assert!(rar_extensions.contains(&"rar"));
    }

    #[tokio::test]
    async fn test_handler_with_large_archive() {
        let temp_dir = tempdir().unwrap();
        let archive_path = temp_dir.path().join("large.zip");
        
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        
        let options = FileOptions::default();
        
        // Create many files to test performance
        for i in 0..100 {
            zip.start_file(format!("file_{:03}.txt", i), options)?;
            zip.write_all(format!("Content for file {}", i).as_bytes())?;
        }
        
        zip.finish()?;
        
        let handler = ZipHandler;
        let start = std::time::Instant::now();
        let result = handler.list_contents(&archive_path).await;
        let duration = start.elapsed();
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        assert_eq!(info.total_files, 100);
        assert!(duration.as_secs() < 5, "Performance issue: took too long to list large archive");
    }

    #[tokio::test]
    async fn test_handler_extract_with_permissions_error() {
        let temp_dir = tempdir().unwrap();
        let files = create_test_directory_structure(temp_dir.path()).unwrap();
        let archive_path = create_test_zip_archive(&files).unwrap();
        
        // Try to extract to a non-existent directory path
        let invalid_path = PathBuf::from("/invalid/nonexistent/path");
        let handler = ZipHandler;
        
        let result = handler.extract_to(&archive_path, &invalid_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_compression_ratio_calculation() {
        let temp_dir = tempdir().unwrap();
        let archive_path = temp_dir.path().join("compressed.zip");
        
        let file = File::create(&archive_path).unwrap();
        let mut zip = ZipWriter::new(file);
        
        // Use deflate compression for better ratio
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        
        // Add highly compressible content
        zip.start_file("repetitive.txt", options)?;
        let repetitive_content = "A".repeat(10000);
        zip.write_all(repetitive_content.as_bytes())?;
        
        zip.finish()?;
        
        let handler = ZipHandler;
        let result = handler.list_contents(&archive_path).await;
        
        assert!(result.is_ok());
        let info = result.unwrap();
        
        // Check compression ratio is calculated
        assert!(info.compressed_size < info.uncompressed_size);
        
        if let Some(entry) = info.entries.first() {
            if let Some(ratio) = entry.compression_ratio {
                assert!(ratio > 0.0 && ratio < 1.0);
            }
        }
    }
}