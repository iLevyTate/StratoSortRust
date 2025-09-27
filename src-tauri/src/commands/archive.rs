use crate::error::{AppError, Result};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Runtime, State};
use crate::utils::security::validate_path;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressOptions {
    pub files: Vec<String>,
    pub output_path: String,
    pub format: Option<String>, // "zip", "tar", "tar.gz"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressResult {
    pub success: bool,
    pub output_file: String,
    pub compressed_size: u64,
    pub original_size: u64,
    pub compression_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractOptions {
    pub archive_path: String,
    pub output_directory: String,
    pub preserve_structure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractResult {
    pub success: bool,
    pub extracted_files: Vec<String>,
    pub total_size: u64,
}

// Internal generic version for flexibility
async fn compress_files_internal<R: Runtime>(
    options: CompressOptions,
    app: AppHandle<R>,
    _state: State<'_, Arc<AppState>>,
) -> Result<CompressResult> {
    // Validate all input paths
    for file_path in &options.files {
        validate_path(file_path, &app)?;
    }

    let _output_path = validate_path(&options.output_path, &app)?.into_path_buf();

    // Default to ZIP format if not specified
    let format = options.format.unwrap_or_else(|| "zip".to_string());

    // Calculate total size of original files
    let mut _original_size = 0u64;
    for file_path in &options.files {
        let path = PathBuf::from(file_path);
        if path.exists() {
            _original_size += std::fs::metadata(&path)?.len();
        }
    }

    match format.as_str() {
        "zip" => {
            #[cfg(feature = "zip")]
            {
                use zip::{CompressionMethod, ZipWriter};
                use zip::write::SimpleFileOptions;
                use std::io::Write;

                let file = std::fs::File::create(&_output_path)?;
                let mut zip = ZipWriter::new(file);

                let zip_options = SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Deflated);

                for file_path in &options.files {
                    let path = PathBuf::from(file_path);
                    if path.exists() && path.is_file() {
                        let file_name = path.file_name()
                            .ok_or_else(|| AppError::ProcessingError {
                                message: "Invalid file name".to_string(),
                            })?
                            .to_str()
                            .ok_or_else(|| AppError::ProcessingError {
                                message: "Non-UTF8 file name".to_string(),
                            })?;

                        zip.start_file(file_name, zip_options)?;
                        let contents = std::fs::read(&path)?;
                        zip.write_all(&contents)?;
                    }
                }

                zip.finish()?;

                // Get compressed file size
                let compressed_size = std::fs::metadata(&_output_path)?.len();
                let compression_ratio = if _original_size > 0 {
                    compressed_size as f32 / _original_size as f32
                } else {
                    1.0
                };

                // Log the operation
                tracing::info!("Compressed {} files to {}", options.files.len(), _output_path.display());

                Ok(CompressResult {
                    success: true,
                    output_file: _output_path.to_string_lossy().to_string(),
                    compressed_size,
                    original_size: _original_size,
                    compression_ratio,
                })
            }

            #[cfg(not(feature = "zip"))]
            {
                Err(AppError::ProcessingError {
                    message: "ZIP compression not available. Enable the 'zip' feature.".to_string(),
                })
            }
        }
        _ => {
            Err(AppError::ProcessingError {
                message: format!("Unsupported archive format: {}", format),
            })
        }
    }
}

// Non-generic wrapper for Tauri command registration
#[tauri::command]
pub async fn compress_files(
    options: CompressOptions,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<CompressResult> {
    compress_files_internal(options, app, state).await
}

// Internal generic version
async fn extract_archive_internal<R: Runtime>(
    options: ExtractOptions,
    app: AppHandle<R>,
    _state: State<'_, Arc<AppState>>,
) -> Result<ExtractResult> {
    let archive_path = validate_path(&options.archive_path, &app)?.into_path_buf();
    let output_dir = validate_path(&options.output_directory, &app)?.into_path_buf();

    if !archive_path.exists() {
        return Err(AppError::FileNotFound {
            path: archive_path.to_string_lossy().to_string(),
        });
    }

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
    }

    let mut extracted_files: Vec<String> = Vec::new();
    let mut total_size = 0u64;

    // Detect archive format from extension
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext.to_lowercase().as_str() {
        "zip" => {
            #[cfg(feature = "zip")]
            {
                use zip::ZipArchive;
                use std::fs::File;

                let file = File::open(&archive_path)?;
                let mut archive = ZipArchive::new(file)?;

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i)?;
                    let output_path = if options.preserve_structure {
                        output_dir.join(file.name())
                    } else {
                        let name = PathBuf::from(file.name())
                            .file_name()
                            .ok_or_else(|| AppError::ProcessingError {
                                message: "Invalid file name in archive".to_string(),
                            })?
                            .to_owned();
                        output_dir.join(name)
                    };

                    if file.is_dir() {
                        std::fs::create_dir_all(&output_path)?;
                    } else {
                        if let Some(parent) = output_path.parent() {
                            if !parent.exists() {
                                std::fs::create_dir_all(parent)?;
                            }
                        }

                        let mut outfile = std::fs::File::create(&output_path)?;
                        std::io::copy(&mut file, &mut outfile)?;
                        total_size += file.size();
                        extracted_files.push(output_path.to_string_lossy().to_string());
                    }
                }

                // Log the operation
                tracing::info!("Extracted {} files from {} to {}",
                    extracted_files.len(),
                    archive_path.display(),
                    output_dir.display());

                Ok(ExtractResult {
                    success: true,
                    extracted_files,
                    total_size,
                })
            }

            #[cfg(not(feature = "zip"))]
            {
                Err(AppError::ProcessingError {
                    message: "ZIP extraction not available. Enable the 'zip' feature.".to_string(),
                })
            }
        }
        _ => {
            Err(AppError::ProcessingError {
                message: format!("Unsupported archive format: {}", ext),
            })
        }
    }
}

// Non-generic wrapper for Tauri command registration
#[tauri::command]
pub async fn extract_archive(
    options: ExtractOptions,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<ExtractResult> {
    extract_archive_internal(options, app, state).await
}

// Internal generic version
async fn get_archive_info_internal<R: Runtime>(
    archive_path: String,
    app: AppHandle<R>,
) -> Result<crate::core::archive_handler::ArchiveInfo> {
    use crate::core::archive_handler::{ArchiveHandler, ZipHandler};

    let path = validate_path(&archive_path, &app)?.into_path_buf();

    if !path.exists() {
        return Err(AppError::FileNotFound {
            path: path.to_string_lossy().to_string(),
        });
    }

    let handler = ZipHandler;
    handler.list_contents(&path).await
}

// Non-generic wrapper for Tauri command registration
#[tauri::command]
pub async fn get_archive_info(
    archive_path: String,
    app: AppHandle,
) -> Result<crate::core::archive_handler::ArchiveInfo> {
    get_archive_info_internal(archive_path, app).await
}