<script lang="ts">
	import { onMount } from 'svelte';
	import { reanalyzeFiles, clearStaleAnalyses } from '$lib/api/tauri';
	import { toast } from '$lib/stores/notifications';
	import { isTauri } from '$lib/api/tauri';

	let recent: Array<{ path: string; category: string; summary: string; confidence: number }> = [];
	let loading = false;
	let categoryFilter = '';
	let pathsInput = '';
	let busy = false;

	async function loadRecent(): Promise<void> {
		if (!isTauri()) return;
		loading = true;
		try {
			const mod = await import('@tauri-apps/api/core');
			recent = await mod.invoke('get_analysis_history', { limit: 50 });
		} catch (e) {
			console.warn('get_analysis_history failed:', e);
		} finally {
			loading = false;
		}
	}

	async function rerun(): Promise<void> {
		const paths = pathsInput
			.split('\n')
			.map((s) => s.trim())
			.filter(Boolean);
		if (paths.length === 0) {
			toast.info('Paste one path per line');
			return;
		}
		busy = true;
		try {
			const out = await reanalyzeFiles(paths);
			toast.success(`Re-analyzed ${out.length} file(s)`);
			await loadRecent();
		} catch (e) {
			toast.error(`Re-analyze failed: ${String(e)}`);
		} finally {
			busy = false;
		}
	}

	async function purgeStale(): Promise<void> {
		try {
			const n = await clearStaleAnalyses();
			toast.success(`Purged ${n} stale entries`);
			await loadRecent();
		} catch (e) {
			toast.error(`Purge failed: ${String(e)}`);
		}
	}

	$: filtered = categoryFilter
		? recent.filter((r) => r.category.toLowerCase().includes(categoryFilter.toLowerCase()))
		: recent;

	onMount(loadRecent);
</script>

<section class="space-y-6" data-testid="analyze-page">
	<header>
		<h1 class="text-2xl font-semibold">Analyze</h1>
		<p class="text-sm text-muted-foreground">
			Re-run AI analysis on specific files or browse recently analyzed results.
		</p>
	</header>

	<div class="border rounded p-4 space-y-2">
		<label class="block text-xs text-muted-foreground" for="rerun-paths">Paths (one per line)</label>
		<textarea
			id="rerun-paths"
			bind:value={pathsInput}
			rows="4"
			class="w-full border rounded px-2 py-1 text-sm font-mono"
			placeholder="/home/me/Downloads/report.pdf"
		></textarea>
		<div class="flex gap-2">
			<button
				class="px-3 py-1.5 bg-primary text-primary-foreground rounded text-sm"
				on:click={rerun}
				disabled={busy}
			>
				{busy ? 'Re-analyzing…' : 'Re-analyze'}
			</button>
			<button class="px-3 py-1.5 border rounded text-sm" on:click={purgeStale}>
				Purge stale cache
			</button>
		</div>
		{#if busy}
			<div data-testid="analysis-progress" class="text-xs text-muted-foreground">Working…</div>
		{/if}
	</div>

	<div class="border rounded p-4 space-y-2">
		<div class="flex justify-between items-center">
			<h2 class="font-medium">Recent analyses</h2>
			<input
				type="text"
				bind:value={categoryFilter}
				placeholder="Filter by category…"
				data-testid="category-filter"
				class="border rounded px-2 py-1 text-sm"
			/>
		</div>
		{#if loading}
			<p class="text-sm text-muted-foreground">Loading…</p>
		{:else if filtered.length === 0}
			<p class="text-sm text-muted-foreground">Nothing analyzed yet.</p>
		{:else}
			<ul class="divide-y">
				{#each filtered as r (r.path)}
					<li class="py-2 text-sm">
						<div class="flex justify-between">
							<span class="font-medium truncate">{r.path}</span>
							<span class="text-xs text-muted-foreground">
								{r.category} · {Math.round(r.confidence * 100)}%
							</span>
						</div>
						<p class="text-xs text-muted-foreground line-clamp-2">{r.summary}</p>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</section>
