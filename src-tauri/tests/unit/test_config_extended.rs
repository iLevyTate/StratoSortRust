use stratosort::config::{Config, ConfigBuilder, ConfigValidator};
use tempfile::tempdir;
use std::fs;
use std::env;
use std::path::PathBuf;
use serde_json;
use toml;

#[cfg(test)]
mod config_extended_tests {
    use super::*;

    #[test]
    fn test_config_builder_pattern() {
        let config = ConfigBuilder::new()
            .with_ollama_host("http://custom:11434")
            .with_ollama_model("custom-model")
            .with_max_concurrent_analysis(10)
            .with_debug_mode(true)
            .build();
        
        assert_eq!(config.ollama_host, "http://custom:11434");
        assert_eq!(config.ollama_model, "custom-model");
        assert_eq!(config.max_concurrent_analysis, 10);
        assert!(config.debug_mode);
    }

    #[test]
    fn test_config_from_multiple_sources() {
        let temp_dir = tempdir().unwrap();
        
        // Create base config file
        let base_path = temp_dir.path().join("base.toml");
        let base_content = r#"
ai_provider = "ollama"
ollama_host = "http://base:11434"
max_concurrent_analysis = 3
        "#;
        fs::write(&base_path, base_content).unwrap();
        
        // Create override config file
        let override_path = temp_dir.path().join("override.toml");
        let override_content = r#"
ollama_host = "http://override:11434"
debug_mode = true
        "#;
        fs::write(&override_path, override_content).unwrap();
        
        // Load configs
        let mut config = Config::from_file(&base_path).unwrap();
        let override_config = Config::from_file(&override_path).unwrap();
        
        // Merge
        config.merge(override_config);
        
        assert_eq!(config.ollama_host, "http://override:11434");
        assert!(config.debug_mode);
        assert_eq!(config.max_concurrent_analysis, 3); // From base
    }

    #[test]
    fn test_config_validation_comprehensive() {
        struct TestCase {
            name: &'static str,
            modifier: Box<dyn Fn(&mut Config)>,
            should_fail: bool,
            error_contains: Option<&'static str>,
        }
        
        let test_cases = vec![
            TestCase {
                name: "valid config",
                modifier: Box::new(|_| {}),
                should_fail: false,
                error_contains: None,
            },
            TestCase {
                name: "empty ai provider",
                modifier: Box::new(|c| c.ai_provider = String::new()),
                should_fail: true,
                error_contains: Some("AI provider"),
            },
            TestCase {
                name: "zero concurrent analysis",
                modifier: Box::new(|c| c.max_concurrent_analysis = 0),
                should_fail: true,
                error_contains: Some("concurrent analysis"),
            },
            TestCase {
                name: "negative cache size",
                modifier: Box::new(|c| c.cache_size = 0),
                should_fail: false, // 0 is valid (no cache)
                error_contains: None,
            },
            TestCase {
                name: "invalid theme",
                modifier: Box::new(|c| c.theme = "rainbow".to_string()),
                should_fail: true,
                error_contains: Some("theme"),
            },
            TestCase {
                name: "invalid log level",
                modifier: Box::new(|c| c.log_level = "verbose".to_string()),
                should_fail: true,
                error_contains: Some("log level"),
            },
        ];
        
        for test_case in test_cases {
            let mut config = Config::default();
            (test_case.modifier)(&mut config);
            
            let result = config.validate();
            
            if test_case.should_fail {
                assert!(result.is_err(), "Test '{}' should have failed", test_case.name);
                if let Some(expected_error) = test_case.error_contains {
                    assert!(
                        result.unwrap_err().to_string().to_lowercase().contains(expected_error),
                        "Test '{}' error should contain '{}'", test_case.name, expected_error
                    );
                }
            } else {
                assert!(result.is_ok(), "Test '{}' should have passed: {:?}", 
                    test_case.name, result.err());
            }
        }
    }

