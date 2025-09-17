import '@testing-library/jest-dom';
import { vi, afterEach } from 'vitest';

// For now, we'll skip MSW setup and focus on component/unit testing
// TODO: Add MSW setup for API mocking if needed

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
	listen: vi.fn(),
	emit: vi.fn(),
	once: vi.fn(),
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
(global.window as any).__TAURI__ = {
	invoke: vi.fn(),
	event: {
		listen: vi.fn(),
		emit: vi.fn(),
		once: vi.fn()
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
global.ResizeObserver = vi.fn().mockImplementation(() => ({
	observe: vi.fn(),
	unobserve: vi.fn(),
	disconnect: vi.fn()
}));

// Mock matchMedia
Object.defineProperty(window, 'matchMedia', {
	writable: true,
	value: vi.fn().mockImplementation(query => ({
		matches: false,
		media: query,
		onchange: null,
		addListener: vi.fn(),
		removeListener: vi.fn(),
		addEventListener: vi.fn(),
		removeEventListener: vi.fn(),
		dispatchEvent: vi.fn()
	}))
});

// Mock localStorage
const localStorageMock = {
	getItem: vi.fn(),
	setItem: vi.fn(),
	removeItem: vi.fn(),
	clear: vi.fn(),
	length: 0,
	key: vi.fn()
};
global.localStorage = localStorageMock as any;

// Mock performance API
global.performance = {
	...global.performance,
	mark: vi.fn(),
	measure: vi.fn(),
	clearMarks: vi.fn(),
	clearMeasures: vi.fn(),
	getEntriesByName: vi.fn(() => []),
	getEntriesByType: vi.fn(() => [])
};

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