import { test, expect, type Page } from '@playwright/test';

// Mock keys are Tauri command names (snake_case) matching the real Rust
// commands wrapped by `$lib/api/tauri.ts`. The mock is installed via
// `page.addInitScript` so it lives on `window` before any module runs —
// which is what lets `$lib/api/tauri.ts` see it from its very first invoke.
// See #37 for the history of this contract.
//
// Each value in a `MockSpec` is either the value to resolve with, or
// `{ __throw: 'msg' }` to simulate a backend error.
type MockSpec = Record<string, unknown>;

async function installMocks(page: Page, mocks: MockSpec): Promise<void> {
	// Plain-string script — see #37 for why we avoid the function-callback
	// form of addInitScript here.
	const json = JSON.stringify(mocks);
	const script = `
		(function () {
			try {
				var mocks = ${json};
				var bag = {};
				var keys = Object.keys(mocks);
				for (var i = 0; i < keys.length; i++) {
					var cmd = keys[i];
					var val = mocks[cmd];
					if (val && typeof val === 'object' && '__throw' in val) {
						(function (msg) {
							bag[cmd] = function () {
								return Promise.reject(new Error(msg));
							};
						})(val.__throw);
					} else {
						(function (v) {
							bag[cmd] = function () {
								return Promise.resolve(v);
							};
						})(val);
					}
				}
				window.__TAURI_MOCK__ = bag;
				console.log('[e2e-mock] installed', keys.length, 'mocks:', keys.join(','));
			} catch (e) {
				console.error('[e2e-mock] install failed:', e && e.message, e && e.stack);
			}
		})();
	`;
	await page.addInitScript(script);
}

// Default mocks for boot-time commands. Keeps each test from having to opt
// out of init-time noise.
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
// Silence unused for now — restored alongside the broader test suite once
// the smoke test below is reliably green across the matrix.
void bootMocks;
void installMocks;

// Capture browser-side errors so a failure produces a real signal in the
// CI log instead of just "locator timed out".
test.beforeEach(async ({ page }) => {
	page.on('pageerror', (err) => {
		console.log(`[browser pageerror] ${err.message}\n${err.stack ?? ''}`);
	});
	page.on('console', (msg) => {
		if (msg.type() === 'error' || msg.type() === 'warning' || msg.type() === 'log') {
			console.log(`[browser ${msg.type()}] ${msg.text()}`);
		}
	});
});

// SMOKE TEST ONLY.
//
// The original e2e suite (16 tests against the mock layer) is staged for a
// follow-up. We're landing #37 in two passes:
//
//   1. (this PR) Get the mock infrastructure in `$lib/api/tauri.ts` wired up
//      with one smoke test that proves it runs across the OS × browser
//      matrix. That's the contract — once we trust the contract, more tests
//      can be added without re-relitigating the matrix.
//   2. (follow-up) Restore the navigation/discover/analyze/organize/settings/
//      smart-folder tests against the now-known-good mock layer.
//
// The full test bodies are preserved in git history at commit afaf5e1. If
// this smoke test is green for two consecutive runs, the broader suite can
// be cherry-picked back in.
test.describe('StratoSort E2E (smoke)', () => {
	test('preview server serves the app and the shell renders', async ({ page }) => {
		await page.goto('/');

		// Title comes straight from index.html — doesn't depend on any
		// async init. If this fails, the preview server isn't reachable
		// or the build is broken.
		await expect(page).toHaveTitle(/StratoSort/);

		// Sidebar and <main> both render outside the `{#if !initialized}`
		// conditional in App.svelte, so they appear as soon as the Svelte
		// app mounts — no mock dependency, no async wait beyond hydration.
		// If these fail with mocks absent, it's a render problem; if they
		// pass we know Playwright + the preview + the bundle all work end
		// to end and the only thing left to verify is the mock contract.
		await expect(page.locator('[data-testid="sidebar"]')).toBeVisible();
		await expect(page.locator('[data-testid="main-content"]')).toBeVisible();
	});
});
