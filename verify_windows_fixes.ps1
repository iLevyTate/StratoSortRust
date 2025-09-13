# Windows CI Fix Verification Script
Write-Host "=== Verifying Windows CI Fixes ===" -ForegroundColor Cyan

# Test 1: Verify PowerShell execution
Write-Host "`nTest 1: PowerShell Date Command" -ForegroundColor Yellow
try {
    $date = powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Get-Date -Format 'yyyy-MM-dd HH:mm:ss'"
    Write-Host "SUCCESS: PowerShell date command works: $date" -ForegroundColor Green
} catch {
    Write-Host "ERROR: PowerShell command failed: $_" -ForegroundColor Red
    exit 1
}

# Test 2: Check SYSTEMROOT environment variable
Write-Host "`nTest 2: SYSTEMROOT Environment Variable" -ForegroundColor Yellow
if ($env:SYSTEMROOT) {
    Write-Host "SUCCESS: SYSTEMROOT is set to: $env:SYSTEMROOT" -ForegroundColor Green
} else {
    Write-Host "WARNING: SYSTEMROOT not set, will use default C:\Windows" -ForegroundColor Yellow
}

# Test 3: Build the project
Write-Host "`nTest 3: Building Project" -ForegroundColor Yellow
Set-Location src-tauri
$buildResult = cargo build --release 2>&1 | Select-String -Pattern "error"
if ($buildResult) {
    Write-Host "ERROR: Build failed with errors:" -ForegroundColor Red
    Write-Host $buildResult
    exit 1
} else {
    Write-Host "SUCCESS: Build completed without errors" -ForegroundColor Green
}

# Test 4: Run tests
Write-Host "`nTest 4: Running Tests" -ForegroundColor Yellow
$testResult = cargo test --lib 2>&1 | Select-String -Pattern "test result: ok"
if ($testResult) {
    Write-Host "SUCCESS: All tests passed" -ForegroundColor Green
    Write-Host $testResult
} else {
    Write-Host "ERROR: Tests failed" -ForegroundColor Red
    exit 1
}

Write-Host "`n=== All Windows CI fixes verified successfully! ===" -ForegroundColor Green