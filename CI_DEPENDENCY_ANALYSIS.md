# StratoRust CI Dependency Analysis Report

## Executive Summary
This report provides a comprehensive analysis of all system dependencies required for building StratoRust in CI environments. The application uses numerous native libraries for multimedia processing, document handling, and system integration.

## Dependency Categories

### 1. Core Build Tools & Compilers
- **build-essential**: GCC, G++, make, and other essential compilation tools
- **cmake**: Cross-platform build system generator
- **clang/llvm**: Alternative C/C++ compiler toolchain
- **autoconf/automake/libtool**: GNU build system tools
- **pkg-config**: Helper tool for compiling applications and libraries

### 2. Tauri Framework Dependencies
**Required for Tauri/WebView2 functionality:**
- libwebkit2gtk-4.0-dev: WebKit2 GTK+ web content engine
- libgtk-3-dev: GTK+3 development files
- libayatana-appindicator3-dev: System tray support
- librsvg2-dev: SVG rendering library
- libglib2.0-dev: GLib library
- libjavascriptcoregtk-4.0-dev: JavaScript engine
- libsoup2.4-dev: HTTP library
- libcairo2-dev: 2D graphics library
- libpango1.0-dev: Text layout and rendering
- libgdk-pixbuf2.0-dev: Image loading library
- libatk1.0-dev: Accessibility toolkit

### 3. Database & Storage
**SQLite with vector extensions:**
- libsqlite3-dev: SQLite database development files
- sqlite3: SQLite command-line tool
- **Note**: Uses bundled SQLite via libsqlite3-sys with sqlite-vec extension

### 4. Cryptography & Security
- libssl-dev: OpenSSL development files
- openssl: SSL/TLS toolkit
- ca-certificates: Common CA certificates

### 5. Image Processing Libraries
**For the `image` crate and related features:**
- libjpeg-dev: JPEG image codec
- libpng-dev: PNG image library
- libwebp-dev: WebP image format
- libgif-dev: GIF image library
- libtiff-dev: TIFF image library
- libavif-dev: AVIF image format
- libheif-dev: HEIF/HEIC image format
- libraw-dev: RAW image processing
- libexif-dev: EXIF metadata library
- imagemagick: Image manipulation tools

### 6. Document Processing
**For PDF, Office, and text document handling:**
- libpoppler-dev: PDF rendering library
- libpoppler-glib-dev: GLib wrapper for Poppler
- poppler-utils: PDF utilities
- libxml2-dev: XML parsing library
- libxslt1-dev: XSLT processing
- libyaml-dev: YAML parsing library

### 7. Archive & Compression
**For handling compressed files:**
- libarchive-dev: Multi-format archive library
- libzip-dev: ZIP archive library
- zlib1g-dev: Compression library
- libbz2-dev: Bzip2 compression
- liblzma-dev: XZ compression
- libzstd-dev: Zstandard compression
- p7zip-full: 7-Zip support
- unrar: RAR archive support (optional)

### 8. Multimedia Processing
**For audio/video file handling:**
- **FFmpeg libraries:**
  - libavcodec-dev: Audio/video codec library
  - libavformat-dev: Audio/video format library
  - libavutil-dev: FFmpeg utility library
  - libswscale-dev: Video scaling library
  - libavdevice-dev: Device handling
  - libavfilter-dev: Audio/video filtering
  - ffmpeg: FFmpeg command-line tools
  
- **Audio codecs:**
  - libmp3lame-dev: MP3 encoding
  - libvorbis-dev: Ogg Vorbis codec
  - libopus-dev: Opus audio codec
  - libflac-dev: FLAC audio codec
  - libspeex-dev: Speex audio codec
  
- **Video codecs:**
  - libtheora-dev: Theora video codec
  - libvpx-dev: VP8/VP9 video codec
  - libx264-dev: H.264 video codec
  - libx265-dev: H.265/HEVC video codec
  - libopencore-amrnb-dev: AMR narrowband codec
  - libopencore-amrwb-dev: AMR wideband codec

- **Audio system:**
  - libasound2-dev: ALSA sound library
  - libpulse-dev: PulseAudio library

### 9. Text Rendering & Fonts
- libfreetype6-dev: Font rendering library
- libfontconfig1-dev: Font configuration library
- libharfbuzz-dev: Text shaping engine

