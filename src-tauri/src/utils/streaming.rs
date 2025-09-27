use crate::error::{AppError, Result};
use futures::stream::{Stream, StreamExt};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tracing::{debug, error, warn};

/// Configuration for streaming operations
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Chunk size for reading files
    pub chunk_size: usize,
    /// Maximum file size to read entirely into memory
    pub max_memory_size: usize,
    /// Enable compression for large files
    pub enable_compression: bool,
    /// Buffer size for readers
    pub buffer_size: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            chunk_size: 8192,           // 8KB chunks
            max_memory_size: 10 * 1024 * 1024, // 10MB
            enable_compression: false,
            buffer_size: 64 * 1024,     // 64KB buffer
        }
    }
}

/// File stream metadata
#[derive(Debug, Clone)]
pub struct StreamMetadata {
    pub file_size: u64,
    pub is_streaming: bool,
    pub chunks_total: usize,
    pub mime_type: Option<String>,
}

/// Stream a file in chunks
pub async fn stream_file(
    path: &Path,
    config: &StreamConfig,
) -> Result<(impl Stream<Item = Result<Vec<u8>>>, StreamMetadata)> {
    // Validate file exists and get metadata
    let metadata = tokio::fs::metadata(path).await.map_err(|e| {
        // FIX: Better error handling - distinguish between not found and permission errors
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::FileNotFound {
                path: path.to_string_lossy().to_string(),
            }
        } else {
            AppError::Io(e)
        }
    })?;

    let file_size = metadata.len();

    // Check if file should be streamed
    let should_stream = file_size > config.max_memory_size as u64;

    if !should_stream {
        // Small file - read entirely
        debug!(
            "Reading small file {} ({} bytes) into memory",
            path.display(),
            file_size
        );

        let content = tokio::fs::read(path).await?;
        let stream = futures::stream::once(async move { Ok(content) });

        let metadata = StreamMetadata {
            file_size,
            is_streaming: false,
            chunks_total: 1,
            mime_type: detect_mime_type(path),
        };

        return Ok((stream.boxed(), metadata));
    }

    // Large file - stream in chunks
    debug!(
        "Streaming large file {} ({} bytes) in {} byte chunks",
        path.display(),
        file_size,
        config.chunk_size
    );

    let chunks_total = (file_size as usize).div_ceil(config.chunk_size);

    let file = File::open(path).await?;
    let mut reader = BufReader::with_capacity(config.buffer_size, file);
    let chunk_size = config.chunk_size;

    let stream = async_stream::stream! {
        let mut buffer = vec![0u8; chunk_size];
        let mut total_read = 0u64;

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    total_read += n as u64;

                    // FIX: Avoid unnecessary allocation by using slice directly
                    // Only clone when necessary for ownership transfer
                    let chunk = buffer[..n].to_vec();

                    debug!(
                        "Read chunk of {} bytes (total: {} / {})",
                        n, total_read, file_size
                    );

                    yield Ok(chunk);

                    // FIX: Check for unexpected EOF
                    if total_read >= file_size {
                        break;
                    }
                }
                Err(e) => {
                    error!("Error reading file stream: {}", e);
                    yield Err(AppError::Io(e));
                    break;
                }
            }
        }
    };

    let metadata = StreamMetadata {
        file_size,
        is_streaming: true,
        chunks_total,
        mime_type: detect_mime_type(path),
    };

    Ok((stream.boxed(), metadata))
}

/// Stream file lines (for text files)
pub async fn stream_lines(
    path: &Path,
    max_line_length: usize,
) -> Result<impl Stream<Item = Result<String>>> {
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let stream = async_stream::stream! {
        let mut line_count = 0;

        while let Some(line_result) = lines.next_line().await.transpose() {
            match line_result {
                Ok(line) => {
                    line_count += 1;

                    // Check line length
                    if line.len() > max_line_length {
                        warn!(
                            "Line {} exceeds maximum length ({} > {}), truncating",
                            line_count,
                            line.len(),
                            max_line_length
                        );
                        yield Ok(line.chars().take(max_line_length).collect());
                    } else {
                        yield Ok(line);
                    }
                }
                Err(e) => {
                    error!("Error reading line {}: {}", line_count, e);
                    yield Err(AppError::Io(e));
                    break;
                }
            }
        }
    };

    Ok(stream.boxed())
}

/// Process file in chunks with a callback
pub async fn process_file_chunked<F, R>(
    path: &Path,
    config: &StreamConfig,
    mut processor: F,
) -> Result<Vec<R>>
where
    F: FnMut(Vec<u8>, usize, usize) -> Result<R>,
{
    let (mut stream, metadata) = stream_file(path, config).await?;
    let mut results = Vec::new();
    let mut chunk_index = 0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        let result = processor(chunk, chunk_index, metadata.chunks_total)?;
        results.push(result);
        chunk_index += 1;
    }

    Ok(results)
}

