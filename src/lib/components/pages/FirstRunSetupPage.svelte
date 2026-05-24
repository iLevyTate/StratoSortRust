<script lang="ts">
	import { onMount } from 'svelte';
	import { checkOllamaStatus, completeFirstRunSetup } from '$lib/api/tauri';
	import { toast } from '$lib/stores/notifications';

	export let onSetupComplete: () => void;

	let step: 1 | 2 | 3 = 1;
	let ollamaReachable: boolean | null = null;
	let saving = false;

	async function recheckOllama(): Promise<void> {
		const s = await checkOllamaStatus();
		ollamaReachable = !!s?.isRunning;
	}

	async function finish(): Promise<void> {
		saving = true;
		try {
			await completeFirstRunSetup();
			toast.success('Setup complete');
			onSetupComplete();
		} catch (e) {
			toast.error(`Could not finish setup: ${String(e)}`);
		} finally {
			saving = false;
		}
	}

	onMount(recheckOllama);
</script>

<section
	class="max-w-xl mx-auto p-8 space-y-6"
	data-testid="first-run-setup"
	aria-label="First run setup"
>
	<header>
		<h1 class="text-2xl font-semibold">Welcome to StratoSort</h1>
		<p class="text-sm text-muted-foreground mt-1">
			Three quick steps and your local-first AI file sorter is ready.
		</p>
	</header>

	<ol class="space-y-4">
		<li class="border rounded p-4 {step >= 1 ? 'border-primary' : ''}">
			<div class="flex justify-between items-start">
				<div>
					<h2 class="font-medium">1. Install Ollama</h2>
					<p class="text-sm text-muted-foreground">
						StratoSort runs models locally via Ollama. Install it from
						<a href="https://ollama.com" target="_blank" rel="noopener" class="underline"
							>ollama.com</a
						>
						and run <code class="px-1 bg-muted rounded">ollama serve</code>.
					</p>
				</div>
				<button class="text-xs underline" on:click={recheckOllama}>Check</button>
			</div>
			<p class="mt-2 text-xs">
				Status:
				{#if ollamaReachable === null}
					<span class="text-muted-foreground">checking…</span>
				{:else if ollamaReachable}
					<span class="text-green-600">reachable ✓</span>
				{:else}
					<span class="text-amber-600">not reachable (fallback mode will be used)</span>
				{/if}
			</p>
			<button class="mt-3 text-sm underline" on:click={() => (step = 2)}>Next →</button>
		</li>

		{#if step >= 2}
			<li class="border rounded p-4 {step >= 2 ? 'border-primary' : ''}">
				<h2 class="font-medium">2. Pull the models</h2>
				<pre class="text-xs bg-muted p-3 rounded mt-2 overflow-auto"><code
						>ollama pull llama3.2:3b
ollama pull llava:7b
ollama pull nomic-embed-text</code
					></pre>
				<button class="mt-3 text-sm underline" on:click={() => (step = 3)}>Next →</button>
			</li>
		{/if}

		{#if step >= 3}
			<li class="border rounded p-4 border-primary">
				<h2 class="font-medium">3. You're ready</h2>
				<p class="text-sm text-muted-foreground mt-1">
					Configure smart folders and watched directories anytime under Settings.
				</p>
				<button
					class="mt-3 px-4 py-2 bg-primary text-primary-foreground rounded text-sm disabled:opacity-50"
					on:click={finish}
					disabled={saving}
					data-testid="first-run-finish"
				>
					{saving ? 'Saving…' : 'Get started'}
				</button>
			</li>
		{/if}
	</ol>
</section>
