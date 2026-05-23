<script lang="ts">
	import { keyboardShortcuts } from '$lib/utils/keyboard-shortcuts';

	export let showHelp: boolean = false;

	function close(): void {
		showHelp = false;
	}

	function onKey(e: KeyboardEvent): void {
		if (e.key === 'Escape') close();
	}
</script>

<svelte:window on:keydown={onKey} />

{#if showHelp}
	<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
		role="dialog"
		aria-modal="true"
		aria-label="Keyboard shortcuts"
		data-testid="keyboard-help-dialog"
		on:click|self={close}
		tabindex="-1"
	>
		<div class="bg-popover border rounded shadow-lg p-6 max-w-md w-full mx-4">
			<div class="flex items-center justify-between mb-4">
				<h2 class="text-lg font-semibold">Keyboard shortcuts</h2>
				<button type="button" class="text-sm underline" on:click={close}>Close</button>
			</div>
			<ul class="space-y-2 text-sm">
				{#each keyboardShortcuts.all as binding (binding.id)}
					<li class="flex justify-between gap-4">
						<span>{binding.description}</span>
						<kbd class="px-2 py-0.5 border rounded text-xs font-mono">{binding.combo}</kbd>
					</li>
				{/each}
			</ul>
		</div>
	</div>
{/if}
