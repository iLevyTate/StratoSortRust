// Svelte stores used app-wide. Kept in one file so the import surface from
// $lib/stores stays small. Notification toast lives in ./notifications.ts.

import { writable, type Writable } from 'svelte/store';
import type { AppSettings, Page } from '$lib/types/backend';
import { listenEvent } from '$lib/api/tauri';

export const currentPage: Writable<Page> = writable('discover');

// Settings hydrate once on boot via getAppSettings(). Null means "not loaded
// yet"; components should treat it as "show defaults / loading state".
export const appSettings: Writable<AppSettings | null> = writable(null);

export const operationInProgress: Writable<boolean> = writable(false);

// Long-running operations report progress via emit("operation-progress",
// { progress, message }). Components can subscribe to this to render a bar.
export interface OperationProgress {
	progress: number;
	message: string;
	operation_id?: string;
}
export const currentOperationProgress: Writable<OperationProgress | null> = writable(null);

// Track backend-emitted events. We hold the unlisten handles here so a single
// cleanupEventListeners() call tears down all subscriptions at teardown.
let unlisteners: Array<() => void> = [];

export async function initializeEventListeners(): Promise<void> {
	// Idempotent — clean up any previous registrations first so
	// onMount->onDestroy->onMount cycles (HMR, route changes) don't leak.
	cleanupEventListeners();

	unlisteners.push(
		await listenEvent<{ progress: number; message: string; operation_id?: string }>(
			'operation-progress',
			(p) => {
				operationInProgress.set(true);
				currentOperationProgress.set(p);
			}
		)
	);

	unlisteners.push(
		await listenEvent<unknown>('operation-complete', () => {
			operationInProgress.set(false);
			currentOperationProgress.set(null);
		})
	);

	unlisteners.push(
		await listenEvent<unknown>('operation-error', () => {
			operationInProgress.set(false);
			currentOperationProgress.set(null);
		})
	);
}

export function cleanupEventListeners(): void {
	for (const u of unlisteners) {
		try {
			u();
		} catch {
			// Swallow — teardown shouldn't fail the parent component.
		}
	}
	unlisteners = [];
}
