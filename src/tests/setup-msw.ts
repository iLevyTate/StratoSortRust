/**
 * Mock Service Worker (MSW) setup for API mocking in tests
 * Provides consistent API responses for testing without backend
 */

import { http, HttpResponse } from 'msw';
import { setupServer } from 'msw/node';
import { beforeAll, afterEach, afterAll } from 'vitest';
import type {
  FileInfo,
  FileAnalysis,
  AppSettings,
  OllamaStatus,
  SystemInfo
} from '$lib/types/backend';

// Default mock responses
const defaultFileInfo: FileInfo = {
  path: '/test/file.txt',
  name: 'file.txt',
  size: 1024,
  mime_type: 'text/plain',
  extension: 'txt',
  modified_at: Date.now(),
  created_at: Date.now(),
  is_directory: false
};

const defaultSettings: AppSettings = {
  ai_provider: 'ollama',
  ollama_host: 'http://localhost:11434',
  ollama_model: 'llama2',
  ollama_vision_model: 'llava',
  ollama_embedding_model: 'nomic-embed-text',
  watch_folders: false,
  watch_paths: [],
  default_smart_folder_location: '',
  file_extensions_to_ignore: ['.tmp', '.cache'],
  max_file_size: 100 * 1024 * 1024,

  // Performance Settings
  max_concurrent_analysis: 3,
  max_concurrent_operations: 5,
  cache_size: 100 * 1024 * 1024,
  enable_gpu: false,

  // Resource Limits
  max_concurrent_reads: 5,
  max_total_memory_mb: 100,
  max_single_file_size_mb: 100,
  max_directory_scan_depth: 10,

  // UI Settings
  theme: 'system',
  language: 'en',
  show_notifications: true,
  notification_duration: 5000,

  // Privacy Settings
  enable_telemetry: false,
  enable_crash_reports: false,
  enable_analytics: false,

  // Behavior Settings
  confirm_before_delete: true,
  confirm_before_move: false,
  auto_analyze_on_add: false,
  preserve_file_timestamps: true,

  // Advanced Settings
  debug_mode: false,
  log_level: 'info',
  history_retention: 30,
  undo_history_size: 50,

  // Naming Convention Settings
  naming_convention: 'lowercase',
  date_format: 'YYYY-MM-DD',
  case_style: 'preserve'
};

const defaultOllamaStatus: OllamaStatus = {
  isRunning: true,
  models: ['llama2', 'llava', 'nomic-embed-text'],
  version: '0.1.0'
};

const defaultSystemInfo: SystemInfo = {
  platform: 'windows',
  arch: 'x64',
  totalMemory: '16GB',
  availableMemory: '8GB',
  cacheSize: '100MB'
};

