import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

export default defineConfig({
	plugins: [svelte({ hot: false })],
	test: {
		globals: true,
		environment: 'happy-dom',
		setupFiles: ['./src/tests/setup.ts'],
		include: ['src/**/*.{test,spec}.{js,ts,svelte}'],
		exclude: ['node_modules', 'dist', '.svelte-kit', 'src-tauri'],
		coverage: {
			provider: 'v8',
			reporter: ['text', 'json', 'html', 'lcov', 'cobertura'], // Add cobertura for CI
			exclude: [
				'node_modules/',
				'src/tests/',
				'src/tests/**/*',
				'*.config.*',
				'**/*.d.ts',
				'**/*.test.*',
				'**/*.spec.*',
				'**/index.ts',
				'dist/',
				'build/',
				'.svelte-kit/',
				'src-tauri/'
			],
			thresholds: {
				branches: 80,
				functions: 80,
				lines: 85,
				statements: 85
			},
			reportsDirectory: './coverage',
			clean: true // Clean coverage directory before running
		},
		reporters: ['verbose'],
		testTimeout: 30000,
		hookTimeout: 30000,
		teardownTimeout: 30000,
		isolate: true,
		threads: true,
		mockReset: true,
		restoreMocks: true,
		clearMocks: true,
		// Performance optimization
		pool: 'threads',
		poolOptions: {
			threads: {
				singleThread: false,
				useAtomics: true,
				isolate: true
			}
		},
		// Better error reporting
		outputFile: {
			junit: './test-results/junit.xml',
			html: './test-results/index.html'
		},
		// Fail fast in CI
		bail: process.env.CI ? 1 : 0
	},
	resolve: {
		alias: {
			'$lib': resolve('./src/lib'),
			'$app': resolve('./src/tests/mocks/app'),
			'svelte-sonner': resolve('./src/tests/mocks/svelte-sonner.ts')
		},
		conditions: ['svelte', 'browser', 'import', 'default']
	},
	define: {
		'import.meta.vitest': 'undefined',
		'process.env.NODE_ENV': JSON.stringify(process.env.NODE_ENV || 'test')
	},
	// Optimization for test builds
	optimizeDeps: {
		include: ['svelte', '@testing-library/svelte'],
		exclude: ['@tauri-apps/cli']
	}
});