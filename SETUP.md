# StratoSort Development Setup Guide

This guide ensures you can run StratoSort in development mode without errors.

## Prerequisites

### 1. System Requirements
- **Node.js**: v18+ (v22 recommended)
- **Rust**: 1.75+ (install from https://rustup.rs/)
- **Ollama**: Latest version (https://ollama.ai/)
- **OS**: Windows 10+, macOS 10.15+, or Linux (Ubuntu 20.04+)
- **RAM**: 4GB minimum (8GB recommended for smooth development)

### 2. Ollama Models Setup

StratoSort requires THREE Ollama models to function properly:

```bash
# Install main language model
ollama pull llama3.2:3b

# Install vision model for image analysis
ollama pull llava:latest

# Install embedding model for semantic search
ollama pull nomic-embed-text
```

**Important**: The README only mentions the first model, but all three are required!

## Installation Steps

### 1. Clone the Repository
```bash
git clone https://github.com/iLevyTate/StratoSortRust.git
cd StratoSortRust
```

### 2. Environment Setup
```bash
# Copy the example environment file
cp .env.example .env

# The .env file should already contain correct defaults:
# OLLAMA_HOST=http://localhost:11434
# OLLAMA_MODEL=llama3.2:3b
# OLLAMA_VISION_MODEL=llava:latest
# OLLAMA_EMBEDDING_MODEL=nomic-embed-text
```

### 3. Install Frontend Dependencies
```bash
# Install npm dependencies (this may take a few minutes)
npm install

# If you see EPERM errors on Windows, run as Administrator or:
npm cache clean --force
npm install
```

### 4. Verify Ollama is Running
```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Should return a JSON list with your installed models
```

### 5. Build and Run

#### Development Mode (Recommended for first-time setup)
```bash
# Run the full Tauri desktop application
npm run tauri:dev

# Note: First run will compile Rust dependencies (5-10 minutes)
```

#### Alternative: Frontend Only
```bash
# If you want to work on frontend only
npm run dev

# This starts Vite dev server on http://localhost:1431
```

## Common Issues and Solutions

### Issue 1: Port 1431 Already in Use
```bash
# Windows: Find and kill the process
netstat -ano | findstr ":1431"
# Then kill the PID: taskkill /PID [PID_NUMBER] /F

# Or change the port in tauri.conf.json
```

### Issue 2: svelte-check Not Found
```bash
# Reinstall dependencies
rm -rf node_modules package-lock.json
npm install
```

### Issue 3: Ollama Connection Failed
```bash
# Ensure Ollama is running
ollama serve

# Verify it's accessible
curl http://localhost:11434/api/tags
```

### Issue 4: Build Hangs
- The production build may hang due to bundle size
- Use `npm run tauri:dev` for development instead
- For production builds, ensure you have enough RAM available

### Issue 5: Test Suite Crashes
- Frontend tests may crash with "Worker exited unexpectedly"
- This is a known issue being investigated
- Run individual test files if needed: `npm test -- src/tests/specific.test.ts`

### Issue 6: Missing Rust Dependencies
```bash
# Windows: Install Visual Studio Build Tools
# Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/

# Linux: Install build essentials
sudo apt-get update
sudo apt-get install build-essential libssl-dev pkg-config

# macOS: Install Xcode Command Line Tools
xcode-select --install
```

## Verification Checklist

Before creating a PR, ensure:

- [ ] All three Ollama models are installed
- [ ] `.env` file exists (copy from `.env.example`)
- [ ] `npm install` completes without errors
- [ ] `npm run tauri:dev` starts without errors
- [ ] Ollama API responds at http://localhost:11434
- [ ] Frontend loads at http://localhost:1431
- [ ] Basic file operations work in the app

## Development Commands

```bash
# Frontend
npm run dev           # Start Vite dev server
npm run build         # Build frontend
npm run check         # Type check (currently broken)
npm test             # Run tests (may crash - known issue)

# Tauri/Desktop
npm run tauri:dev    # Start desktop app in dev mode
npm run tauri:build  # Build desktop app for production

# Rust Backend (run from src-tauri/)
cargo check          # Check compilation
cargo test           # Run backend tests
cargo clippy         # Linting
```

## Known Issues

1. **A11y Warnings**: KeyboardShortcutsHelp component has accessibility warnings (non-critical)
2. **Build Performance**: Production builds may timeout - being optimized
3. **Test Runner**: Vitest worker crashes in some test suites
4. **TypeScript**: `svelte-check` command not working properly

## Support

- Report issues: https://github.com/iLevyTate/StratoSortRust/issues
- Main README: [README.md](README.md)

---

Last updated: 2025-09-17