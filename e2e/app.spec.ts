import { test, expect, type Page } from '@playwright/test';

// Mock keys are Tauri command names (snake_case) matching the real Rust
// commands wrapped by `$lib/api/tauri.ts`. The mock is installed via
// `page.addInitScript` so it lives on `window` before any module runs —
// which is what lets `$lib/api/tauri.ts` see it from its very first invoke.
// See #37 for the history of this contract.
//
// Each value in a `MockSpec` is either the value to resolve with, or
// `{ __throw: 'msg' }` to simulate a backend error. Functions can't cross
// the Node↔browser boundary, so we serialize a declarative spec instead.
type MockSpec = Record<string, unknown>;

async function installMocks(page: Page, mocks: MockSpec): Promise<void> {
	await page.addInitScript((serialized: string) => {
		const parsed = JSON.parse(serialized) as Record<string, unknown>;
		const bag: Record<string, (a?: Record<string, unknown>) => Promise<unknown>> = {};
		for (const [cmd, value] of Object.entries(parsed)) {
			if (value && typeof value === 'object' && '__throw' in value) {
				const msg = (value as { __throw: string }).__throw;
				bag[cmd] = async () => {
					throw new Error(msg);
				};
			} else {
				bag[cmd] = async () => value;
			}
		}
		(window as unknown as { __TAURI_MOCK__?: typeof bag }).__TAURI_MOCK__ = bag;
	}, JSON.stringify(mocks));
}

// Default mocks for commands the app fires at boot. Keeps each test from
// having to opt out of init-time noise.
const bootMocks: MockSpec = {
	get_settings: {
		theme: 'auto',
		language: 'en',
		ollama_host: 'http://localhost:11434',
		ollama_model: 'llama3.2:3b',
		ollama_vision_model: 'llava',
		ollama_embedding_model: 'nomic-embed-text',
		enable_telemetry: false,
		enable_crash_reports: false,
		auto_analyze_on_add: false
	},
	check_ollama_status: {
		is_running: true,
		is_installed: true,
		version: '0.1.0',
		models: [],
		default_model: 'llama3.2:3b'
	},
	get_watch_mode_status: {
		enabled: false,
		watching_directories: [],
		pending_files_count: 0,
		auto_organize_threshold: 0,
		learning_enabled: false,
		recent_actions_count: 0
	},
	list_smart_folders: [],
	get_analysis_history: []
};

// Capture browser-side errors so CI logs show *why* the app didn't render,
// instead of just "locator timeout after 5s". Set on every test via a fixture-
// like beforeEach so all 16 tests get the same diagnostic if they fail.
test.beforeEach(async ({ page }) => {
	page.on('pageerror', (err) => {
		console.log(`[browser pageerror] ${err.message}\n${err.stack ?? ''}`);
	});
	page.on('console', (msg) => {
		if (msg.type() === 'error' || msg.type() === 'warning') {
			console.log(`[browser ${msg.type()}] ${msg.text()}`);
		}
	});
});

