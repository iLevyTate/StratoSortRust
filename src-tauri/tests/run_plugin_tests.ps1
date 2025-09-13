# Comprehensive test runner for Tauri v2 plugin tests (Windows)
# This script runs all plugin tests with proper configuration

$ErrorActionPreference = "Stop"

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Running StratoSort Tauri Plugin Tests" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# Set test environment variables
$env:RUST_BACKTRACE = "1"
$env:RUST_LOG = "debug"
$env:TEST_MODE = "true"

# Function to run tests for a specific plugin
function Run-PluginTest {
    param($PluginName)
    
    Write-Host "Testing $PluginName..." -ForegroundColor Yellow
    
    $result = cargo test --test test_tauri_plugins "plugins::$PluginName" -- --nocapture 2>&1
    $exitCode = $LASTEXITCODE
    
    if ($exitCode -eq 0) {
        Write-Host "✓ $PluginName tests passed" -ForegroundColor Green
        return $true
    } else {
        Write-Host "✗ $PluginName tests failed" -ForegroundColor Red
        Write-Host $result
        return $false
    }
}

# Track test results
$FailedTests = @()
$PassedTests = @()

# Run individual plugin tests
Write-Host ""
Write-Host "Running individual plugin tests..." -ForegroundColor Cyan
Write-Host "---------------------------------" -ForegroundColor Cyan

$plugins = @(
    "test_process",
    "test_os",
    "test_updater",
    "test_window_state",
    "test_positioner",
    "test_localhost",
    "test_single_instance",
    "test_http"
)

foreach ($plugin in $plugins) {
    if (Run-PluginTest -PluginName $plugin) {
        $PassedTests += $plugin
    } else {
        $FailedTests += $plugin
    }
    Write-Host ""
}

# Run integration tests
Write-Host "Running plugin integration tests..." -ForegroundColor Cyan
Write-Host "-----------------------------------" -ForegroundColor Cyan

$result = cargo test --test test_tauri_plugins "plugins::plugin_integration" -- --nocapture 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ Integration tests passed" -ForegroundColor Green
    $PassedTests += "integration"
} else {
    Write-Host "✗ Integration tests failed" -ForegroundColor Red
    Write-Host $result
    $FailedTests += "integration"
}

Write-Host ""
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Test Results Summary" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# Display results
if ($PassedTests.Count -gt 0) {
    Write-Host "Passed ($($PassedTests.Count)):" -ForegroundColor Green
    foreach ($test in $PassedTests) {
        Write-Host "  ✓ $test" -ForegroundColor Green
    }
}

if ($FailedTests.Count -gt 0) {
    Write-Host "Failed ($($FailedTests.Count)):" -ForegroundColor Red
    foreach ($test in $FailedTests) {
        Write-Host "  ✗ $test" -ForegroundColor Red
    }
}

# Calculate pass rate
$TotalTests = $PassedTests.Count + $FailedTests.Count
$PassRate = [math]::Round(($PassedTests.Count * 100 / $TotalTests), 0)

Write-Host ""
Write-Host "Pass rate: $PassRate% ($($PassedTests.Count)/$TotalTests)"

# Exit with appropriate code
if ($FailedTests.Count -eq 0) {
    Write-Host "All plugin tests passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "Some tests failed. Please review the output above." -ForegroundColor Red
    exit 1
}