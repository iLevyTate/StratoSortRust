<script lang="ts">
	import { browseFolder, scanDirectory, semanticSearch, batchAnalyzeFiles } from '$lib/api/tauri';
	import { toast } from '$lib/stores/notifications';
	import type { FileInfo, SearchResult } from '$lib/types/backend';

	let dirPath = '';
	let files: FileInfo[] = [];
	let selected: Set<string> = new Set();
	let query = '';
	let results: SearchResult[] = [];
	let scanning = false;
	let searching = false;
	let analyzing = false;

	async function pickAndScan(): Promise<void> {
		const p = await browseFolder();
		if (!p) return;
		dirPath = p;
		await scan();
	}

	async function scan(): Promise<void> {
		if (!dirPath) return;
		scanning = true;
		try {
			files = await scanDirectory(dirPath);
		} catch (e) {
			toast.error(`Scan failed: ${String(e)}`);
		} finally {
			scanning = false;
		}
	}

	function toggle(path: string): void {
		const next = new Set(selected);
		if (next.has(path)) next.delete(path);
		else next.add(path);
		selected = next;
	}

	async function analyzeSelected(): Promise<void> {
		if (selected.size === 0) {
			toast.info('Select some files first');
			return;
		}
		analyzing = true;
		try {
			const out = await batchAnalyzeFiles(Array.from(selected));
			toast.success(`Analyzed ${out.length} file(s)`);
		} catch (e) {
			toast.error(`Batch analyze failed: ${String(e)}`);
		} finally {
			analyzing = false;
		}
	}

	async function runSearch(): Promise<void> {
		if (!query.trim()) return;
		searching = true;
		try {
			results = await semanticSearch(query, 20);
		} catch (e) {
			toast.error(`Search failed: ${String(e)}`);
		} finally {
			searching = false;
		}
	}
</script>

<section class="space-y-6" data-testid="discover-page">
	<header>
		<h1 class="text-2xl font-semibold">Discover</h1>
		<p class="text-sm text-muted-foreground">
			Browse a folder or run a semantic search across analyzed files.
		</p>
	</header>

	<div class="border rounded p-4 space-y-3" data-testid="drop-zone">
		<div class="flex gap-2">
			<input
				type="text"
				bind:value={dirPath}
				placeholder="/path/to/folder"
				class="flex-1 border rounded px-2 py-1"
				aria-label="Directory path"
			/>
			<button class="px-3 py-1.5 border rounded text-sm" on:click={pickAndScan}>Browse…</button>
			<button class="px-3 py-1.5 border rounded text-sm" on:click={scan} disabled={scanning}>
				{scanning ? 'Scanning…' : 'Scan'}
			</button>
		</div>

		{#if files.length > 0}
			<div class="flex justify-between text-xs text-muted-foreground">
				<span>{files.length} item(s) · {selected.size} selected</span>
				<button
					class="underline disabled:opacity-50"
					on:click={analyzeSelected}
					disabled={analyzing || selected.size === 0}
				>
					{analyzing ? 'Analyzing…' : 'Analyze selected'}
				</button>
			</div>
			<ul class="divide-y border rounded max-h-72 overflow-auto">
				{#each files as f, i (f.path)}
					<li class="flex items-center gap-2 px-3 py-1.5 text-sm">
						<input
							type="checkbox"
							data-testid={`file-checkbox-${i}`}
							checked={selected.has(f.path)}
							on:change={() => toggle(f.path)}
							aria-label="Select {f.name}"
						/>
						<span class="flex-1 truncate" title={f.path}>{f.name}</span>
						<span class="text-xs text-muted-foreground">
							{f.is_directory ? 'dir' : `${Math.round((f.size ?? 0) / 1024)} KB`}
						</span>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<div class="border rounded p-4 space-y-3">
		<form class="flex gap-2" on:submit|preventDefault={runSearch}>
			<input
				type="search"
				bind:value={query}
				placeholder="Semantic search across analyzed files…"
				class="flex-1 border rounded px-2 py-1"
				data-testid="search-input"
			/>
			<button class="px-3 py-1.5 bg-primary text-primary-foreground rounded text-sm">
				{searching ? 'Searching…' : 'Search'}
			</button>
		</form>
		{#if results.length > 0}
			<ul class="divide-y" data-testid="search-results">
				{#each results as r (r.path)}
					<li class="py-2 text-sm">
						<div class="flex justify-between">
							<span class="font-medium truncate">{r.name ?? r.path}</span>
							<span class="text-xs text-muted-foreground">score {r.score.toFixed(2)}</span>
						</div>
						{#if r.snippet}
							<p class="text-xs text-muted-foreground line-clamp-2">{r.snippet}</p>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</section>
