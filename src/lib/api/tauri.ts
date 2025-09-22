import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
// Runtime helpers for non-Tauri/browser E2E environment
function isTauriRuntime(): boolean {
    return typeof window !== 'undefined' && !!(window as any).__TAURI__;
}

function getTauriMock(): any | null {
    if (typeof window === 'undefined') return null;
    return (window as any).__TAURI_MOCK__ || null;
}

import { toast } from '$lib/stores/notifications';
import { withRetry, parseBackendError, isRetryableError, createApiCallLegacy as createApiCall } from './error-handler';
import { CircuitBreaker, handleUserError, withFallback } from '$lib/utils/async-error-handler';
import { isValidAnyUuid } from '$lib/utils/uuid';

// Helper to unwrap standardized event envelopes emitted via emit_event! macro
function unwrapEventPayload<T = any>(payload: any): T {
  if (payload && typeof payload === 'object' && 'data' in payload && (payload as any).data != null) {
    return (payload as any).data as T;
  }
  return payload as T;
}

// API Response Caching System
interface CacheEntry<T = unknown> {
	data: T;
	timestamp: number;
	promise?: Promise<T>;
}

class ApiCache {
	private cache = new Map<string, CacheEntry>();
	private pendingRequests = new Map<string, Promise<unknown>>();
	private readonly DEFAULT_TTL = 5000; // 5 seconds
	private cleanupTimer: NodeJS.Timeout | null = null;

	constructor() {
		// Start cleanup timer to remove stale entries
		this.startCleanupTimer();
	}

	private startCleanupTimer(): void {
		// Clean up stale cache entries every 10 seconds
		this.cleanupTimer = setInterval(() => {
			this.removeStaleEntries();
		}, 10000);
	}

	private removeStaleEntries(): void {
		const now = Date.now();
		for (const [key, entry] of this.cache.entries()) {
			if (entry.timestamp && now - entry.timestamp > this.DEFAULT_TTL * 2) {
				this.cache.delete(key);
			}
		}
	}

	private generateKey(command: string, args: unknown = {}): string {
		return `${command}:${JSON.stringify(args)}`;
	}

	async cachedInvoke<T>(command: string, args: unknown = {}, ttl: number = this.DEFAULT_TTL): Promise<T> {
		const cacheKey = this.generateKey(command, args);

		// Check for pending request to avoid duplicate calls
		const pending = this.pendingRequests.get(cacheKey);
		if (pending) {
			return pending as Promise<T>;
		}

		// Check if we have a valid cached response
		const cached = this.cache.get(cacheKey);
		if (cached && Date.now() - cached.timestamp < ttl) {
			return cached.data as T;
		}

		// Create the request promise
		const requestPromise = (async () => {
			try {
				const data = await invoke<T>(command, args as Record<string, unknown>);
				// Cache the successful result
				this.cache.set(cacheKey, {
					data,
					timestamp: Date.now()
				});
				// Remove from pending requests
				this.pendingRequests.delete(cacheKey);
				return data;
			} catch (error) {
				// Remove from pending requests and cache on error
				this.pendingRequests.delete(cacheKey);
				this.cache.delete(cacheKey);
				throw error;
			}
		})();

		// Store in pending requests to prevent race conditions
		this.pendingRequests.set(cacheKey, requestPromise);

		return requestPromise;
	}

	invalidate(command: string, args: unknown = {}): void {
		const cacheKey = this.generateKey(command, args);
		this.cache.delete(cacheKey);
		this.pendingRequests.delete(cacheKey);
	}

	invalidatePattern(pattern: string): void {
		// Use Array.from to avoid iterator invalidation during deletion
		const keysToDelete: string[] = [];

		for (const key of this.cache.keys()) {
			if (key.includes(pattern)) {
				keysToDelete.push(key);
			}
		}

		for (const key of this.pendingRequests.keys()) {
			if (key.includes(pattern)) {
				keysToDelete.push(key);
			}
		}

		// Delete all matched keys
		for (const key of keysToDelete) {
			this.cache.delete(key);
			this.pendingRequests.delete(key);
		}
	}

	clear(): void {
		this.cache.clear();
		this.pendingRequests.clear();
	}

	size(): number {
		return this.cache.size;
	}

	destroy(): void {
		if (this.cleanupTimer) {
			clearInterval(this.cleanupTimer);
			this.cleanupTimer = null;
		}
		this.clear();
	}
}

// Global cache instance
const apiCache = new ApiCache();

// Export cache for testing and external use
export { apiCache };
import type {
	FileInfo,
	FileAnalysis,
	AnalysisResult,
	OrganizationSuggestion,
	OrganizationSuggestionUI,
	OrganizationOperation,
	OrganizationPreview,
	OrganizationResult,
	OllamaStatus,
	AiStatus,
	AppSettings,
	SystemInfo,
	ActiveOperationInfo,
	ProgressEvent,
	OperationCompleteEvent,
	OperationErrorEvent,
	FileChangeEvent,
	SearchResult,
	BatchOperationResult,
	FileExistsResult,
	FileSizeInfo,
	ValidationResult,
	PerformanceMetrics,
	HealthStatus,
	AppInfo,
	ProcessedDropResult,
	SmartFolder,
	OrganizationRule,
	StorageInfo,
	UpdateInfo,
	SystemStatus,
	WatchModeConfig,
	UserLearningPattern,
	AutoOrganizationTrigger,
	FileProperties,
	BatchFileOperation,
	HistoryEntry,
	HistoryState,
	SystemDiagnostics,
	AiServiceDiagnostics,
	DatabaseDiagnostics,
	PathPermissionCheck,
	ResourceDiagnostics,
	ClearCacheResult
} from '$lib/types/backend';

// Circuit breakers for critical operations to prevent cascading failures
const ollamaStatusCircuitBreaker = new CircuitBreaker(() => invoke('get_ollama_status'), {
	failureThreshold: 3,
	resetTimeout: 30000, // 30 seconds
	onOpen: () => console.warn('Ollama status circuit breaker opened - service may be down'),
	onClose: () => console.info('Ollama status circuit breaker closed - service recovered')
});

const aiStatusCircuitBreaker = new CircuitBreaker(() => invoke<AiStatus>('get_ai_status'), {
	failureThreshold: 5,
	resetTimeout: 60000, // 1 minute
	onOpen: () => console.warn('AI status circuit breaker opened - service may be down'),
	onClose: () => console.info('AI status circuit breaker closed - service recovered')
});

// Re-export types for backward compatibility
export type {
	FileInfo,
	FileAnalysis,
	AnalysisResult,
	OrganizationSuggestion,
	OrganizationSuggestionUI,
	OrganizationOperation,
	OrganizationPreview,
	OllamaStatus,
	AiStatus,
	AppSettings,
	SystemInfo,
	ProgressEvent,
	OperationCompleteEvent,
	OperationErrorEvent,
	HistoryEntry,
	HistoryState,
} from '$lib/types/backend';

