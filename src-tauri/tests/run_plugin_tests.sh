#!/bin/bash
# Comprehensive test runner for Tauri v2 plugin tests
# This script runs all plugin tests with proper configuration

set -e

echo "========================================="
echo "Running StratoSort Tauri Plugin Tests"
echo "========================================="

# Set test environment variables
export RUST_BACKTRACE=1
export RUST_LOG=debug
export TEST_MODE=true

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to run tests for a specific plugin
run_plugin_test() {
    local plugin_name=$1
    echo -e "${YELLOW}Testing $plugin_name...${NC}"
    
    if cargo test --test test_tauri_plugins plugins::$plugin_name -- --nocapture; then
        echo -e "${GREEN}✓ $plugin_name tests passed${NC}"
        return 0
    else
        echo -e "${RED}✗ $plugin_name tests failed${NC}"
        return 1
    fi
}

# Track test results
FAILED_TESTS=()
PASSED_TESTS=()

# Run individual plugin tests
echo ""
echo "Running individual plugin tests..."
echo "---------------------------------"

plugins=(
    "test_process"
    "test_os"
    "test_updater"
    "test_window_state"
    "test_positioner"
    "test_localhost"
    "test_single_instance"
    "test_http"
)

for plugin in "${plugins[@]}"; do
    if run_plugin_test "$plugin"; then
        PASSED_TESTS+=("$plugin")
    else
        FAILED_TESTS+=("$plugin")
    fi
    echo ""
done

# Run integration tests
echo "Running plugin integration tests..."
echo "-----------------------------------"
if cargo test --test test_tauri_plugins plugins::plugin_integration -- --nocapture; then
    echo -e "${GREEN}✓ Integration tests passed${NC}"
    PASSED_TESTS+=("integration")
else
    echo -e "${RED}✗ Integration tests failed${NC}"
    FAILED_TESTS+=("integration")
fi

echo ""
echo "========================================="
echo "Test Results Summary"
echo "========================================="

# Display results
if [ ${#PASSED_TESTS[@]} -gt 0 ]; then
    echo -e "${GREEN}Passed (${#PASSED_TESTS[@]}):${NC}"
    for test in "${PASSED_TESTS[@]}"; do
        echo -e "  ${GREEN}✓${NC} $test"
    done
fi

if [ ${#FAILED_TESTS[@]} -gt 0 ]; then
    echo -e "${RED}Failed (${#FAILED_TESTS[@]}):${NC}"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "  ${RED}✗${NC} $test"
    done
fi

# Calculate pass rate
TOTAL_TESTS=$((${#PASSED_TESTS[@]} + ${#FAILED_TESTS[@]}))
PASS_RATE=$((${#PASSED_TESTS[@]} * 100 / TOTAL_TESTS))

echo ""
echo "Pass rate: $PASS_RATE% (${#PASSED_TESTS[@]}/$TOTAL_TESTS)"

# Exit with appropriate code
if [ ${#FAILED_TESTS[@]} -eq 0 ]; then
    echo -e "${GREEN}All plugin tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed. Please review the output above.${NC}"
    exit 1
fi