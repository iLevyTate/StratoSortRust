# Tauri v2 Plugin Test Suite

This directory contains comprehensive test suites for all Tauri v2 plugins integrated into StratoSort.

## Plugin Tests Overview

### Core Plugin Tests

1. **test_process.rs** - Process management plugin tests
   - Process monitoring during file operations
   - Subprocess spawning for AI analysis
   - Process cleanup and resource limits
   - IPC communication between processes

2. **test_os.rs** - OS information plugin tests
   - OS-specific path handling
   - Memory availability checks
   - File permission handling
   - Architecture-based AI model selection

3. **test_updater.rs** - Auto-updater plugin tests
   - Update checking and downloading
   - Signature verification
   - User dialog interactions
   - Rollback mechanisms

4. **test_window_state.rs** - Window state persistence tests
   - State saving and restoration
   - Multi-monitor support
   - Window bounds validation
   - Different view states

5. **test_positioner.rs** - Window positioning tests
   - Centering and docking
   - Context menu positioning
   - Notification placement
   - Smart positioning to avoid overlaps

6. **test_localhost.rs** - Local server plugin tests (port 3030)
   - AI service integration endpoints
   - WebSocket support
   - CORS configuration
   - Static file serving

7. **test_single_instance.rs** - Single instance plugin tests
   - Preventing multiple instances
   - IPC between instances
   - Deep link handling
   - Command-line argument passing

8. **test_http.rs** - Enhanced HTTP client tests
   - AI service requests
   - Multipart file uploads
   - Retry logic and timeouts
   - Connection pooling

### Integration Tests

**plugin_integration.rs** - Tests plugin interactions
- Full plugin stack initialization
- Complete file operation workflows
- AI analysis pipeline
- Error recovery scenarios
- Performance monitoring

## Running Tests

### All Plugin Tests
```bash
cargo test --test test_tauri_plugins
```

### Individual Plugin Tests
```bash
cargo test --test test_tauri_plugins plugins::test_process
cargo test --test test_tauri_plugins plugins::test_os
# etc...
```

### Integration Tests Only
```bash
cargo test --test test_tauri_plugins plugins::plugin_integration
```

### With Output
```bash
cargo test --test test_tauri_plugins -- --nocapture
```

## Test Infrastructure

### Fixtures (plugin_fixtures.rs)
- `MockAppHandle` - Simulates Tauri app handle
- `MockWindowState` - Window state management
- `MockProcessInfo` - Process information
- `MockOsInfo` - OS details
- `MockHttpResponse` - HTTP responses
- `MockUpdateInfo` - Update information
- `MockLocalhostServer` - Local server simulation

### Assertions
- `PluginAssertions` - Common assertion helpers for all plugins

## CI Integration

### Windows
```powershell
.\tests\run_plugin_tests.ps1
```

### Linux/macOS
```bash
./tests/run_plugin_tests.sh
```

## Test Coverage Areas

1. **File Operations**
   - Organization with AI analysis
   - Batch processing
   - Real-time monitoring
   - Error handling

2. **AI Integration**
   - Ollama service communication
   - Model selection based on system
   - Fallback mechanisms
   - Resource management

3. **System Integration**
   - Cross-platform compatibility
   - Resource monitoring
   - Permission handling
   - Security features

4. **User Experience**
   - Window management
   - Update notifications
   - Progress tracking
   - Error recovery

## Adding New Tests

1. Create test file in `tests/plugins/`
2. Add module declaration in `mod.rs`
3. Follow existing test patterns
4. Use provided fixtures and assertions
5. Update this README

## Known Limitations

- Some tests require mocking due to system dependencies
- WebSocket tests are simulated
- Update download tests use mock data
- Process monitoring is platform-specific

## Troubleshooting

If tests fail:
1. Check Rust version compatibility
2. Ensure all dev dependencies are installed
3. Verify no port conflicts (especially 3030)
4. Check file system permissions
5. Review test output with `--nocapture`