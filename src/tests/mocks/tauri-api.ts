import { vi } from 'vitest';
import type {
	FileInfo,
	FileAnalysis,
	OllamaStatus,
	AppSettings,
	SmartFolder,
	SearchResult
} from '$lib/types/backend';

// Mock data generators
export const mockFileInfo = (overrides?: Partial<FileInfo>): FileInfo => ({
	path: '/test/file.txt',
	name: 'file.txt',
	size: 1024,
	mime_type: 'text/plain',
	is_directory: false,
	extension: 'txt',
	created_at: Date.now(),
	modified_at: Date.now(),
	...overrides
});

export const mockFileAnalysis = (overrides?: Partial<FileAnalysis>): FileAnalysis => ({
	path: '/test/file.txt',
	category: 'Documents',
	tags: ['test', 'sample'],
	summary: 'Test file content',
	confidence: 0.85,
	metadata: {},
	...overrides
});

export const mockOllamaStatus = (overrides?: Partial<OllamaStatus>): OllamaStatus => ({
	isRunning: true,
	version: '0.1.0',
	models: ['llama3.2:3b', 'llava:7b'],
	...overrides
});

export const mockSearchResult = (overrides?: Partial<SearchResult>): SearchResult => ({
	path: '/test/result.txt',
	name: 'result.txt',
	score: 0.85,
	snippet: 'This is a search result snippet...',
	highlights: ['match', 'result'],
	metadata: {},
	...overrides
});

export const mockAppSettings = (): AppSettings => ({
	// AI Settings
	ai_provider: 'ollama',
	ollama_host: 'http://localhost:11434',
	ollama_model: 'llama3.2:3b',
	ollama_vision_model: 'llava:7b',
	ollama_embedding_model: 'nomic-embed-text',

	// File Settings
	watch_folders: false,
	watch_paths: [],
	default_smart_folder_location: '',
	file_extensions_to_ignore: ['.tmp', '.cache'],
	max_file_size: 100 * 1024 * 1024,

	// Performance Settings
	max_concurrent_analysis: 3,
	max_concurrent_operations: 5,
	cache_size: 100 * 1024 * 1024,
	enable_gpu: false,

	// Resource Limits
	max_concurrent_reads: 5,
	max_total_memory_mb: 100,
	max_single_file_size_mb: 10,
	max_directory_scan_depth: 10,

	// UI Settings
	theme: 'auto',
	language: 'en',
	show_notifications: true,
	notification_duration: 3000,

	// Privacy Settings
	enable_telemetry: false,
	enable_crash_reports: false,
	enable_analytics: false,

	// Behavior Settings
	confirm_before_delete: true,
	confirm_before_move: true,
	auto_analyze_on_add: false,
	preserve_file_timestamps: true,

	// Advanced Settings
	debug_mode: false,
	log_level: 'info',
	history_retention: 30,
	undo_history_size: 50,

	// Naming Convention Settings
	naming_convention: 'kebab-case',
	date_format: 'YYYY-MM-DD',
	case_style: 'lower'
});

export const mockSmartFolder = (overrides?: Partial<SmartFolder>): SmartFolder => ({
	id: 'test-folder-1',
	name: 'Test Smart Folder',
	target_path: '/test/smart-folders/test',
	rules: [],
	created_at: new Date().toISOString(),
	updated_at: new Date().toISOString(),
	enabled: true,
	description: 'Test smart folder',
	...overrides
});

