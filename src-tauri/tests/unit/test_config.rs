use stratosort::config::Config;
use tempfile::tempdir;
use std::fs;

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        
        assert_eq!(config.ai_provider, "ollama");
        assert_eq!(config.ollama_host, "http://localhost:11434");
        assert_eq!(config.ollama_model, "llama3.2:3b");
        assert_eq!(config.max_concurrent_analysis, 3);
        assert_eq!(config.max_concurrent_operations, 5);
        assert_eq!(config.cache_size, 100 * 1024 * 1024);
        assert!(!config.watch_folders);
        assert!(config.confirm_before_delete);
        assert!(!config.confirm_before_move);
        assert!(config.show_notifications);
        assert_eq!(config.theme, "auto");
        assert_eq!(config.language, "en");
        assert_eq!(config.log_level, "info");
        assert!(!config.debug_mode);
    }

    #[test]
    fn test_config_validation_success() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_ai_provider() {
        let mut config = Config::default();
        config.ai_provider = String::new();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AI provider cannot be empty"));
    }

    #[test]
    fn test_config_validation_zero_concurrent_analysis() {
        let mut config = Config::default();
        config.max_concurrent_analysis = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max concurrent analysis must be at least 1"));
    }

    #[test]
    fn test_config_validation_zero_concurrent_operations() {
        let mut config = Config::default();
        config.max_concurrent_operations = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max concurrent operations must be at least 1"));
    }

    #[test]
    fn test_config_validation_zero_file_size() {
        let mut config = Config::default();
        config.max_file_size = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max file size must be greater than 0"));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = config.export();
        
        assert!(!json.is_empty());
        assert!(json.contains("ai_provider"));
        assert!(json.contains("ollama"));
    }

    #[test]
    fn test_config_import_success() {
        let config = Config::default();
        let json = config.export();
        
        let imported_config = Config::import(&json);
        assert!(imported_config.is_ok());
        
        let imported = imported_config.unwrap();
        assert_eq!(imported.ai_provider, config.ai_provider);
        assert_eq!(imported.ollama_host, config.ollama_host);
    }

    #[test]
    fn test_config_import_invalid_json() {
        let result = Config::import("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_import_validation_failure() {
        let invalid_config = r#"{"ai_provider":"","ollama_host":"localhost","ollama_model":"llama3.2:3b","max_concurrent_analysis":3}"#;
        let result = Config::import(invalid_config);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_reset() {
        let mut config = Config::default();
        config.ai_provider = "custom".to_string();
        config.max_concurrent_analysis = 10;
        
        config.reset();
        
        let default_config = Config::default();
        assert_eq!(config.ai_provider, default_config.ai_provider);
        assert_eq!(config.max_concurrent_analysis, default_config.max_concurrent_analysis);
    }

    #[test]
    fn test_config_is_development() {
        let mut config = Config::default();
        
        // Test debug_mode flag
        config.debug_mode = true;
        assert!(config.is_development());
        
        config.debug_mode = false;
        // This will depend on whether tests are run in debug mode
        let expected = cfg!(debug_assertions);
        assert_eq!(config.is_development(), expected);
    }

    #[test]
    fn test_config_get_log_filter() {
        let mut config = Config::default();
        
        config.log_level = "error".to_string();
        assert_eq!(config.get_log_filter(), "stratosort=error,tauri=error");
        
        config.log_level = "warn".to_string();
        assert_eq!(config.get_log_filter(), "stratosort=warn,tauri=warn");
        
        config.log_level = "info".to_string();
        assert_eq!(config.get_log_filter(), "stratosort=info,tauri=info");
        
        config.log_level = "debug".to_string();
        assert_eq!(config.get_log_filter(), "stratosort=debug,tauri=debug");
        
        config.log_level = "invalid".to_string();
        assert_eq!(config.get_log_filter(), "stratosort=info,tauri=info");
    }

    #[test]
    fn test_config_file_extensions() {
        let config = Config::default();
        
        assert!(config.file_extensions_to_ignore.contains(&".tmp".to_string()));
        assert!(config.file_extensions_to_ignore.contains(&".cache".to_string()));
        assert!(config.file_extensions_to_ignore.contains(&".temp".to_string()));
        assert!(config.file_extensions_to_ignore.contains(&".part".to_string()));
    }

    #[test]
    fn test_config_privacy_defaults() {
        let config = Config::default();
        
        assert!(!config.enable_telemetry);
        assert!(!config.enable_crash_reports);
        assert!(!config.enable_analytics);
    }

    #[test]
    fn test_config_performance_defaults() {
        let config = Config::default();
        
        assert_eq!(config.max_file_size, 100 * 1024 * 1024);
        assert!(!config.enable_gpu);
        assert_eq!(config.history_retention, 30);
        assert_eq!(config.undo_history_size, 50);
    }

    #[test]
    fn test_config_ui_defaults() {
        let config = Config::default();
        
        assert_eq!(config.notification_duration, 3000);
        assert!(config.auto_analyze_on_add);
        assert!(config.preserve_file_timestamps);
    }

    #[test]
    fn test_config_watch_folders_default() {
        let config = Config::default();
        
        assert!(!config.watch_folders);
        assert!(config.watch_paths.is_empty());
    }

    #[test]
    fn test_config_models_default() {
        let config = Config::default();
        
        assert_eq!(config.ollama_vision_model, "llava:7b");
        assert_eq!(config.ollama_embedding_model, "nomic-embed-text");
    }
}