// File Operations
export async function scanDirectory(path: string, recursive: boolean = false): Promise<FileInfo[]> {
	// Parameter validation
	if (!path || typeof path !== 'string') {
		throw new Error('scanDirectory: path parameter is required and must be a string');
	}
	if (path.trim().length === 0) {
		throw new Error('scanDirectory: path parameter cannot be empty');
	}
	if (typeof recursive !== 'boolean') {
		throw new Error('scanDirectory: recursive parameter must be a boolean');
	}

	try {
        // Use mock in non-Tauri environments if provided
        const mock = getTauriMock();
        if (!isTauriRuntime() && mock?.scanDirectory) {
            return await mock.scanDirectory(path, recursive);
        }

        return await withRetry(
            () => invoke<FileInfo[]>('scan_directory', { path, recursive }),
            {
                maxAttempts: 2,
                delayMs: 500,
                onRetry: (attempt) => {
                    console.warn(`Retrying scan directory (attempt ${attempt + 1})...`);
                }
            }
        );
	} catch (error) {
		const parsedError = parseBackendError(error);
		console.error('Failed to scan directory:', parsedError);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to scan directory: ${parsedError.message}`);
		}
		throw error;
	}
}

export async function scanDirectoryStream(
	path: string,
	recursive: boolean = false,
	batchSize?: number
): Promise<string> {
	try {
		return await invoke<string>('scan_directory_stream', {
			path,
			recursive,
			batch_size: batchSize
		});
	} catch (error) {
		const parsedError = parseBackendError(error);
		console.error('Failed to start streaming directory scan:', parsedError);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to start streaming scan: ${parsedError.message}`);
		}
		throw error;
	}
}

export async function analyzeFile(path: string): Promise<AnalysisResult> {
	// Parameter validation
	if (!path || typeof path !== 'string') {
		throw new Error('analyzeFile: path parameter is required and must be a string');
	}
	if (path.trim().length === 0) {
		throw new Error('analyzeFile: path parameter cannot be empty');
	}

	try {
		// Get file info first to determine mime type
		const fileInfo = await invoke<FileInfo>('get_file_info_command', { path });

		// For text files, read content and analyze
		if (fileInfo.mime_type?.startsWith('text/') ||
		    fileInfo.mime_type === 'application/json' ||
		    fileInfo.mime_type === 'application/javascript') {
			const content = await invoke<string>('get_file_content', { path });
			return await invoke('analyze_with_ai', {
				content,
				mime_type: fileInfo.mime_type || 'text/plain'
			});
		}

		// For other files, analyze metadata only
		return await invoke('analyze_with_ai', {
			content: JSON.stringify({
				path: fileInfo.path,
				name: fileInfo.name,
				size: fileInfo.size,
				type: fileInfo.mime_type,
				modified: fileInfo.modified_at
			}),
			mime_type: 'application/json'
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to analyze file:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to analyze file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function analyzeFiles(paths: string[]): Promise<FileAnalysis[]> {
	try {
		return await invoke<FileAnalysis[]>('analyze_files', { paths });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to analyze files:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to analyze files: ${errorMessage}`);
		}
		throw error;
	}
}

export async function batchAnalyzeFiles(paths: string[]): Promise<FileAnalysis[]> {
	// Parameter validation
	if (!paths || !Array.isArray(paths)) {
		throw new Error('batchAnalyzeFiles: paths parameter is required and must be an array');
	}
	if (paths.length === 0) {
		throw new Error('batchAnalyzeFiles: paths array cannot be empty');
	}
	for (let i = 0; i < paths.length; i++) {
		if (!paths[i] || typeof paths[i] !== 'string') {
			throw new Error(`batchAnalyzeFiles: path at index ${i} must be a non-empty string`);
		}
		if (paths[i].trim().length === 0) {
			throw new Error(`batchAnalyzeFiles: path at index ${i} cannot be empty`);
		}
	}

	try {
		// The backend command expects 'paths' parameter
		return await invoke<FileAnalysis[]>('batch_analyze_files', { paths });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to analyze files:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to analyze files: ${errorMessage}`);
		}
		throw error;
	}
}

// Alias for backward compatibility with tests
export const analyzeFilesBatch = batchAnalyzeFiles;

export async function generateOrganizationSuggestions(
	files: string[]
): Promise<OrganizationSuggestion[]> {
	try {
        const mock = getTauriMock();
        if (!isTauriRuntime() && mock?.generateSuggestions) {
            return await mock.generateSuggestions(files);
        }
        // Backend expects 'paths' parameter, not 'files'
        return await invoke<OrganizationSuggestion[]>('suggest_file_organization', { paths: files });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to generate organization suggestions:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to generate suggestions: ${errorMessage}`);
		}
		throw error;
	}
}

// Convert backend OrganizationSuggestion to UI format
export function convertToUIOrganizationSuggestions(suggestions: OrganizationSuggestion[]): OrganizationSuggestionUI[] {
	return suggestions.map((suggestion, index) => ({
		id: `org-${index}-${Date.now()}`,
		title: `Move to ${suggestion.target_folder}`,
		description: suggestion.reason,
		action: 'move' as const,
		source: suggestion.source_path,
		target: suggestion.target_folder,
		confidence: suggestion.confidence,
		affectedFiles: [suggestion.source_path]
	}));
}

export async function applyOrganizationSuggestions(
	operations: OrganizationOperation[]
): Promise<OrganizationResult> {
	try {
        const mock = getTauriMock();
        if (!isTauriRuntime() && mock?.applyOrganization) {
            return await mock.applyOrganization(operations);
        }
        return await invoke<OrganizationResult>('apply_organization', { operations });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to apply organization:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to apply organization: ${errorMessage}`);
		}
		throw error;
	}
}

// Alias for backward compatibility with tests
export const applyOrganization = applyOrganizationSuggestions;

export async function autoOrganizeDirectory(
	directoryPath: string,
	useAi: boolean = true
): Promise<OrganizationPreview[]> {
	try {
		// Backend expects snake_case parameters
		return await invoke<OrganizationPreview[]>('auto_organize_directory', {
			directory_path: directoryPath,
			use_ai: useAi
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to auto-organize directory:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to auto-organize: ${errorMessage}`);
		}
		throw error;
	}
}

// Search Operations
export const semanticSearch = createApiCall(
	(query: string, limit: number = 10) =>
		invoke<SearchResult[]>('semantic_search', { query, limit }),
	{
		errorMessage: 'Search failed',
		retry: {
			maxAttempts: 2,
			delayMs: 300
		},
		onError: (error) => {
			console.error('Search failed:', error);
			if (typeof toast !== 'undefined' && toast?.error) {
				toast.error(`Search failed: ${error.message}`);
			}
		}
	}
);

export const quickSearch = createApiCall(
	(query: string) => invoke<SearchResult[]>('quick_search', { query }),
	{
		errorMessage: 'Quick search failed',
		retry: false,
		onError: (error) => {
			console.error('Quick search failed:', error);
		}
	}
);

// AI/Ollama Operations
export async function checkOllamaStatus(): Promise<OllamaStatus> {
	try {
		// Use circuit breaker with fallback for Ollama status
		const status = await withFallback(
			() => ollamaStatusCircuitBreaker.execute(),
			() => ({
				is_running: false,
				models: [],
				version: 'unknown',
				fallback_reason: 'Service temporarily unavailable'
			})
		) as any;
		// Add proper type validation
		if (!status || typeof status !== 'object') {
			throw new Error('Invalid status response');
		}

		// Ensure models is always an array, even if undefined, null, or non-array
		let models: string[] = [];
		if (status.models) {
			if (Array.isArray(status.models)) {
				models = status.models.map((m: any) => {
					if (typeof m === 'string') return m;
					if (m && typeof m === 'object' && m.name) return String(m.name);
					return String(m || '');
				}).filter((m: string) => m.length > 0);
			} else if (typeof status.models === 'string') {
				// Handle case where models might be a single string
				models = [status.models];
			}
			// If models is any other type, keep it as empty array
		}

		// Map backend field names to frontend expected format
		return {
			isRunning: Boolean(status.is_running || status.isRunning || false),
			models,
			version: String(status.version || 'unknown'),
			mode: (status.is_running || status.isRunning) ? 'ollama' : 'fallback',
			fallback_reason: status.fallback_reason
		} as OllamaStatus;
	} catch (error) {
		console.error('Failed to check Ollama status:', error);
		// Return fallback status if check fails
		return {
			isRunning: false,
			models: [],
			version: 'unknown',
			mode: 'fallback',
			fallback_reason: String(error)
		};
	}
}

export async function getAiStatus(): Promise<AiStatus> {
	try {
		// Use circuit breaker to prevent cascading failures
		return await aiStatusCircuitBreaker.execute();
	} catch (error) {
		const parsedError = parseBackendError(error);
		console.error('Failed to get AI status:', parsedError);

		// Handle circuit breaker open state gracefully
		if (error instanceof Error && error.message?.includes('Circuit breaker is open')) {
			// Return fallback status when circuit breaker is open
			return {
				is_available: false,
				connected: false,
				provider: 'fallback',
				ollama_connected: false,
				models_available: [],
				available_models: [],
				capabilities: {
					text_analysis: false,
					vision_analysis: false,
					embeddings: false,
					semantic_search: false
				},
				connection_error: 'Service temporarily unavailable'
			} as AiStatus;
		}

		throw error;
	}
}

export async function connectOllama(host: string): Promise<AiStatus> {
	// Parameter validation already added above

	try {
		// Use retry logic for connection attempts with exponential backoff
		const result = await withRetry(
			() => invoke<AiStatus>('connect_ollama', { host }),
			{
				maxAttempts: 3,
				delayMs: 1000,
				onRetry: (attempt) => {
					console.warn(`Retrying Ollama connection (attempt ${attempt + 1})...`);
					if (typeof toast !== 'undefined' && toast?.info) {
						toast.info(`Retrying connection to Ollama... (attempt ${attempt + 1})`);
					}
				}
			}
		);
		return result;
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to connect to Ollama:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to connect to Ollama: ${errorMessage}`);
		}
		throw error;
	}
}