// Define API handlers
export const handlers = [
  // File operations
  http.post('tauri://localhost/scan_directory', () => {
    return HttpResponse.json([defaultFileInfo]);
  }),

  http.post('tauri://localhost/get_file_info_command', () => {
    return HttpResponse.json(defaultFileInfo);
  }),

  http.post('tauri://localhost/get_file_content', () => {
    return HttpResponse.json('File content');
  }),

  http.post('tauri://localhost/move_file', () => {
    return HttpResponse.json(true);
  }),

  http.post('tauri://localhost/copy_file', () => {
    return HttpResponse.json(true);
  }),

  http.post('tauri://localhost/delete_file', () => {
    return HttpResponse.json(true);
  }),

  http.post('tauri://localhost/rename_file', () => {
    return HttpResponse.json(true);
  }),

  http.post('tauri://localhost/create_directory', () => {
    return HttpResponse.json(true);
  }),

  http.post('tauri://localhost/file_exists', () => {
    return HttpResponse.json({
      exists: true,
      is_file: true,
      is_directory: false,
      is_accessible: true
    });
  }),

  // Settings operations
  http.post('tauri://localhost/get_settings', () => {
    return HttpResponse.json(defaultSettings);
  }),

  http.post('tauri://localhost/update_settings', () => {
    return HttpResponse.json(null);
  }),

  http.post('tauri://localhost/reset_settings', () => {
    return HttpResponse.json(null);
  }),

  // AI operations
  http.post('tauri://localhost/check_ollama_status', () => {
    return HttpResponse.json(defaultOllamaStatus);
  }),

  http.post('tauri://localhost/analyze_with_ai', () => {
    return HttpResponse.json({
      path: '/test/file.txt',
      category: 'Documents',
      tags: ['text', 'document'],
      summary: 'A test file',
      confidence: 0.95
    });
  }),

  http.post('tauri://localhost/analyze_files', () => {
    return HttpResponse.json([{
      path: '/test/file.txt',
      category: 'Documents',
      tags: ['text', 'document'],
      summary: 'A test file',
      confidence: 0.95
    }]);
  }),

  http.post('tauri://localhost/generate_embeddings', () => {
    return HttpResponse.json(Array(384).fill(0.1));
  }),

  http.post('tauri://localhost/semantic_search', () => {
    return HttpResponse.json([{
      path: '/test/file.txt',
      name: 'file.txt',
      score: 0.95,
      snippet: 'Test content',
      file_type: 'text/plain',
      size: 1024,
      modified_at: Date.now()
    }]);
  }),

  // System operations
  http.post('tauri://localhost/get_system_info', () => {
    return HttpResponse.json(defaultSystemInfo);
  }),

  http.post('tauri://localhost/get_basic_system_info', () => {
    return HttpResponse.json(defaultSystemInfo);
  }),

  http.post('tauri://localhost/frontend_ready', () => {
    return HttpResponse.json(null);
  }),

  http.post('tauri://localhost/check_first_run_status', () => {
    return HttpResponse.json({
      is_first_run: false,
      setup_completed: true
    });
  }),

  http.post('tauri://localhost/complete_first_run_setup', () => {
    return HttpResponse.json('Setup completed');
  }),

  // Organization operations
  http.post('tauri://localhost/suggest_file_organization', () => {
    return HttpResponse.json([{
      source_path: '/test/file.txt',
      target_folder: '/Documents',
      reason: 'Text document',
      confidence: 0.9
    }]);
  }),

  http.post('tauri://localhost/apply_organization', () => {
    return HttpResponse.json({
      success: true,
      message: 'Organization applied',
      applied: 1,
      failed: 0
    });
  }),

  http.post('tauri://localhost/auto_organize_directory', () => {
    return HttpResponse.json([{
      id: 'org-1',
      file_path: '/test/file.txt',
      current_location: '/test',
      suggested_location: '/Documents',
      reason: 'Text document',
      confidence: 0.9
    }]);
  }),

  // Batch operations
  http.post('tauri://localhost/move_files', () => {
    return HttpResponse.json({
      total: 1,
      successful: 1,
      failed: 0,
      results: [{
        path: '/test/file.txt',
        success: true
      }]
    });
  }),

  http.post('tauri://localhost/batch_file_operations', () => {
    return HttpResponse.json({
      total: 1,
      successful: 1,
      failed: 0,
      results: [{
        path: '/test/file.txt',
        success: true
      }]
    });
  }),

  // Diagnostics
  http.post('tauri://localhost/run_diagnostics', () => {
    return HttpResponse.json({
      overall_health: 'healthy',
      checks: {
        backend: { status: 'pass', message: 'Backend is running' },
        database: { status: 'pass', message: 'Database is connected' },
        ai_service: { status: 'pass', message: 'AI service is available' }
      },
      performance: {
        cpu_usage: 25,
        memory_usage: 50,
        disk_usage: 30
      },
      timestamp: Date.now()
    });
  }),

  http.post('tauri://localhost/check_database_health', () => {
    return HttpResponse.json({
      status: 'healthy',
      connection_test: {
        success: true,
        latency_ms: 5
      },
      integrity_check: {
        passed: true,
        issues: []
      },
      performance: {
        query_count: 100,
        avg_query_time_ms: 10,
        slow_queries: 0
      },
      size_info: {
        total_size_bytes: 1024000,
        table_count: 10,
        record_count: 1000
      }
    });
  })
];

// Create server instance
export const server = setupServer(...handlers);

// Setup and teardown functions for tests
export function setupMSW(): void {
  // Start server before all tests
  beforeAll(() => server.listen({ onUnhandledRequest: 'bypass' }));

  // Reset handlers after each test
  afterEach(() => server.resetHandlers());

  // Clean up after all tests
  afterAll(() => server.close());
}

// Helper to add custom handlers for specific tests
export function mockApiResponse<T>(
  method: 'get' | 'post' | 'put' | 'delete',
  endpoint: string,
  response: T,
  status = 200
): void {
  const httpMethod = method === 'get' ? http.get : method === 'post' ? http.post : method === 'put' ? http.put : http.delete;
  const handler = httpMethod(`tauri://localhost/${endpoint}`, () => {
    return new HttpResponse(JSON.stringify(response), {
      status,
      headers: { 'Content-Type': 'application/json' }
    });
  });

  server.use(handler);
}

// Helper to simulate API errors
export function mockApiError(
  method: 'get' | 'post' | 'put' | 'delete',
  endpoint: string,
  errorMessage = 'Internal Server Error',
  status = 500
): void {
  const httpMethod = method === 'get' ? http.get : method === 'post' ? http.post : method === 'put' ? http.put : http.delete;
  const handler = httpMethod(`tauri://localhost/${endpoint}`, () => {
    return new HttpResponse(JSON.stringify({ error: errorMessage }), {
      status,
      headers: { 'Content-Type': 'application/json' }
    });
  });

  server.use(handler);
}

// Helper to delay API responses (for testing loading states)
export function mockApiDelay(
  method: 'get' | 'post' | 'put' | 'delete',
  endpoint: string,
  response: unknown,
  delayMs = 1000
): void {
  const httpMethod = method === 'get' ? http.get : method === 'post' ? http.post : method === 'put' ? http.put : http.delete;
  const handler = httpMethod(`tauri://localhost/${endpoint}`, async () => {
    await new Promise(resolve => setTimeout(resolve, delayMs));
    return HttpResponse.json(response as any);
  });

  server.use(handler);
}

// Export for use in individual test files
export default server;