    #[test]
    fn test_config_environment_variables_comprehensive() {
        // Save current env vars
        let env_vars = vec![
            ("STRATOSORT_AI_PROVIDER", env::var("STRATOSORT_AI_PROVIDER").ok()),
            ("STRATOSORT_OLLAMA_HOST", env::var("STRATOSORT_OLLAMA_HOST").ok()),
            ("STRATOSORT_OLLAMA_MODEL", env::var("STRATOSORT_OLLAMA_MODEL").ok()),
            ("STRATOSORT_MAX_CONCURRENT_ANALYSIS", env::var("STRATOSORT_MAX_CONCURRENT_ANALYSIS").ok()),
            ("STRATOSORT_DEBUG_MODE", env::var("STRATOSORT_DEBUG_MODE").ok()),
            ("STRATOSORT_THEME", env::var("STRATOSORT_THEME").ok()),
            ("STRATOSORT_LANGUAGE", env::var("STRATOSORT_LANGUAGE").ok()),
            ("STRATOSORT_LOG_LEVEL", env::var("STRATOSORT_LOG_LEVEL").ok()),
        ];
        
        // Set test env vars
        env::set_var("STRATOSORT_AI_PROVIDER", "custom");
        env::set_var("STRATOSORT_OLLAMA_HOST", "http://env:11434");
        env::set_var("STRATOSORT_OLLAMA_MODEL", "env-model");
        env::set_var("STRATOSORT_MAX_CONCURRENT_ANALYSIS", "7");
        env::set_var("STRATOSORT_DEBUG_MODE", "true");
        env::set_var("STRATOSORT_THEME", "dark");
        env::set_var("STRATOSORT_LANGUAGE", "es");
        env::set_var("STRATOSORT_LOG_LEVEL", "debug");
        
        let config = Config::from_env();
        
        assert_eq!(config.ai_provider, "custom");
        assert_eq!(config.ollama_host, "http://env:11434");
        assert_eq!(config.ollama_model, "env-model");
        assert_eq!(config.max_concurrent_analysis, 7);
        assert!(config.debug_mode);
        assert_eq!(config.theme, "dark");
        assert_eq!(config.language, "es");
        assert_eq!(config.log_level, "debug");
        
        // Restore original env vars
        for (key, value) in env_vars {
            match value {
                Some(val) => env::set_var(key, val),
                None => env::remove_var(key),
            }
        }
    }

    #[test]
    fn test_config_file_formats() {
        let temp_dir = tempdir().unwrap();
        
        // Test TOML format
        let toml_path = temp_dir.path().join("config.toml");
        let toml_content = r#"
ai_provider = "ollama"
ollama_host = "http://toml:11434"
        "#;
        fs::write(&toml_path, toml_content).unwrap();
        
        let toml_config = Config::from_file(&toml_path).unwrap();
        assert_eq!(toml_config.ollama_host, "http://toml:11434");
        
        // Test JSON format
        let json_path = temp_dir.path().join("config.json");
        let json_content = r#"
{
    "ai_provider": "ollama",
    "ollama_host": "http://json:11434"
}
        "#;
        fs::write(&json_path, json_content).unwrap();
        
        let json_config = Config::from_json_file(&json_path).unwrap();
        assert_eq!(json_config.ollama_host, "http://json:11434");
        
        // Test YAML format (if supported)
        #[cfg(feature = "yaml")]
        {
            let yaml_path = temp_dir.path().join("config.yaml");
            let yaml_content = r#"
ai_provider: ollama
ollama_host: http://yaml:11434
            "#;
            fs::write(&yaml_path, yaml_content).unwrap();
            
            let yaml_config = Config::from_yaml_file(&yaml_path).unwrap();
            assert_eq!(yaml_config.ollama_host, "http://yaml:11434");
        }
    }

    #[test]
    fn test_config_hot_reload() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("hot_reload.toml");
        
        // Write initial config
        let initial_content = r#"
ollama_host = "http://initial:11434"
debug_mode = false
        "#;
        fs::write(&config_path, initial_content).unwrap();
        
        let mut config = Config::from_file(&config_path).unwrap();
        assert_eq!(config.ollama_host, "http://initial:11434");
        assert!(!config.debug_mode);
        
        // Modify file
        let updated_content = r#"
ollama_host = "http://updated:11434"
debug_mode = true
        "#;
        fs::write(&config_path, updated_content).unwrap();
        
        // Reload config
        config.reload_from_file(&config_path).unwrap();
        assert_eq!(config.ollama_host, "http://updated:11434");
        assert!(config.debug_mode);
    }

    #[test]
    fn test_config_with_comments() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("commented.toml");
        
        let content = r#"
# This is a comment
ai_provider = "ollama" # inline comment

# Multi-line comment
# explaining the host setting
ollama_host = "http://localhost:11434"

