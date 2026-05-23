// Thin, typed wrappers over `@tauri-apps/api/core::invoke`. One function per
// command keeps call sites readable and lets us migrate to taurpc-generated
// bindings later without touching components.
//
// In non-Tauri contexts (vitest jsdom, Playwright without the webview),
// `window.__TAURI_INTERNALS__` is absent and `invoke()` would throw. Each
// wrapper either falls back to a safe default or, when an e2e test has
// installed `window.__TAURI_MOCK__[cmd]`, routes the call through the mock.

import type {
	AppSettings,
	FileAnalysis,
	FileInfo,
	FirstRunStatus,
	OllamaStatus,
	SearchResult,
	SmartFolder,
	SystemInfo
} from '$lib/types/backend';

export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// Mock hook for Playwright. Tests set `window.__TAURI_MOCK__[command_name] =
// async (args) => ...` (snake_case command names matching the Rust side) via
// `page.addInitScript` before navigation. When set, the mock takes precedence
// over both real Tauri invokes and the per-wrapper safe defaults — that's
// what lets data-driven e2e tests actually exercise UI paths.
type MockFn = (args?: Record<string, unknown>) => Promise<unknown>;
function getMock(cmd: string): MockFn | undefined {
	if (typeof window === 'undefined') return undefined;
	const bag = (window as unknown as { __TAURI_MOCK__?: Record<string, MockFn> }).__TAURI_MOCK__;
	return bag?.[cmd];
}

function hasInvocationPath(cmd: string): boolean {
	return isTauri() || getMock(cmd) !== undefined;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	const mock = getMock(cmd);
	if (mock) return (await mock(args)) as T;
	if (!isTauri()) {
		throw new Error(`Tauri invoke called outside Tauri context: ${cmd}`);
	}
	const mod = await import('@tauri-apps/api/core');
	return mod.invoke<T>(cmd, args);
}

export async function listenEvent<T>(
	event: string,
	handler: (payload: T) => void
): Promise<() => void> {
	if (!isTauri()) {
		return () => {};
	}
	const mod = await import('@tauri-apps/api/event');
	const unlisten = await mod.listen<T>(event, (e) => handler(e.payload));
	return unlisten;
}

// --- Lifecycle ---------------------------------------------------------------

export async function frontendReady(): Promise<void> {
	if (!hasInvocationPath('frontend_ready')) return;
	try {
		await invoke<void>('frontend_ready');
	} catch (e) {
		console.warn('frontend_ready failed (backend may not implement it):', e);
	}
}

export async function getSystemInfo(): Promise<SystemInfo | null> {
	if (!hasInvocationPath('get_system_info') && !hasInvocationPath('get_basic_system_info')) {
		return { os: 'web', version: '0', arch: 'unknown' };
	}
	try {
		return await invoke<SystemInfo>('get_system_info');
	} catch {
		try {
			return await invoke<SystemInfo>('get_basic_system_info');
		} catch (e) {
			console.warn('getSystemInfo: both system_info commands failed', e);
			return null;
		}
	}
}

// --- Settings & first-run ----------------------------------------------------

export async function getAppSettings(): Promise<AppSettings | null> {
	if (!hasInvocationPath('get_settings')) return null;
	try {
		return await invoke<AppSettings>('get_settings');
	} catch (e) {
		console.warn('getAppSettings failed:', e);
		return null;
	}
}

export async function updateAppSettings(settings: Partial<AppSettings>): Promise<void> {
	await invoke<void>('update_settings', { settings });
}

export async function checkFirstRunStatus(): Promise<FirstRunStatus> {
	if (!hasInvocationPath('check_first_run_status')) {
		return { is_first_run: false };
	}
	try {
		return await invoke<FirstRunStatus>('check_first_run_status');
	} catch {
		return { is_first_run: false };
	}
}

export async function completeFirstRunSetup(): Promise<void> {
	await invoke<void>('complete_first_run_setup');
}

// --- AI / Ollama -------------------------------------------------------------

export async function checkOllamaStatus(): Promise<OllamaStatus | null> {
	if (!hasInvocationPath('check_ollama_status')) return null;
	try {
		const raw = await invoke<Record<string, unknown>>('check_ollama_status');
		return {
			isRunning: Boolean(raw.is_running ?? raw.isRunning ?? false),
			is_installed: Boolean(raw.is_installed),
			version: (raw.version as string | null) ?? null,
			models: (raw.models as OllamaStatus['models']) ?? [],
			default_model: (raw.default_model as string | null) ?? null,
			host: raw.host as string | undefined
		};
	} catch (e) {
		console.warn('checkOllamaStatus failed:', e);
		return null;
	}
}

export async function reconnectOllama(host: string): Promise<OllamaStatus> {
	return invoke<OllamaStatus>('reconnect_ollama', { host });
}

export async function batchAnalyzeFiles(paths: string[]): Promise<FileAnalysis[]> {
	return invoke<FileAnalysis[]>('batch_analyze_files', { paths });
}

export async function reanalyzeFiles(paths: string[]): Promise<FileAnalysis[]> {
	return invoke<FileAnalysis[]>('reanalyze_files', { paths });
}

export async function clearStaleAnalyses(): Promise<number> {
	return invoke<number>('clear_stale_analyses');
}

export async function semanticSearch(query: string, limit = 20): Promise<SearchResult[]> {
	return invoke<SearchResult[]>('semantic_search', { query, limit });
}

// --- Watch mode --------------------------------------------------------------

export interface WatchModeStatus {
	enabled: boolean;
	watching_directories: string[];
	pending_files_count: number;
	auto_organize_threshold: number;
	learning_enabled: boolean;
	recent_actions_count: number;
}

export async function getWatchModeStatus(): Promise<WatchModeStatus | null> {
	if (!hasInvocationPath('get_watch_mode_status')) return null;
	try {
		return await invoke<WatchModeStatus>('get_watch_mode_status');
	} catch (e) {
		console.warn('getWatchModeStatus failed:', e);
		return null;
	}
}

export async function enableWatchMode(directories: string[]): Promise<void> {
	return invoke<void>('enable_watch_mode', { directories });
}

export async function disableWatchMode(): Promise<void> {
	return invoke<void>('disable_watch_mode');
}

// --- Files -------------------------------------------------------------------

export async function scanDirectory(path: string): Promise<FileInfo[]> {
	return invoke<FileInfo[]>('scan_directory', { path });
}

export async function browseFolder(): Promise<string | null> {
	if (!hasInvocationPath('browse_folder')) return null;
	try {
		return await invoke<string | null>('browse_folder');
	} catch (e) {
		console.warn('browseFolder failed:', e);
		return null;
	}
}

// --- Smart folders -----------------------------------------------------------

export async function listSmartFolders(): Promise<SmartFolder[]> {
	if (!hasInvocationPath('list_smart_folders')) return [];
	try {
		return await invoke<SmartFolder[]>('list_smart_folders');
	} catch (e) {
		console.warn('listSmartFolders failed:', e);
		return [];
	}
}

export async function createSmartFolder(folder: {
	name: string;
	description?: string;
	target_path: string;
	rules: SmartFolder['rules'];
}): Promise<SmartFolder> {
	return invoke<SmartFolder>('create_smart_folder', folder);
}

export async function deleteSmartFolder(id: string): Promise<void> {
	return invoke<void>('delete_smart_folder', { id });
}
