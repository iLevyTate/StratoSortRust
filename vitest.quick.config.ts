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
		// Exclude slow and problematic tests for quick runs
		exclude: [
			'node_modules',
			'dist',
			'.svelte-kit',
			'src-tauri',
			// Skip integration tests that may hang
			'**/tests/integration/**',
			// Skip security tests that have event emitter issues
			'**/tests/security/frontend-security.test.ts',
			// Skip performance tests for quick runs
			'**/tests/performance/**'
		],
		testTimeout: 5000, // Shorter timeout for quick feedback
		hookTimeout: 5000,
		teardownTimeout: 5000,
		isolate: false, // Faster execution
		threads: true,
		maxThreads: 4,
		pool: 'threads',
		reporters: ['basic'], // Minimal output
		mockReset: true,
		restoreMocks: true,
		clearMocks: true
	},
	resolve: {
		alias: {
			'$lib': resolve('./src/lib'),
			'$app': resolve('./src/tests/mocks/app')
		},
		conditions: ['svelte', 'browser', 'import', 'default']
	},
	define: {
		'import.meta.vitest': 'undefined'
	}
});