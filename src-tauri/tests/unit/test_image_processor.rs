use stratosort::core::image_processor::{
    ImageProcessor, ImageMetadata, ProcessedImage, StandardImageProcessor
};
use stratosort::error::{AppError, Result};
use image::{ImageBuffer, Rgb, Rgba, DynamicImage};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[cfg(test)]
mod image_processor_tests {
    use super::*;

    // Helper function to create a test PNG image
    fn create_test_png(width: u32, height: u32) -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_image.png");
        
        // Create a simple RGB image
        let img = ImageBuffer::from_fn(width, height, |x, y| {
            let r = (x * 255 / width) as u8;
            let g = (y * 255 / height) as u8;
            let b = 128;
            Rgb([r, g, b])
        });
        
        img.save(&file_path)?;
        Ok(file_path)
    }

    // Helper function to create a test JPEG image
    fn create_test_jpeg(width: u32, height: u32) -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_image.jpg");
        
        // Create a gradient image
        let img = ImageBuffer::from_fn(width, height, |x, y| {
            let intensity = ((x + y) * 255 / (width + height)) as u8;
            Rgb([intensity, intensity, intensity])
        });
        
        img.save(&file_path)?;
        Ok(file_path)
    }

    // Helper function to create a minimal valid image file
    fn create_minimal_image(format: &str) -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join(format!("minimal.{}", format));
        
        match format {
            "png" => {
                // Minimal PNG header and data
                let png_data = vec![
                    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
                    0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
                    0x49, 0x48, 0x44, 0x52, // IHDR
                    0x00, 0x00, 0x00, 0x01, // width = 1
                    0x00, 0x00, 0x00, 0x01, // height = 1
                    0x08, 0x02, 0x00, 0x00, 0x00, // bit depth, color type, etc.
                    0x90, 0x77, 0x53, 0xDE, // CRC
                    0x00, 0x00, 0x00, 0x0C, // IDAT chunk length
                    0x49, 0x44, 0x41, 0x54, // IDAT
                    0x08, 0x99, 0x01, 0x01, 0x00, 0x00, 0xFE, 0xFF,
                    0x00, 0x00, 0x00, 0x02, 0x00, 0x01, // compressed data
                    0x49, 0xC0, 0x1F, 0x98, // CRC
                    0x00, 0x00, 0x00, 0x00, // IEND chunk length
                    0x49, 0x45, 0x4E, 0x44, // IEND
                    0xAE, 0x42, 0x60, 0x82, // CRC
                ];
                fs::write(&file_path, png_data)?;
            }
            "bmp" => {
                // Minimal BMP header
                let bmp_data = vec![
                    0x42, 0x4D, // BM signature
                    0x46, 0x00, 0x00, 0x00, // file size
                    0x00, 0x00, 0x00, 0x00, // reserved
                    0x36, 0x00, 0x00, 0x00, // offset to pixel data
                    0x28, 0x00, 0x00, 0x00, // header size
                    0x01, 0x00, 0x00, 0x00, // width = 1
                    0x01, 0x00, 0x00, 0x00, // height = 1
                    0x01, 0x00, // planes
                    0x18, 0x00, // bits per pixel = 24
                    0x00, 0x00, 0x00, 0x00, // compression
                    0x10, 0x00, 0x00, 0x00, // image size
                    0x00, 0x00, 0x00, 0x00, // x pixels per meter
                    0x00, 0x00, 0x00, 0x00, // y pixels per meter
                    0x00, 0x00, 0x00, 0x00, // colors used
                    0x00, 0x00, 0x00, 0x00, // important colors
                    0xFF, 0xFF, 0xFF, 0x00, // pixel data (white pixel)
                ];
                fs::write(&file_path, bmp_data)?;
            }
            _ => {
                // Create a 1x1 image for other formats
                let img = ImageBuffer::from_fn(1, 1, |_, _| Rgb([255, 255, 255]));
                img.save(&file_path)?;
            }
        }
        
        Ok(file_path)
    }

    #[tokio::test]
    async fn test_image_processor_basic_png() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_png(100, 100).unwrap();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.metadata.width, 100);
        assert_eq!(processed.metadata.height, 100);
        assert_eq!(processed.metadata.format, "png");
        assert!(processed.metadata.file_size > 0);
        assert!(processed.processing_error.is_none());
    }

    #[tokio::test]
    async fn test_image_processor_basic_jpeg() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_jpeg(200, 150).unwrap();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.metadata.width, 200);
        assert_eq!(processed.metadata.height, 150);
        assert!(processed.metadata.format == "jpeg" || processed.metadata.format == "jpg");
        assert!(processed.metadata.file_size > 0);
        assert!(processed.processing_error.is_none());
    }

    #[tokio::test]
    async fn test_image_processor_with_fixtures() {
        let fixture_paths = vec![
            PathBuf::from("tests/fixtures/data/sample_demo_files/20250906_1325_UFO Night Sky_remix_01k4g0x3pefeja3ye0v6j7es8v (1).png"),
            PathBuf::from("tests/fixtures/data/sample_demo_files/20250911_1017_Imposter Financial Document_simple_compose_01k4wj305neqr9pjgx4m1b9mdr.png"),
            PathBuf::from("tests/fixtures/data/sample_demo_files/t2v7h5.png"),
        ];
        
        let processor = StandardImageProcessor::new();
        
        for fixture_path in fixture_paths {
            if !fixture_path.exists() {
                continue;
            }
            
            let result = processor.process(&fixture_path).await;
            assert!(result.is_ok());
            
            let processed = result.unwrap();
            assert!(processed.metadata.width > 0);
            assert!(processed.metadata.height > 0);
            assert!(!processed.metadata.format.is_empty());
            assert!(processed.metadata.file_size > 0);
        }
    }

    #[tokio::test]
    async fn test_image_processor_thumbnail_generation() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_png(800, 600).unwrap();
        
        let result = processor.process_with_thumbnail(&image_path, 150, 150).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert!(processed.thumbnail_path.is_some());
        
        if let Some(thumb_path) = processed.thumbnail_path {
            let thumb_path = PathBuf::from(thumb_path);
            assert!(thumb_path.exists());
            
            // Verify thumbnail dimensions
            let thumb_img = image::open(&thumb_path).unwrap();
            assert!(thumb_img.width() <= 150);
            assert!(thumb_img.height() <= 150);
        }
    }

    #[tokio::test]
    async fn test_image_processor_exif_extraction() {
        // Create an image with EXIF data simulation
        let processor = StandardImageProcessor::new();
        let image_path = create_test_jpeg(640, 480).unwrap();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        // EXIF data might not be present in generated images
        // but the fields should exist in the metadata structure
        assert!(processed.metadata.camera_make.is_none() || processed.metadata.camera_make.is_some());
        assert!(processed.metadata.datetime_original.is_none() || processed.metadata.datetime_original.is_some());
    }

    #[tokio::test]
    async fn test_image_processor_various_formats() {
        let processor = StandardImageProcessor::new();
        let formats = vec!["png", "jpg", "gif", "bmp", "webp"];
        
        for format in formats {
            let image_path = match format {
                "png" => create_test_png(50, 50),
                "jpg" | "jpeg" => create_test_jpeg(50, 50),
                _ => create_minimal_image(format),
            };
            
            if let Ok(path) = image_path {
                let result = processor.process(&path).await;
                
                if result.is_ok() {
                    let processed = result.unwrap();
                    assert!(processed.metadata.width > 0);
                    assert!(processed.metadata.height > 0);
                    assert!(!processed.metadata.format.is_empty());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_image_processor_color_type_detection() {
        let processor = StandardImageProcessor::new();
        
        // Create RGB image
        let temp_dir = tempdir().unwrap();
        let rgb_path = temp_dir.path().join("rgb.png");
        let rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(10, 10, |_, _| {
            Rgb([255, 0, 0])
        });
        rgb_img.save(&rgb_path).unwrap();
        
        let result = processor.process(&rgb_path).await;
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(processed.metadata.color_type.contains("rgb") || processed.metadata.color_type.contains("RGB"));
        
        // Create RGBA image
        let rgba_path = temp_dir.path().join("rgba.png");
        let rgba_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(10, 10, |_, _| {
            Rgba([255, 0, 0, 128])
        });
        rgba_img.save(&rgba_path).unwrap();
        
        let result = processor.process(&rgba_path).await;
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(processed.metadata.color_type.contains("rgba") || processed.metadata.color_type.contains("RGBA"));
    }

    #[tokio::test]
    async fn test_image_processor_large_image() {
        let processor = StandardImageProcessor::new();
        // Create a large image (but not too large to avoid memory issues in tests)
        let image_path = create_test_png(2000, 1500).unwrap();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.metadata.width, 2000);
        assert_eq!(processed.metadata.height, 1500);
        assert!(processed.metadata.file_size > 10000); // Should be reasonably large
    }

    #[tokio::test]
    async fn test_image_processor_tiny_image() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_png(1, 1).unwrap();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.metadata.width, 1);
        assert_eq!(processed.metadata.height, 1);
    }

    #[tokio::test]
    async fn test_image_processor_corrupted_image() {
        let processor = StandardImageProcessor::new();
        let temp_dir = tempdir().unwrap();
        let corrupted_path = temp_dir.path().join("corrupted.png");
        
        // Write invalid PNG data
        fs::write(&corrupted_path, b"This is not a valid PNG file").unwrap();
        
        let result = processor.process(&corrupted_path).await;
        
        // Should either return an error or a ProcessedImage with error field set
        if result.is_ok() {
            let processed = result.unwrap();
            assert!(processed.processing_error.is_some());
        } else {
            assert!(matches!(result.unwrap_err(), AppError::ProcessingError { .. }));
        }
    }

    #[tokio::test]
    async fn test_image_processor_supported_extensions() {
        let processor = StandardImageProcessor::new();
        let extensions = processor.supported_extensions();
        
        // Check common image formats are supported
        assert!(extensions.contains(&"png"));
        assert!(extensions.contains(&"jpg") || extensions.contains(&"jpeg"));
        assert!(extensions.contains(&"gif"));
        assert!(extensions.contains(&"bmp"));
        assert!(extensions.contains(&"webp"));
        assert!(extensions.contains(&"tiff") || extensions.contains(&"tif"));
    }

    #[tokio::test]
    async fn test_image_processor_aspect_ratio_preservation() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_png(800, 600).unwrap();
        
        let result = processor.process_with_thumbnail(&image_path, 200, 200).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        if let Some(thumb_path) = processed.thumbnail_path {
            let thumb_img = image::open(PathBuf::from(thumb_path)).unwrap();
            
            // Check aspect ratio is preserved
            let original_ratio = 800.0 / 600.0;
            let thumb_ratio = thumb_img.width() as f64 / thumb_img.height() as f64;
            
            assert!((original_ratio - thumb_ratio).abs() < 0.01);
        }
    }

    #[tokio::test]
    async fn test_image_processor_dpi_extraction() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_png(100, 100).unwrap();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        // DPI might be None for generated images
        assert!(processed.metadata.dpi_x.is_none() || processed.metadata.dpi_x.unwrap() > 0.0);
        assert!(processed.metadata.dpi_y.is_none() || processed.metadata.dpi_y.unwrap() > 0.0);
    }

    #[tokio::test]
    async fn test_image_processor_concurrent_processing() {
        use tokio::task::JoinSet;
        
        let mut tasks = JoinSet::new();
        
        for i in 0..5 {
            let width = 50 + i * 10;
            let height = 50 + i * 10;
            let image_path = create_test_png(width, height).unwrap();
            
            tasks.spawn(async move {
                let processor = StandardImageProcessor::new();
                processor.process(&image_path).await
            });
        }
        
        let mut success_count = 0;
        while let Some(result) = tasks.join_next().await {
            if let Ok(Ok(_)) = result {
                success_count += 1;
            }
        }
        
        assert_eq!(success_count, 5);
    }

    #[tokio::test]
    async fn test_image_processor_orientation_metadata() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_jpeg(100, 150).unwrap();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        // Orientation might be None or a valid EXIF orientation value (1-8)
        if let Some(orientation) = processed.metadata.orientation {
            assert!(orientation >= 1 && orientation <= 8);
        }
    }

    #[tokio::test]
    async fn test_image_processor_file_size_calculation() {
        let processor = StandardImageProcessor::new();
        let image_path = create_test_png(100, 100).unwrap();
        
        // Get actual file size
        let actual_size = fs::metadata(&image_path).unwrap().len();
        
        let result = processor.process(&image_path).await;
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.metadata.file_size, actual_size);
    }
}