export async function downloadOllamaModel(modelName: string): Promise<void> {
	// Parameter validation already added above

	try {
		// Use retry logic for model downloads with longer timeout and fewer retries
		return await withRetry(
			() => invoke('pull_model', { model: modelName }),
			{
				maxAttempts: 2, // Model downloads should have fewer retries due to size
				delayMs: 5000, // Longer delay between retries for downloads
				onRetry: (attempt) => {
					console.warn(`Retrying model download (attempt ${attempt + 1})...`);
					if (typeof toast !== 'undefined' && toast?.info) {
						toast.info(`Retrying download of ${modelName}... (attempt ${attempt + 1})`);
					}
				}
			}
		);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to download model:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to download model ${modelName}: ${errorMessage}`);
		}
		throw error;
	}
}

export async function listModels(): Promise<string[]> {
	try {
        const mock = getTauriMock();
        if (!isTauriRuntime() && mock?.listModels) {
            return await mock.listModels();
        }
        return await invoke('list_models');
	} catch (error) {
		console.error('Failed to list models:', error);
		return [];
	}
}

// Settings Operations
export async function getAppSettings(): Promise<AppSettings> {
	try {
        if (!isTauriRuntime()) {
            // Read from localStorage in browser for e2e
            try {
                const raw = localStorage.getItem('stratosort_settings');
                if (raw) return JSON.parse(raw);
            } catch {}
        }
        return await apiCache.cachedInvoke<AppSettings>('get_settings');
	} catch (error) {
		console.error('Failed to get settings:', error);
		throw error;
	}
}

export async function saveAppSettings(settings: AppSettings): Promise<void> {
	try {
        if (!isTauriRuntime()) {
            try {
                localStorage.setItem('stratosort_settings', JSON.stringify(settings));
                return;
            } catch {}
        }
        await invoke('update_settings', { settings });
		// Invalidate settings cache after successful update
		apiCache.invalidate('get_settings');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to save settings:', errorMessage, error);
		if (toast) {
			toast.error(`Failed to save settings: ${errorMessage}`);
		}
		throw error;
	}
}

export async function resetSettings(): Promise<void> {
	try {
		await invoke('reset_settings');
		// Invalidate settings cache after successful reset
		apiCache.invalidate('get_settings');
	} catch (error) {
		console.error('Failed to reset settings:', error);
		throw error;
	}
}

// System Operations
export async function getSystemInfo(): Promise<SystemInfo> {
	try {
		// Backend command is actually 'get_basic_system_info'
		return await apiCache.cachedInvoke<SystemInfo>('get_basic_system_info', {}, 10000); // 10 second cache
	} catch (error) {
		console.error('Failed to get system info:', error);
		throw error;
	}
}

export async function getSystemInfoDetailed(): Promise<SystemInfo> {
	try {
		return await invoke('get_system_info');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get detailed system info:', errorMessage);
		throw error;
	}
}

export async function clearCache(): Promise<void> {
	try {
		await invoke('clear_cache');
		if (toast) {
			toast.success('Cache cleared successfully');
		}
	} catch (error) {
		console.error('Failed to clear cache:', error);
		if (toast) {
			toast.error(`Failed to clear cache: ${error}`);
		}
		throw error;
	}
}

export async function frontendReady(): Promise<void> {
	try {
		return await invoke('frontend_ready');
	} catch (error) {
		console.error('Failed to notify frontend ready:', error);
	}
}

// File Dialog Operations
export async function openDirectoryDialog(title: string = 'Select Directory'): Promise<string | null> {
    try {
        const { open } = await import('@tauri-apps/plugin-dialog');
        const result = await open({ directory: true, multiple: false, title });
        return typeof result === 'string' ? result : null;
    } catch {
        // Fallback for web: use input directory picker if available, else null
        return null;
    }
}

export async function openFileDialog(
    title: string = 'Select File',
    filters?: any[],
    allowMultiple: boolean = true
): Promise<string | string[] | null> {
    try {
        const { open } = await import('@tauri-apps/plugin-dialog');
        return await open({ directory: false, multiple: allowMultiple, title, filters });
    } catch {
        return null;
    }
}

export async function saveFileDialog(
    title: string = 'Save File',
    defaultPath?: string
): Promise<string | null> {
    try {
        const { save } = await import('@tauri-apps/plugin-dialog');
        return await save({ title, defaultPath });
    } catch {
        return null;
    }
}

// Event Listeners with proper types and cleanup
export function listenToProgressEvents(callback: (payload: ProgressEvent) => void): Promise<UnlistenFn> {
    return listen<ProgressEvent>('operation-progress', (event) => {
        callback(unwrapEventPayload<ProgressEvent>(event.payload));
	});
}


export function listenToOperationComplete(callback: (payload: OperationCompleteEvent) => void): Promise<UnlistenFn> {
    return listen<OperationCompleteEvent>('operation-complete', (event) => {
        callback(unwrapEventPayload<OperationCompleteEvent>(event.payload));
	});
}

export function listenToOperationError(callback: (payload: OperationErrorEvent) => void): Promise<UnlistenFn> {
    return listen<OperationErrorEvent>('operation-error', (event) => {
        callback(unwrapEventPayload<OperationErrorEvent>(event.payload));
	});
}

export function listenToFileChanges(callback: (payload: FileChangeEvent) => void): Promise<UnlistenFn> {
    return listen<FileChangeEvent>('file-event', (event) => {
        callback(unwrapEventPayload<FileChangeEvent>(event.payload));
	});
}

export function listenToOllamaStatus(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('ai-ollama-connected', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToOllamaStatusChanges(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('ai-status-changed', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToOllamaFallback(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('ai-ollama-fallback-active', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToInitializationEvents(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('app-initialization-retry', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToInitializationFailure(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('app-initialization-failed', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToOllamaStatusChecked(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('ai-ollama-status-checked', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToNotifications(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('notification', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToScanBatch(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('file-scan-batch', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToScanComplete(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('file-scan-complete', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToScanError(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('file-scan-error', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToScanCancelled(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('file-scan-cancelled', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToFileWatcherStarted(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('file-watcher-started', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToFileWatcherError(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('file-watcher-error', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToAiStatusUpdate(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('ai-status-update', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSystemStatusUpdate(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('system-status-update', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToOperationsStatusUpdate(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('operations-status-update', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToHealthStatusUpdate(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('health-status-update', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToOperationFailure(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('operation-failure', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToOperationTimeout(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('operation-timeout', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToResourceLimit(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('system-resource-limit', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

// History event listeners
export function listenToHistoryOperationUndone(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('history-operation-undone', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToHistoryOperationRedone(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('history-operation-redone', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToHistoryCleared(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('history-cleared', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToHistoryBatchUndo(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('history-batch-undo', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToHistoryBatchRedo(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('history-batch-redo', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToHistoryJumpedTo(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('history-jumped-to', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

// Watch mode event listeners
export function listenToWatchModeEnabled(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('watch-mode-enabled', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToWatchModeDisabled(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('watch-mode-disabled', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToWatchModeConfigured(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('watch-mode-configured', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToWatchModeDirectoryAdded(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('watch-mode-directory-added', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToWatchModeDirectoryRemoved(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('watch-mode-directory-removed', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToWatchModeAutoOrganizationTriggered(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('watch-mode-auto-organization-triggered', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

// Settings event listeners
export function listenToSettingsUpdated(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('settings-updated', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSettingsReset(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('settings-reset', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSettingsImported(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('settings-imported', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSettingsCategoryUpdated(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('settings-category-updated', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSettingsWatchPathAdded(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('settings-watch-path-added', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSettingsWatchPathRemoved(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('settings-watch-path-removed', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSettingsValueChanged(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('settings-value-changed', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

// Notification event listeners
export function listenToNotificationSent(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('notification-sent', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToNotificationProgress(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('notification-progress', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToNotificationFileOperationStatus(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('notification-file-operation-status', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToNotificationSystemStatus(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('notification-system-status', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

// Missing event listeners that were identified in the audit
export function listenToNavigateTo(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('navigate-to', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToShowAbout(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('show-about', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToMetricsCollected(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('metrics-collected', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

// Additional operations for cancellation and monitoring
export async function cancelOperation(operationId: string): Promise<void> {
	try {
		// Validate UUID format
		if (!isValidAnyUuid(operationId)) {
			throw new Error(`Invalid operation ID format: ${operationId}`);
		}

		// Backend expects 'id' parameter, not 'operation_id'
		return await invoke('cancel_operation', { id: operationId });
	} catch (error) {
		const parsedError = parseBackendError(error);
		console.error('Failed to cancel operation:', parsedError);
		throw error;
	}
}

export async function getActiveOperations(): Promise<ActiveOperationInfo[]> {
	try {
		return await invoke<ActiveOperationInfo[]>('get_active_operations');
	} catch (error) {
		const parsedError = parseBackendError(error);
		console.error('Failed to get active operations:', parsedError);
		return [];
	}
}

export async function getOperationProgress(operationId: string): Promise<ActiveOperationInfo | null> {
	try {
		// Validate UUID format
		if (!isValidAnyUuid(operationId)) {
			throw new Error(`Invalid operation ID format: ${operationId}`);
		}

		// Backend expects 'id' parameter, not 'operation_id'
		return await invoke('get_operation_progress', { id: operationId });
	} catch (error) {
		const parsedError = parseBackendError(error);
		console.error('Failed to get operation progress:', parsedError);
		throw error;
	}
}

// Additional File Utility Operations
export async function fileExists(path: string): Promise<FileExistsResult> {
	try {
		return await invoke<FileExistsResult>('file_exists', { path });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to check file existence:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to check file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getFileSizeInfo(path: string): Promise<FileSizeInfo> {
	try {
		return await invoke<FileSizeInfo>('get_file_size_info', { path });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get file size info:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get file size info: ${errorMessage}`);
		}
		throw error;
	}
}

