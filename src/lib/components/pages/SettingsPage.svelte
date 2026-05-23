<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getAppSettings,
		updateAppSettings,
		checkOllamaStatus,
		reconnectOllama
	} from '$lib/api/tauri';
	import { appSettings } from '$lib/stores';
	import { toast } from '$lib/stores/notifications';
	import type { AppSettings, OllamaStatus } from '$lib/types/backend';

	type Tab = 'general' | 'ai' | 'privacy';
	let tab: Tab = 'general';
	const tabs: Array<{ id: Tab; label: string }> = [
		{ id: 'general', label: 'General' },
		{ id: 'ai', label: 'AI' },
		{ id: 'privacy', label: 'Privacy' }
	];
	let local: AppSettings | null = null;
	let ollama: OllamaStatus | null = null;
	let saving = false;
	let highContrast = false;

	async function load(): Promise<void> {
		local = (await getAppSettings()) ?? null;
		ollama = await checkOllamaStatus();
	}

	async function save(): Promise<void> {
		if (!local) return;
		saving = true;
		try {
			await updateAppSettings(local);
			appSettings.set(local);
			toast.success('Settings saved');
		} catch (e) {
			toast.error(`Save failed: ${String(e)}`);
		} finally {
			saving = false;
		}
	}

	async function reconnect(): Promise<void> {
		if (!local?.ollama_host) return;
		try {
			ollama = await reconnectOllama(local.ollama_host);
			toast.success('Reconnected to Ollama');
		} catch (e) {
			toast.error(`Reconnect failed: ${String(e)}`);
		}
	}

	function toggleHighContrast(): void {
		highContrast = !highContrast;
		document.documentElement.classList.toggle('high-contrast', highContrast);
	}

	onMount(load);
</script>

<section class="space-y-6" data-testid="settings-page">
	<header>
		<h1 class="text-2xl font-semibold">Settings</h1>
	</header>

	<div role="tablist" class="flex gap-2 border-b">
		{#each tabs as t (t.id)}
			<button
				type="button"
				role="tab"
				aria-selected={tab === t.id}
				data-testid={`tab-${t.id}`}
				class="px-3 py-2 text-sm border-b-2 -mb-px {tab === t.id
					? 'border-primary font-medium'
					: 'border-transparent'}"
				on:click={() => (tab = t.id)}
			>
				{t.label}
			</button>
		{/each}
	</div>

	{#if !local}
		<p class="text-sm text-muted-foreground">Loading settings…</p>
	{:else}
		{#if tab === 'general'}
			<div class="space-y-3 border rounded p-4 max-w-lg">
				<label class="block">
					<span class="text-xs text-muted-foreground">Theme</span>
					<select
						bind:value={local.theme}
						data-testid="theme-select"
						class="w-full border rounded px-2 py-1"
					>
						<option value="auto">Auto</option>
						<option value="light">Light</option>
						<option value="dark">Dark</option>
					</select>
				</label>
				<label class="block">
					<span class="text-xs text-muted-foreground">Language</span>
					<input
						type="text"
						bind:value={local.language}
						class="w-full border rounded px-2 py-1"
					/>
				</label>
				<label class="flex items-center gap-2">
					<input
						type="checkbox"
						bind:checked={highContrast}
						on:change={toggleHighContrast}
						data-testid="high-contrast-toggle"
					/>
					<span class="text-sm">High contrast</span>
				</label>
			</div>
		{:else if tab === 'ai'}
			<div class="space-y-3 border rounded p-4 max-w-lg">
				<label class="block">
					<span class="text-xs text-muted-foreground">Ollama host</span>
					<div class="flex gap-2">
						<input
							type="text"
							bind:value={local.ollama_host}
							data-testid="ollama-host"
							class="flex-1 border rounded px-2 py-1"
						/>
						<button type="button" class="px-3 py-1 border rounded text-sm" on:click={reconnect}>
							Reconnect
						</button>
					</div>
					<p class="text-xs mt-1">
						{#if ollama?.isRunning}
							<span class="text-green-600">connected</span>
						{:else}
							<span class="text-amber-600">not connected — fallback active</span>
						{/if}
					</p>
				</label>
				<label class="block">
					<span class="text-xs text-muted-foreground">Text model</span>
					<input
						type="text"
						bind:value={local.ollama_model}
						data-testid="ollama-model"
						class="w-full border rounded px-2 py-1"
					/>
				</label>
				<label class="block">
					<span class="text-xs text-muted-foreground">Vision model</span>
					<input
						type="text"
						bind:value={local.ollama_vision_model}
						class="w-full border rounded px-2 py-1"
					/>
				</label>
				<label class="block">
					<span class="text-xs text-muted-foreground">Embedding model</span>
					<input
						type="text"
						bind:value={local.ollama_embedding_model}
						class="w-full border rounded px-2 py-1"
					/>
				</label>
			</div>
		{:else if tab === 'privacy'}
			<div class="space-y-3 border rounded p-4 max-w-lg">
				<label class="flex items-center justify-between">
					<span class="text-sm">Send anonymous telemetry</span>
					<input
						type="checkbox"
						bind:checked={local.enable_telemetry}
						data-testid="telemetry-switch"
					/>
				</label>
				<label class="flex items-center justify-between">
					<span class="text-sm">Send crash reports</span>
					<input
						type="checkbox"
						bind:checked={local.enable_crash_reports}
						data-testid="crash-reports-switch"
					/>
				</label>
				<label class="flex items-center justify-between">
					<span class="text-sm">Auto-analyze on add</span>
					<input type="checkbox" bind:checked={local.auto_analyze_on_add} />
				</label>
			</div>
		{/if}

		<button
			class="px-4 py-2 bg-primary text-primary-foreground rounded text-sm disabled:opacity-50"
			on:click={save}
			disabled={saving}
		>
			{saving ? 'Saving…' : 'Save'}
		</button>
	{/if}
</section>
