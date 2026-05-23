// Plugin test modules - comprehensive test suites for Tauri v2 plugins
// This module tests the integration of Tauri v2 plugins with StratoSort's
// file organization and AI features

pub mod plugin_fixtures;
pub mod plugin_integration;
pub mod test_http;
pub mod test_localhost;
pub mod test_os;
pub mod test_positioner;
pub mod test_process;
pub mod test_single_instance;
pub mod test_updater;
pub mod test_window_state;

// Re-export common test utilities
pub use plugin_fixtures::*;

#[cfg(test)]
mod plugin_test_setup {
    #[test]
    fn test_plugin_modules_compile() {
        // Smoke test to ensure all plugin test modules compile correctly
        // This test passes if all modules are properly structured
    }
}
