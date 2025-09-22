#!/usr/bin/env node

/**
 * StratoSort Ollama Checker
 * 
 * This script checks if Ollama is running and has the required models.
 * Run before starting development or production builds.
 */

import { spawn, exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

const REQUIRED_MODELS = [
  'llama3.2:3b',
  'llava:7b', 
  'nomic-embed-text'
];

const OLLAMA_HOST = 'http://localhost:11434';

// Colors for console output
const colors = {
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  reset: '\x1b[0m',
  bold: '\x1b[1m'
};

function log(color, message) {
  console.log(`${color}${message}${colors.reset}`);
}

async function checkOllamaRunning() {
  try {
    // Validate URL format to prevent SSRF
    const url = new URL('/api/tags', OLLAMA_HOST);
    if (!['http:', 'https:'].includes(url.protocol)) {
      throw new Error('Invalid protocol in OLLAMA_HOST');
    }
    if (!['localhost', '127.0.0.1', '::1'].includes(url.hostname)) {
      throw new Error('OLLAMA_HOST must be localhost for security');
    }

    const response = await fetch(url.toString());
    if (response.ok) {
      const data = await response.json();
      return data.models || [];
    }
    return false;
  } catch (error) {
    return false;
  }
}

async function checkOllamaInstalled() {
  try {
    await execAsync('ollama --version');
    return true;
  } catch (error) {
    return false;
  }
}

async function startOllama() {
  return new Promise((resolve, reject) => {
    log(colors.blue, '🚀 Starting Ollama service...');

    const ollama = spawn('ollama', ['serve'], {
      detached: true,
      stdio: 'pipe'
    });

    // Store PID for cleanup
    const ollamaPid = ollama.pid;

    // Add cleanup handler
    const cleanup = () => {
      if (ollamaPid) {
        try {
          process.kill(ollamaPid, 'SIGTERM');
        } catch (e) {
          // Process may have already exited
        }
      }
    };

    process.on('exit', cleanup);
    process.on('SIGINT', cleanup);
    process.on('SIGTERM', cleanup);

    // Give Ollama time to start
    setTimeout(async () => {
      const isRunning = await checkOllamaRunning();
      if (isRunning !== false) {
        log(colors.green, '✅ Ollama service started successfully');
        ollama.unref();
        resolve(true);
      } else {
        log(colors.red, '❌ Failed to start Ollama service');
        cleanup();
        reject(new Error('Failed to start Ollama'));
      }
    }, 3000);

    ollama.on('error', (error) => {
      log(colors.red, `❌ Error starting Ollama: ${error.message}`);
      cleanup();
      reject(error);
    });
  });
}

async function pullModel(modelName) {
  return new Promise((resolve, reject) => {
    log(colors.blue, `📥 Pulling model: ${modelName}`);
    
    const pull = spawn('ollama', ['pull', modelName], {
      stdio: 'inherit'
    });

    pull.on('close', (code) => {
      if (code === 0) {
        log(colors.green, `✅ Successfully pulled ${modelName}`);
        resolve(true);
      } else {
        log(colors.red, `❌ Failed to pull ${modelName}`);
        reject(new Error(`Failed to pull ${modelName}`));
      }
    });

    pull.on('error', (error) => {
      log(colors.red, `❌ Error pulling ${modelName}: ${error.message}`);
      reject(error);
    });
  });
}

async function main() {
  log(colors.bold, '🔍 StratoSort Ollama Health Check');
  console.log('');

  // Check if Ollama is installed
  const isInstalled = await checkOllamaInstalled();
  if (!isInstalled) {
    log(colors.red, '❌ Ollama is not installed');
    log(colors.yellow, '📝 Please install Ollama from: https://ollama.ai');
    log(colors.yellow, '   - Windows/Mac: Download installer');
    log(colors.yellow, '   - Linux: curl -fsSL https://ollama.ai/install.sh | sh');
    process.exit(1);
  }
  
  log(colors.green, '✅ Ollama is installed');

  // Check if Ollama is running
  let availableModels = await checkOllamaRunning();
  if (availableModels === false) {
    log(colors.yellow, '⚠️  Ollama service is not running');
    
    try {
      await startOllama();
      availableModels = await checkOllamaRunning();
      if (availableModels === false) {
        throw new Error('Failed to verify Ollama after starting');
      }
    } catch (error) {
      log(colors.red, '❌ Could not start Ollama automatically');
      log(colors.yellow, '📝 Please start Ollama manually:');
      log(colors.yellow, '   Run: ollama serve');
      process.exit(1);
    }
  } else {
    log(colors.green, '✅ Ollama service is running');
  }

  // Check for required models
  const modelNames = availableModels.map(model => model.name);
  const missingModels = REQUIRED_MODELS.filter(required => {
    return !modelNames.some(available => available.startsWith(required));
  });

  if (missingModels.length > 0) {
    log(colors.yellow, `⚠️  Missing required models: ${missingModels.join(', ')}`);
    log(colors.blue, '📥 Pulling missing models...');
    
    for (const model of missingModels) {
      try {
        await pullModel(model);
      } catch (error) {
        log(colors.red, `❌ Failed to pull ${model}: ${error.message}`);
        log(colors.yellow, `📝 You can pull it manually with: ollama pull ${model}`);
      }
    }
  } else {
    log(colors.green, '✅ All required models are available');
  }

  // Final status check
  const finalModels = await checkOllamaRunning();
  if (finalModels !== false) {
    log(colors.green, `✅ Ollama is ready with ${finalModels.length} models`);
    log(colors.green, '🎉 StratoSort can now use AI features!');
    console.log('');
    log(colors.blue, 'Available models:');
    finalModels.forEach(model => {
      const isRequired = REQUIRED_MODELS.some(req => model.name.startsWith(req));
      const marker = isRequired ? '🎯' : '📦';
      console.log(`  ${marker} ${model.name}`);
    });
  } else {
    log(colors.red, '❌ Ollama check failed');
    process.exit(1);
  }
}

main().catch(error => {
  log(colors.red, `💥 Unexpected error: ${error.message}`);
  process.exit(1);
});
