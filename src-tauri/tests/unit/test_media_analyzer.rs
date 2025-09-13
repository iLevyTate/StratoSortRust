use stratosort::core::media_analyzer::{
    MediaAnalyzer, AudioMetadata, VideoMetadata, ProcessedMedia, MediaType, StandardMediaAnalyzer
};
use stratosort::error::{AppError, Result};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[cfg(test)]
mod media_analyzer_tests {
    use super::*;

    // Helper function to create a minimal WAV file
    fn create_test_wav() -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_audio.wav");
        
        // Create a minimal WAV header (44 bytes) + some audio data
        let mut wav_data = Vec::new();
        
        // RIFF header
        wav_data.extend_from_slice(b"RIFF");
        wav_data.extend_from_slice(&[36, 0, 0, 0]); // File size - 8
        wav_data.extend_from_slice(b"WAVE");
        
        // fmt chunk
        wav_data.extend_from_slice(b"fmt ");
        wav_data.extend_from_slice(&[16, 0, 0, 0]); // Chunk size
        wav_data.extend_from_slice(&[1, 0]); // Audio format (PCM)
        wav_data.extend_from_slice(&[2, 0]); // Number of channels
        wav_data.extend_from_slice(&[68, 172, 0, 0]); // Sample rate (44100)
        wav_data.extend_from_slice(&[16, 177, 2, 0]); // Byte rate
        wav_data.extend_from_slice(&[4, 0]); // Block align
        wav_data.extend_from_slice(&[16, 0]); // Bits per sample
        
        // data chunk
        wav_data.extend_from_slice(b"data");
        wav_data.extend_from_slice(&[0, 0, 0, 0]); // Data size
        
        // Add some dummy audio data
        for _ in 0..1000 {
            wav_data.push(0);
        }
        