// Mock API responses
export const mockApiResponses = {
	// File operations
	scan_directory: vi.fn().mockResolvedValue([
		mockFileInfo({ path: '/test/file1.txt', name: 'file1.txt' }),
		mockFileInfo({ path: '/test/file2.pdf', name: 'file2.pdf', mime_type: 'application/pdf' }),
		mockFileInfo({ path: '/test/folder', name: 'folder', is_directory: true })
	]),

	get_file_info_command: vi.fn().mockResolvedValue(mockFileInfo()),

	analyze_file: vi.fn().mockResolvedValue({
		success: true,
		analysis: mockFileAnalysis()
	}),

	analyze_files_batch: vi.fn().mockResolvedValue({
		successful: [mockFileAnalysis()],
		failed: [],
		total: 1
	}),

	move_file: vi.fn().mockResolvedValue({ success: true }),
	copy_file: vi.fn().mockResolvedValue({ success: true }),
	delete_file: vi.fn().mockResolvedValue({ success: true }),
	rename_file: vi.fn().mockResolvedValue({ success: true }),

	// AI operations
	check_ollama_status: vi.fn().mockResolvedValue(mockOllamaStatus()),
	list_models: vi.fn().mockResolvedValue(['llama3.2:3b', 'llava:7b']),
	analyze_with_ai: vi.fn().mockResolvedValue({
		success: true,
		analysis: mockFileAnalysis()
	}),

	// Settings
	get_app_settings: vi.fn().mockResolvedValue(mockAppSettings()),
	save_app_settings: vi.fn().mockResolvedValue({ success: true }),

	// Smart folders
	get_smart_folders: vi.fn().mockResolvedValue([mockSmartFolder()]),
	create_smart_folder: vi.fn().mockResolvedValue(mockSmartFolder()),
	update_smart_folder: vi.fn().mockResolvedValue(mockSmartFolder()),
	delete_smart_folder: vi.fn().mockResolvedValue({ success: true }),

	// First run
	check_first_run_status: vi.fn().mockResolvedValue({
		is_first_run: false,
		setup_completed: true
	}),
	complete_first_run_setup: vi.fn().mockResolvedValue({ success: true }),

	// History
	get_history: vi.fn().mockResolvedValue({
		entries: [],
		current_index: -1,
		can_undo: false,
		can_redo: false
	}),
	undo_operation: vi.fn().mockResolvedValue({ success: true }),
	redo_operation: vi.fn().mockResolvedValue({ success: true }),

	// Notifications
	get_notifications: vi.fn().mockResolvedValue([]),
	mark_notification_read: vi.fn().mockResolvedValue({ success: true }),
	clear_notifications: vi.fn().mockResolvedValue({ success: true }),

	// System
	get_system_info: vi.fn().mockResolvedValue({
		os: 'Windows',
		version: '10.0.0',
		arch: 'x64',
		cpu_cores: 8,
		total_memory: 16384,
		available_memory: 8192
	}),

	frontend_ready: vi.fn().mockResolvedValue(undefined)
};

// Event emitters for testing
export class MockEventEmitter {
	private listeners: Map<string, Set<Function>> = new Map();

	listen(event: string, handler: Function) {
		if (!this.listeners.has(event)) {
			this.listeners.set(event, new Set());
		}
		this.listeners.get(event)!.add(handler);

		// Return unlisten function
		return () => {
			const handlers = this.listeners.get(event);
			if (handlers) {
				handlers.delete(handler);
			}
		};
	}

	emit(event: string, payload: any) {
		const handlers = this.listeners.get(event);
		if (handlers) {
			handlers.forEach(handler => handler({ payload }));
		}
	}

	clear() {
		this.listeners.clear();
	}
}

export const mockEventEmitter = new MockEventEmitter();

// Create mock invoke function with type safety
export const createMockInvoke = (overrides?: Record<string, any>) => {
	const responses = { ...mockApiResponses, ...overrides };

	return vi.fn(async (cmd: string, args?: any) => {
		const handler = (responses as Record<string, any>)[cmd];
		if (handler) {
			return typeof handler === 'function' ? handler(args) : handler;
		}
		throw new Error(`Unknown command: ${cmd}`);
	});
};

// Create mock listen function
export const createMockListen = () => {
	return vi.fn((event: string, handler: Function) => {
		return mockEventEmitter.listen(event, handler);
	});
};

// Helper to simulate backend events
export const simulateBackendEvent = (event: string, payload: any) => {
	mockEventEmitter.emit(event, payload);
};

// Helper to simulate progress events
export const simulateProgress = (operationId: string, progress: number, message: string) => {
	simulateBackendEvent('operation-progress', {
		operation_id: operationId,
		progress,
		message,
		timestamp: Date.now()
	});
};

// Helper to simulate operation completion
export const simulateOperationComplete = (operationId: string, success: boolean = true) => {
	simulateBackendEvent('operation-complete', {
		operation_id: operationId,
		success,
		message: success ? 'Operation completed successfully' : 'Operation failed',
		timestamp: Date.now()
	});
};

// Helper to simulate errors
export const simulateError = (operationId: string, error: string) => {
	simulateBackendEvent('operation-error', {
		operation_id: operationId,
		error,
		timestamp: Date.now()
	});
};

// Helper to reset all mocks
export const resetAllMocks = () => {
	Object.values(mockApiResponses).forEach(mock => {
		if (typeof mock === 'function' && 'mockReset' in mock) {
			(mock as any).mockReset();
		}
	});
	mockEventEmitter.clear();
};