test.describe('StratoSort E2E', () => {
	test.describe('Application Launch', () => {
		test('renders main shell on Discover by default', async ({ page }) => {
			await installMocks(page, bootMocks);
			await page.goto('/');

			await expect(page).toHaveTitle(/StratoSort/);
			await expect(page.locator('[data-testid="sidebar"]')).toBeVisible();
			await expect(page.locator('[data-testid="main-content"]')).toBeVisible();

			// If `[data-testid="discover-page"]` doesn't show up, dump the page
			// HTML so CI logs reveal whether we're stuck on the "Initializing
			// StratoSort..." spinner (App.svelte's !initialized branch) or
			// somewhere else entirely. The a11y suite passes without ever
			// reaching this branch, so a11y green ≠ this passing.
			const discover = page.locator('[data-testid="discover-page"]');
			try {
				await expect(discover).toBeVisible({ timeout: 10000 });
			} catch (e) {
				const html = await page.locator('body').innerHTML();
				const mockState = await page.evaluate(() => ({
					hasMockBag: typeof (window as unknown as { __TAURI_MOCK__?: unknown })
						.__TAURI_MOCK__ !== 'undefined',
					mockKeys: Object.keys(
						(window as unknown as { __TAURI_MOCK__?: Record<string, unknown> })
							.__TAURI_MOCK__ ?? {}
					),
					hasTauri: '__TAURI__' in window,
					hasTauriInternals: '__TAURI_INTERNALS__' in window
				}));
				console.log('[diag] mock/window state:', JSON.stringify(mockState));
				console.log('[diag] body html (first 2000 chars):', html.slice(0, 2000));
				throw e;
			}
		});

		test('navigates through all main sections', async ({ page }) => {
			await installMocks(page, bootMocks);
			await page.goto('/');
			await page.locator('[data-testid="discover-page"]').waitFor();

			await page.click('[data-testid="nav-analyze"]');
			await expect(page.locator('[data-testid="analyze-page"]')).toBeVisible();

			await page.click('[data-testid="nav-organize"]');
			await expect(page.locator('[data-testid="organize-page"]')).toBeVisible();

			await page.click('[data-testid="nav-settings"]');
			await expect(page.locator('[data-testid="settings-page"]')).toBeVisible();

			await page.click('[data-testid="nav-discover"]');
			await expect(page.locator('[data-testid="discover-page"]')).toBeVisible();
		});
	});

	test.describe('Discover', () => {
		test('scans a directory and renders the file list', async ({ page }) => {
			await installMocks(page, {
				...bootMocks,
				scan_directory: [
					{ path: '/test/file1.txt', name: 'file1.txt', size: 1024, is_directory: false },
					{ path: '/test/file2.pdf', name: 'file2.pdf', size: 2048, is_directory: false }
				]
			});
			await page.goto('/');
			await page.locator('[data-testid="discover-page"]').waitFor();

			await page.fill('input[aria-label="Directory path"]', '/test');
			await page.click('button:has-text("Scan")');

			await expect(page.locator('text=file1.txt')).toBeVisible();
			await expect(page.locator('text=file2.pdf')).toBeVisible();
			await expect(page.locator('text=2 item(s) · 0 selected')).toBeVisible();
		});

		test('selecting files updates the counter', async ({ page }) => {
			await installMocks(page, {
				...bootMocks,
				scan_directory: [
					{ path: '/test/a.txt', name: 'a.txt', size: 1024, is_directory: false },
					{ path: '/test/b.txt', name: 'b.txt', size: 1024, is_directory: false },
					{ path: '/test/c.txt', name: 'c.txt', size: 1024, is_directory: false }
				]
			});
			await page.goto('/');
			await page.locator('[data-testid="discover-page"]').waitFor();

			await page.fill('input[aria-label="Directory path"]', '/test');
			await page.click('button:has-text("Scan")');
			await page.locator('text=a.txt').waitFor();

			await page.check('[data-testid="file-checkbox-0"]');
			await page.check('[data-testid="file-checkbox-1"]');
			await expect(page.locator('text=3 item(s) · 2 selected')).toBeVisible();
		});

		test('semantic search renders results', async ({ page }) => {
			await installMocks(page, {
				...bootMocks,
				semantic_search: [
					{ path: '/test/doc.pdf', name: 'doc.pdf', score: 0.91, snippet: 'matched snippet' }
				]
			});
			await page.goto('/');
			await page.locator('[data-testid="discover-page"]').waitFor();

			await page.fill('[data-testid="search-input"]', 'doc');
			await page.click('button:has-text("Search")');

			await expect(page.locator('[data-testid="search-results"]')).toBeVisible();
			await expect(page.locator('text=doc.pdf')).toBeVisible();
		});

		test('scan failure surfaces an error toast', async ({ page }) => {
			await installMocks(page, {
				...bootMocks,
				scan_directory: { __throw: 'Network error: Unable to connect' }
			});
			await page.goto('/');
			await page.locator('[data-testid="discover-page"]').waitFor();

			await page.fill('input[aria-label="Directory path"]', '/nope');
			await page.click('button:has-text("Scan")');

			await expect(page.locator('text=/Scan failed.*Network error/')).toBeVisible();
		});
	});

	test.describe('Analyze', () => {
		test('re-analyze with no paths shows an info toast', async ({ page }) => {
			await installMocks(page, bootMocks);
			await page.goto('/');
			await page.click('[data-testid="nav-analyze"]');
			await page.locator('[data-testid="analyze-page"]').waitFor();

			await page.click('button:has-text("Re-analyze")');
			await expect(page.locator('text=Paste one path per line')).toBeVisible();
		});

		test('re-analyze fires the wrapper and surfaces a success toast', async ({ page }) => {
			await installMocks(page, {
				...bootMocks,
				reanalyze_files: [
					{ file_path: '/test/report.pdf', category: 'Documents', summary: 'q4', confidence: 0.85 }
				]
			});
			await page.goto('/');
			await page.click('[data-testid="nav-analyze"]');
			await page.locator('[data-testid="analyze-page"]').waitFor();

			await page.fill('#rerun-paths', '/test/report.pdf');
			await page.click('button:has-text("Re-analyze")');

			await expect(page.locator('text=Re-analyzed 1 file(s)')).toBeVisible();
		});
	});

	test.describe('Organize', () => {
		test('watch mode panel reflects status', async ({ page }) => {
			await installMocks(page, {
				...bootMocks,
				get_watch_mode_status: {
					enabled: true,
					watching_directories: ['/home/me/Downloads'],
					pending_files_count: 0,
					auto_organize_threshold: 0,
					learning_enabled: false,
					recent_actions_count: 0
				}
			});
			await page.goto('/');
			await page.click('[data-testid="nav-organize"]');
			await page.locator('[data-testid="organize-page"]').waitFor();

			await expect(page.locator('[data-testid="watch-mode-panel"]')).toContainText('enabled');
			await expect(page.locator('text=/home/me/Downloads')).toBeVisible();
		});

		test('smart folder manager creates a folder', async ({ page }) => {
			// This test needs stateful behavior across two invokes
			// (create_smart_folder → list_smart_folders should now see the new one),
			// which the declarative MockSpec can't express. Use a custom init script.
			await page.addInitScript(() => {
				const bag: Record<string, (a?: Record<string, unknown>) => Promise<unknown>> = {
					get_settings: async () => ({
						theme: 'auto', language: 'en',
						ollama_host: '', ollama_model: '',
						ollama_vision_model: '', ollama_embedding_model: '',
						enable_telemetry: false, enable_crash_reports: false, auto_analyze_on_add: false
					}),
					check_ollama_status: async () => ({
						is_running: false, is_installed: false, version: null, models: [], default_model: null
					}),
					get_watch_mode_status: async () => ({
						enabled: false, watching_directories: [], pending_files_count: 0,
						auto_organize_threshold: 0, learning_enabled: false, recent_actions_count: 0
					}),
					get_analysis_history: async () => [],
					list_smart_folders: async () => {
						const w = window as unknown as { __CREATED__?: unknown };
						return w.__CREATED__ ? [w.__CREATED__] : [];
					},
					create_smart_folder: async (args) => {
						const folder = { id: 'sf-1', ...(args ?? {}) };
						(window as unknown as { __CREATED__?: unknown }).__CREATED__ = folder;
						return folder;
					}
				};
				(window as unknown as { __TAURI_MOCK__?: typeof bag }).__TAURI_MOCK__ = bag;
			});
			await page.goto('/');
			await page.click('[data-testid="nav-organize"]');
			await page.locator('[data-testid="smart-folders-manager"]').waitFor();

			await page.fill('[data-testid="folder-name"]', 'Invoices');
			await page.fill('input[placeholder="/home/me/Documents/Invoices"]', '/tmp/invoices');
			await page.selectOption('[data-testid="rule-field"]', 'extension');
			await page.selectOption('[data-testid="rule-operator"]', 'equals');
			await page.fill('[data-testid="rule-value"]', 'pdf');

			await page.click('button:has-text("Add folder")');

			await expect(page.locator('text=Created smart folder')).toBeVisible();
			await expect(page.locator('[data-testid="smart-folders-manager"]')).toContainText('Invoices');
		});
	});

	test.describe('Settings', () => {
		test('theme select is wired to the bound value', async ({ page }) => {
			await installMocks(page, bootMocks);
			await page.goto('/');
			await page.click('[data-testid="nav-settings"]');
			await page.locator('[data-testid="settings-page"]').waitFor();

			await page.selectOption('[data-testid="theme-select"]', 'dark');
			await expect(page.locator('[data-testid="theme-select"]')).toHaveValue('dark');
		});

		test('saving settings calls update_settings and toasts', async ({ page }) => {
			// Capture the invoke side effect so we can assert update_settings actually fired.
			await page.addInitScript(() => {
				const bag: Record<string, (a?: Record<string, unknown>) => Promise<unknown>> = {
					get_settings: async () => ({
						theme: 'auto', language: 'en',
						ollama_host: 'http://localhost:11434', ollama_model: 'llama3.2:3b',
						ollama_vision_model: 'llava', ollama_embedding_model: 'nomic-embed-text',
						enable_telemetry: false, enable_crash_reports: false, auto_analyze_on_add: false
					}),
					check_ollama_status: async () => ({
						is_running: true, is_installed: true, version: '0.1', models: [], default_model: 'llama3.2:3b'
					}),
					get_watch_mode_status: async () => ({
						enabled: false, watching_directories: [], pending_files_count: 0,
						auto_organize_threshold: 0, learning_enabled: false, recent_actions_count: 0
					}),
					get_analysis_history: async () => [],
					list_smart_folders: async () => [],
					update_settings: async () => {
						(window as unknown as { __SAVED__?: boolean }).__SAVED__ = true;
						return null;
					}
				};
				(window as unknown as { __TAURI_MOCK__?: typeof bag }).__TAURI_MOCK__ = bag;
			});
			await page.goto('/');
			await page.click('[data-testid="nav-settings"]');
			await page.locator('[data-testid="settings-page"]').waitFor();

			await page.click('button:has-text("Save")');
			await expect(page.locator('text=Settings saved')).toBeVisible();
			expect(await page.evaluate(() => (window as unknown as { __SAVED__?: boolean }).__SAVED__)).toBe(true);
		});

		test('AI tab shows Ollama host and model inputs', async ({ page }) => {
			await installMocks(page, bootMocks);
			await page.goto('/');
			await page.click('[data-testid="nav-settings"]');
			await page.locator('[data-testid="settings-page"]').waitFor();

			await page.click('[data-testid="tab-ai"]');
			await expect(page.locator('[data-testid="ollama-host"]')).toBeVisible();
			await expect(page.locator('[data-testid="ollama-model"]')).toBeVisible();
		});

		test('privacy tab telemetry toggle responds to clicks', async ({ page }) => {
			await installMocks(page, bootMocks);
			await page.goto('/');
			await page.click('[data-testid="nav-settings"]');
			await page.locator('[data-testid="settings-page"]').waitFor();

			await page.click('[data-testid="tab-privacy"]');
			const telemetry = page.locator('[data-testid="telemetry-switch"]');
			await expect(telemetry).not.toBeChecked();
			await telemetry.check();
			await expect(telemetry).toBeChecked();
		});
	});
});