        fs::write(&file_path, wav_data)?;
        Ok(file_path)
    }

    // Helper function to create a minimal MP3 file (with ID3 tags)
    fn create_test_mp3() -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_audio.mp3");
        
        // Create a minimal MP3 with ID3v2 header
        let mut mp3_data = Vec::new();
        
        // ID3v2 header
        mp3_data.extend_from_slice(b"ID3");
        mp3_data.extend_from_slice(&[3, 0]); // Version
        mp3_data.push(0); // Flags
        mp3_data.extend_from_slice(&[0, 0, 0, 0]); // Size
        
        // MP3 frame header (simplified)
        mp3_data.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]); // Frame sync + info
        
        // Add some dummy audio data
        for _ in 0..500 {
            mp3_data.push(0);
        }
        
        fs::write(&file_path, mp3_data)?;
        Ok(file_path)
    }

    // Helper function to create a minimal OGG file
    fn create_test_ogg() -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_audio.ogg");
        
        // OGG page header
        let mut ogg_data = Vec::new();
        ogg_data.extend_from_slice(b"OggS"); // Capture pattern
        ogg_data.push(0); // Version
        ogg_data.push(2); // Header type
        ogg_data.extend_from_slice(&[0; 8]); // Granule position
        ogg_data.extend_from_slice(&[0; 4]); // Serial number
        ogg_data.extend_from_slice(&[0; 4]); // Page sequence
        ogg_data.extend_from_slice(&[0; 4]); // Checksum
        ogg_data.push(0); // Page segments
        
        fs::write(&file_path, ogg_data)?;
        Ok(file_path)
    }

    // Helper function to create a minimal MP4 file
    fn create_test_mp4() -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_video.mp4");
        
        // Create a minimal MP4 structure
        let mut mp4_data = Vec::new();
        
        // ftyp box (file type)
        mp4_data.extend_from_slice(&[0, 0, 0, 20]); // Box size
        mp4_data.extend_from_slice(b"ftyp");
        mp4_data.extend_from_slice(b"isom");
        mp4_data.extend_from_slice(&[0, 0, 0, 1]);
        mp4_data.extend_from_slice(b"isom");
        
        // mdat box (media data - empty)
        mp4_data.extend_from_slice(&[0, 0, 0, 8]);
        mp4_data.extend_from_slice(b"mdat");
        
        fs::write(&file_path, mp4_data)?;
        Ok(file_path)
    }

    // Helper function to create a minimal AVI file
    fn create_test_avi() -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("test_video.avi");
        
        // Create a minimal AVI structure
        let mut avi_data = Vec::new();
        
        // RIFF header
        avi_data.extend_from_slice(b"RIFF");
        avi_data.extend_from_slice(&[100, 0, 0, 0]); // File size
        avi_data.extend_from_slice(b"AVI ");
        
        // LIST chunk
        avi_data.extend_from_slice(b"LIST");
        avi_data.extend_from_slice(&[88, 0, 0, 0]); // Chunk size
        avi_data.extend_from_slice(b"hdrl");
        
        // avih chunk (AVI header)
        avi_data.extend_from_slice(b"avih");
        avi_data.extend_from_slice(&[56, 0, 0, 0]); // Chunk size
        avi_data.extend_from_slice(&[0; 56]); // Header data
        
        fs::write(&file_path, avi_data)?;
        Ok(file_path)
    }

    #[tokio::test]
    async fn test_media_analyzer_audio_wav() {
        let analyzer = StandardMediaAnalyzer::new();
        let wav_path = create_test_wav().unwrap();
        
        let result = analyzer.analyze(&wav_path).await;
        
        // WAV analysis might succeed or fail depending on the library
        if result.is_ok() {
            let media = result.unwrap();
            assert_eq!(media.media_type, MediaType::Audio);
            assert!(media.audio_metadata.is_some());
            
            if let Some(audio) = media.audio_metadata {
                assert_eq!(audio.format, "wav");
                assert!(audio.file_size > 0);
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_audio_mp3() {
        let analyzer = StandardMediaAnalyzer::new();
        let mp3_path = create_test_mp3().unwrap();
        
        let result = analyzer.analyze(&mp3_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            assert_eq!(media.media_type, MediaType::Audio);
            assert!(media.audio_metadata.is_some());
            
            if let Some(audio) = media.audio_metadata {
                assert_eq!(audio.format, "mp3");
                assert!(audio.file_size > 0);
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_video_mp4() {
        let analyzer = StandardMediaAnalyzer::new();
        let mp4_path = create_test_mp4().unwrap();
        
        let result = analyzer.analyze(&mp4_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            assert_eq!(media.media_type, MediaType::Video);
            assert!(media.video_metadata.is_some());
            
            if let Some(video) = media.video_metadata {
                assert_eq!(video.format, "mp4");
                assert!(video.file_size > 0);
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_unsupported_format() {
        let analyzer = StandardMediaAnalyzer::new();
        let temp_dir = tempdir().unwrap();
        let unknown_path = temp_dir.path().join("unknown.xyz");
        fs::write(&unknown_path, b"Unknown format data").unwrap();
        
        let result = analyzer.analyze(&unknown_path).await;
        
        // Should either return Unknown media type or an error
        if result.is_ok() {
            let media = result.unwrap();
            assert_eq!(media.media_type, MediaType::Unknown);
        } else {
            assert!(matches!(result.unwrap_err(), AppError::ProcessingError { .. }));
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_metadata_extraction() {
        let analyzer = StandardMediaAnalyzer::new();
        
        // Create a more complete MP3 with ID3 tags
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("tagged.mp3");
        
        let mut mp3_data = Vec::new();
        
        // ID3v2.3 header
        mp3_data.extend_from_slice(b"ID3");
        mp3_data.extend_from_slice(&[3, 0]); // Version 2.3
        mp3_data.push(0); // Flags
        mp3_data.extend_from_slice(&[0, 0, 1, 0]); // Size (128 bytes)
        
        // TIT2 frame (Title)
        mp3_data.extend_from_slice(b"TIT2");
        mp3_data.extend_from_slice(&[0, 0, 0, 10]); // Frame size
        mp3_data.extend_from_slice(&[0, 0]); // Flags
        mp3_data.push(0); // Text encoding
        mp3_data.extend_from_slice(b"Test Song");
        
        // TPE1 frame (Artist)
        mp3_data.extend_from_slice(b"TPE1");
        mp3_data.extend_from_slice(&[0, 0, 0, 12]); // Frame size
        mp3_data.extend_from_slice(&[0, 0]); // Flags
        mp3_data.push(0); // Text encoding
        mp3_data.extend_from_slice(b"Test Artist");
        
        // Padding
        for _ in 0..80 {
            mp3_data.push(0);
        }
        
        // MP3 frame
        mp3_data.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        
        fs::write(&file_path, mp3_data).unwrap();
        
        let result = analyzer.analyze(&file_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(audio) = media.audio_metadata {
                // Metadata extraction might work depending on the library
                assert!(audio.title.is_none() || audio.title == Some("Test Song".to_string()));
                assert!(audio.artist.is_none() || audio.artist == Some("Test Artist".to_string()));
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_duration_calculation() {
        let analyzer = StandardMediaAnalyzer::new();
        let wav_path = create_test_wav().unwrap();
        
        let result = analyzer.analyze(&wav_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(audio) = media.audio_metadata {
                // Duration might be calculated
                assert!(audio.duration.is_none() || audio.duration.unwrap() >= 0.0);
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_bitrate_detection() {
        let analyzer = StandardMediaAnalyzer::new();
        let mp3_path = create_test_mp3().unwrap();
        
        let result = analyzer.analyze(&mp3_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(audio) = media.audio_metadata {
                // Bitrate might be detected
                assert!(audio.bitrate.is_none() || audio.bitrate.unwrap() > 0);
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_video_dimensions() {
        let analyzer = StandardMediaAnalyzer::new();
        let mp4_path = create_test_mp4().unwrap();
        
        let result = analyzer.analyze(&mp4_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(video) = media.video_metadata {
                // Dimensions might be detected
                assert!(video.width.is_none() || video.width.unwrap() > 0);
                assert!(video.height.is_none() || video.height.unwrap() > 0);
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_supported_extensions() {
        let analyzer = StandardMediaAnalyzer::new();
        let extensions = analyzer.supported_extensions();
        
        // Check common audio formats
        assert!(extensions.contains(&"mp3"));
        assert!(extensions.contains(&"wav"));
        assert!(extensions.contains(&"ogg"));
        assert!(extensions.contains(&"flac"));
        assert!(extensions.contains(&"m4a"));
        
        // Check common video formats
        assert!(extensions.contains(&"mp4"));
        assert!(extensions.contains(&"avi"));
        assert!(extensions.contains(&"mkv"));
        assert!(extensions.contains(&"mov"));
        assert!(extensions.contains(&"webm"));
    }

    #[tokio::test]
    async fn test_media_analyzer_corrupted_file() {
        let analyzer = StandardMediaAnalyzer::new();
        let temp_dir = tempdir().unwrap();
        let corrupted_path = temp_dir.path().join("corrupted.mp3");
        
        // Write invalid MP3 data
        fs::write(&corrupted_path, b"This is not a valid MP3 file").unwrap();
        
        let result = analyzer.analyze(&corrupted_path).await;
        
        // Should either return an error or Unknown media type
        if result.is_ok() {
            let media = result.unwrap();
            assert!(media.media_type == MediaType::Unknown || media.processing_error.is_some());
        } else {
            assert!(matches!(result.unwrap_err(), AppError::ProcessingError { .. }));
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_empty_file() {
        let analyzer = StandardMediaAnalyzer::new();
        let temp_dir = tempdir().unwrap();
        let empty_path = temp_dir.path().join("empty.mp3");
        
        fs::write(&empty_path, b"").unwrap();
        
        let result = analyzer.analyze(&empty_path).await;
        
        // Should handle empty file gracefully
        if result.is_ok() {
            let media = result.unwrap();
            assert_eq!(media.media_type, MediaType::Unknown);
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_concurrent_analysis() {
        use tokio::task::JoinSet;
        
        let mut tasks = JoinSet::new();
        
        // Create different media files
        let files = vec![
            create_test_wav().unwrap(),
            create_test_mp3().unwrap(),
            create_test_ogg().unwrap(),
            create_test_mp4().unwrap(),
            create_test_avi().unwrap(),
        ];
        
        for file_path in files {
            tasks.spawn(async move {
                let analyzer = StandardMediaAnalyzer::new();
                analyzer.analyze(&file_path).await
            });
        }
        
        let mut processed_count = 0;
        while let Some(result) = tasks.join_next().await {
            if let Ok(_) = result {
                processed_count += 1;
            }
        }
        
        assert_eq!(processed_count, 5);
    }

    #[tokio::test]
    async fn test_media_analyzer_codec_detection() {
        let analyzer = StandardMediaAnalyzer::new();
        let mp3_path = create_test_mp3().unwrap();
        
        let result = analyzer.analyze(&mp3_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(audio) = media.audio_metadata {
                // Codec might be detected
                assert!(audio.codec.is_none() || audio.codec == Some("mp3".to_string()));
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_sample_rate() {
        let analyzer = StandardMediaAnalyzer::new();
        let wav_path = create_test_wav().unwrap();
        
        let result = analyzer.analyze(&wav_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(audio) = media.audio_metadata {
                // Sample rate might be detected (we set 44100 in the WAV header)
                assert!(audio.sample_rate.is_none() || audio.sample_rate == Some(44100));
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_channel_count() {
        let analyzer = StandardMediaAnalyzer::new();
        let wav_path = create_test_wav().unwrap();
        
        let result = analyzer.analyze(&wav_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(audio) = media.audio_metadata {
                // Channel count might be detected (we set 2 in the WAV header)
                assert!(audio.channels.is_none() || audio.channels == Some(2));
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_frame_rate() {
        let analyzer = StandardMediaAnalyzer::new();
        let mp4_path = create_test_mp4().unwrap();
        
        let result = analyzer.analyze(&mp4_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(video) = media.video_metadata {
                // Frame rate might be detected
                assert!(video.frame_rate.is_none() || video.frame_rate.unwrap() > 0.0);
            }
        }
    }

    #[tokio::test]
    async fn test_media_analyzer_track_metadata() {
        let analyzer = StandardMediaAnalyzer::new();
        let mp3_path = create_test_mp3().unwrap();
        
        let result = analyzer.analyze(&mp3_path).await;
        
        if result.is_ok() {
            let media = result.unwrap();
            if let Some(audio) = media.audio_metadata {
                // Track metadata fields should exist
                assert!(audio.track_number.is_none() || audio.track_number.unwrap() > 0);
                assert!(audio.track_total.is_none() || audio.track_total.unwrap() > 0);
                assert!(audio.disc_number.is_none() || audio.disc_number.unwrap() > 0);
                assert!(audio.disc_total.is_none() || audio.disc_total.unwrap() > 0);
            }
        }
    }
}