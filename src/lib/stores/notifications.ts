// Re-exports the svelte-sonner toast under a stable name so call sites
// import from $lib/stores/notifications without coupling to the underlying
// library. If we ever swap toast libraries, this is the only file to change.

import { toast as sonnerToast } from 'svelte-sonner';
import type { Writable } from 'svelte/store';
import { writable } from 'svelte/store';

export interface ToastAction {
	label: string;
	onClick: () => void;
}

export interface ToastOptions {
	persistent?: boolean;
	duration?: number;
	action?: ToastAction;
}

function normalize(opts?: ToastOptions): Record<string, unknown> {
	if (!opts) return {};
	const out: Record<string, unknown> = {};
	if (opts.persistent) {
		out.duration = Infinity;
	} else if (typeof opts.duration === 'number') {
		out.duration = opts.duration;
	}
	if (opts.action) {
		out.action = { label: opts.action.label, onClick: opts.action.onClick };
	}
	return out;
}

export const toast = {
	success(message: string, opts?: ToastOptions) {
		sonnerToast.success(message, normalize(opts));
	},
	error(message: string, opts?: ToastOptions) {
		sonnerToast.error(message, normalize(opts));
	},
	info(message: string, opts?: ToastOptions) {
		sonnerToast.info(message, normalize(opts));
	},
	warning(message: string, opts?: ToastOptions) {
		sonnerToast.warning(message, normalize(opts));
	},
	message(message: string, opts?: ToastOptions) {
		sonnerToast(message, normalize(opts));
	}
};

// In-app notification center backing store. Distinct from toasts — toasts
// auto-dismiss, these persist in the NotificationCenter dropdown.
export interface Notification {
	id: string;
	type: 'info' | 'success' | 'warning' | 'error';
	title: string;
	message: string;
	timestamp: number;
	read: boolean;
}

export const notifications: Writable<Notification[]> = writable([]);

export function pushNotification(n: Omit<Notification, 'id' | 'timestamp' | 'read'>): void {
	notifications.update((list) => [
		{
			...n,
			id: crypto.randomUUID(),
			timestamp: Date.now(),
			read: false
		},
		...list
	]);
}

export function markAllRead(): void {
	notifications.update((list) => list.map((n) => ({ ...n, read: true })));
}

export function clearNotifications(): void {
	notifications.set([]);
}