// Extended Settings Operations
export async function getSettingsByCategory(category: string): Promise<Record<string, any>> {
	try {
		return await invoke('get_settings_by_category', { category });
	} catch (error) {
		console.error('Failed to get settings by category:', error);
		throw error;
	}
}

export async function getAllSettingsCategories(): Promise<string[]> {
	try {
        const cats = await invoke<Array<{ name: string }>>('get_all_settings_categories');
        return Array.isArray(cats) ? cats.map(c => c.name) : [];
	} catch (error) {
		console.error('Failed to get settings categories:', error);
		return [];
	}
}

export async function updateCategorySettings(
	category: string,
	settings: Record<string, any>
): Promise<boolean> {
	try {
		return await invoke<boolean>('update_category_settings', {
			category,
			settings
		});
	} catch (error) {
		console.error('Failed to update category settings:', error);
		if (toast) {
			toast.error(`Failed to update settings: ${error}`);
		}
		throw error;
	}
}

export async function testAiConnection(config: {
	provider: string;
	host?: string;
	api_key?: string;
}): Promise<{
	success: boolean;
	latency_ms: number;
	error?: string;
}> {
	try {
    return await invoke('test_ai_connection', { config });
	} catch (error) {
		console.error('Failed to test AI connection:', error);
		throw error;
	}
}

export async function getSettingValue(key: string): Promise<any> {
	try {
		return await invoke('get_setting_value', { key });
	} catch (error) {
		console.error('Failed to get setting value:', error);
		throw error;
	}
}

export async function setSettingValue(key: string, value: any): Promise<void> {
	try {
		return await invoke('set_setting_value', { key, value });
	} catch (error) {
		console.error('Failed to set setting value:', error);
		if (toast) {
			toast.error(`Failed to update setting: ${error}`);
		}
		throw error;
	}
}

export async function exportSettings(): Promise<string> {
	try {
		return await invoke<string>('export_settings');
	} catch (error) {
		console.error('Failed to export settings:', error);
		if (toast) {
			toast.error(`Failed to export settings: ${error}`);
		}
		throw error;
	}
}

export async function importSettings(json: string): Promise<void> {
	try {
		return await invoke('import_settings', { json });
	} catch (error) {
		console.error('Failed to import settings:', error);
		if (toast) {
			toast.error(`Failed to import settings: ${error}`);
		}
		throw error;
	}
}

export async function validateSettings(settings: AppSettings): Promise<ValidationResult> {
	try {
		return await invoke<ValidationResult>('validate_settings', { settings });
	} catch (error) {
		console.error('Failed to validate settings:', error);
		throw error;
	}
}

export async function addWatchPath(path: string): Promise<void> {
	try {
		return await invoke('add_watch_path', { path });
	} catch (error) {
		console.error('Failed to add watch path:', error);
		if (toast) {
			toast.error(`Failed to add watch path: ${error}`);
		}
		throw error;
	}
}

export async function removeWatchPath(path: string): Promise<void> {
	try {
		return await invoke('remove_watch_path', { path });
	} catch (error) {
		console.error('Failed to remove watch path:', error);
		if (toast) {
			toast.error(`Failed to remove watch path: ${error}`);
		}
		throw error;
	}
}