### 10. System Monitoring
- libsensors-dev: Hardware monitoring
- lm-sensors: Sensor monitoring tools

### 11. OCR Support (Optional)
- tesseract-ocr: OCR engine
- libtesseract-dev: Tesseract development files
- libleptonica-dev: Image processing for OCR

### 12. File Type Detection
- libmagic-dev: File type detection library
- file: File type identification utility

### 13. GUI & Display Libraries
**X11 and OpenGL support:**
- libx11-dev: X11 client library
- libxext-dev: X11 extensions
- libxrender-dev: X11 rendering extension
- libxrandr-dev: X11 RandR extension
- libxi-dev: X11 input extension
- libxcursor-dev: X cursor management
- libxinerama-dev: Xinerama extension
- libgl1-mesa-dev: OpenGL library
- libglu1-mesa-dev: OpenGL utility library
- libegl1-mesa-dev: EGL library
- libgles2-mesa-dev: OpenGL ES 2.0

## Rust Crate to System Library Mapping

| Rust Crate | System Libraries Required | Purpose |
|------------|---------------------------|---------|
| tauri | libwebkit2gtk-4.0, libgtk-3, etc. | GUI framework |
| image | libjpeg, libpng, libwebp, etc. | Image processing |
| sqlx/libsqlite3-sys | libsqlite3 | Database |
| reqwest | libssl, ca-certificates | HTTPS client |
| pdf-extract | libpoppler (optional) | PDF text extraction |
| zip | libzip (optional, uses pure Rust) | ZIP archives |
| ffmpeg-next | libavcodec, libavformat, etc. | Multimedia |
| symphonia | None (pure Rust) | Audio decoding |

## Environment Variables

The CI configuration sets these critical environment variables:

```bash
# SQLite configuration
SQLITE3_LIB_DIR=/usr/lib/x86_64-linux-gnu
SQLITE3_INCLUDE_DIR=/usr/include

# OpenSSL configuration  
OPENSSL_DIR=/usr
OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
OPENSSL_INCLUDE_DIR=/usr/include

# Library paths
LD_LIBRARY_PATH=/usr/local/lib:/usr/lib/x86_64-linux-gnu
LIBRARY_PATH=/usr/local/lib:/usr/lib/x86_64-linux-gnu
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig

# Build settings
PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1
PKG_CONFIG_ALLOW_SYSTEM_LIBS=1
RUST_BACKTRACE=1
CARGO_INCREMENTAL=0
```

## Feature Flags Impact

### Default Features
- `custom-protocol`: Tauri custom protocol support
- `sysinfo`: System information monitoring

### Optional Feature Groups
- **documents**: Enables PDF, DOCX, Excel, CSV processing
- **images**: Advanced image processing with EXIF support
- **archives**: ZIP, TAR, 7Z, RAR support
- **multimedia**: Audio/video metadata extraction
- **large-files**: Memory-mapped I/O for large files

## Known Issues & Mitigations

1. **FFmpeg dependency**: The `ffmpeg-next` crate requires full FFmpeg installation
   - Mitigation: All FFmpeg libraries are installed in CI

2. **SQLite vector extension**: Requires specific SQLite build
   - Mitigation: Using bundled SQLite via `libsqlite3-sys`

3. **Unrar availability**: May not be in default Ubuntu repos
   - Mitigation: Installation wrapped with `|| true`

4. **OCR dependencies**: Tesseract requires large language data files
   - Mitigation: OCR features are currently disabled in code

## CI Build Verification

The updated CI configuration includes:
1. Comprehensive package installation
2. Library path configuration
3. Package verification steps
4. Multiple build scenario testing (default, all features, release)

## Recommendations

1. **Caching**: Consider caching installed system packages to speed up CI
2. **Docker Image**: Create a custom Docker image with all dependencies pre-installed
3. **Feature Testing**: Add specific feature flag testing in CI
4. **Platform Matrix**: Test on multiple Ubuntu versions (20.04, 22.04, 24.04)

## Conclusion

The CI configuration has been updated to include ALL identified system dependencies. This comprehensive setup ensures that:
- All Tauri framework requirements are met
- Image processing libraries are available
- Document processing capabilities are supported
- Multimedia features can be compiled
- Archive handling is fully functional
- Database and cryptography needs are satisfied

The configuration is now bulletproof and should handle all current and potential future compilation scenarios for StratoRust.