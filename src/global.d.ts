/// <reference types="@tauri-apps/api" />

/**
 * Global type definitions for the application
 */

declare global {
	interface Window {
		__TAURI__?: {
			invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
			event: {
				emit: (event: string, payload?: unknown) => Promise<void>;
				listen: (event: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>;
			};
			dialog: {
				open: (options?: Record<string, unknown>) => Promise<string | string[] | null>;
				save: (options?: Record<string, unknown>) => Promise<string | null>;
			};
		};
		__TAURI_MOCK__?: {
			invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
		};
	}

	// Performance API types that may not be available in all environments
	interface Performance {
		memory?: {
			usedJSHeapSize: number;
			totalJSHeapSize: number;
			jsHeapSizeLimit: number;
		};
	}
}

// Make this a module to avoid global pollution
export {};