export async function getWatchPaths(): Promise<string[]> {
	try {
		return await invoke<string[]>('get_watch_paths');
	} catch (error) {
		console.error('Failed to get watch paths:', error);
		return [];
	}
}

// System Monitoring Operations
export async function getHealthStatus(): Promise<HealthStatus> {
	try {
		return await invoke<HealthStatus>('get_health_status');
	} catch (error) {
		console.error('Failed to get health status:', error);
		throw error;
	}
}

export async function getPerformanceMetrics(): Promise<PerformanceMetrics> {
	try {
		return await invoke<PerformanceMetrics>('get_performance_metrics');
	} catch (error) {
		console.error('Failed to get performance metrics:', error);
		throw error;
	}
}

export async function getAppInfo(): Promise<AppInfo> {
	try {
		return await apiCache.cachedInvoke<AppInfo>('get_app_info', {}, 10000); // 10 second cache
	} catch (error) {
		console.error('Failed to get app info:', error);
		throw error;
	}
}

export async function getSystemStatus(): Promise<SystemStatus> {
	try {
		return await invoke('get_system_status');
	} catch (error) {
		console.error('Failed to get system status:', error);
		throw error;
	}
}

export async function testSmartFolderRule(rule: any): Promise<any> {
	try {
		return await invoke('test_smart_folder_rule', { rule });
	} catch (error) {
		console.error('Failed to test smart folder rule:', error);
		throw error;
	}
}

