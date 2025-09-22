import { render as svelteRender, type RenderResult } from '@testing-library/svelte';
import { writable, type Writable } from 'svelte/store';
import type { ComponentType, SvelteComponent } from 'svelte';
import { vi } from 'vitest';

// Define types for context stores
interface TestContext {
	theme: Writable<string>;
	user: Writable<unknown>;
	settings: Writable<Record<string, unknown>>;
	[key: string]: unknown;
}

// Custom render function that provides common context and utilities
export function render<T extends Record<string, unknown> = Record<string, unknown>>(
	Component: ComponentType<SvelteComponent<T>>,
	options?: {
		props?: T;
		context?: Map<string, unknown>;
		target?: HTMLElement;
	}
) {
	// Create default context with stores
	const defaultContext = new Map<string, unknown>([
		['theme', writable('light')],
		['user', writable(null)],
		['settings', writable({})]
	]);

	// Merge with provided context
	const context = options?.context
		? new Map<string, unknown>([...defaultContext, ...options.context])
		: defaultContext;

	return svelteRender(Component, {
		...options,
		context
	}) as any;
}

// Helper to wait for async operations with better error messages
export async function waitForWithTimeout(
	callback: () => void | Promise<void>,
	timeout: number = 5000,
	errorMessage?: string
): Promise<void> {
	const startTime = Date.now();

	while (Date.now() - startTime < timeout) {
		try {
			await callback();
			return;
		} catch (error) {
			if (Date.now() - startTime >= timeout) {
				throw new Error(
					errorMessage || `Timeout waiting for condition after ${timeout}ms: ${error}`
				);
			}
			await new Promise(resolve => setTimeout(resolve, 50));
		}
	}
}

// Helper to create mock file objects for testing
export function createMockFile(
	content: string,
	fileName: string,
	mimeType: string = 'text/plain'
): File {
	return new File([content], fileName, { type: mimeType });
}

// Helper to create mock drag event
export function createDragEvent(
	type: string,
	files: File[]
): DragEvent {
	const dataTransfer = new DataTransfer();
	files.forEach(file => {
		dataTransfer.items.add(file);
	});

	return new DragEvent(type, {
		bubbles: true,
		cancelable: true,
		dataTransfer
	});
}

// Helper to simulate file input change
export async function selectFiles(
	input: HTMLInputElement,
	files: File[]
): Promise<void> {
	const fileList = createFileList(files);
	Object.defineProperty(input, 'files', {
		value: fileList,
		writable: false
	});

	const event = new Event('change', { bubbles: true });
	input.dispatchEvent(event);
}

// Helper to create FileList from File array
function createFileList(files: File[]): FileList {
	const dataTransfer = new DataTransfer();
	files.forEach(file => dataTransfer.items.add(file));
	return dataTransfer.files;
}

// Helper to mock IntersectionObserver for components that use it
export function mockIntersectionObserver(
	options: {
		isIntersecting?: boolean;
		threshold?: number;
	} = {}
): void {
	const mockObserver = {
		observe: vi.fn(),
		unobserve: vi.fn(),
		disconnect: vi.fn(),
		takeRecords: vi.fn().mockReturnValue([]),
		root: null,
		rootMargin: '',
		thresholds: [options.threshold || 0]
	};

	window.IntersectionObserver = vi.fn().mockImplementation((callback) => {
		// Immediately call the callback with mock entries
		callback([
			{
				isIntersecting: options.isIntersecting ?? true,
				boundingClientRect: {} as DOMRectReadOnly,
				intersectionRatio: 1,
				intersectionRect: {} as DOMRectReadOnly,
				rootBounds: {} as DOMRectReadOnly,
				target: document.createElement('div'),
				time: Date.now()
			}
		], mockObserver);

		return mockObserver;
	});
}

// Helper to mock ResizeObserver for components that observe size changes
export function mockResizeObserver(): void {
	window.ResizeObserver = vi.fn().mockImplementation(() => ({
		observe: vi.fn(),
		unobserve: vi.fn(),
		disconnect: vi.fn()
	}));
}

// Helper to flush all pending promises
export function flushPromises(): Promise<void> {
	return new Promise(resolve => setTimeout(resolve, 0));
}

// Helper to create mock store for testing
export function createMockStore<T>(initialValue: T) {
	const { subscribe, set, update } = writable(initialValue);

	return {
		subscribe,
		set: vi.fn(set),
		update: vi.fn(update),
		mockReset: () => {
			set(initialValue);
		}
	};
}

// Helper to test accessibility
export async function testA11y(container: HTMLElement): Promise<void> {
	// Check for basic accessibility requirements
	const errors: string[] = [];

	// Check images have alt text
	const images = container.querySelectorAll('img');
	images.forEach(img => {
		if (!img.hasAttribute('alt')) {
			errors.push(`Image missing alt text: ${img.src}`);
		}
	});

	// Check buttons have accessible text
	const buttons = container.querySelectorAll('button');
	buttons.forEach(button => {
		const text = button.textContent?.trim();
		const ariaLabel = button.getAttribute('aria-label');
		if (!text && !ariaLabel) {
			errors.push('Button missing accessible text');
		}
	});

	// Check form inputs have labels
	const inputs = container.querySelectorAll('input, select, textarea');
	inputs.forEach(input => {
		const id = input.id;
		const ariaLabel = input.getAttribute('aria-label');
		const ariaLabelledBy = input.getAttribute('aria-labelledby');

		if (!ariaLabel && !ariaLabelledBy) {
			const label = container.querySelector(`label[for="${id}"]`);
			if (!label) {
				errors.push(`Input missing label: ${input.tagName} ${id || '(no id)'}`);
			}
		}
	});

	// Check headings are in order
	const headings = Array.from(container.querySelectorAll('h1, h2, h3, h4, h5, h6'));
	let lastLevel = 0;
	headings.forEach(heading => {
		const level = parseInt(heading.tagName[1]);
		if (level - lastLevel > 1) {
			errors.push(`Heading level skipped: ${heading.tagName} after H${lastLevel}`);
		}
		lastLevel = level;
	});

	if (errors.length > 0) {
		throw new Error(`Accessibility issues found:\n${errors.join('\n')}`);
	}
}

// Helper to test keyboard navigation
export async function testKeyboardNavigation(
	container: HTMLElement,
	expectedOrder: string[]
): Promise<void> {
	const focusableElements = container.querySelectorAll(
		'a, button, input, select, textarea, [tabindex]:not([tabindex="-1"])'
	);

	const actualOrder: string[] = [];

	focusableElements.forEach(element => {
		const identifier =
			element.getAttribute('aria-label') ||
			element.textContent?.trim() ||
			element.id ||
			element.tagName.toLowerCase();
		actualOrder.push(identifier);
	});

	expectedOrder.forEach((expected, index) => {
		if (actualOrder[index] !== expected) {
			throw new Error(
				`Keyboard navigation order mismatch at index ${index}: expected "${expected}", got "${actualOrder[index]}"`
			);
		}
	});
}

// Helper to mock Tauri events
export function createMockTauriEvent<T>(payload: T) {
	return {
		event: 'mock-event',
		id: Math.random(),
		windowLabel: 'main',
		payload
	};
}

// Export everything for convenience
export * from '@testing-library/svelte';
export { default as userEvent } from '@testing-library/user-event';