<script lang="ts">
	import { currentPage } from '$lib/stores';
	import type { Page } from '$lib/types/backend';

	interface NavItem {
		id: Page;
		label: string;
		testId: string;
	}

	const items: NavItem[] = [
		{ id: 'discover', label: 'Discover', testId: 'nav-discover' },
		{ id: 'analyze', label: 'Analyze', testId: 'nav-analyze' },
		{ id: 'organize', label: 'Organize', testId: 'nav-organize' },
		{ id: 'settings', label: 'Settings', testId: 'nav-settings' }
	];

	function go(p: Page): void {
		currentPage.set(p);
	}
</script>

<aside
	class="w-56 border-r bg-card flex flex-col"
	data-testid="sidebar"
	role="navigation"
	aria-label="Primary"
>
	<div class="p-4 border-b">
		<h1 class="font-semibold text-lg">StratoSort</h1>
		<p class="text-xs text-muted-foreground">Local AI file sort</p>
	</div>
	<nav class="flex-1 p-2 space-y-1">
		{#each items as item (item.id)}
			<button
				type="button"
				data-testid={item.testId}
				class="w-full text-left px-3 py-2 rounded hover:bg-accent transition-colors {$currentPage ===
				item.id
					? 'bg-accent font-medium'
					: ''}"
				aria-current={$currentPage === item.id ? 'page' : undefined}
				on:click={() => go(item.id)}
			>
				{item.label}
			</button>
		{/each}
	</nav>
	<div class="p-3 text-xs text-muted-foreground border-t">
		v0.1.0 · local-first
	</div>
</aside>