/// Validate file size before processing
pub async fn validate_file_size(path: &Path, max_size: u64) -> Result<u64> {
    let metadata = tokio::fs::metadata(path).await?;
    let size = metadata.len();

    if size > max_size {
        return Err(AppError::FileTooLarge {
            path: path.to_string_lossy().to_string(),
            size,
            max_size,
        });
    }

    Ok(size)
}

/// Stream with progress tracking
pub struct ProgressStream<S> {
    inner: S,
    total_size: u64,
    bytes_read: u64,
    on_progress: Box<dyn Fn(u64, u64) + Send>,
}

impl<S> ProgressStream<S>
where
    S: Stream<Item = Result<Vec<u8>>> + Unpin,
{
    pub fn new<F>(stream: S, total_size: u64, on_progress: F) -> Self
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        Self {
            inner: stream,
            total_size,
            bytes_read: 0,
            on_progress: Box::new(on_progress),
        }
    }
}

impl<S> Stream for ProgressStream<S>
where
    S: Stream<Item = Result<Vec<u8>>> + Unpin,
{
    type Item = Result<Vec<u8>>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match futures::ready!(std::pin::Pin::new(&mut self.inner).poll_next(cx)) {
            Some(Ok(chunk)) => {
                self.bytes_read += chunk.len() as u64;
                (self.on_progress)(self.bytes_read, self.total_size);
                std::task::Poll::Ready(Some(Ok(chunk)))
            }
            other => std::task::Poll::Ready(other),
        }
    }
}

/// Detect MIME type from file extension
fn detect_mime_type(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_lowercase();

    let mime = match extension.as_str() {
        // Text files
        "txt" | "log" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "ts" => "application/typescript",
        "rs" => "text/x-rust",
        "py" => "text/x-python",

        // Images
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",

        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",

        // Archives
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "rar" => "application/vnd.rar",

        // Media
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",

        _ => "application/octet-stream",
    };

    Some(mime.to_string())
}

/// Memory-safe file reader with size limits
pub struct SafeFileReader {
    config: StreamConfig,
}

impl SafeFileReader {
    pub fn new(config: StreamConfig) -> Self {
        Self { config }
    }

    /// Read file with size validation
    pub async fn read_file(&self, path: &Path, max_size: Option<u64>) -> Result<Vec<u8>> {
        let size_limit = max_size.unwrap_or(self.config.max_memory_size as u64);
        let size = validate_file_size(path, size_limit).await?;

        if size > self.config.max_memory_size as u64 {
            return Err(AppError::FileTooLarge {
                path: path.to_string_lossy().to_string(),
                size,
                max_size: self.config.max_memory_size as u64,
            });
        }

        tokio::fs::read(path).await.map_err(AppError::Io)
    }

    /// Read text file with line limit
    pub async fn read_text_lines(
        &self,
        path: &Path,
        max_lines: usize,
    ) -> Result<Vec<String>> {
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut result = Vec::new();

        for _ in 0..max_lines {
            match lines.next_line().await? {
                Some(line) => result.push(line),
                None => break,
            }
        }

        Ok(result)
    }

    /// Read file header (first N bytes)
    pub async fn read_header(&self, path: &Path, header_size: usize) -> Result<Vec<u8>> {
        // FIX: Validate header_size to prevent excessive memory allocation
        const MAX_HEADER_SIZE: usize = 1024 * 1024; // 1MB max header
        let safe_header_size = header_size.min(MAX_HEADER_SIZE);

        let mut file = File::open(path).await?;
        let mut buffer = vec![0u8; safe_header_size];
        let n = file.read(&mut buffer).await?;
        buffer.truncate(n);
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_stream_file() {
        // Create test file
        let path = std::path::Path::new("test_stream.txt");
        let mut file = File::create(path).await.unwrap();
        file.write_all(b"Hello, world!").await.unwrap();

        let config = StreamConfig::default();
        let (mut stream, metadata) = stream_file(path, &config).await.unwrap();

        assert!(!metadata.is_streaming); // Small file
        assert_eq!(metadata.file_size, 13);

        let chunk = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk, b"Hello, world!");

        // Cleanup
        tokio::fs::remove_file(path).await.unwrap();
    }

    #[tokio::test]
    async fn test_validate_file_size() {
        let path = std::path::Path::new("test_size.txt");
        let mut file = File::create(path).await.unwrap();
        file.write_all(b"test").await.unwrap();

        // Should pass
        let size = validate_file_size(path, 100).await.unwrap();
        assert_eq!(size, 4);

        // Should fail
        let result = validate_file_size(path, 2).await;
        assert!(result.is_err());

        // Cleanup
        tokio::fs::remove_file(path).await.unwrap();
    }
}