import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for Accessibility testing of StratoSort
 */
export default defineConfig({
	testDir: './e2e',
	testMatch: '**/*.accessibility.spec.ts',
	/* Run tests in files in parallel */
	fullyParallel: true,
	/* Fail the build on CI if you accidentally left test.only in the source code */
	forbidOnly: !!process.env.CI,
	/* Retry on CI only */
	retries: process.env.CI ? 2 : 0,
	/* Opt out of parallel tests on CI */
	workers: process.env.CI ? 1 : undefined,
	/* Reporter to use */
	reporter: [
		['html', { outputFolder: 'accessibility-report' }],
		['json', { outputFile: 'test-results/accessibility-results.json' }],
		['junit', { outputFile: 'test-results/accessibility-junit.xml' }],
		['list']
	],
	/* Shared settings for all the projects below */
	use: {
		/* Base URL to use in actions like `await page.goto('/')` */
		baseURL: 'http://localhost:1431',
		/* Collect trace when retrying the failed test */
		trace: 'on-first-retry',
		/* Screenshot on failure */
		screenshot: 'only-on-failure',
		/* Video on failure */
		video: 'retain-on-failure',
		/* Timeout for each action */
		actionTimeout: 10000,
		/* Navigation timeout */
		navigationTimeout: 30000
	},

	/* Configure projects for major browsers */
	projects: [
		{
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] }
		},
		{
			name: 'firefox',
			use: { ...devices['Desktop Firefox'] }
		},
		{
			name: 'webkit',
			use: { ...devices['Desktop Safari'] }
		}
	],

	/* Run the static vite preview before tests. Matches playwright.config.ts
	 * (the e2e config). The previous `npm run tauri:dev` setup required a
	 * full Rust build + Ollama for tests to even start — not viable in CI
	 * without a Tauri runtime, and unnecessary for a11y checks that just
	 * need the rendered DOM. */
	webServer: {
		command: 'npm run preview -- --port 1431',
		url: 'http://localhost:1431',
		reuseExistingServer: !process.env.CI,
		timeout: 120000
	}
});