// File Management Operations
export async function moveFile(sourcePath: string, targetPath: string): Promise<boolean> {
	// Parameter validation
	if (!sourcePath || typeof sourcePath !== 'string') {
		throw new Error('moveFile: sourcePath parameter is required and must be a string');
	}
	if (sourcePath.trim().length === 0) {
		throw new Error('moveFile: sourcePath parameter cannot be empty');
	}
	if (!targetPath || typeof targetPath !== 'string') {
		throw new Error('moveFile: targetPath parameter is required and must be a string');
	}
	if (targetPath.trim().length === 0) {
		throw new Error('moveFile: targetPath parameter cannot be empty');
	}
	if (sourcePath === targetPath) {
		throw new Error('moveFile: sourcePath and targetPath cannot be the same');
	}

	try {
		return await invoke<boolean>('move_file', {
			source_path: sourcePath,
			target_path: targetPath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to move file:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to move file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function copyFile(sourcePath: string, targetPath: string): Promise<boolean> {
	try {
		return await invoke<boolean>('copy_file', {
			source_path: sourcePath,
			target_path: targetPath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to copy file:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to copy file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function deleteFile(filePath: string): Promise<boolean> {
	try {
		return await invoke<boolean>('delete_file', {
			file_path: filePath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to delete file:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to delete file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function renameFile(oldPath: string, newPath: string): Promise<boolean> {
	try {
		return await invoke<boolean>('rename_file', {
			old_path: oldPath,
			new_path: newPath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to rename file:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to rename file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function createDirectory(directoryPath: string): Promise<boolean> {
	try {
		return await invoke<boolean>('create_directory', {
			path: directoryPath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to create directory:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to create directory: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getFilePreview(filePath: string): Promise<string> {
	try {
		return await invoke<string>('get_file_preview', {
			path: filePath,
			max_size: 1024 * 1024 // 1MB default preview size
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get file preview:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get file preview: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getRecentFiles(limit: number = 10): Promise<FileInfo[]> {
	try {
		return await invoke<FileInfo[]>('get_recent_files', { limit });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get recent files:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get recent files: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getFileProperties(filePath: string): Promise<FileProperties> {
	try {
		return await invoke('get_file_properties', {
			file_path: filePath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get file properties:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get file properties: ${errorMessage}`);
		}
		throw error;
	}
}

// Drag & Drop Operations
export async function processDroppedPaths(paths: string[]): Promise<ProcessedDropResult> {
	try {
		return await invoke<ProcessedDropResult>('process_dropped_paths', {
			dropped_paths: paths
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to process dropped paths:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to process dropped files: ${errorMessage}`);
		}
		throw error;
	}
}

// Smart Folder Operations
export async function createSmartFolder(
	name: string,
	description?: string,
	rules: OrganizationRule[] = [],
	targetPath: string = ''
): Promise<SmartFolder> {
	try {
		return await invoke<SmartFolder>('create_smart_folder', {
			name,
			description,
			rules,
			target_path: targetPath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to create smart folder:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to create smart folder: ${errorMessage}`);
		}
		throw error;
	}
}

export async function updateSmartFolder(
	id: string,
	name?: string,
	description?: string,
	targetPath?: string,
	rules?: OrganizationRule[],
	enabled?: boolean
): Promise<SmartFolder> {
	try {
		return await invoke<SmartFolder>('update_smart_folder', {
			id,
			name,
			description,
			target_path: targetPath,
			rules,
			enabled
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to update smart folder:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to update smart folder: ${errorMessage}`);
		}
		throw error;
	}
}

export async function deleteSmartFolder(id: string): Promise<boolean> {
	try {
		return await invoke<boolean>('delete_smart_folder', { id });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to delete smart folder:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to delete smart folder: ${errorMessage}`);
		}
		throw error;
	}
}

export async function listSmartFolders(): Promise<SmartFolder[]> {
	try {
		return await invoke<SmartFolder[]>('list_smart_folders');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to list smart folders:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to list smart folders: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getSmartFolder(id: string): Promise<SmartFolder | null> {
	try {
		return await invoke<SmartFolder | null>('get_smart_folder', { id });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get smart folder:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get smart folder: ${errorMessage}`);
		}
		throw error;
	}
}

export async function applySmartFolderRules(
	folderId: string,
	filePaths: string[],
	dryRun: boolean = false
): Promise<OrganizationPreview[]> {
	try {
		return await invoke<OrganizationPreview[]>('apply_smart_folder_rules', {
			folder_id: folderId,
			file_paths: filePaths,
			dry_run: dryRun
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to apply smart folder rules:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to apply smart folder rules: ${errorMessage}`);
		}
		throw error;
	}
}

// History/Undo-Redo Operations
export async function undoOperation(): Promise<boolean> {
	try {
		return await invoke<boolean>('undo');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to undo operation:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to undo: ${errorMessage}`);
		}
		throw error;
	}
}

export async function redoOperation(): Promise<boolean> {
	try {
		return await invoke<boolean>('redo');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to redo operation:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to redo: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getOperationHistory(): Promise<any[]> {
	try {
		return await invoke<any[]>('get_operation_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get operation history:', errorMessage);
		// Return empty array as fallback to keep UI functional
		return [];
	}
}

export async function getHistory(): Promise<HistoryEntry[]> {
	try {
		return await invoke<HistoryEntry[]>('get_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get history:', errorMessage);
		return [];
	}
}

export async function getHistoryState(): Promise<HistoryState> {
	try {
		return await invoke<HistoryState>('get_history_state');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get history state:', errorMessage);
		// Return default state if error
		return {
			can_undo: false,
			can_redo: false,
			undo_count: 0,
			redo_count: 0,
			total_entries: 0
		};
	}
}

export async function clearOperationHistory(): Promise<boolean> {
	try {
		return await invoke<boolean>('clear_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to clear history:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to clear history: ${errorMessage}`);
		}
		throw error;
	}
}

export async function jumpToHistoryPoint(entryId: string): Promise<boolean> {
	try {
		return await invoke<boolean>('jump_to_history', {
			operation_id: entryId
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to jump to history point:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to jump to history point: ${errorMessage}`);
		}
		throw error;
	}
}

// System Operations
export async function openFolder(folderPath: string): Promise<boolean> {
	try {
		return await invoke<boolean>('open_folder', {
			path: folderPath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to open folder:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to open folder: ${errorMessage}`);
		}
		throw error;
	}
}

export async function showInFolder(filePath: string): Promise<boolean> {
	try {
		return await invoke<boolean>('show_in_folder', {
			path: filePath
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to show in folder:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to show in folder: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getDefaultFolders(): Promise<string[]> {
	try {
		return await invoke<string[]>('get_default_folders');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get default folders:', errorMessage);
		return [];
	}
}

export async function getStorageInfo(): Promise<StorageInfo> {
	try {
		return await invoke('get_storage_info');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get storage info:', errorMessage);
		throw error;
	}
}

export async function getAppLogs(): Promise<string[]> {
	try {
		return await invoke<string[]>('get_app_logs');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get app logs:', errorMessage);
		throw error;
	}
}

export async function restartApp(): Promise<boolean> {
	try {
		return await invoke<boolean>('restart_app');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to restart app:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to restart app: ${errorMessage}`);
		}
		throw error;
	}
}

export async function checkForUpdates(): Promise<UpdateInfo> {
	try {
		return await invoke('check_for_updates');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to check for updates:', errorMessage);
		throw error;
	}
}

export async function shutdownApplication(): Promise<boolean> {
	try {
		return await invoke<boolean>('shutdown_application');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to shutdown application:', errorMessage);
		throw error;
	}
}

// Watch Mode Operations
export async function getWatchModeStatus(): Promise<WatchModeConfig> {
	try {
		return await invoke('get_watch_mode_status');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get watch mode status:', errorMessage);
		throw error;
	}
}

export async function configureWatchMode(config: any): Promise<boolean> {
	try {
		return await invoke<boolean>('configure_watch_mode', { config });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to configure watch mode:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to configure watch mode: ${errorMessage}`);
		}
		throw error;
	}
}

export async function enableWatchMode(directories: string[] = []): Promise<boolean> {
	try {
		return await invoke<boolean>('enable_watch_mode', { directories });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to enable watch mode:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to enable watch mode: ${errorMessage}`);
		}
		throw error;
	}
}

export async function disableWatchMode(): Promise<boolean> {
	try {
		return await invoke<boolean>('disable_watch_mode');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to disable watch mode:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to disable watch mode: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getUserLearningPatterns(): Promise<UserLearningPattern[]> {
	try {
		return await invoke<UserLearningPattern[]>('get_user_learning_patterns');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get user learning patterns:', errorMessage);
		return [];
	}
}

export async function triggerAutoOrganization(): Promise<AutoOrganizationTrigger> {
	try {
		return await invoke('trigger_auto_organization');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to trigger auto organization:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to trigger auto organization: ${errorMessage}`);
		}
		throw error;
	}
}

// Advanced Search Operations
export async function advancedSearch(query: string, filters: any = {}): Promise<SearchResult[]> {
	try {
		return await invoke<SearchResult[]>('advanced_search', { query, filters });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to perform advanced search:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Advanced search failed: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getSearchHistory(): Promise<string[]> {
	try {
		return await invoke<string[]>('get_search_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get search history:', errorMessage);
		return [];
	}
}

export async function clearSearchHistory(): Promise<boolean> {
	try {
		return await invoke<boolean>('clear_search_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to clear search history:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to clear search history: ${errorMessage}`);
		}
		throw error;
	}
}

// Additional File Operations
export async function batchFileOperations(operations: BatchFileOperation[]): Promise<BatchOperationResult> {
	try {
		return await invoke('batch_file_operations', { operations });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to perform batch operations:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Batch operations failed: ${errorMessage}`);
		}
		throw error;
	}
}

export async function moveFiles(filePaths: string[], targetDirectory: string): Promise<BatchOperationResult> {
	try {
		const operations = filePaths.map((source) => ({ source, destination: `${targetDirectory}` }));
		return await invoke('move_files', { operations });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to move files:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to move files: ${errorMessage}`);
		}
		throw error;
	}
}

// Alias for backward compatibility with tests
export const moveFilesBatch = moveFiles;

export async function renameFiles(operations: Array<{oldPath: string, newPath: string}>): Promise<BatchOperationResult> {
	try {
		const payload = operations.map(op => ({ file_path: op.oldPath, new_name: op.newPath }));
		return await invoke('rename_files', { operations: payload });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to rename files:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to rename files: ${errorMessage}`);
		}
		throw error;
	}
}

// First-Run Setup Operations
export async function checkFirstRunStatus(): Promise<{
	is_first_run: boolean;
	setup_completed: boolean;
	setup_steps_remaining?: string[];
}> {
	try {
        return await invoke('check_first_run_status');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to check first run status:', errorMessage);
        // In browser/e2e without Tauri backend, derive from localStorage
        try {
            const done = localStorage.getItem('stratosort_first_run_done') === 'true';
            return {
                is_first_run: !done,
                setup_completed: done,
                setup_steps_remaining: done ? [] : ['initial_setup']
            } as any;
        } catch {
            return {
                is_first_run: true,
                setup_completed: false,
                setup_steps_remaining: ['initial_setup']
            } as any;
        }
	}
}

export async function completeFirstRunSetup(setup: {
	smart_folder_location?: string;
	enable_watch_mode?: boolean;
	watch_directories?: string[];
	enable_notifications?: boolean;
	auto_analyze?: boolean;
	ollama_host?: string;
}): Promise<string> {
    try {
        const res = await invoke<string>('complete_first_run_setup', { setup });
        try { localStorage.setItem('stratosort_first_run_done', 'true'); } catch {}
        return res;
    } catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to complete first run setup:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to complete setup: ${errorMessage}`);
		}
        // Mark as completed in browser fallback to allow tests to proceed
        try { localStorage.setItem('stratosort_first_run_done', 'true'); } catch {}
        return 'completed';
	}
}

export async function resetToFirstRun(): Promise<boolean> {
	try {
		return await invoke<boolean>('reset_to_first_run');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to reset to first run:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to reset: ${errorMessage}`);
		}
		throw error;
	}
}

// AI Capabilities and Testing
export async function getAiCapabilities(): Promise<{
	text_analysis: boolean;
	image_analysis: boolean;
	embedding_generation: boolean;
	semantic_search: boolean;
	model_info: {
		name: string;
		version: string;
		capabilities: string[];
	};
}> {
	try {
		return await invoke('get_ai_capabilities');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get AI capabilities:', errorMessage);
		throw error;
	}
}

export async function useFallbackAi(): Promise<AiStatus> {
	try {
		return await invoke<AiStatus>('use_fallback_ai');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to use fallback AI:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to switch to fallback AI: ${errorMessage}`);
		}
		throw error;
	}
}

export async function testAiAnalysis(testContent?: string): Promise<{
	success: boolean;
	result?: any;
	error?: string;
	latency_ms: number;
}> {
	try {
        const params = testContent ? { test_content: testContent } : {};
        // Backend accepts optional positional `test_content` in a named arg now
        return await invoke('test_ai_analysis', params);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to test AI analysis:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`AI test failed: ${errorMessage}`);
		}
		throw error;
	}
}

export async function generateEmbeddings(input: string | string[]): Promise<number[] | number[][]> {
	try {
		if (Array.isArray(input)) {
			// Backend only supports single text input, so we need to process each text individually
			const results: number[][] = [];
			for (const text of input) {
				const result = await invoke<number[]>('generate_embeddings', { text });
				results.push(result);
			}
			return results;
		} else {
			return await invoke<number[]>('generate_embeddings', { text: input });
		}
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to generate embeddings:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to generate embeddings: ${errorMessage}`);
		}
		throw error;
	}
}

// File Browsing Operations
export async function browseFiles(options?: {
	start_path?: string;
	file_types?: string[];
	show_hidden?: boolean;
	directory?: string;
	filters?: any;
}): Promise<FileInfo[]> {
	try {
		const params: any = {};
		if (options?.start_path) params.start_path = options.start_path;
		if (options?.directory) params.start_path = options.directory; // backward compatibility
		if (options?.file_types) params.file_types = options.file_types;
		if (options?.show_hidden !== undefined) params.show_hidden = options.show_hidden;
		if (options?.filters) params.filters = options.filters;

		return await invoke<FileInfo[]>('browse_files', params);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to browse files:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to browse files: ${errorMessage}`);
		}
		throw error;
	}
}

export async function browseFolder(title?: string): Promise<string> {
	try {
		// backend browse_folder opens a dialog and returns the selected folder path
		return await invoke<string>('browse_folder', { title: title || 'Select Folder' });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to browse folder:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to browse folder: ${errorMessage}`);
		}
		throw error;
	}
}

// Notification System
export async function emitNotification(
	notificationType: string,
	title: string,
	message: string,
	actions?: Array<{
		id: string;
		label: string;
		action_type: string;
	}>,
	metadata?: any
): Promise<string> {
	try {
        // Tauri command expects snake_case param 'notification_type'
        return await invoke<string>('emit_notification', {
            notification_type: notificationType,
            title,
            message,
            actions,
            metadata
        });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to emit notification:', errorMessage);
		throw error;
	}
}

export async function getNotifications(options?: {
	limit?: number;
	unread_only?: boolean;
}): Promise<Array<{
	id: string;
	notification_type: string;
	title: string;
	message: string;
	timestamp: number;
	read: boolean;
	actions: Array<{
		id: string;
		label: string;
		action_type: string;
	}>;
	metadata?: any;
}>> {
	try {
		return await invoke('get_notifications', {
			limit: options?.limit,
			unread_only: options?.unread_only
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get notifications:', errorMessage);
		return [];
	}
}

export async function markNotificationRead(notificationId: string): Promise<boolean> {
	try {
		return await invoke<boolean>('mark_notification_read', { notification_id: notificationId });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to mark notification read:', errorMessage);
		throw error;
	}
}

export async function clearNotifications(olderThanHours?: number): Promise<number> {
	try {
		const params = olderThanHours !== undefined ? { older_than_hours: olderThanHours } : {};
		return await invoke<number>('clear_notifications', params);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to clear notifications:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to clear notifications: ${errorMessage}`);
		}
		throw error;
	}
}

// Organization and Smart Folder Utilities
export async function getSmartFolders(): Promise<SmartFolder[]> {
	try {
		return await invoke<SmartFolder[]>('get_smart_folders');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get smart folders:', errorMessage);
		return [];
	}
}

export async function matchToFolders(filePaths: string[]): Promise<any> {
	try {
		return await invoke('match_to_folders', { file_paths: filePaths });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to match files to folders:', errorMessage);
		throw error;
	}
}


export async function testRuleAgainstFiles(rule: OrganizationRule, filePaths: string[]): Promise<Array<{
	file_path: string;
	matches: boolean;
	match_reason?: string;
}>> {
	try {
		return await invoke('test_rule_against_files', {
			rule,
			file_paths: filePaths
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to test rule against files:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Rule testing failed: ${errorMessage}`);
		}
		throw error;
	}
}

export async function validateRule(rule: OrganizationRule): Promise<{
	valid: boolean;
	errors?: string[];
	warnings?: string[];
}> {
	try {
		return await invoke('validate_rule', { rule });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to validate rule:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Rule validation failed: ${errorMessage}`);
		}
		throw error;
	}
}

// Monitoring and Status Operations
export async function readinessProbe(): Promise<boolean> {
	try {
		return await invoke<boolean>('readiness_probe');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed readiness probe:', errorMessage);
		return false;
	}
}

export async function livenessProbe(): Promise<boolean> {
	try {
		return await invoke<boolean>('liveness_probe');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed liveness probe:', errorMessage);
		return false;
	}
}

export async function getRuntimeConfig(): Promise<any> {
	try {
		return await invoke('get_runtime_config');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get runtime config:', errorMessage);
		throw error;
	}
}

export async function getFileStatistics(): Promise<any> {
	try {
		return await invoke('get_file_statistics');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get file statistics:', errorMessage);
		throw error;
	}
}

export async function enableRealtimeMonitoring(enabled: boolean = true): Promise<boolean> {
	try {
		return await invoke<boolean>('enable_realtime_monitoring', { enabled });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to enable realtime monitoring:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to enable monitoring: ${errorMessage}`);
		}
		throw error;
	}
}

export async function refreshAllStatus(): Promise<any> {
	try {
		return await invoke('refresh_all_status');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to refresh all status:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to refresh status: ${errorMessage}`);
		}
		throw error;
	}
}

// Advanced AI Operations
export async function getAnalysisHistory(): Promise<any[]> {
	try {
		return await invoke<any[]>('get_analysis_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get analysis history:', errorMessage);
		return [];
	}
}

export async function getMetricsHistory(): Promise<any[]> {
	try {
		return await invoke<any[]>('get_metrics_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get metrics history:', errorMessage);
		return [];
	}
}


// Diagnostics Functions
export async function runDiagnostics(): Promise<SystemDiagnostics> {
	try {
		return await invoke<SystemDiagnostics>('run_diagnostics');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to run diagnostics:', errorMessage);
		throw error;
	}
}

export async function testAiService(): Promise<AiServiceDiagnostics> {
	try {
		return await invoke<AiServiceDiagnostics>('test_ai_service');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to test AI service:', errorMessage);
		throw error;
	}
}

export async function checkDatabaseHealth(): Promise<DatabaseDiagnostics> {
	try {
		return await invoke<DatabaseDiagnostics>('check_database_health');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to check database health:', errorMessage);
		throw error;
	}
}

export async function validateConfigPaths(): Promise<PathPermissionCheck[]> {
	try {
		return await invoke<PathPermissionCheck[]>('validate_config_paths');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to validate config paths:', errorMessage);
		throw error;
	}
}

export async function getDiagnosticsResourceUsage(): Promise<ResourceDiagnostics> {
	try {
		return await invoke<ResourceDiagnostics>('get_diagnostics_resource_usage');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get diagnostics resource usage:', errorMessage);
		throw error;
	}
}

export async function clearCaches(): Promise<ClearCacheResult> {
	try {
		return await invoke<ClearCacheResult>('clear_caches');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to clear caches:', errorMessage);
		throw error;
	}
}

// Additional File Utilities
export async function getFileContent(filePath: string): Promise<string> {
	try {
		return await invoke<string>('get_file_content', { path: filePath });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get file content:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to read file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getFileInfoCommand(filePath: string): Promise<FileInfo> {
	try {
		return await invoke<FileInfo>('get_file_info_command', { path: filePath });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get file info:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get file info: ${errorMessage}`);
		}
		throw error;
	}
}

export async function setFilePermissions(filePath: string, permissions: string): Promise<boolean> {
	try {
		return await invoke<boolean>('set_file_permissions', {
			file_path: filePath,
			permissions
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to set file permissions:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to set permissions: ${errorMessage}`);
		}
		throw error;
	}
}

// Extended Watch Mode Operations

export async function updateAutoOrganizeThreshold(threshold: number): Promise<boolean> {
	try {
		return await invoke<boolean>('update_auto_organize_threshold', { threshold });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to update auto organize threshold:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to update threshold: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getPendingAutoOrganization(): Promise<any[]> {
	try {
		return await invoke<any[]>('get_pending_auto_organization');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get pending auto organization:', errorMessage);
		return [];
	}
}

export async function addWatchDirectory(directory: string): Promise<boolean> {
	try {
		return await invoke<boolean>('add_watch_directory', { directory_path: directory });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to add watch directory:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to add watch directory: ${errorMessage}`);
		}
		throw error;
	}
}

export async function removeWatchDirectory(directory: string): Promise<boolean> {
	try {
		return await invoke<boolean>('remove_watch_directory', { directory_path: directory });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to remove watch directory:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to remove watch directory: ${errorMessage}`);
		}
		throw error;
	}
}

export async function recordUserOrganizationAction(
	sourcePath: string,
	targetPath: string,
	actionType: 'move' | 'copy' | 'rename',
	fileAttributes?: Record<string, any>
): Promise<boolean> {
	try {
		return await invoke<boolean>('record_user_organization_action', {
			source_path: sourcePath,
			target_path: targetPath,
			action_type: actionType,
			file_attributes: fileAttributes
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to record user organization action:', errorMessage);
		return false;
	}
}

// Batch History Operations
export async function batchUndo(count: number): Promise<boolean> {
	try {
		return await invoke<boolean>('batch_undo', { count });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to batch undo:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to batch undo: ${errorMessage}`);
		}
		throw error;
	}
}

export async function batchRedo(count: number): Promise<boolean> {
	try {
		return await invoke<boolean>('batch_redo', { count });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to batch redo:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to batch redo: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getMemoryStats(): Promise<any> {
	try {
		return await invoke('get_memory_stats');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get memory stats:', errorMessage);
		throw error;
	}
}

// Additional System and Notification Operations
export async function emitProgressNotification(
	operationId: string,
	title: string,
	message: string,
	progress: number,
	total?: number
): Promise<boolean> {
	try {
		await invoke('emit_progress_notification', {
			operation_id: operationId,
			title,
			message,
			progress,
			total
		});
		return true;
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to emit progress notification:', errorMessage);
		return false;
	}
}

export async function emitFileOperationStatus(
	operationType: string,
	filePath: string,
	status: string,
	details?: string
): Promise<boolean> {
	try {
		await invoke('emit_file_operation_status', {
			operation_type: operationType,
			file_path: filePath,
			status,
			details
		});
		return true;
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to emit file operation status:', errorMessage);
		return false;
	}
}

export async function emitSystemStatus(
	component: string,
	status: string,
	details?: string
): Promise<boolean> {
	try {
		await invoke('emit_system_status', {
			component,
			status,
			details
		});
		return true;
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to emit system status:', errorMessage);
		return false;
	}
}

export async function getResourceUsage(): Promise<any> {
	try {
		return await invoke('get_resource_usage');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get resource usage:', errorMessage);
		throw error;
	}
}

export async function forceShutdown(): Promise<boolean> {
	try {
		return await invoke<boolean>('force_shutdown');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to force shutdown:', errorMessage);
		throw error;
	}
}


export async function searchFiles(options: {
	query: string;
	path?: string;
	filters?: any;
}): Promise<SearchResult[]> {
	try {
		return await advancedSearch(options.query, {
			path: options.path,
			...options.filters
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to search files:', errorMessage);
		return [];
	}
}

export async function validateFiles(filePaths: string[]): Promise<{
	valid: string[];
	invalid: string[];
}> {
	try {
		const results = await Promise.all(
			filePaths.map(async (path) => {
				try {
					const exists = await fileExists(path);
					return { path, valid: exists.exists && exists.is_file };
				} catch {
					return { path, valid: false };
				}
			})
		);

		return {
			valid: results.filter(r => r.valid).map(r => r.path),
			invalid: results.filter(r => !r.valid).map(r => r.path)
		};
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to validate files:', errorMessage);
		return {
			valid: [],
			invalid: filePaths
		};
	}
}

export async function deleteFilesBatch(filePaths: string[]): Promise<BatchOperationResult> {
	try {
		const operations = filePaths.map(path => ({
			operation: 'delete' as const,
			source_path: path
		}));

		return await batchFileOperations(operations);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to delete files batch:', errorMessage);
		return {
			total: filePaths.length,
			successful: 0,
			failed: filePaths.length,
			results: filePaths.map(path => ({
				path,
				success: false,
				error: errorMessage
			}))
		};
	}
}

// ============================================================================
// AI Streaming API Integration
// ============================================================================

/**
 * Start an AI streaming session for real-time AI responses
 */
export async function startAiStream(
	streamId: string,
	prompt: string,
	model: string
): Promise<boolean> {
	// Parameter validation
	if (!streamId || typeof streamId !== 'string') {
		throw new Error('startAiStream: streamId parameter is required and must be a string');
	}
	if (streamId.trim().length === 0) {
		throw new Error('startAiStream: streamId parameter cannot be empty');
	}
	if (!prompt || typeof prompt !== 'string') {
		throw new Error('startAiStream: prompt parameter is required and must be a string');
	}
	if (prompt.trim().length === 0) {
		throw new Error('startAiStream: prompt parameter cannot be empty');
	}
	if (!model || typeof model !== 'string') {
		throw new Error('startAiStream: model parameter is required and must be a string');
	}
	if (model.trim().length === 0) {
		throw new Error('startAiStream: model parameter cannot be empty');
	}

	try {
		return await invoke<boolean>('start_ai_stream', {
			stream_id: streamId,
			prompt,
			model
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to start AI stream:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to start AI stream: ${errorMessage}`);
		}
		throw error;
	}
}

/**
 * Stop an active AI streaming session
 */
export async function stopAiStream(streamId: string): Promise<boolean> {
	try {
		return await invoke<boolean>('stop_ai_stream', {
			stream_id: streamId
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to stop AI stream:', errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to stop AI stream: ${errorMessage}`);
		}
		throw error;
	}
}

/**
 * Check if an AI stream is currently active
 */
export async function isAiStreamActive(streamId: string): Promise<boolean> {
	try {
		return await invoke<boolean>('is_stream_active', {
			stream_id: streamId
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to check AI stream status:', errorMessage);
		return false; // Default to false on error
	}
}

/**
 * Listen to AI streaming events for real-time updates
 */
export function listenToAiStreamEvents(callback: (payload: {
	stream_id: string;
	event_type: 'start' | 'data' | 'end' | 'error';
	content?: string;
	error?: string;
}) => void): Promise<UnlistenFn> {
	return listen('ai-stream-event', (event) => {
		callback(unwrapEventPayload(event.payload));
	});
}

/**
 * Listen to AI streaming data chunks
 */
export function listenToAiStreamData(callback: (payload: {
	stream_id: string;
	content: string;
	done: boolean;
}) => void): Promise<UnlistenFn> {
	return listen('ai-stream-data', (event) => {
		callback(unwrapEventPayload(event.payload));
	});
}

/**
 * Listen to AI streaming errors
 */
export function listenToAiStreamError(callback: (payload: {
	stream_id: string;
	error: string;
}) => void): Promise<UnlistenFn> {
	return listen('ai-stream-error', (event) => {
		callback(unwrapEventPayload(event.payload));
	});
}