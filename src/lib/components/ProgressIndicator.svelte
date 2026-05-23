<script lang="ts">
	import { currentOperationProgress } from '$lib/stores';

	export let variant: 'inline' | 'block' = 'inline';
	export let compact: boolean = false;
	export let showCancel: boolean = false;
	export let showProgress: boolean = true;

	$: pct = $currentOperationProgress
		? Math.max(0, Math.min(100, Math.round($currentOperationProgress.progress * 100)))
		: 0;
	$: message = $currentOperationProgress?.message ?? 'Working...';
</script>

<div
	class="flex items-center gap-3 {variant === 'block' ? 'w-full p-3 border rounded' : ''}"
	role="status"
	aria-live="polite"
	data-testid="progress-indicator"
>
	<div class="flex-1 min-w-0">
		<div class="flex justify-between text-xs mb-1">
			<span class="truncate {compact ? 'max-w-[200px]' : ''}">{message}</span>
			{#if showProgress}
				<span class="tabular-nums">{pct}%</span>
			{/if}
		</div>
		<div class="h-1.5 bg-muted rounded overflow-hidden">
			<div class="h-full bg-primary transition-all" style="width: {pct}%"></div>
		</div>
	</div>
	{#if showCancel}
		<button type="button" class="text-xs underline">Cancel</button>
	{/if}
</div>
