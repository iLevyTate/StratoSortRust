# StratoSort Development Scripts

This directory contains development and deployment helper scripts for StratoSort.

## Ollama Health Check

The `check-ollama.js` script ensures that Ollama is running and has the required AI models for StratoSort to function properly.

### Usage

```bash
# Check Ollama status manually
npm run check:ollama

# Development with Ollama check
npm run tauri:dev        # Checks Ollama, then starts dev mode
npm run tauri:dev:fast   # Skips Ollama check for faster startup

# Production build with Ollama check
npm run tauri:build      # Checks Ollama, then builds
npm run tauri:build:fast # Skips Ollama check for faster build
```

### Required Models

StratoSort requires these Ollama models to function:

- **llama3.2:3b** - Main text analysis model
- **llava:7b** - Vision/image analysis model  
- **nomic-embed-text** - Text embeddings for semantic search

### What the script does

1. ✅ Checks if Ollama is installed
2. ✅ Verifies Ollama service is running (starts it if needed)
3. ✅ Confirms required models are available
4. 📥 Automatically downloads missing models
5. 🎯 Shows final status with available models

### Manual Ollama Setup

If you prefer to set up Ollama manually:

```bash
# Install Ollama (if not already installed)
# Windows/Mac: Download from https://ollama.ai
# Linux: curl -fsSL https://ollama.ai/install.sh | sh

# Start Ollama service
ollama serve

# Pull required models
ollama pull llama3.2:3b
ollama pull llava:7b
ollama pull nomic-embed-text
```

### Troubleshooting

**Ollama not found**: Install Ollama from https://ollama.ai

**Models failing to download**: Check your internet connection and try pulling manually

**Permission errors**: Ensure Ollama has necessary permissions to download models

**Port conflicts**: Ollama runs on port 11434 by default - ensure it's available
