// Single global keyboard shortcut registry. A singleton because there's only
// one document and we want a single source of truth for which keys are bound.
//
// Bindings registered here drive the help dialog (KeyboardShortcutsHelp.svelte)
// — keep `description` user-readable.

import { currentPage } from '$lib/stores';

export interface KeyBinding {
	id: string;
	description: string;
	combo: string; // e.g. "Mod+1", "?"
	matches: (e: KeyboardEvent) => boolean;
	run: (e: KeyboardEvent) => void;
}

function isMod(e: KeyboardEvent): boolean {
	return e.metaKey || e.ctrlKey;
}

class KeyboardShortcutsManager {
	private bindings: KeyBinding[] = [];
	private listener: ((e: KeyboardEvent) => void) | null = null;

	constructor() {
		this.register({
			id: 'nav-discover',
			description: 'Go to Discover',
			combo: 'Mod+1',
			matches: (e) => isMod(e) && e.key === '1',
			run: () => currentPage.set('discover')
		});
		this.register({
			id: 'nav-analyze',
			description: 'Go to Analyze',
			combo: 'Mod+2',
			matches: (e) => isMod(e) && e.key === '2',
			run: () => currentPage.set('analyze')
		});
		this.register({
			id: 'nav-organize',
			description: 'Go to Organize',
			combo: 'Mod+3',
			matches: (e) => isMod(e) && e.key === '3',
			run: () => currentPage.set('organize')
		});
		this.register({
			id: 'nav-settings',
			description: 'Go to Settings',
			combo: 'Mod+,',
			matches: (e) => isMod(e) && e.key === ',',
			run: () => currentPage.set('settings')
		});
		this.register({
			id: 'show-help',
			description: 'Show keyboard shortcuts',
			combo: '?',
			matches: (e) => e.key === '?' && !isMod(e),
			run: () => document.dispatchEvent(new Event('show-keyboard-help'))
		});
	}

	register(binding: KeyBinding): void {
		this.bindings.push(binding);
	}

	get all(): readonly KeyBinding[] {
		return this.bindings;
	}

	attach(): void {
		if (this.listener || typeof document === 'undefined') return;
		this.listener = (e) => {
			// Don't capture shortcuts while the user is typing in an input.
			const tgt = e.target as HTMLElement | null;
			if (tgt && (tgt.tagName === 'INPUT' || tgt.tagName === 'TEXTAREA' || tgt.isContentEditable)) {
				return;
			}
			for (const b of this.bindings) {
				if (b.matches(e)) {
					e.preventDefault();
					b.run(e);
					return;
				}
			}
		};
		document.addEventListener('keydown', this.listener);
	}

	destroy(): void {
		if (this.listener && typeof document !== 'undefined') {
			document.removeEventListener('keydown', this.listener);
		}
		this.listener = null;
	}
}

export const keyboardShortcuts = new KeyboardShortcutsManager();

if (typeof document !== 'undefined') {
	keyboardShortcuts.attach();
}
