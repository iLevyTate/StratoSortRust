<script lang="ts">
	import SmartFoldersManager from '$lib/components/SmartFoldersManager.svelte';
	import {
		getWatchModeStatus,
		enableWatchMode,
		disableWatchMode,
		browseFolder
	} from '$lib/api/tauri';
	import { toast } from '$lib/stores/notifications';
	import type { WatchModeStatus } from '$lib/api/tauri';
	import { onMount } from 'svelte';

	let status: WatchModeStatus | null = null;
	let newDir = '';
	let saving = false;

	async function refresh(): Promise<void> {
		status = await getWatchModeStatus();
	}

	async function add(): Promise<void> {
		if (!newDir.trim()) {
			const picked = await browseFolder();
			if (picked) newDir = picked;
			if (!newDir) return;
		}
		const next = [...(status?.watching_directories ?? []), newDir];
		saving = true;
		try {
			await enableWatchMode(next);
			toast.success(`Now watching ${newDir}`);
			newDir = '';
			await refresh();
		} catch (e) {
			toast.error(`Could not enable watch mode: ${String(e)}`);
		} finally {
			saving = false;
		}
	}

	async function stopWatching(): Promise<void> {
		try {
			await disableWatchMode();
			toast.success('Watch mode disabled');
			await refresh();
		} catch (e) {
			toast.error(`Could not disable watch mode: ${String(e)}`);
		}
	}

	onMount(refresh);
</script>

<section class="space-y-6" data-testid="organize-page">
	<header>
		<h1 class="text-2xl font-semibold">Organize</h1>
		<p class="text-sm text-muted-foreground">
			Configure smart folders and the directories the app watches for new files.
		</p>
	</header>

	<div class="border rounded p-4 space-y-3" data-testid="watch-mode-panel">
		<div class="flex items-center justify-between">
			<h2 class="font-medium">Watch mode</h2>
			<span class="text-xs">
				{#if status?.enabled}
					<span class="text-green-600">enabled · {status.watching_directories.length} dir(s)</span>
				{:else}
					<span class="text-muted-foreground">disabled</span>
				{/if}
			</span>
		</div>

		<div class="flex gap-2">
			<input
				type="text"
				bind:value={newDir}
				placeholder="/path/to/watch"
				class="flex-1 border rounded px-2 py-1"
				aria-label="Directory to watch"
			/>
			<button class="px-3 py-1.5 border rounded text-sm" on:click={add} disabled={saving}>
				{saving ? 'Saving…' : 'Add directory'}
			</button>
			{#if status?.enabled}
				<button class="px-3 py-1.5 border rounded text-sm" on:click={stopWatching}>
					Disable
				</button>
			{/if}
		</div>

		{#if status?.watching_directories?.length}
			<ul class="text-sm divide-y border-t pt-2">
				{#each status.watching_directories as d (d)}
					<li class="py-1.5 font-mono text-xs truncate">{d}</li>
				{/each}
			</ul>
		{/if}
	</div>

	<SmartFoldersManager />
</section>
