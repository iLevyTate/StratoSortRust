use crate::error::{AppError, Result};
use std::path::Path;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use image::{GenericImageView, ImageReader, ImageFormat};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_type: String,
    pub bit_depth: Option<u8>,
    pub file_size: u64,
    
    // EXIF data
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub datetime_original: Option<String>,
    pub datetime_digitized: Option<String>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub gps_altitude: Option<f64>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub iso_speed: Option<u32>,
    pub exposure_time: Option<String>,
    pub flash: Option<String>,
    pub orientation: Option<u16>,
    
    // Additional metadata
    pub dpi_x: Option<f64>,
    pub dpi_y: Option<f64>,
    pub color_space: Option<String>,
    pub white_balance: Option<String>,
    pub metering_mode: Option<String>,
    pub exposure_mode: Option<String>,
    pub scene_capture_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedImage {
    pub metadata: ImageMetadata,
    pub thumbnail_path: Option<String>,
    pub processing_error: Option<String>,
}

#[async_trait]
pub trait ImageProcessor {
    async fn process(&self, image_path: &Path, thumbnail_dir: Option<&Path>) -> Result<ProcessedImage>;
    async fn create_thumbnail(&self, image_path: &Path, thumbnail_path: &Path, max_size: u32) -> Result<()>;
    fn supported_formats(&self) -> Vec<&'static str>;
}

pub struct StandardImageProcessor;

#[async_trait]
impl ImageProcessor for StandardImageProcessor {
    async fn process(&self, image_path: &Path, thumbnail_dir: Option<&Path>) -> Result<ProcessedImage> {
        // Get basic image info using image crate
        let basic_metadata = self.extract_basic_metadata(image_path).await?;
        
        // Extract EXIF data if available
        let exif_metadata = self.extract_exif_metadata(image_path).await.unwrap_or_default();
        
        // Combine metadata
        let metadata = ImageMetadata {
            width: basic_metadata.0,
            height: basic_metadata.1,
            format: basic_metadata.2,
            color_type: basic_metadata.3,
            bit_depth: basic_metadata.4,
            file_size: std::fs::metadata(image_path)?.len(),
            ..exif_metadata
        };
        
        // Create thumbnail if thumbnail directory is provided
        let thumbnail_path = if let Some(thumb_dir) = thumbnail_dir {
            match self.create_thumbnail_for_image(image_path, thumb_dir, 200).await {
                Ok(path) => Some(path),
                Err(e) => {
                    tracing::warn!("Failed to create thumbnail for {}: {}", image_path.display(), e);
                    None
                }
            }
        } else {
            None
        };
        
        Ok(ProcessedImage {
            metadata,
            thumbnail_path,
            processing_error: None,
        })
    }
    
    async fn create_thumbnail(&self, image_path: &Path, thumbnail_path: &Path, max_size: u32) -> Result<()> {
        use std::fs;
        
        // Ensure thumbnail directory exists
        if let Some(parent) = thumbnail_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let img = ImageReader::open(image_path)?
            .decode()
            .map_err(|e| AppError::ProcessingError { 
                message: format!("Failed to decode image: {}", e) 
            })?;
        
        let (width, height) = img.dimensions();
        let (new_width, new_height) = self.calculate_thumbnail_size(width, height, max_size);
        
        let thumbnail = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
        
        thumbnail.save_with_format(thumbnail_path, ImageFormat::Jpeg)
            .map_err(|e| AppError::ProcessingError { 
                message: format!("Failed to save thumbnail: {}", e) 
            })?;
            
        Ok(())
    }
    
    fn supported_formats(&self) -> Vec<&'static str> {
        vec!["jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "webp", "ico", "avif"]
    }
}

