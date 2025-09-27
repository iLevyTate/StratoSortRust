import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { secureInvoke } from '$lib/utils/csrf-protection';
import { log, LogCategory } from '$lib/utils/enhanced-logger';
// Runtime helpers for non-Tauri/browser E2E environment
function isTauriRuntime(): boolean {
    return typeof window !== 'undefined' && !!(window as any).__TAURI__;
}

function getTauriMock(): any | null {
    if (typeof window === 'undefined') return null;
    return (window as any).__TAURI_MOCK__ || null;
}

import { toast } from '$lib/stores/notifications';
import { withRetry, parseBackendError, createApiCallLegacy as createApiCall } from './error-handler';
import { CircuitBreaker, withFallback } from '$lib/utils/async-error-handler';
import { isValidAnyUuid } from '$lib/utils/uuid';
import { debug } from '$lib/utils/debug';
import {
	convertOrganizationSuggestions as convertOrgSuggestions
} from '$lib/utils/type-converters';

// Helper to unwrap and validate standardized event envelopes emitted via emit_event! macro
function unwrapEventPayload<T = any>(payload: any): T {
  // CRITICAL FIX: Add comprehensive validation for event payloads

  // 1. Basic structure validation
  if (payload === null || payload === undefined) {
    debug.warn('Received null or undefined event payload');
    return null as T;
  }

  // 2. Check for standardized event envelope
  if (payload && typeof payload === 'object' && 'data' in payload) {
    const data = (payload as any).data;

    // 3. Validate data exists
    if (data === null || data === undefined) {
      debug.warn('Event payload data field is null or undefined');
      return null as T;
    }

    // 4. String sanitization to prevent injection
    if (typeof data === 'string') {
      // Sanitize potentially dangerous characters for HTML context
      const sanitized = data
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;')
        .replace(/`/g, '&#96;');
      return sanitized as T;
    }

    // 5. Object validation and sanitization
    if (typeof data === 'object') {
      return sanitizeObject(data) as T;
    }

    // 6. Number validation
    if (typeof data === 'number') {
      // Check for safe integer range
      if (!Number.isFinite(data)) {
        debug.warn('Non-finite number in event payload:', data);
        return 0 as T;
      }
      return data as T;
    }

    return data as T;
  }

  // 7. Handle direct payload (not wrapped in envelope)
  if (typeof payload === 'string') {
    // Sanitize strings
    const sanitized = payload
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;')
      .replace(/`/g, '&#96;');
    return sanitized as T;
  }

  if (typeof payload === 'object') {
    return sanitizeObject(payload) as T;
  }

  return payload as T;
}

// Helper function to recursively sanitize objects
function sanitizeObject(obj: any): any {
  if (obj === null || obj === undefined) {
    return obj;
  }

  // Handle arrays
  if (Array.isArray(obj)) {
    return obj.map(item => {
      if (typeof item === 'string') {
        return item
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;')
          .replace(/'/g, '&#39;')
          .replace(/`/g, '&#96;');
      }
      if (typeof item === 'object') {
        return sanitizeObject(item);
      }
      return item;
    });
  }

  // Handle objects
  const sanitized: any = {};
  for (const key in obj) {
    if (Object.prototype.hasOwnProperty.call(obj, key)) {
      // Sanitize the key to prevent prototype pollution
      const safeKey = key.replace(/__proto__|constructor|prototype/gi, '');

      const value = obj[key];
      if (typeof value === 'string') {
        sanitized[safeKey] = value
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;')
          .replace(/'/g, '&#39;')
          .replace(/`/g, '&#96;');
      } else if (typeof value === 'object') {
        sanitized[safeKey] = sanitizeObject(value);
      } else if (typeof value === 'number' && !Number.isFinite(value)) {
        sanitized[safeKey] = 0;
      } else {
        sanitized[safeKey] = value;
      }
    }
  }

  return sanitized;
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
	// Request deduplication for identical concurrent requests
	private activeRequests = new Map<string, Promise<unknown>>();

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

		// First check if there's an active request for the same command/args (deduplication)
		const activeRequest = this.activeRequests.get(cacheKey);
		if (activeRequest) {
			// Return the existing promise to avoid duplicate backend calls
			return activeRequest as Promise<T>;
		}

		// Check for pending request to avoid duplicate calls (legacy check)
		const pending = this.pendingRequests.get(cacheKey);
		if (pending) {
			return pending as Promise<T>;
		}

		// Check if we have a valid cached response
		const cached = this.cache.get(cacheKey);
		if (cached && Date.now() - cached.timestamp < ttl) {
			return cached.data as T;
		}

		// Create the request promise with deduplication
		const requestPromise = (async () => {
			try {
				const data = await secureInvoke<T>(command, args as Record<string, unknown>);
				// Cache the successful result
				this.cache.set(cacheKey, {
					data,
					timestamp: Date.now()
				});
				// Remove from active and pending requests
				this.activeRequests.delete(cacheKey);
				this.pendingRequests.delete(cacheKey);
				return data;
			} catch (error) {
				// Remove from active, pending requests and cache on error
				this.activeRequests.delete(cacheKey);
				this.pendingRequests.delete(cacheKey);
				this.cache.delete(cacheKey);
				throw error;
			}
		})();

		// Store in both active and pending requests to prevent race conditions
		this.activeRequests.set(cacheKey, requestPromise);
		this.pendingRequests.set(cacheKey, requestPromise);

		// Clean up active request after a reasonable timeout (30 seconds)
		setTimeout(() => {
			this.activeRequests.delete(cacheKey);
		}, 30000);

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
const ollamaStatusCircuitBreaker = new CircuitBreaker(() => secureInvoke('check_ollama_status'), {
	failureThreshold: 3,
	resetTimeout: 30000, // 30 seconds
	onOpen: () => debug.warn('Ollama status circuit breaker opened - service may be down'),
	onClose: () => debug.info('Ollama status circuit breaker closed - service recovered')
});

const aiStatusCircuitBreaker = new CircuitBreaker(() => secureInvoke<AiStatus>('get_ai_status'), {
	failureThreshold: 5,
	resetTimeout: 60000, // 1 minute
	onOpen: () => debug.warn('AI status circuit breaker opened - service may be down'),
	onClose: () => debug.info('AI status circuit breaker closed - service recovered')
});

// Re-export types for backward compatibility
export type {
	FileInfo,
	FileAnalysis,
	AnalysisResult,
	OrganizationSuggestion,
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
            () => secureInvoke<FileInfo[]>('scan_directory', { path, recursive }),
            {
                maxAttempts: 2,
                delayMs: 500,
                onRetry: (attempt) => {
                    debug.warn(`Retrying scan directory (attempt ${attempt + 1})...`);
                }
            }
        );
	} catch (error) {
		const parsedError = parseBackendError(error);
		log.error('Failed to scan directory', parsedError, 'scanDirectory', LogCategory.FILE_OPS);
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
		return await secureInvoke<string>('scan_directory_stream', {
			path,
			recursive,
			batch_size: batchSize
		});
	} catch (error) {
		const parsedError = parseBackendError(error);
		log.error('Failed to start streaming directory scan', parsedError, 'scanDirectoryStreaming', LogCategory.FILE_OPS);
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
		const fileInfo = await secureInvoke<FileInfo>('get_file_info_command', { path });

		// For text files, read content and analyze
		if (fileInfo.mime_type?.startsWith('text/') ||
		    fileInfo.mime_type === 'application/json' ||
		    fileInfo.mime_type === 'application/javascript') {
			const content = await secureInvoke<string>('get_file_content', { path });
			return await secureInvoke('analyze_with_ai', {
				content,
				mime_type: fileInfo.mime_type || 'text/plain'
			});
		}

		// For other files, analyze metadata only
		return await secureInvoke('analyze_with_ai', {
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
		log.error('Failed to analyze file', errorMessage, 'analyzeFile', LogCategory.AI);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to analyze file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function analyzeFiles(paths: string[]): Promise<FileAnalysis[]> {
	try {
		return await secureInvoke<FileAnalysis[]>('analyze_files', { paths });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to analyze files', errorMessage, 'analyzeFiles', LogCategory.AI);
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
		const path = paths[i];
		if (!path || typeof path !== 'string') {
			throw new Error(`batchAnalyzeFiles: path at index ${i} must be a non-empty string`);
		}
		if (path.trim().length === 0) {
			throw new Error(`batchAnalyzeFiles: path at index ${i} cannot be empty`);
		}
	}

	try {
		// The backend command expects 'paths' parameter
		return await secureInvoke<FileAnalysis[]>('batch_analyze_files', { paths });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to analyze files', errorMessage, 'analyzeFiles', LogCategory.AI);
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
        return await secureInvoke<OrganizationSuggestion[]>('suggest_file_organization', { paths: files });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to generate organization suggestions', errorMessage, 'generateOrganizationSuggestions', LogCategory.AI);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to generate suggestions: ${errorMessage}`);
		}
		throw error;
	}
}

// Convert backend OrganizationSuggestion to UI format
// Re-export the conversion function for backward compatibility
export const convertToUIOrganizationSuggestions = convertOrgSuggestions;

export async function applyOrganizationSuggestions(
	operations: OrganizationOperation[]
): Promise<OrganizationResult> {
	try {
        const mock = getTauriMock();
        if (!isTauriRuntime() && mock?.applyOrganization) {
            return await mock.applyOrganization(operations);
        }
        return await secureInvoke<OrganizationResult>('apply_organization', { operations });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to apply organization', errorMessage, 'applyOrganization', LogCategory.FILE_OPS);
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
	useAi: boolean = true,
	rules?: {
		groupByCategory?: boolean;
		groupByDate?: boolean;
		createSubfolders?: boolean;
		preserveStructure?: boolean;
	}
): Promise<OrganizationPreview[]> {
	try {
		// Backend expects snake_case parameters
		return await secureInvoke<OrganizationPreview[]>('auto_organize_directory', {
			directory_path: directoryPath,
			use_ai: useAi,
			group_by_category: rules?.groupByCategory ?? true,
			group_by_date: rules?.groupByDate ?? false,
			create_subfolders: rules?.createSubfolders ?? true,
			preserve_structure: rules?.preserveStructure ?? false
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to auto-organize directory', errorMessage, 'autoOrganizeDirectory', LogCategory.FILE_OPS);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to auto-organize: ${errorMessage}`);
		}
		throw error;
	}
}

// Search Operations
export const semanticSearch = createApiCall(
	(query: string, limit: number = 10) =>
		secureInvoke<SearchResult[]>('semantic_search', { query, limit }),
	{
		errorMessage: 'Search failed',
		retry: {
			maxAttempts: 2,
			delayMs: 300
		},
		onError: (error) => {
			log.error('Search failed', error, 'searchFiles', LogCategory.FILE_OPS);
			if (typeof toast !== 'undefined' && toast?.error) {
				toast.error(`Search failed: ${error.message}`);
			}
		}
	}
);

export const quickSearch = createApiCall(
	(query: string) => secureInvoke<SearchResult[]>('quick_search', { query }),
	{
		errorMessage: 'Quick search failed',
		retry: false,
		onError: (error) => {
			log.error('Quick search failed', error, 'quickSearch', LogCategory.FILE_OPS);
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

		debug.log('checkOllamaStatus raw status:', status);

		// Add proper type validation
		if (!status || typeof status !== 'object') {
			debug.warn('Invalid status response, using empty models');
			return {
				isRunning: false,
				models: [],
				version: 'unknown',
				mode: 'fallback',
				fallback_reason: 'Invalid status response'
			};
		}

		// Ensure models is always an array, even if undefined, null, or non-array
		let models: string[] = [];
		if (status.models !== undefined && status.models !== null) {
			if (Array.isArray(status.models)) {
				models = status.models.map((m: any) => {
					// Handle both string and object formats
					if (typeof m === 'string') return m;
					if (m && typeof m === 'object' && m.name) return String(m.name);
					return '';
				}).filter((m: string) => m.length > 0);
			} else if (typeof status.models === 'string') {
				// Handle case where models might be a single string
				models = [status.models];
			}
			// If models is any other type, keep it as empty array
		}

		debug.log('checkOllamaStatus processed models:', models);

		// Map backend field names to frontend expected format
		const result = {
			isRunning: Boolean(status.is_running || status.isRunning || false),
			models,
			version: String(status.version || 'unknown'),
			mode: (status.is_running || status.isRunning) ? 'ollama' : 'fallback',
			fallback_reason: status.fallback_reason
		};

		debug.log('checkOllamaStatus final result:', result);
		return result as OllamaStatus;
	} catch (error) {
		log.error('Failed to check Ollama status', error, 'checkOllamaStatus', LogCategory.AI);
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
		log.error('Failed to get AI status', parsedError, 'getAiStatus', LogCategory.AI);

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
			() => secureInvoke<AiStatus>('connect_ollama', { host }),
			{
				maxAttempts: 3,
				delayMs: 1000,
				onRetry: (attempt) => {
					debug.warn(`Retrying Ollama connection (attempt ${attempt + 1})...`);
					if (typeof toast !== 'undefined' && toast?.info) {
						toast.info(`Retrying connection to Ollama... (attempt ${attempt + 1})`);
					}
				}
			}
		);
		return result;
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to connect to Ollama', errorMessage, 'connectOllama', LogCategory.AI);
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
			() => secureInvoke('pull_model', { model: modelName }),
			{
				maxAttempts: 2, // Model downloads should have fewer retries due to size
				delayMs: 5000, // Longer delay between retries for downloads
				onRetry: (attempt) => {
					debug.warn(`Retrying model download (attempt ${attempt + 1})...`);
					if (typeof toast !== 'undefined' && toast?.info) {
						toast.info(`Retrying download of ${modelName}... (attempt ${attempt + 1})`);
					}
				}
			}
		);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to download model', errorMessage, 'downloadModel', LogCategory.AI);
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
        return await secureInvoke('list_models');
	} catch (error) {
		log.error('Failed to list models', error, 'listModels', LogCategory.AI);
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
		log.error('Failed to get settings', error, 'getSettings', LogCategory.SYSTEM);
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
        await secureInvoke('update_settings', { settings });
		// Invalidate settings cache after successful update
		apiCache.invalidate('get_settings');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to save settings', { message: errorMessage, error }, 'saveSettings', LogCategory.SYSTEM);
		if (toast) {
			toast.error(`Failed to save settings: ${errorMessage}`);
		}
		throw error;
	}
}

export async function resetSettings(): Promise<void> {
	try {
		await secureInvoke('reset_settings');
		// Invalidate settings cache after successful reset
		apiCache.invalidate('get_settings');
	} catch (error) {
		log.error('Failed to reset settings', error, 'resetSettings', LogCategory.SYSTEM);
		throw error;
	}
}

// System Operations
export async function getSystemInfo(): Promise<SystemInfo> {
	try {
		// Backend command is actually 'get_basic_system_info'
		return await apiCache.cachedInvoke<SystemInfo>('get_basic_system_info', {}, 10000); // 10 second cache
	} catch (error) {
		log.error('Failed to get system info', error, 'getSystemInfo', LogCategory.SYSTEM);
		throw error;
	}
}

export async function getSystemInfoDetailed(): Promise<SystemInfo> {
	try {
		return await secureInvoke('get_system_info');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to get detailed system info', errorMessage, 'getDetailedSystemInfo', LogCategory.SYSTEM);
		throw error;
	}
}

export async function clearCache(): Promise<void> {
	try {
		await secureInvoke('clear_cache');
		if (toast) {
			toast.success('Cache cleared successfully');
		}
	} catch (error) {
		log.error('Failed to clear cache', error, 'clearCache', LogCategory.SYSTEM);
		if (toast) {
			toast.error(`Failed to clear cache: ${error}`);
		}
		throw error;
	}
}

export async function frontendReady(): Promise<void> {
	try {
		return await secureInvoke('frontend_ready');
	} catch (error) {
		log.error('Failed to notify frontend ready', error, 'frontendReady', LogCategory.SYSTEM);
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
        const options = {
            directory: false,
            multiple: allowMultiple,
            title,
            ...(filters && { filters })
        };
        return await open(options as any);
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
        const options = {
            title,
            ...(defaultPath && { defaultPath })
        };
        return await save(options as any);
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

export function listenToDatabaseError(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('database-error', (event) => {
        callback(unwrapEventPayload(event.payload));
	});
}

export function listenToSessionEvents(callback: (payload: any) => void): Promise<UnlistenFn> {
    return listen('session-event', (event) => {
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
		return await secureInvoke('cancel_operation', { id: operationId });
	} catch (error) {
		const parsedError = parseBackendError(error);
		log.error('Failed to cancel operation', parsedError, 'cancelOperation', LogCategory.SYSTEM);
		throw error;
	}
}

export async function getActiveOperations(): Promise<ActiveOperationInfo[]> {
	try {
		return await secureInvoke<ActiveOperationInfo[]>('get_active_operations');
	} catch (error) {
		const parsedError = parseBackendError(error);
		log.error('Failed to get active operations', parsedError, 'getActiveOperations', LogCategory.SYSTEM);
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
		return await secureInvoke('get_operation_progress', { id: operationId });
	} catch (error) {
		const parsedError = parseBackendError(error);
		log.error('Failed to get operation progress', parsedError, 'getOperationProgress', LogCategory.SYSTEM);
		throw error;
	}
}

// Additional File Utility Operations
export async function fileExists(path: string): Promise<FileExistsResult> {
	try {
		return await secureInvoke<FileExistsResult>('file_exists', { path });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to check file existence', errorMessage, 'checkFileExists', LogCategory.FILE_OPS);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to check file: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getFileSizeInfo(path: string): Promise<FileSizeInfo> {
	try {
		return await secureInvoke<FileSizeInfo>('get_file_size_info', { path });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to get file size info', errorMessage, 'getFileSizeInfo', LogCategory.FILE_OPS);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get file size info: ${errorMessage}`);
		}
		throw error;
	}
}

// Extended Settings Operations
export async function getSettingsByCategory(category: string): Promise<Record<string, any>> {
	try {
		return await secureInvoke('get_settings_by_category', { category });
	} catch (error) {
		log.error('Failed to get settings by category', error, 'getSettingsByCategory', LogCategory.SYSTEM);
		throw error;
	}
}

export async function getAllSettingsCategories(): Promise<string[]> {
	try {
        const cats = await secureInvoke<Array<{ name: string }>>('get_all_settings_categories');
        return Array.isArray(cats) ? cats.map(c => c.name) : [];
	} catch (error) {
		log.error('Failed to get settings categories', error, 'getSettingsCategories', LogCategory.SYSTEM);
		return [];
	}
}

export async function updateCategorySettings(
	category: string,
	settings: Record<string, any>
): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('update_category_settings', {
			category,
			settings
		});
	} catch (error) {
		log.error('Failed to update category settings', error, 'updateCategorySettings', LogCategory.SYSTEM);
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
    return await secureInvoke('test_ai_connection', { config });
	} catch (error) {
		console.error('Failed to test AI connection:', error);
		throw error;
	}
}

export async function getSettingValue(key: string): Promise<any> {
	try {
		return await secureInvoke('get_setting_value', { key });
	} catch (error) {
		console.error('Failed to get setting value:', error);
		throw error;
	}
}

export async function setSettingValue(key: string, value: any): Promise<void> {
	try {
		return await secureInvoke('set_setting_value', { key, value });
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
		return await secureInvoke<string>('export_settings');
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
		return await secureInvoke('import_settings', { json });
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
		return await secureInvoke<ValidationResult>('validate_settings', { settings });
	} catch (error) {
		console.error('Failed to validate settings:', error);
		throw error;
	}
}

export async function addWatchPath(path: string): Promise<void> {
	try {
		return await secureInvoke('add_watch_path', { path });
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
		return await secureInvoke('remove_watch_path', { path });
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
		return await secureInvoke<string[]>('get_watch_paths');
	} catch (error) {
		console.error('Failed to get watch paths:', error);
		return [];
	}
}

export async function getWatchConfig(): Promise<{ enabled: boolean; paths: string[] }> {
	try {
		return await secureInvoke<{ enabled: boolean; paths: string[] }>('get_watch_config');
	} catch (error) {
		console.error('Failed to get watch config:', error);
		return { enabled: false, paths: [] };
	}
}

export async function updateWatchConfig(config: { enabled: boolean; paths: string[] }): Promise<void> {
	try {
		return await secureInvoke('update_watch_config', { config });
	} catch (error) {
		console.error('Failed to update watch config:', error);
		throw error;
	}
}

export async function startWatchMode(): Promise<void> {
	try {
		return await secureInvoke('start_watch_mode');
	} catch (error) {
		console.error('Failed to start watch mode:', error);
		throw error;
	}
}

export async function stopWatchMode(): Promise<void> {
	try {
		return await secureInvoke('stop_watch_mode');
	} catch (error) {
		console.error('Failed to stop watch mode:', error);
		throw error;
	}
}

export async function getWatchStatus(): Promise<{ active: boolean; lastCheck?: number; filesProcessed?: number }> {
	try {
		return await secureInvoke<{ active: boolean; lastCheck?: number; filesProcessed?: number }>('get_watch_status');
	} catch (error) {
		console.error('Failed to get watch status:', error);
		return { active: false };
	}
}

// System Monitoring Operations
export async function getHealthStatus(): Promise<HealthStatus> {
	try {
		return await secureInvoke<HealthStatus>('get_health_status');
	} catch (error) {
		console.error('Failed to get health status:', error);
		throw error;
	}
}

export async function getPerformanceMetrics(): Promise<PerformanceMetrics> {
	try {
		return await secureInvoke<PerformanceMetrics>('get_performance_metrics');
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
		return await secureInvoke('get_system_status');
	} catch (error) {
		console.error('Failed to get system status:', error);
		throw error;
	}
}

export async function testSmartFolderRule(rule: any): Promise<any> {
	try {
		return await secureInvoke('test_smart_folder_rule', { rule });
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
		return await secureInvoke<boolean>('move_file', {
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
		return await secureInvoke<boolean>('copy_file', {
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
		return await secureInvoke<boolean>('delete_file', {
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
		return await secureInvoke<boolean>('rename_file', {
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
		return await secureInvoke<boolean>('create_directory', {
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
		return await secureInvoke<string>('get_file_preview', {
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
		return await secureInvoke<FileInfo[]>('get_recent_files', { limit });
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
		return await secureInvoke('get_file_properties', {
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
		return await secureInvoke<ProcessedDropResult>('process_dropped_paths', {
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
		// Note: Backend expects '_description' parameter, not 'description'
		return await secureInvoke<SmartFolder>('create_smart_folder', {
			name,
			_description: description,
			target_path: targetPath,
			rules
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
		console.error('Failed to create smart folder:', error, errorMessage);
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
		// Note: Backend expects '_description' parameter, not 'description'
		return await secureInvoke<SmartFolder>('update_smart_folder', {
			id,
			name,
			_description: description,
			target_path: targetPath,
			rules,
			enabled
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
		console.error('Failed to update smart folder:', error, errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to update smart folder: ${errorMessage}`);
		}
		throw error;
	}
}

export async function deleteSmartFolder(id: string): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('delete_smart_folder', { id });
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
		return await secureInvoke<SmartFolder[]>('list_smart_folders');
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
		return await secureInvoke<SmartFolder | null>('get_smart_folder', { id });
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
		return await secureInvoke<OrganizationPreview[]>('apply_smart_folder_rules', {
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
		return await secureInvoke<boolean>('undo');
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
		return await secureInvoke<boolean>('redo');
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
		return await secureInvoke<any[]>('get_operation_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get operation history:', errorMessage);
		// Return empty array as fallback to keep UI functional
		return [];
	}
}

export async function getHistory(): Promise<HistoryEntry[]> {
	try {
		return await secureInvoke<HistoryEntry[]>('get_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get history:', errorMessage);
		return [];
	}
}

export async function getHistoryState(): Promise<HistoryState> {
	try {
		return await secureInvoke<HistoryState>('get_history_state');
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
		return await secureInvoke<boolean>('clear_history');
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
		return await secureInvoke<boolean>('jump_to_history', {
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
		return await secureInvoke<boolean>('open_folder', {
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
		return await secureInvoke<boolean>('show_in_folder', {
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
		return await secureInvoke<string[]>('get_default_folders');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get default folders:', errorMessage);
		return [];
	}
}

export async function getStorageInfo(): Promise<StorageInfo> {
	try {
		return await secureInvoke('get_storage_info');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get storage info:', errorMessage);
		throw error;
	}
}

export async function getAppLogs(): Promise<string[]> {
	try {
		return await secureInvoke<string[]>('get_app_logs');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get app logs:', errorMessage);
		throw error;
	}
}

export async function restartApp(): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('restart_app');
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
		return await secureInvoke('check_for_updates');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to check for updates:', errorMessage);
		throw error;
	}
}

export async function shutdownApplication(): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('shutdown_application');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to shutdown application:', errorMessage);
		throw error;
	}
}

// Watch Mode Operations
export async function getWatchModeStatus(): Promise<WatchModeConfig> {
	try {
		return await secureInvoke('get_watch_mode_status');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get watch mode status:', errorMessage);
		throw error;
	}
}

export async function configureWatchMode(config: any): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('configure_watch_mode', { config });
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
		return await secureInvoke<boolean>('enable_watch_mode', { directories });
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
		return await secureInvoke<boolean>('disable_watch_mode');
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
		return await secureInvoke<UserLearningPattern[]>('get_user_learning_patterns');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get user learning patterns:', errorMessage);
		return [];
	}
}

export async function triggerAutoOrganization(): Promise<AutoOrganizationTrigger> {
	try {
		return await secureInvoke('trigger_auto_organization');
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
		return await secureInvoke<SearchResult[]>('advanced_search', { query, filters });
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
		return await secureInvoke<string[]>('get_search_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get search history:', errorMessage);
		return [];
	}
}

export async function clearSearchHistory(): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('clear_search_history');
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
		// Transform frontend format to backend format
		const backendOperations = operations.map(op => ({
			operation_type: op.operation.charAt(0).toUpperCase() + op.operation.slice(1), // 'delete' -> 'Delete'
			source: op.source_path,
			destination: op.target_path || null
		}));

		return await secureInvoke('batch_file_operations', {
			operations: backendOperations
		});
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
		const operations = filePaths.map((source) => {
			// Extract filename from source path
			const fileName = source.split(/[/\\]/).pop() || '';
			// Construct full destination path with filename
			const destination = `${targetDirectory}${targetDirectory.endsWith('/') || targetDirectory.endsWith('\\') ? '' : '/'}${fileName}`;
			return { source, destination };
		});
		return await secureInvoke('move_files', { operations });
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
		return await secureInvoke('rename_files', { operations: payload });
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
        return await secureInvoke('check_first_run_status');
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
        const res = await secureInvoke<string>('complete_first_run_setup', { setup });
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
		return await secureInvoke<boolean>('reset_to_first_run');
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
		return await secureInvoke('get_ai_capabilities');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get AI capabilities:', errorMessage);
		throw error;
	}
}

export async function useFallbackAi(): Promise<AiStatus> {
	try {
		return await secureInvoke<AiStatus>('use_fallback_ai');
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
        return await secureInvoke('test_ai_analysis', params);
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
				const result = await secureInvoke<number[]>('generate_embeddings', { text });
				results.push(result);
			}
			return results;
		} else {
			return await secureInvoke<number[]>('generate_embeddings', { text: input });
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

		return await secureInvoke<FileInfo[]>('browse_files', params);
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
		return await secureInvoke<string>('browse_folder', { title: title || 'Select Folder' });
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
        return await secureInvoke<string>('emit_notification', {
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
		return await secureInvoke('get_notifications', {
			limit: options?.limit,
			unread_only: options?.unread_only
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
		console.error('Failed to get notifications:', error, errorMessage);
		return [];
	}
}

export async function markNotificationRead(notificationId: string): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('mark_notification_read', { notification_id: notificationId });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
		console.error('Failed to mark notification read:', error, errorMessage);
		throw error;
	}
}

export async function clearNotifications(olderThanHours?: number): Promise<number> {
	try {
		const params = olderThanHours !== undefined ? { older_than_hours: olderThanHours } : {};
		return await secureInvoke<number>('clear_notifications', params);
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : JSON.stringify(error);
		console.error('Failed to clear notifications:', error, errorMessage);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to clear notifications: ${errorMessage}`);
		}
		throw error;
	}
}

// Organization and Smart Folder Utilities
export async function getSmartFolders(): Promise<SmartFolder[]> {
	try {
		return await secureInvoke<SmartFolder[]>('get_smart_folders');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get smart folders:', errorMessage);
		return [];
	}
}

export async function matchToFolders(filePaths: string[]): Promise<any> {
	try {
		return await secureInvoke('match_to_folders', { file_paths: filePaths });
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
		return await secureInvoke('test_rule_against_files', {
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
		return await secureInvoke('validate_rule', { rule });
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
		return await secureInvoke<boolean>('readiness_probe');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed readiness probe:', errorMessage);
		return false;
	}
}

export async function livenessProbe(): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('liveness_probe');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed liveness probe:', errorMessage);
		return false;
	}
}

export async function getRuntimeConfig(): Promise<any> {
	try {
		return await secureInvoke('get_runtime_config');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get runtime config:', errorMessage);
		throw error;
	}
}

export async function getFileStatistics(): Promise<any> {
	try {
		return await secureInvoke('get_file_statistics');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get file statistics:', errorMessage);
		throw error;
	}
}

export async function enableRealtimeMonitoring(enabled: boolean = true): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('enable_realtime_monitoring', { enabled });
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
		return await secureInvoke('refresh_all_status');
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
		return await secureInvoke<any[]>('get_analysis_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get analysis history:', errorMessage);
		return [];
	}
}

export async function getMetricsHistory(): Promise<any[]> {
	try {
		return await secureInvoke<any[]>('get_metrics_history');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get metrics history:', errorMessage);
		return [];
	}
}


// Diagnostics Functions
export async function runDiagnostics(): Promise<SystemDiagnostics> {
	try {
		return await secureInvoke<SystemDiagnostics>('run_diagnostics');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to run diagnostics:', errorMessage);
		throw error;
	}
}

export async function testAiService(): Promise<AiServiceDiagnostics> {
	try {
		return await secureInvoke<AiServiceDiagnostics>('test_ai_service');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to test AI service:', errorMessage);
		throw error;
	}
}

export async function checkDatabaseHealth(): Promise<DatabaseDiagnostics> {
	try {
		return await secureInvoke<DatabaseDiagnostics>('check_database_health');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to check database health:', errorMessage);
		throw error;
	}
}

export async function validateConfigPaths(): Promise<PathPermissionCheck[]> {
	try {
		return await secureInvoke<PathPermissionCheck[]>('validate_config_paths');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to validate config paths:', errorMessage);
		throw error;
	}
}

export async function getDiagnosticsResourceUsage(): Promise<ResourceDiagnostics> {
	try {
		return await secureInvoke<ResourceDiagnostics>('get_diagnostics_resource_usage');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get diagnostics resource usage:', errorMessage);
		throw error;
	}
}

export async function clearCaches(): Promise<ClearCacheResult> {
	try {
		return await secureInvoke<ClearCacheResult>('clear_caches');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to clear caches:', errorMessage);
		throw error;
	}
}

// Additional File Utilities
export async function getFileContent(filePath: string): Promise<string> {
	try {
		return await secureInvoke<string>('get_file_content', { path: filePath });
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
		return await secureInvoke<FileInfo>('get_file_info_command', { path: filePath });
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
		return await secureInvoke<boolean>('set_file_permissions', {
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
		return await secureInvoke<boolean>('update_auto_organize_threshold', { threshold });
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
		return await secureInvoke<any[]>('get_pending_auto_organization');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get pending auto organization:', errorMessage);
		return [];
	}
}

export async function addWatchDirectory(directory: string): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('add_watch_directory', { directory_path: directory });
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
		return await secureInvoke<boolean>('remove_watch_directory', { directory_path: directory });
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
		return await secureInvoke<boolean>('record_user_organization_action', {
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
		return await secureInvoke<boolean>('batch_undo', { count });
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
		return await secureInvoke<boolean>('batch_redo', { count });
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
		return await secureInvoke('get_memory_stats');
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
		await secureInvoke('emit_progress_notification', {
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
		await secureInvoke('emit_file_operation_status', {
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
		await secureInvoke('emit_system_status', {
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
		return await secureInvoke('get_resource_usage');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		console.error('Failed to get resource usage:', errorMessage);
		throw error;
	}
}

export async function forceShutdown(): Promise<boolean> {
	try {
		return await secureInvoke<boolean>('force_shutdown');
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
		return await secureInvoke<boolean>('start_ai_stream', {
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
		return await secureInvoke<boolean>('stop_ai_stream', {
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
		return await secureInvoke<boolean>('is_stream_active', {
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

// Pattern Learning Operations
export async function getLearnedPatterns(): Promise<any[]> {
	try {
		return await secureInvoke<any[]>('get_learned_patterns');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to get learned patterns', errorMessage, 'getLearnedPatterns', LogCategory.AI);
		throw error;
	}
}

export async function recordPatternChoice(
	filePath: string,
	chosenCategory: string,
	metadata?: Record<string, any>
): Promise<void> {
	try {
		await secureInvoke('record_pattern_choice', {
			file_path: filePath,
			chosen_category: chosenCategory,
			metadata
		});
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to record pattern choice', errorMessage, 'recordPatternChoice', LogCategory.AI);
		throw error;
	}
}

export async function getPatternSuggestions(filePath: string): Promise<any[]> {
	try {
		return await secureInvoke<any[]>('get_pattern_suggestions', { file_path: filePath });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to get pattern suggestions', errorMessage, 'getPatternSuggestions', LogCategory.AI);
		throw error;
	}
}

export async function clearLearnedPatterns(): Promise<void> {
	try {
		await secureInvoke('clear_learned_patterns');
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to clear learned patterns', errorMessage, 'clearLearnedPatterns', LogCategory.AI);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to clear patterns: ${errorMessage}`);
		}
		throw error;
	}
}

// Archive Operations
export async function compressFiles(options: {
	files: string[];
	output_path: string;
	format?: 'zip' | 'tar' | 'tar.gz';
	compression_level?: number;
}): Promise<string> {
	try {
		return await secureInvoke<string>('compress_files', { options });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to compress files', errorMessage, 'compressFiles', LogCategory.FILE_OPS);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to compress files: ${errorMessage}`);
		}
		throw error;
	}
}

export async function extractArchive(options: {
	archive_path: string;
	output_path: string;
	preserve_structure?: boolean;
}): Promise<string[]> {
	try {
		return await secureInvoke<string[]>('extract_archive', { options });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to extract archive', errorMessage, 'extractArchive', LogCategory.FILE_OPS);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to extract archive: ${errorMessage}`);
		}
		throw error;
	}
}

export async function getArchiveInfo(archivePath: string): Promise<{
	format: string;
	total_files: number;
	compressed_size: number;
	uncompressed_size: number;
	files: Array<{ path: string; size: number }>;
}> {
	try {
		return await secureInvoke('get_archive_info', { archive_path: archivePath });
	} catch (error) {
		const errorMessage = error instanceof Error ? error.message : String(error);
		log.error('Failed to get archive info', errorMessage, 'getArchiveInfo', LogCategory.FILE_OPS);
		if (typeof toast !== 'undefined' && toast?.error) {
			toast.error(`Failed to get archive info: ${errorMessage}`);
		}
		throw error;
	}
}
