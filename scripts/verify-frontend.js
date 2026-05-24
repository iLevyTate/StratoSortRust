#!/usr/bin/env node

/**
 * StratoSortRust Frontend Verification Script
 * Automates key verification steps for frontend setup
 */

import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Colors for console output
const colors = {
  reset: '\x1b[0m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m'
};

// Test results tracking
const results = {
  passed: 0,
  failed: 0,
  tests: []
};

function log(message, color = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function runTest(testName, testFn) {
  try {
    log(`\n🧪 Running: ${testName}`, 'cyan');
    const result = testFn();
    if (result === true || result === undefined) {
      log(`✅ PASS: ${testName}`, 'green');
      results.passed++;
      results.tests.push({ name: testName, status: 'PASS' });
    } else {
      log(`❌ FAIL: ${testName} - ${result}`, 'red');
      results.failed++;
      results.tests.push({ name: testName, status: 'FAIL', error: result });
    }
  } catch (error) {
    log(`❌ ERROR: ${testName} - ${error.message}`, 'red');
    results.failed++;
    results.tests.push({ name: testName, status: 'ERROR', error: error.message });
  }
}

function execCommand(command, options = {}) {
  try {
    const result = execSync(command, {
      encoding: 'utf8',
      stdio: options.silent ? 'pipe' : 'inherit',
      ...options
    });
    return result;
  } catch (error) {
    throw new Error(`Command failed: ${command}\n${error.message}`);
  }
}

function fileExists(filePath) {
  return fs.existsSync(filePath);
}

function directoryExists(dirPath) {
  return fs.existsSync(dirPath) && fs.lstatSync(dirPath).isDirectory();
}

// =============== TEST SUITES ===============

function testProjectStructure() {
  runTest('Project structure exists', () => {
    const requiredFiles = [
      'package.json',
      'src/App.svelte',
      'src/lib/api/tauri.ts',
      'src/lib/stores/index.ts',
      'src/lib/types/backend.ts',
      'src-tauri/Cargo.toml',
      'src-tauri/src/lib.rs'
    ];

    const requiredDirs = [
      'src/lib/components',
      'src/lib/components/pages',
      'src/lib/components/ui',
      'src-tauri/src'
    ];

    for (const file of requiredFiles) {
      if (!fileExists(file)) {
        return `Missing required file: ${file}`;
      }
    }

    for (const dir of requiredDirs) {
      if (!directoryExists(dir)) {
        return `Missing required directory: ${dir}`;
      }
    }

    return true;
  });
}

function testDependencies() {
  runTest('Dependencies are installed', () => {
    if (!directoryExists('node_modules')) {
      return 'node_modules directory not found. Run npm install.';
    }

    const packageJson = JSON.parse(fs.readFileSync('package.json', 'utf8'));
    const requiredDeps = [
      '@tauri-apps/api',
      'svelte',
      'vite',
      'typescript',
      'lucide-svelte'
    ];

    for (const dep of requiredDeps) {
      if (!packageJson.dependencies?.[dep] && !packageJson.devDependencies?.[dep]) {
        return `Missing required dependency: ${dep}`;
      }
    }

    return true;
  });
}

function testTypeScript() {
  runTest('TypeScript compilation', () => {
    try {
      execCommand('npx tsc --noEmit', { silent: true });
      return true;
    } catch (error) {
      return 'TypeScript compilation failed';
    }
  });
}

function testBuild() {
  runTest('Production build', () => {
    try {
      execCommand('npm run build', { silent: true });

      if (!directoryExists('dist')) {
        return 'Build did not create dist directory';
      }

      const distFiles = fs.readdirSync('dist');
      if (distFiles.length === 0) {
        return 'Build created empty dist directory';
      }

      return true;
    } catch (error) {
      return 'Build process failed';
    }
  });
}

function testComponentStructure() {
  runTest('Required components exist', () => {
    const requiredComponents = [
      'src/lib/components/pages/DiscoverPage.svelte',
      'src/lib/components/pages/AnalyzePage.svelte',
      'src/lib/components/pages/OrganizePage.svelte',
      'src/lib/components/pages/SettingsPage.svelte',
      'src/lib/components/pages/FirstRunSetupPage.svelte',
      'src/lib/components/Sidebar.svelte',
      'src/lib/components/NotificationCenter.svelte',
      'src/lib/components/FileOperations.svelte',
      'src/lib/components/HistoryManager.svelte',
      'src/lib/components/SmartFoldersManager.svelte'
    ];

    for (const component of requiredComponents) {
      if (!fileExists(component)) {
        return `Missing required component: ${component}`;
      }
    }

    return true;
  });
}

function testUIComponents() {
  runTest('UI components exist', () => {
    const uiComponents = [
      'src/lib/components/ui/button',
      'src/lib/components/ui/card',
      'src/lib/components/ui/input',
      'src/lib/components/ui/label',
      'src/lib/components/ui/switch'
    ];

    for (const component of uiComponents) {
      if (!directoryExists(component)) {
        return `Missing UI component directory: ${component}`;
      }
    }

    // Check index.ts exports
    if (!fileExists('src/lib/components/ui/index.ts')) {
      return 'Missing UI components index.ts';
    }

    return true;
  });
}

function testAPIIntegration() {
  runTest('API functions are defined', () => {
    const tauriApiPath = 'src/lib/api/tauri.ts';
    if (!fileExists(tauriApiPath)) {
      return 'tauri.ts API file missing';
    }

    const apiContent = fs.readFileSync(tauriApiPath, 'utf8');

    const criticalFunctions = [
      'checkFirstRunStatus',
      'getAppSettings',
      'checkOllamaStatus',
      'scanDirectory',
      'batchAnalyzeFiles',
      'generateOrganizationSuggestions',
      'emitNotification',
      'getNotifications'
    ];

    for (const func of criticalFunctions) {
      if (!apiContent.includes(`export async function ${func}`)) {
        return `Missing critical API function: ${func}`;
      }
    }

    return true;
  });
}

function testTypeDefinitions() {
  runTest('Type definitions are complete', () => {
    const typesPath = 'src/lib/types/backend.ts';
    if (!fileExists(typesPath)) {
      return 'backend.ts types file missing';
    }

    const typesContent = fs.readFileSync(typesPath, 'utf8');

    const criticalTypes = [
      'FileInfo',
      'FileAnalysis',
      'OrganizationSuggestion',
      'AppSettings',
      'OllamaStatus',
      'FirstRunStatus',
      'SystemStatus'
    ];

    for (const type of criticalTypes) {
      if (!typesContent.includes(`interface ${type}`) && !typesContent.includes(`type ${type}`)) {
        return `Missing critical type definition: ${type}`;
      }
    }

    return true;
  });
}

function testStoreConfiguration() {
  runTest('Store configuration is correct', () => {
    const storePath = 'src/lib/stores/index.ts';
    if (!fileExists(storePath)) {
      return 'stores/index.ts file missing';
    }

    const storeContent = fs.readFileSync(storePath, 'utf8');

    const requiredStores = [
      'currentPage',
      'selectedFiles',
      'notifications',
      'backendNotifications',
      'appSettings'
    ];

    for (const store of requiredStores) {
      if (!storeContent.includes(`export const ${store}`)) {
        return `Missing required store: ${store}`;
      }
    }

    return true;
  });
}

function testTauriConfiguration() {
  runTest('Tauri configuration is valid', () => {
    const tauriConfigPath = 'src-tauri/tauri.conf.json';
    if (!fileExists(tauriConfigPath)) {
      return 'tauri.conf.json missing';
    }

    try {
      const config = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'));

      if (!config.build?.devPath) {
        return 'Missing build.devPath in tauri.conf.json';
      }

      if (!config.build?.distDir) {
        return 'Missing build.distDir in tauri.conf.json';
      }

      return true;
    } catch (error) {
      return 'Invalid JSON in tauri.conf.json';
    }
  });
}

function testPageRoutingSetup() {
  runTest('Page routing is configured', () => {
    const appPath = 'src/App.svelte';
    const appContent = fs.readFileSync(appPath, 'utf8');

    const requiredPages = [
      'discover',
      'analyze',
      'organize',
      'settings'
    ];

    for (const page of requiredPages) {
      if (!appContent.includes(`page === '${page}'`)) {
        return `Missing page route: ${page}`;
      }
    }

    return true;
  });
}

// =============== MAIN EXECUTION ===============

function main() {
  log('🚀 StratoSortRust Frontend Verification', 'magenta');
  log('=========================================', 'magenta');

  // Change to project directory if script is run from elsewhere
  if (!fileExists('package.json')) {
    log('❌ Please run this script from the project root directory', 'red');
    process.exit(1);
  }

  // Run all test suites
  log('\n📁 Testing Project Structure...', 'blue');
  testProjectStructure();
  testDependencies();

  log('\n🔧 Testing Build System...', 'blue');
  testTypeScript();
  testBuild();

  log('\n🧩 Testing Component Structure...', 'blue');
  testComponentStructure();
  testUIComponents();

  log('\n🔌 Testing Backend Integration...', 'blue');
  testAPIIntegration();
  testTypeDefinitions();
  testStoreConfiguration();

  log('\n⚙️  Testing Configuration...', 'blue');
  testTauriConfiguration();
  testPageRoutingSetup();

  // Print summary
  log('\n📊 Verification Summary', 'magenta');
  log('======================', 'magenta');

  const total = results.passed + results.failed;
  const passRate = ((results.passed / total) * 100).toFixed(1);

  log(`Total Tests: ${total}`, 'cyan');
  log(`Passed: ${results.passed}`, 'green');
  log(`Failed: ${results.failed}`, 'red');
  log(`Pass Rate: ${passRate}%`, passRate >= 90 ? 'green' : passRate >= 70 ? 'yellow' : 'red');

  if (results.failed > 0) {
    log('\n❌ Failed Tests:', 'red');
    results.tests
      .filter(test => test.status !== 'PASS')
      .forEach(test => {
        log(`   ${test.name}: ${test.error || test.status}`, 'red');
      });
  }

  // Exit with appropriate code
  const exitCode = results.failed === 0 ? 0 : 1;
  log(`\n${exitCode === 0 ? '✅ All tests passed!' : '❌ Some tests failed.'}`, exitCode === 0 ? 'green' : 'red');

  if (exitCode === 0) {
    log('\n🎉 Frontend setup verification completed successfully!', 'green');
    log('You can now proceed with manual testing using the verification plan.', 'cyan');
  } else {
    log('\n🔧 Please fix the failing tests before proceeding.', 'yellow');
    log('Refer to FRONTEND_VERIFICATION_PLAN.md for detailed testing steps.', 'cyan');
  }

  process.exit(exitCode);
}

// Run if called directly
if (import.meta.url === `file://${__filename}`) {
  main();
}

export { main, runTest, results };