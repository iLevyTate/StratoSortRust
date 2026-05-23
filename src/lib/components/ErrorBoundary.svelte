<script lang="ts">
	import { createEventDispatcher, setContext } from 'svelte';

	export let fallback: boolean = true;

	const dispatch = createEventDispatcher();
	let error: Error | null = null;

	function captureError(e: unknown): void {
		error = e instanceof Error ? e : new Error(String(e));
		console.error('ErrorBoundary captured:', error);
	}

	setContext('captureError', captureError);

	function reset(): void {
		error = null;
	}

	function goHome(): void {
		reset();
		dispatch('goHome');
	}
</script>

{#if error}
	{#if fallback}
		<div class="p-8 max-w-xl mx-auto text-center" role="alert" data-testid="error-boundary">
			<h2 class="text-xl font-semibold mb-2">Something went wrong</h2>
			<p class="text-sm text-muted-foreground mb-4">{error.message}</p>
			<div class="flex gap-2 justify-center">
				<button class="px-4 py-2 border rounded" on:click={reset}>Try again</button>
				<button class="px-4 py-2 bg-primary text-primary-foreground rounded" on:click={goHome}>
					Go home
				</button>
			</div>
		</div>
	{/if}
{:else}
	<slot {captureError} />
{/if}
