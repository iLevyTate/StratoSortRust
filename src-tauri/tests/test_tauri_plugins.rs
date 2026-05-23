// Comprehensive test suite for Tauri v2 plugins
// This file orchestrates all plugin tests for StratoSort

mod common;
mod plugins;

#[cfg(test)]
mod tauri_plugin_tests {
    use super::*;

    // Run all plugin tests
    #[test]
    fn test_all_plugins_load() {
        // This test ensures all plugin test modules compile and load correctly
        // Individual tests are in the respective plugin test files
        // All plugin test modules loaded successfully
    }

    #[tokio::test]
    async fn test_plugin_integration_smoke_test() {
        // Quick smoke test to verify plugin test infrastructure
        let mock_app = plugins::MockAppHandle::new();
        assert!(
            mock_app.data_dir.exists(),
            "Mock app handle should create directories"
        );
    }
}
