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
			reporter: ['text', 'json', 'html', 'lcov'],
			exclude: [
				'node_modules/',
				'src/tests/',
				'*.config.*',
				'**/*.d.ts',
				'**/*.test.*',
				'**/*.spec.*',
				'**/index.ts'
			],
			thresholds: {
				branches: 80,
				functions: 80,
				lines: 85,
				statements: 85
			}
		},
		reporters: ['verbose'],
		testTimeout: 10000,
		hookTimeout: 10000,
		teardownTimeout: 10000,
		isolate: true,
		threads: true,
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