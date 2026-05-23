<script lang="ts">
	import { notifications, markAllRead, clearNotifications } from '$lib/stores/notifications';

	let open = false;

	$: unread = $notifications.filter((n) => !n.read).length;

	function toggle(): void {
		open = !open;
		if (open) markAllRead();
	}
</script>

<div class="relative" data-testid="notification-center">
	<button
		type="button"
		class="relative p-2 rounded hover:bg-accent"
		on:click={toggle}
		aria-label="Notifications ({unread} unread)"
		data-testid="notification-toggle"
	>
		<span aria-hidden="true">🔔</span>
		{#if unread > 0}
			<span
				class="absolute -top-1 -right-1 w-4 h-4 text-[10px] flex items-center justify-center rounded-full bg-destructive text-destructive-foreground"
			>
				{unread}
			</span>
		{/if}
	</button>

	{#if open}
		<div
			class="absolute right-0 top-full mt-2 w-80 max-h-96 overflow-auto bg-popover border rounded shadow z-50"
			role="dialog"
			aria-label="Notifications"
		>
			<div class="flex items-center justify-between p-3 border-b">
				<span class="font-medium">Notifications</span>
				<button type="button" class="text-xs underline" on:click={clearNotifications}>Clear</button>
			</div>
			{#if $notifications.length === 0}
				<p class="p-4 text-sm text-muted-foreground">No notifications.</p>
			{:else}
				<ul class="divide-y">
					{#each $notifications as n (n.id)}
						<li class="p-3 text-sm">
							<div class="font-medium">{n.title}</div>
							<div class="text-muted-foreground">{n.message}</div>
							<div class="text-[10px] text-muted-foreground mt-1">
								{new Date(n.timestamp).toLocaleString()}
							</div>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>
