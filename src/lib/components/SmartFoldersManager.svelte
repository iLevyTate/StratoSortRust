<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listSmartFolders,
		createSmartFolder,
		deleteSmartFolder,
		browseFolder
	} from '$lib/api/tauri';
	import { toast } from '$lib/stores/notifications';
	import type { SmartFolder } from '$lib/types/backend';

	let folders: SmartFolder[] = [];
	let loading = true;
	let name = '';
	let targetPath = '';
	let ruleField = 'extension';
	let ruleOperator = 'equals';
	let ruleValue = '';

	async function refresh(): Promise<void> {
		loading = true;
		try {
			folders = await listSmartFolders();
		} catch (e) {
			toast.error(`Failed to load smart folders: ${String(e)}`);
		} finally {
			loading = false;
		}
	}

	async function pickPath(): Promise<void> {
		const p = await browseFolder();
		if (p) targetPath = p;
	}

	async function create(): Promise<void> {
		if (!name.trim() || !targetPath.trim()) {
			toast.warning('Name and target path are required');
			return;
		}
		try {
			await createSmartFolder({
				name,
				target_path: targetPath,
				rules: ruleValue
					? [
							{
								field: ruleField,
								operator: ruleOperator,
								value: ruleValue,
								action: { type: 'move', target_folder: targetPath }
							}
						]
					: []
			});
			toast.success(`Created smart folder “${name}”`);
			name = '';
			targetPath = '';
			ruleValue = '';
			await refresh();
		} catch (e) {
			toast.error(`Failed to create: ${String(e)}`);
		}
	}

	async function remove(id: string): Promise<void> {
		try {
			await deleteSmartFolder(id);
			await refresh();
		} catch (e) {
			toast.error(`Failed to delete: ${String(e)}`);
		}
	}

	onMount(refresh);
</script>

<section data-testid="smart-folders-manager" class="space-y-4">
	<h2 class="text-lg font-semibold">Smart folders</h2>

	<form class="space-y-2 border rounded p-4" on:submit|preventDefault={create}>
		<div class="grid grid-cols-1 md:grid-cols-2 gap-2">
			<label class="block">
				<span class="text-xs text-muted-foreground">Name</span>
				<input
					data-testid="folder-name"
					bind:value={name}
					class="w-full border rounded px-2 py-1"
					placeholder="e.g. Invoices"
				/>
			</label>
			<label class="block">
				<span class="text-xs text-muted-foreground">Target folder</span>
				<div class="flex gap-2">
					<input
						bind:value={targetPath}
						class="flex-1 border rounded px-2 py-1"
						placeholder="/home/me/Documents/Invoices"
					/>
					<button type="button" class="px-2 py-1 border rounded text-sm" on:click={pickPath}>
						Browse
					</button>
				</div>
			</label>
		</div>
		<fieldset class="grid grid-cols-3 gap-2 pt-2">
			<legend class="text-xs text-muted-foreground col-span-3">Rule (optional)</legend>
			<select
				bind:value={ruleField}
				data-testid="rule-field"
				class="border rounded px-2 py-1"
			>
				<option value="extension">Extension</option>
				<option value="name">Filename</option>
				<option value="category">Category</option>
				<option value="tag">Tag</option>
			</select>
			<select
				bind:value={ruleOperator}
				data-testid="rule-operator"
				class="border rounded px-2 py-1"
			>
				<option value="equals">equals</option>
				<option value="contains">contains</option>
				<option value="starts_with">starts with</option>
			</select>
			<input
				bind:value={ruleValue}
				data-testid="rule-value"
				class="border rounded px-2 py-1"
				placeholder="pdf"
			/>
		</fieldset>
		<button type="submit" class="px-3 py-1.5 bg-primary text-primary-foreground rounded text-sm">
			Add folder
		</button>
	</form>

	{#if loading}
		<p class="text-sm text-muted-foreground">Loading…</p>
	{:else if folders.length === 0}
		<p class="text-sm text-muted-foreground">No smart folders configured yet.</p>
	{:else}
		<ul class="divide-y border rounded">
			{#each folders as f (f.id)}
				<li class="flex items-center justify-between p-3">
					<div>
						<div class="font-medium">{f.name}</div>
						<div class="text-xs text-muted-foreground">{f.target_path}</div>
					</div>
					<button class="text-xs underline" on:click={() => remove(f.id)}>Remove</button>
				</li>
			{/each}
		</ul>
	{/if}
</section>