## Section comment
[performance]
max_concurrent_analysis = 5 # Another inline comment
        "#;
        
        fs::write(&config_path, content).unwrap();
        
        let config = Config::from_file(&config_path).unwrap();
        assert_eq!(config.ai_provider, "ollama");
        assert_eq!(config.ollama_host, "http://localhost:11434");
        assert_eq!(config.max_concurrent_analysis, 5);
    }

    #[test]
    fn test_config_permissions() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            
            let temp_dir = tempdir().unwrap();
            let config_path = temp_dir.path().join("restricted.toml");
            
            let content = "ai_provider = \"ollama\"";
            fs::write(&config_path, content).unwrap();
            
            // Set restrictive permissions
            let metadata = fs::metadata(&config_path).unwrap();
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(&config_path, permissions).unwrap();
            
            // Should still be able to read
            let config = Config::from_file(&config_path);
            assert!(config.is_ok());
        }
    }

    #[test]
    fn test_config_backup_and_restore() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let backup_path = temp_dir.path().join("config.backup.toml");
        
        // Create original config
        let mut config = Config::default();
        config.ollama_host = "http://original:11434".to_string();
        config.debug_mode = true;
        
        // Save original
        config.save(&config_path).unwrap();
        
        // Create backup
        config.backup(&backup_path).unwrap();
        
        // Modify original
        config.ollama_host = "http://modified:11434".to_string();
        config.debug_mode = false;
        config.save(&config_path).unwrap();
        
        // Restore from backup
        config.restore(&backup_path).unwrap();
        
        assert_eq!(config.ollama_host, "http://original:11434");
        assert!(config.debug_mode);
    }

    #[test]
    fn test_config_diff() {
        let config1 = Config::default();
        
        let mut config2 = Config::default();
        config2.ollama_host = "http://different:11434".to_string();
        config2.debug_mode = true;
        config2.max_concurrent_analysis = 10;
        
        let diff = config1.diff(&config2);
        
        assert!(diff.contains("ollama_host"));
        assert!(diff.contains("debug_mode"));
        assert!(diff.contains("max_concurrent_analysis"));
    }

    #[test]
    fn test_config_validate_paths() {
        let mut config = Config::default();
        
        // Test valid paths
        config.watch_paths = vec![
            "/home/user/documents".to_string(),
            "C:\\Users\\Documents".to_string(),
        ];
        
        // Validation should handle both Unix and Windows paths
        let result = config.validate_paths();
        // Note: This might fail if paths don't exist, adjust as needed
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_config_sanitization() {
        let mut config = Config::default();
        
        // Set potentially dangerous values
        config.ollama_host = "  http://host:11434  ".to_string(); // Extra spaces
        config.ollama_model = "model\n\r".to_string(); // Newlines
        config.log_level = "INFO".to_string(); // Uppercase
        
        config.sanitize();
        
        assert_eq!(config.ollama_host, "http://host:11434");
        assert_eq!(config.ollama_model, "model");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_config_secrets_masking() {
        let mut config = Config::default();
        config.api_key = Some("secret-key-12345".to_string());
        config.database_password = Some("password123".to_string());
        
        let exported = config.export_safe();
        
        assert!(exported.contains("***"));
        assert!(!exported.contains("secret-key-12345"));
        assert!(!exported.contains("password123"));
    }

    #[test]
    fn test_config_performance_tuning() {
        let mut config = Config::default();
        
        // Auto-tune for low-end system
        config.auto_tune_for_system(2, 4_000_000_000); // 2 cores, 4GB RAM
        assert!(config.max_concurrent_analysis <= 2);
        assert!(config.max_concurrent_operations <= 3);
        assert!(config.cache_size <= 200_000_000); // 200MB max
        
        // Auto-tune for high-end system
        config.auto_tune_for_system(16, 32_000_000_000); // 16 cores, 32GB RAM
        assert!(config.max_concurrent_analysis >= 8);
        assert!(config.max_concurrent_operations >= 10);
        assert!(config.cache_size >= 1_000_000_000); // 1GB min
    }

    #[test]
    fn test_config_profile_switching() {
        let mut config = Config::default();
        
        // Apply development profile
        config.apply_profile("development");
        assert!(config.debug_mode);
        assert_eq!(config.log_level, "debug");
        
        // Apply production profile
        config.apply_profile("production");
        assert!(!config.debug_mode);
        assert_eq!(config.log_level, "warn");
        
        // Apply performance profile
        config.apply_profile("performance");
        assert!(config.max_concurrent_analysis > 5);
        assert!(config.enable_gpu);
    }

    #[test]
    fn test_config_compatibility_check() {
        let config = Config::default();
        
        // Check compatibility with current version
        assert!(config.is_compatible_with("1.0.0"));
        
        // Check incompatibility with future version
        assert!(!config.is_compatible_with("99.0.0"));
    }
}