import '@testing-library/jest-dom';
import { vi, afterEach } from 'vitest';
// Temporarily disabled MSW due to import issues with msw/node in browser environment
// import { setupMSW } from './setup-msw';

// Setup Mock Service Worker for API mocking
// setupMSW();

// Reset mocks after each test
afterEach(() => {
	vi.clearAllMocks();
});

// Mock Tauri API globally
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
	convertFileSrc: vi.fn((src) => src)
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn(() => Promise.resolve(() => {})),
	emit: vi.fn(() => Promise.resolve()),
	once: vi.fn(() => Promise.resolve(() => {})),
	UnlistenFn: vi.fn()
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
	open: vi.fn(),
	save: vi.fn(),
	message: vi.fn(),
	ask: vi.fn(),
	confirm: vi.fn()
}));

// Mock window.__TAURI__ for tests
global.window = global.window || {};
window.__TAURI__ = {
	invoke: vi.fn(),
	event: {
		listen: vi.fn(() => Promise.resolve(() => {})),
		emit: vi.fn(() => Promise.resolve())
	},
	dialog: {
		open: vi.fn(),
		save: vi.fn()
	}
};

// Mock IntersectionObserver
global.IntersectionObserver = vi.fn().mockImplementation(() => ({
	observe: vi.fn(),
	unobserve: vi.fn(),
	disconnect: vi.fn(),
	takeRecords: vi.fn(),
	root: null,
	rootMargin: '',
	thresholds: []
}));

// Mock ResizeObserver
global.ResizeObserver = class ResizeObserver {
	observe = vi.fn();
	unobserve = vi.fn();
	disconnect = vi.fn();
	
	constructor(_callback: ResizeObserverCallback) {
		// Store the callback if needed for testing
	}
} as any;

// Mock matchMedia
window.matchMedia = window.matchMedia || vi.fn().mockImplementation((query: string) => ({
	matches: false,
	media: query,
	onchange: null,
	addListener: vi.fn(),
	removeListener: vi.fn(),
	addEventListener: vi.fn(),
	removeEventListener: vi.fn(),
	dispatchEvent: vi.fn()
}));

// Ensure matchMedia is always available
Object.defineProperty(window, 'matchMedia', {
	writable: true,
	configurable: true,
	value: window.matchMedia
});

// Mock localStorage with in-memory backing store
(() => {
    const store: Record<string, string> = {};
    const localStorageMock = {
        getItem: vi.fn((key: string) => (key in store ? store[key] : null)),
        setItem: vi.fn((key: string, value: string) => {
            store[key] = String(value);
        }),
        removeItem: vi.fn((key: string) => {
            delete store[key];
        }),
        clear: vi.fn(() => {
            for (const k of Object.keys(store)) delete store[k];
        }),
        get length() {
            return Object.keys(store).length;
        },
        key: vi.fn((index: number) => Object.keys(store)[index] ?? null)
    };
    // Assign the mock to global
    global.localStorage = localStorageMock as Storage;
})();

// Mock performance API (including now)
// Some libs expect performance.now to exist in happy-dom
const perf = global.performance || {} as Partial<Performance>;
global.performance = {
	...perf,
	now: perf.now || (() => Date.now()),
	mark: perf.mark || vi.fn(),
	measure: perf.measure || vi.fn(),
	clearMarks: perf.clearMarks || vi.fn(),
	clearMeasures: perf.clearMeasures || vi.fn(),
	getEntriesByName: perf.getEntriesByName || vi.fn(() => []),
	getEntriesByType: perf.getEntriesByType || vi.fn(() => [])
} as Performance;

// Mock document for DOM-dependent components
if (typeof document !== 'undefined') {
	// Only set if not already set
	if (!document.body) {
		Object.defineProperty(document, 'body', {
			value: document.createElement('body'),
			writable: true
		});
	}
	if (!document.documentElement) {
		Object.defineProperty(document, 'documentElement', {
			value: document.createElement('html'),
			writable: true
		});
	}

	// Ensure createElement returns elements with addEventListener
	const originalCreateElement = document.createElement;
	document.createElement = function(tagName: string) {
		const element = originalCreateElement.call(this, tagName);
		if (!element.addEventListener) {
			element.addEventListener = vi.fn();
			element.removeEventListener = vi.fn();
			element.dispatchEvent = vi.fn();
		}
		return element;
	};
}