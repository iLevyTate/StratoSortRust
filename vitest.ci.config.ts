import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    globals: true,
    environment: 'happy-dom',
    setupFiles: ['./src/tests/setup.ts'],
    testTimeout: 10000, // Increase timeout for CI
    hookTimeout: 10000,
    isolate: false, // Faster test execution
    threads: true,
    maxThreads: 2, // Limit threads in CI
    minThreads: 1,
    reporters: ['json', 'default'],
    outputFile: 'test-results.json',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'lcov'],
      exclude: [
        'node_modules/**',
        'src/tests/**',
        '**/*.test.ts',
        '**/*.spec.ts',
        '**/test-*.ts',
        '**/*.config.ts',
        'src/mocks/**'
      ]
    },
    // Exclude tests that require Tauri runtime in CI
    exclude: [
      '**/node_modules/**',
      '**/dist/**',
      '**/cypress/**',
      '**/.{idea,git,cache,output,temp}/**',
      // Exclude tests that need actual Tauri runtime
      '**/tests/integration/workflows.test.ts',
      '**/tests/security/frontend-security.test.ts'
    ]
  },
  resolve: {
    alias: {
      $lib: path.resolve('./src/lib'),
      '@': path.resolve('./src')
    }
  }
});