impl StandardImageProcessor {
    async fn extract_basic_metadata(&self, image_path: &Path) -> Result<(u32, u32, String, String, Option<u8>)> {
        
        let reader = ImageReader::open(image_path)?;
        let format = reader.format()
            .map(|f| format!("{:?}", f))
            .unwrap_or_else(|| "Unknown".to_string());
        
        let img = reader.decode()
            .map_err(|e| AppError::ProcessingError { 
                message: format!("Failed to decode image: {}", e) 
            })?;
        
        let (width, height) = img.dimensions();
        let color_type = format!("{:?}", img.color());
        
        Ok((width, height, format, color_type, None))
    }
    
    async fn extract_exif_metadata(&self, image_path: &Path) -> Result<ImageMetadata> {
        #[cfg(feature = "kamadak-exif")]
        {
            use kamadak_exif::{Reader, In, Tag, Value};
            use std::fs::File;
            use std::io::BufReader;
            
            let file = File::open(image_path)?;
            let mut bufreader = BufReader::new(file);
            
            let exifreader = Reader::new();
            
            match exifreader.read_from_container(&mut bufreader) {
                Ok(exif) => {
                    let mut metadata = ImageMetadata::default();
                    
                    for field in exif.fields() {
                        match field.tag {
                            Tag::Make => {
                                if let Value::Ascii(ref vec) = field.value {
                                    if let Some(make) = vec.first() {
                                        metadata.camera_make = Some(String::from_utf8_lossy(make).trim_matches('\0').to_string());
                                    }
                                }
                            },
                            Tag::Model => {
                                if let Value::Ascii(ref vec) = field.value {
                                    if let Some(model) = vec.first() {
                                        metadata.camera_model = Some(String::from_utf8_lossy(model).trim_matches('\0').to_string());
                                    }
                                }
                            },
                            Tag::LensModel => {
                                if let Value::Ascii(ref vec) = field.value {
                                    if let Some(lens) = vec.first() {
                                        metadata.lens_model = Some(String::from_utf8_lossy(lens).trim_matches('\0').to_string());
                                    }
                                }
                            },
                            Tag::DateTimeOriginal => {
                                if let Value::Ascii(ref vec) = field.value {
                                    if let Some(datetime) = vec.first() {
                                        metadata.datetime_original = Some(String::from_utf8_lossy(datetime).trim_matches('\0').to_string());
                                    }
                                }
                            },
                            Tag::DateTimeDigitized => {
                                if let Value::Ascii(ref vec) = field.value {
                                    if let Some(datetime) = vec.first() {
                                        metadata.datetime_digitized = Some(String::from_utf8_lossy(datetime).trim_matches('\0').to_string());
                                    }
                                }
                            },
                            Tag::FocalLength => {
                                if let Value::Rational(ref rationals) = field.value {
                                    if let Some(focal) = rationals.first() {
                                        metadata.focal_length = Some(focal.to_f64());
                                    }
                                }
                            },
                            Tag::FNumber => {
                                if let Value::Rational(ref rationals) = field.value {
                                    if let Some(fnumber) = rationals.first() {
                                        metadata.aperture = Some(fnumber.to_f64());
                                    }
                                }
                            },
                            Tag::PhotographicSensitivity => {
                                if let Value::Short(ref shorts) = field.value {
                                    if let Some(iso) = shorts.first() {
                                        metadata.iso_speed = Some(*iso as u32);
                                    }
                                }
                            },
                            Tag::ExposureTime => {
                                if let Value::Rational(ref rationals) = field.value {
                                    if let Some(exp_time) = rationals.first() {
                                        metadata.exposure_time = Some(format!("{}/{}", exp_time.num, exp_time.denom));
                                    }
                                }
                            },
                            Tag::Flash => {
                                if let Value::Short(ref shorts) = field.value {
                                    if let Some(flash) = shorts.first() {
                                        metadata.flash = Some(self.decode_flash_value(*flash));
                                    }
                                }
                            },
                            Tag::Orientation => {
                                if let Value::Short(ref shorts) = field.value {
                                    if let Some(orientation) = shorts.first() {
                                        metadata.orientation = Some(*orientation);
                                    }
                                }
                            },
                            Tag::WhiteBalance => {
                                if let Value::Short(ref shorts) = field.value {
                                    if let Some(wb) = shorts.first() {
                                        metadata.white_balance = Some(self.decode_white_balance_value(*wb));
                                    }
                                }
                            },
                            Tag::MeteringMode => {
                                if let Value::Short(ref shorts) = field.value {
                                    if let Some(metering) = shorts.first() {
                                        metadata.metering_mode = Some(self.decode_metering_mode_value(*metering));
                                    }
                                }
                            },
                            Tag::ExposureMode => {
                                if let Value::Short(ref shorts) = field.value {
                                    if let Some(exp_mode) = shorts.first() {
                                        metadata.exposure_mode = Some(self.decode_exposure_mode_value(*exp_mode));
                                    }
                                }
                            },
                            Tag::SceneCaptureType => {
                                if let Value::Short(ref shorts) = field.value {
                                    if let Some(scene) = shorts.first() {
                                        metadata.scene_capture_type = Some(self.decode_scene_capture_type_value(*scene));
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                    
                    // Extract GPS coordinates if available
                    if let (Some(lat_ref), Some(lat), Some(lon_ref), Some(lon)) = (
                        exif.get_field(Tag::GPSLatitudeRef, In::PRIMARY),
                        exif.get_field(Tag::GPSLatitude, In::PRIMARY),
                        exif.get_field(Tag::GPSLongitudeRef, In::PRIMARY),
                        exif.get_field(Tag::GPSLongitude, In::PRIMARY),
                    ) {
                        if let (
                            Value::Ascii(lat_ref_vec),
                            Value::Rational(lat_vec),
                            Value::Ascii(lon_ref_vec),
                            Value::Rational(lon_vec),
                        ) = (&lat_ref.value, &lat.value, &lon_ref.value, &lon.value) {
                            if lat_vec.len() >= 3 && lon_vec.len() >= 3 {
                                let lat_decimal = self.dms_to_decimal(&lat_vec[0..3]);
                                let lon_decimal = self.dms_to_decimal(&lon_vec[0..3]);
                                
                                let lat_sign = if lat_ref_vec.first().map(|b| b[0] == b'S').unwrap_or(false) { -1.0 } else { 1.0 };
                                let lon_sign = if lon_ref_vec.first().map(|b| b[0] == b'W').unwrap_or(false) { -1.0 } else { 1.0 };
                                
                                metadata.gps_latitude = Some(lat_decimal * lat_sign);
                                metadata.gps_longitude = Some(lon_decimal * lon_sign);
                            }
                        }
                    }
                    
                    // Extract GPS altitude if available
                    if let (Some(alt_ref), Some(alt)) = (
                        exif.get_field(Tag::GPSAltitudeRef, In::PRIMARY),
                        exif.get_field(Tag::GPSAltitude, In::PRIMARY),
                    ) {
                        if let (Value::Byte(alt_ref_vec), Value::Rational(alt_vec)) = 
                            (&alt_ref.value, &alt.value) {
                            if let (Some(alt_ref_byte), Some(alt_rational)) = (alt_ref_vec.first(), alt_vec.first()) {
                                let altitude = alt_rational.to_f64();
                                let altitude_sign = if *alt_ref_byte == 1 { -1.0 } else { 1.0 };
                                metadata.gps_altitude = Some(altitude * altitude_sign);
                            }
                        }
                    }
                    
                    Ok(metadata)
                }
                Err(_) => {
                    // No EXIF data or failed to read, return defaults
                    Ok(ImageMetadata::default())
                }
            }
        }
        #[cfg(not(feature = "kamadak-exif"))]
        {
            let _ = image_path;
            Ok(ImageMetadata::default())
        }
    }
    
    fn calculate_thumbnail_size(&self, width: u32, height: u32, max_size: u32) -> (u32, u32) {
        let aspect_ratio = width as f32 / height as f32;
        
        if width > height {
            let new_width = max_size.min(width);
            let new_height = (new_width as f32 / aspect_ratio) as u32;
            (new_width, new_height)
        } else {
            let new_height = max_size.min(height);
            let new_width = (new_height as f32 * aspect_ratio) as u32;
            (new_width, new_height)
        }
    }
    
    async fn create_thumbnail_for_image(&self, image_path: &Path, thumbnail_dir: &Path, max_size: u32) -> Result<String> {
        let filename = image_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("thumbnail");
        
        let thumbnail_filename = format!("{}_thumb.jpg", filename);
        let thumbnail_path = thumbnail_dir.join(&thumbnail_filename);
        
        self.create_thumbnail(image_path, &thumbnail_path, max_size).await?;
        
        Ok(thumbnail_path.to_string_lossy().to_string())
    }
    
    #[cfg(feature = "kamadak-exif")]
    fn dms_to_decimal(&self, dms: &[kamadak_exif::Rational]) -> f64 {
        if dms.len() >= 3 {
            let degrees = dms[0].to_f64();
            let minutes = dms[1].to_f64();
            let seconds = dms[2].to_f64();
            degrees + minutes / 60.0 + seconds / 3600.0
        } else {
            0.0
        }
    }
    
}

impl Default for ImageMetadata {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            format: "Unknown".to_string(),
            color_type: "Unknown".to_string(),
            bit_depth: None,
            file_size: 0,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            datetime_original: None,
            datetime_digitized: None,
            gps_latitude: None,
            gps_longitude: None,
            gps_altitude: None,
            focal_length: None,
            aperture: None,
            iso_speed: None,
            exposure_time: None,
            flash: None,
            orientation: None,
            dpi_x: None,
            dpi_y: None,
            color_space: None,
            white_balance: None,
            metering_mode: None,
            exposure_mode: None,
            scene_capture_type: None,
        }
    }
}

// Image processor manager
pub struct ImageProcessorManager {
    processor: StandardImageProcessor,
}

impl ImageProcessorManager {
    pub fn new() -> Self {
        Self {
            processor: StandardImageProcessor,
        }
    }
    
    pub async fn process_image(&self, image_path: &Path, thumbnail_dir: Option<&Path>) -> Result<ProcessedImage> {
        if !self.is_supported_format(image_path) {
            return Err(AppError::ProcessingError { 
                message: format!("Unsupported image format: {:?}", image_path.extension()) 
            });
        }
        
        self.processor.process(image_path, thumbnail_dir).await
    }
    
    pub async fn create_thumbnail(&self, image_path: &Path, thumbnail_path: &Path, max_size: u32) -> Result<()> {
        self.processor.create_thumbnail(image_path, thumbnail_path, max_size).await
    }
    
    pub fn is_supported_format(&self, image_path: &Path) -> bool {
        let extension = image_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        self.processor.supported_formats().contains(&extension.as_str())
    }
}

impl Default for ImageProcessorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_thumbnail_size_calculation() {
        let processor = StandardImageProcessor;
        
        // Test landscape image
        let (width, height) = processor.calculate_thumbnail_size(1920, 1080, 200);
        assert_eq!(width, 200);
        assert_eq!(height, 112);
        
        // Test portrait image
        let (width, height) = processor.calculate_thumbnail_size(1080, 1920, 200);
        assert_eq!(width, 112);
        assert_eq!(height, 200);
        
        // Test square image
        let (width, height) = processor.calculate_thumbnail_size(1000, 1000, 200);
        assert_eq!(width, 200);
        assert_eq!(height, 200);
    }
    
    #[test]
    fn test_image_format_support() {
        let manager = ImageProcessorManager::new();
        
        assert!(manager.is_supported_format(Path::new("test.jpg")));
        assert!(manager.is_supported_format(Path::new("test.png")));
        assert!(manager.is_supported_format(Path::new("test.gif")));
        assert!(manager.is_supported_format(Path::new("test.webp")));
        assert!(!manager.is_supported_format(Path::new("test.unknown")));
    }
}